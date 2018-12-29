use super::super::class::{class::Class, classheap::ClassHeap};
use super::super::exec::vm::load_class;
use super::super::gc::{gc, gc::GcType};
use super::frame::{Object, Variable};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug)]
pub struct ObjectHeap {
    pub object_map: FxHashMap<usize, GcType<ObjectBody>>,
    pub id: usize,
}

#[derive(Debug, Clone)]
pub struct ObjectBody {
    pub class: Variable,
    pub variables: FxHashMap<String, Variable>,
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {
            object_map: FxHashMap::default(),
            id: 0,
        }
    }

    pub fn get_object(&self, heap_id: usize) -> Option<GcType<ObjectBody>> {
        self.object_map
            .get(&heap_id)
            .and_then(|object_body| Some(*object_body))
    }

    pub fn create_object(&mut self, class: GcType<Class>) -> Object {
        let mut object = Object { heap_id: 0 };

        // let class_field_count = unsafe { &*class }.get_object_field_count() + 1; // plus 1 for class pointer
        let obj = gc::new(ObjectBody {
            class: Variable::Pointer(class as *mut u64),
            variables: FxHashMap::default(),
        });

        object.heap_id = self.id;
        self.id += 1;

        self.object_map.insert(object.heap_id, obj);

        object
    }

    pub fn create_string_object(&mut self, string: String, classheap: GcType<ClassHeap>) -> Object {
        let class = load_class(classheap, self, "java/lang/String");
        let object = self.create_object(class);

        let vars = *self.object_map.get(&object.heap_id).unwrap();
        unsafe { &mut *vars }.variables.insert(
            "str".to_string(),
            Variable::Pointer(Box::into_raw(Box::new(string)) as GcType<u64>),
        );

        object
    }
}
