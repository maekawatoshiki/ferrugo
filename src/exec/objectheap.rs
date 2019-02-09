use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::gc::{gc::GcType, gc::GC};
use super::frame::{AType, Array, ObjectBody};
use std::slice::Iter;

#[derive(Clone, Debug)]
pub struct ObjectHeap {
    pub gc: GC,
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap { gc: GC::new() }
    }

    pub fn create_object(&mut self, class: GcType<Class>) -> u64 {
        let class_field_count = unsafe { &*class }.get_object_field_count();
        let obj = self.gc.alloc(ObjectBody {
            class,
            variables: vec![0; class_field_count],
        });

        obj as u64
    }

    pub fn create_string_object(&mut self, string: String, classheap: GcType<ClassHeap>) -> u64 {
        let class = unsafe { &*classheap }
            .get_class("java/lang/String")
            .unwrap();
        let object = self.create_object(class);

        unsafe { &mut *(object as GcType<ObjectBody>) }
            .variables
            .insert(
                0,
                self.gc.alloc(Array::new(AType::Char, 0, Some(string))) as u64,
            );

        object
    }

    pub fn create_array(&mut self, atype: AType, size: usize) -> u64 {
        self.gc.alloc(Array::new(atype, size, None)) as u64
    }

    pub fn create_obj_array(&mut self, class: GcType<Class>, size: usize) -> u64 {
        self.gc.alloc(Array::new(AType::Class(class), size, None)) as u64
    }

    pub fn create_multi_array(&mut self, atype: AType, mut counts: Iter<usize>) -> u64 {
        let count = *counts.next().unwrap();

        if atype.get_multi_dimensions() <= 1 {
            return self.gc.alloc(Array::new(
                atype.get_multi_element_type().clone(),
                count,
                None,
            )) as u64;
        }

        let mut array = Array::new(atype.clone(), count, None);

        for i in 0..count {
            array.store(
                i as isize,
                self.create_multi_array(atype.clone().reduce_dimension(), counts.clone()),
            );
        }

        self.gc.alloc(array) as u64
    }
}
