use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::gc::{gc, gc::GcType};
use super::frame::{Object, Variable};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ObjectHeap {
    pub object_map: HashMap<usize, GcType<ObjectBody>>,
    pub id: usize,
}

#[derive(Debug, Clone)]
pub struct ObjectBody {
    pub variables: Vec<Variable>,
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {
            object_map: HashMap::new(),
            id: 0,
        }
    }

    pub fn get_object(&mut self, heap_id: usize) -> Option<GcType<ObjectBody>> {
        self.object_map
            .get(&heap_id)
            .and_then(|object_body| Some(*object_body))
    }

    pub fn create_object(&mut self, class: GcType<Class>) -> Object {
        let mut object = Object { heap_id: 0 };

        let class_field_count = unsafe { &*class }.get_object_field_count() + 1; // plus 1 for class pointer
        let obj = gc::new(ObjectBody {
            variables: {
                let mut vars = vec![];
                for _ in 0..class_field_count {
                    vars.push(Variable::Int(0))
                }
                vars[0] = Variable::Pointer(class as *mut u64);
                vars
            },
        });

        object.heap_id = self.id;
        self.id += 1;

        self.object_map.insert(object.heap_id, obj);

        object
    }

    pub fn create_string_object(&mut self, string: String, classheap: GcType<ClassHeap>) -> Object {
        let classheap = unsafe { &*classheap };

        let class = *classheap.class_map.get("java/lang/String").unwrap();
        let object = self.create_object(class);

        let vars = *self.object_map.get(&object.heap_id).unwrap();
        unsafe { &mut *vars }.variables[1] =
            Variable::Pointer(Box::into_raw(Box::new(string)) as GcType<u64>);

        object
    }
}
