use super::super::gc::gc::GcType;
use super::classfile::read::ClassFileReader;
use super::classfile::{classfile::ClassFile, method::MethodInfo};
use super::classheap::ClassHeap;

#[derive(Debug, Clone)]
pub struct Class {
    pub classfile: ClassFile,
    pub classheap: Option<GcType<ClassHeap>>,
}

impl Class {
    pub fn new() -> Self {
        Class {
            classfile: ClassFile::new(),
            classheap: None,
        }
    }

    pub fn load_classfile(&mut self, filename: &str) -> Option<()> {
        let mut cf_reader = ClassFileReader::new(filename)?;
        let cf = cf_reader.read()?;
        self.classfile = cf;
        Some(())
    }

    pub fn get_name(&self) -> Option<&String> {
        let this_class = self.classfile.this_class as usize;
        let const_class = &self.classfile.constant_pool[this_class];
        self.classfile.constant_pool[const_class.get_class_name_index()?].get_utf8()
    }

    pub fn get_super_class_name(&self) -> Option<&String> {
        let super_class = self.classfile.super_class as usize;
        let const_class = &self.classfile.constant_pool[super_class];
        self.classfile.constant_pool[const_class.get_class_name_index()?].get_utf8()
    }

    pub fn get_method(
        &self,
        method_name: &str,
        method_descriptor: &str,
    ) -> Option<(GcType<Class>, MethodInfo)> {
        let mut cur_class_ptr = *unsafe { &(*self.classheap.unwrap()) }
            .class_map
            .get(self.get_name().unwrap())
            .unwrap();

        loop {
            let mut cur_class = unsafe { &mut *cur_class_ptr };

            for i in 0..cur_class.classfile.methods_count as usize {
                let name = cur_class.classfile.constant_pool
                    [(cur_class.classfile.methods[i].name_index) as usize]
                    .get_utf8()
                    .unwrap();
                if name != method_name {
                    continue;
                }

                let descriptor = cur_class.classfile.constant_pool
                    [(cur_class.classfile.methods[i].descriptor_index) as usize]
                    .get_utf8()
                    .unwrap();
                if descriptor == method_descriptor {
                    return Some((cur_class_ptr, cur_class.classfile.methods[i].clone()));
                }
            }

            if let Some(x) = cur_class.get_super_class() {
                cur_class_ptr = x;
            } else {
                break;
            }
        }
        None
    }

    pub fn get_super_class(&self) -> Option<GcType<Class>> {
        let name = self.get_super_class_name()?;
        if let Some(x) = unsafe { &(*self.classheap.unwrap()) }
            .class_map
            .get(self.get_name().unwrap())
        {
            Some(*x)
        } else {
            None
        }
    }
}
