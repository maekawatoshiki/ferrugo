use super::classfile::classfile::ClassFile;
use super::classfile::read::ClassFileReader;
use super::classheap::ClassHeap;

#[derive(Debug, Clone)]
pub struct Class {
    pub classfile: ClassFile,
}

impl Class {
    pub fn new() -> Self {
        Class {
            classfile: ClassFile::new(),
        }
    }

    pub fn load_classfile(&mut self, filename: String) -> Option<()> {
        let mut cf_reader = ClassFileReader::new(filename.as_str())?;
        let cf = cf_reader.read()?;
        self.classfile = cf;
        Some(())
    }

    pub fn get_name(&mut self) -> Option<&String> {
        let this_class = self.classfile.this_class as usize;
        let const_class = &self.classfile.constant_pool[this_class];
        self.classfile.constant_pool[const_class.get_class_name_index()?].get_utf8()
    }
}
