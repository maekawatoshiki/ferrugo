// TODO: CAUTION: Am I doing wrong thing?

use super::super::class::{class::Class, classfile::constant::Constant, classheap::ClassHeap};
use super::super::exec::{
    frame::{Array, Frame, ObjectBody, Variable},
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

#[derive(Debug, Clone, PartialEq)]
enum GcState {
    Marked,
    Unmarked,
}

#[derive(Debug, Clone)]
enum GcTargetType {
    Array,
    Object,
    Class,
    ClassHeap,
    ObjectHeap,
    RuntimeEnvironment,
    Unknown,
}

#[derive(Debug, Clone)]
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
    let ptr = Box::into_raw(Box::new(val));
    let info = GcTargetInfo::new_unmarked(unsafe { std::intrinsics::type_name::<T>() });
    let size = get_size(ptr as *mut u64, &info);
    ALLOCATED_MEM_SIZE_BYTE.fetch_add(size, Ordering::SeqCst);
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

    fn over16kb_allocated() -> bool {
        ALLOCATED_MEM_SIZE_BYTE.load(Ordering::SeqCst) > 16 * 1024
    }

    if !over16kb_allocated() {
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
        val.trace(m);
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
                ALLOCATED_MEM_SIZE_BYTE.fetch_sub(released_size, Ordering::SeqCst);
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

impl Variable {
    fn trace(&self, m: &mut GcStateMap) {
        match self {
            Variable::Pointer(ptr) => trace_ptr(*ptr, m),
            _ => {}
        }
    }
}

impl Class {
    fn trace(&self, m: &mut GcStateMap) {
        self.static_variables.iter().for_each(|(_, v)| v.trace(m));
        for constant in &self.classfile.constant_pool {
            match constant {
                Constant::Utf8 { java_string, .. } => {
                    if let Some(java_string) = java_string {
                        java_string.trace(m);
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

    let mut info = GC_MEM.with(|m| m.borrow().get(&ptr).unwrap().clone());

    match info.ty {
        GcTargetType::Array => {
            let ary = unsafe { &*(ptr as *mut Array) };
            ary.elements.iter().for_each(|v| v.trace(m));
        }
        GcTargetType::Object => {
            let obj = unsafe { &*(ptr as *mut ObjectBody) };
            obj.class.trace(m);
            obj.variables.iter().for_each(|(_, v)| v.trace(m));
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

    m.insert(ptr, {
        info.state = GcState::Marked;
        info
    });
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

// TODO: How can we get the size of Vec or something in bytes?
fn get_size(_ptr: *mut u64, info: &GcTargetInfo) -> usize {
    match info.ty {
        GcTargetType::Array | GcTargetType::Object | GcTargetType::Class => 16,
        GcTargetType::ClassHeap
        | GcTargetType::ObjectHeap
        | GcTargetType::RuntimeEnvironment
        | GcTargetType::Unknown => 0,
    }
}
