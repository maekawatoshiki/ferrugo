use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::exec::vm::load_class;
use super::super::gc::{gc, gc::GcType};
use super::frame::{AType, Array, ObjectBody, Variable};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug)]
pub struct ObjectHeap {
    // TODO: Add fields for support of GC
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {}
    }

    pub fn create_object(&mut self, class: GcType<Class>) -> Variable {
        let class_field_count = unsafe { &*class }.get_object_field_count();
        let obj = gc::new(ObjectBody {
            class: Variable::Pointer(class as *mut u64),
            variables: {
                let mut vars = FxHashMap::default();
                vars.reserve(class_field_count);
                vars
            },
        });

        Variable::Object(obj)
    }

    pub fn create_string_object(
        &mut self,
        string: String,
        classheap: GcType<ClassHeap>,
    ) -> Variable {
        let class = load_class(classheap, self, "java/lang/String");
        let object = self.create_object(class);

        unsafe { &mut *object.get_object() }.variables.insert(
            "str".to_string(),
            Variable::Pointer(Box::into_raw(Box::new(string)) as GcType<u64>),
        );

        object
    }

    pub fn create_array(&mut self, atype: AType, size: usize) -> Variable {
        let array = Array {
            atype,
            elements: {
                let mut elements = vec![];
                for _ in 0..size {
                    elements.push(Variable::Int(0));
                }
                elements
            },
        };

        Variable::Pointer(gc::new(array) as GcType<u64>)
    }

    pub fn create_obj_array(&mut self, class: GcType<Class>, size: usize) -> Variable {
        let array = Array {
            atype: AType::Class(class),
            elements: {
                let mut elements = vec![];
                for _ in 0..size {
                    elements.push(Variable::Pointer(0 as *mut u64));
                }
                elements
            },
        };

        Variable::Pointer(gc::new(array) as GcType<u64>)
    }
}
