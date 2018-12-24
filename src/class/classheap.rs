use super::super::gc::gc::GcType;
use super::class::Class;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ClassHeap {
    pub class_map: HashMap<String, GcType<Class>>,
}

impl ClassHeap {
    pub fn new() -> Self {
        ClassHeap {
            class_map: HashMap::new(),
        }
    }

    pub fn get_class(&self, class_name: &str) -> Option<GcType<Class>> {
        self.class_map
            .get(class_name)
            .and_then(|class| Some(*class))
    }

    pub fn load_class(&mut self, class_name: &str, class: GcType<Class>) -> Option<()> {
        let class = unsafe { &mut *class };
        class.load_classfile(class_name);
        self.add_class(class);
        Some(())
    }

    pub fn add_class(&mut self, class: GcType<Class>) -> Option<()> {
        let class = unsafe { &mut *class };
        self.class_map.insert(class.get_name()?.to_owned(), class);
        Some(())
    }
}
