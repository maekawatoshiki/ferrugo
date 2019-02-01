// TODO: CAUTION: Am I doing wrong thing?

use super::super::class::{class::Class, classfile::constant::Constant, classheap::ClassHeap};
use super::super::exec::{
    frame::{AType, Array, Frame, ObjectBody},
    vm::{RuntimeEnvironment, VM},
};
use rustc_hash::FxHashMap;
use std::{
    cell::RefCell,
    mem,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub type GcType<T> = *mut T;

static ALLOCATED_MEM_SIZE_BYTE: AtomicUsize = AtomicUsize::new(0);
static GC_DISABLED: AtomicBool = AtomicBool::new(false);

thread_local!(static GC_MEM: RefCell<GcStateMap> = {
    RefCell::new(FxHashMap::default())
});

type GcStateMap = FxHashMap<*mut u64, GcTargetInfo>;

#[derive(Debug, Clone, PartialEq, Copy)]
enum GcState {
    Marked,
    Unmarked,
}

#[derive(Debug, Clone, Copy)]
enum GcTargetType {
    Array,
    Object,
    Class,
    ClassHeap,
    ObjectHeap,
    RuntimeEnvironment,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct GcTargetInfo {
    pub ty: GcTargetType,
    pub state: GcState,
}

impl GcTargetInfo {
    pub fn new_unmarked(ty_name: &str) -> Self {
        GcTargetInfo {
            ty: match ty_name {
                s if s.ends_with("Array") => GcTargetType::Array,
                s if s.ends_with("ObjectBody") => GcTargetType::Object,
                s if s.ends_with("Class") => GcTargetType::Class,
                s if s.ends_with("ClassHeap") => GcTargetType::ClassHeap,
                s if s.ends_with("ObjectHeap") => GcTargetType::ObjectHeap,
                s if s.ends_with("RuntimeEnvironment") => GcTargetType::RuntimeEnvironment,
                _ => GcTargetType::Unknown,
            },
            state: GcState::Unmarked,
        }
    }
}

pub fn new<T>(val: T) -> GcType<T> {
    let size = mem::size_of_val(&val);
    let ptr = Box::into_raw(Box::new(val));
    let info = GcTargetInfo::new_unmarked(unsafe { std::intrinsics::type_name::<T>() });
    ALLOCATED_MEM_SIZE_BYTE.fetch_add(size, Ordering::Relaxed);
    GC_MEM.with(|m| {
        m.borrow_mut().insert(ptr as *mut u64, info);
    });
    ptr
}

pub fn enable() {
    GC_DISABLED.store(false, Ordering::Relaxed);
}

pub fn disable() {
    GC_DISABLED.store(true, Ordering::Relaxed);
}

pub fn mark_and_sweep(vm: &VM) {
    if GC_DISABLED.load(Ordering::Relaxed) {
        return;
    }

    fn over10mb_allocated() -> bool {
        ALLOCATED_MEM_SIZE_BYTE.load(Ordering::Relaxed) > 10 * 1024 * 1024
    }

    if !over10mb_allocated() {
        return;
    }

    let mut m = GcStateMap::default();
    trace(&vm, &mut m);
    free(&m);
}

fn trace(vm: &VM, m: &mut GcStateMap) {
    trace_ptr(vm.runtime_env as *mut u64, m);
    trace_ptr(vm.classheap as *mut u64, m);
    trace_ptr(vm.objectheap as *mut u64, m);

    // trace frame stack
    for frame in &vm.frame_stack {
        frame.trace(m);
    }

    // trace variable stack
    for val in &vm.stack {
        trace_ptr(*val as *mut u64, m);
    }
}

fn free(m: &GcStateMap) {
    GC_MEM.with(|mem| {
        mem.borrow_mut().retain(|p, info| {
            let is_marked = m
                .get(p)
                .and_then(|info| Some(info.state == GcState::Marked))
                .unwrap_or(false);
            if !is_marked {
                let released_size = free_ptr(*p, info);
                if ALLOCATED_MEM_SIZE_BYTE.load(Ordering::Relaxed) as isize - released_size as isize
                    > 0
                {
                    // println!("mem {}", ALLOCATED_MEM_SIZE_BYTE.load(Ordering::Relaxed));
                    ALLOCATED_MEM_SIZE_BYTE.fetch_sub(released_size, Ordering::Relaxed);
                }
            }
            is_marked
        });
    });
}

impl Frame {
    fn trace(&self, m: &mut GcStateMap) {
        trace_ptr(self.class.unwrap() as *mut u64, m);
    }
}

impl Class {
    fn trace(&self, m: &mut GcStateMap) {
        self.static_variables
            .iter()
            .for_each(|(_, v)| trace_ptr(*v as *mut u64, m));
        for constant in &self.classfile.constant_pool {
            match constant {
                Constant::Utf8 { java_string, .. } => {
                    if let Some(java_string) = java_string {
                        trace_ptr(*java_string as *mut u64, m);
                    }
                }
                _ => {}
            }
        }
    }
}

fn trace_ptr(ptr: *mut u64, m: &mut GcStateMap) {
    if ptr == 0 as *mut u64 {
        return;
    }

    if m.contains_key(&ptr) {
        return;
    }

    let mut info = if let Some(info) = GC_MEM.with(|m| m.borrow().get(&ptr).map(|x| *x)) {
        info
    } else {
        return;
    };

    m.insert(ptr, {
        info.state = GcState::Marked;
        info
    });

    match info.ty {
        GcTargetType::Array => {
            // TODO: FIX
            let ary = unsafe { &*(ptr as *mut Array) };
            match ary.atype {
                AType::Class(_) => {
                    let len = ary.get_length();
                    for i in 0..len {
                        trace_ptr(ary.at::<u64>(i as isize) as *mut u64, m);
                    }
                }
                _ => {}
            }
        }
        GcTargetType::Object => {
            let obj = unsafe { &*(ptr as *mut ObjectBody) };
            unsafe { &*obj.class }.trace(m);
            obj.variables
                .iter()
                .for_each(|v| trace_ptr(*v as *mut u64, m));
        }
        GcTargetType::Class => {
            let class = unsafe { &*(ptr as *mut Class) };
            class.trace(m);
        }
        GcTargetType::ClassHeap => {
            let classheap = unsafe { &*(ptr as *mut ClassHeap) };
            for (_, class_ptr) in &classheap.class_map {
                trace_ptr(*class_ptr as *mut u64, m);
            }
        }
        GcTargetType::ObjectHeap => {}
        GcTargetType::RuntimeEnvironment => {
            let renv = unsafe { &*(ptr as *mut RuntimeEnvironment) };
            trace_ptr(renv.classheap as *mut u64, m);
            trace_ptr(renv.objectheap as *mut u64, m);
        }
        GcTargetType::Unknown => panic!(),
    };
}

fn free_ptr(ptr: *mut u64, info: &GcTargetInfo) -> usize {
    match info.ty {
        GcTargetType::Array => mem::size_of_val(&*unsafe { Box::from_raw(ptr as *mut Array) }),
        GcTargetType::Object => {
            mem::size_of_val(&*unsafe { Box::from_raw(ptr as *mut ObjectBody) })
        }
        GcTargetType::Class => mem::size_of_val(&*unsafe { Box::from_raw(ptr as *mut Class) }),
        GcTargetType::ClassHeap
        | GcTargetType::ObjectHeap
        | GcTargetType::RuntimeEnvironment
        | GcTargetType::Unknown => 0,
    }
}
