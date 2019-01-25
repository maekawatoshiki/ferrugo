use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::exec::vm::load_class;
use super::super::gc::{gc, gc::GcType};
use super::frame::{AType, Array, ObjectBody, Variable};

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
            variables: vec![Variable::Int(0); class_field_count],
        });

        Variable::Pointer(obj as *mut u64)
    }

    pub fn create_string_object(
        &mut self,
        string: String,
        classheap: GcType<ClassHeap>,
    ) -> Variable {
        let class = load_class(classheap, self, "java/lang/String");
        let object = self.create_object(class);

        unsafe { &mut *object.get_pointer::<ObjectBody>() }
            .variables
            .insert(
                0,
                Variable::Pointer(gc::new(Array {
                    atype: AType::Char,
                    elements: vec![],
                    string: Some(string),
                }) as GcType<u64>),
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
            string: None,
        };

        Variable::Pointer(gc::new(array) as GcType<u64>)
    }

    pub fn create_obj_array(&mut self, class: GcType<Class>, size: usize) -> Variable {
        let array = Array {
            atype: AType::Class(class),
            elements: {
                let mut elements = vec![];
                for _ in 0..size {
                    elements.push(Variable::Int(0));
                }
                elements
            },
            string: None,
        };

        Variable::Pointer(gc::new(array) as GcType<u64>)
    }
}
