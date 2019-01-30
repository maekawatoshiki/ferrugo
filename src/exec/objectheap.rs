use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::exec::vm::load_class;
use super::super::gc::{gc, gc::GcType};
use super::frame::{AType, Array, ObjectBody};

#[derive(Clone, Debug)]
pub struct ObjectHeap {
    // TODO: Add fields for support of GC
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {}
    }

    pub fn create_object(&mut self, class: GcType<Class>) -> u64 {
        let class_field_count = unsafe { &*class }.get_object_field_count();
        let obj = gc::new(ObjectBody {
            class,
            variables: vec![0; class_field_count],
        });

        obj as u64
    }

    pub fn create_string_object(&mut self, string: String, classheap: GcType<ClassHeap>) -> u64 {
        let class = load_class(classheap, self, "java/lang/String");
        let object = self.create_object(class);

        unsafe { &mut *(object as GcType<ObjectBody>) }
            .variables
            .insert(
                0,
                gc::new(Array {
                    atype: AType::Char,
                    elements: vec![],
                    string: Some(string),
                }) as u64,
            );

        object
    }

    pub fn create_array(&mut self, atype: AType, size: usize) -> u64 {
        gc::new(Array::new(atype, size, None)) as u64
    }

    pub fn create_obj_array(&mut self, class: GcType<Class>, size: usize) -> u64 {
        gc::new(Array::new(AType::Class(class), size, None)) as u64
    }
}
