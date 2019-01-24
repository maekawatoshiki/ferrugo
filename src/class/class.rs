use super::super::exec::frame::Variable;
use super::super::exec::jit::{FuncJITExecInfo, LoopJITExecInfo};
use super::super::exec::objectheap::ObjectHeap;
use super::super::gc::gc::GcType;
use super::classfile::read::ClassFileReader;
use super::classfile::{
    classfile::ClassFile, constant::Constant, field::FieldInfo, method::MethodInfo,
};
use super::classheap::ClassHeap;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct JITInfoManager {
    loop_count: FxHashMap<usize, (usize, usize, Option<LoopJITExecInfo>)>, // loop (start, (end, count, exec_info)
    whole_method: (usize, Option<FuncJITExecInfo>),                        // count, function addr
}

#[derive(Debug, Clone)]
pub struct Class {
    pub classfile: ClassFile,
    pub classheap: Option<GcType<ClassHeap>>,
    pub static_variables: FxHashMap<String, Variable>,
    pub jit_info_mgr: FxHashMap<(/*(name_index, descriptor_index)=*/ usize, usize), JITInfoManager>,
}

impl Class {
    pub fn new() -> Self {
        Class {
            classfile: ClassFile::new(),
            classheap: None,
            static_variables: FxHashMap::default(),
            jit_info_mgr: FxHashMap::default(),
        }
    }

    pub fn get_static_variable(&self, name: &str) -> Option<Variable> {
        self.static_variables
            .get(name)
            .and_then(|var| Some(var.clone()))
    }

    pub fn put_static_variable(&mut self, name: &str, val: Variable) {
        self.static_variables.insert(name.to_string(), val);
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

    pub fn get_utf8_from_const_pool(&self, index: usize) -> Option<&String> {
        self.classfile.constant_pool[index].get_utf8()
    }

    pub fn get_java_string_utf8_from_const_pool(
        &mut self,
        objectheap: GcType<ObjectHeap>,
        index: usize,
    ) -> Option<Variable> {
        let (s, java_string) = match self.classfile.constant_pool[index] {
            Constant::Utf8 {
                ref s,
                ref mut java_string,
            } => (s, java_string),
            _ => return None,
        };

        if let Some(java_string) = java_string {
            return Some(*java_string);
        }

        let jstring =
            unsafe { &mut *objectheap }.create_string_object(s.clone(), self.classheap.unwrap());
        *java_string = Some(jstring);

        Some(jstring)
    }

    pub fn get_method(
        &self,
        method_name: &str,
        method_descriptor: &str,
    ) -> Option<(GcType<Class>, MethodInfo)> {
        let mut cur_class_ptr = unsafe { &(*self.classheap.unwrap()) }
            .get_class(self.get_name().unwrap())
            .unwrap();

        loop {
            let cur_class = unsafe { &mut *cur_class_ptr };

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

    pub fn get_field(
        &self,
        field_name: &str,
        field_descriptor: &str,
    ) -> Option<(GcType<Class>, FieldInfo)> {
        let mut cur_class_ptr = unsafe { &(*self.classheap.unwrap()) }
            .get_class(self.get_name().unwrap())
            .unwrap();

        loop {
            let cur_class = unsafe { &mut *cur_class_ptr };

            for i in 0..cur_class.classfile.fields_count as usize {
                let name = cur_class.classfile.constant_pool
                    [(cur_class.classfile.fields[i].name_index) as usize]
                    .get_utf8()
                    .unwrap();
                if name != field_name {
                    continue;
                }

                let descriptor = cur_class.classfile.constant_pool
                    [(cur_class.classfile.fields[i].descriptor_index) as usize]
                    .get_utf8()
                    .unwrap();
                if descriptor == field_descriptor {
                    return Some((cur_class_ptr, cur_class.classfile.fields[i].clone()));
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
        unsafe { &(*self.classheap.unwrap()) }.get_class(name)
    }

    pub fn get_object_field_count(&self) -> usize {
        let mut count = self.classfile.fields_count as usize;
        if let Some(super_class) = self.get_super_class() {
            count += unsafe { &*super_class }.get_object_field_count();
        }
        count
    }

    pub fn get_jit_info_mgr(
        &mut self,
        name_index: usize,
        descriptor_index: usize,
    ) -> &mut JITInfoManager {
        self.jit_info_mgr
            .entry((name_index, descriptor_index))
            .or_insert(JITInfoManager::new())
    }
}

impl JITInfoManager {
    pub fn new() -> Self {
        JITInfoManager {
            loop_count: FxHashMap::default(),
            whole_method: (0, None),
        }
    }

    pub fn inc_count_of_loop_exec(&mut self, start: usize, end: usize) {
        let (_, count, _) = self.loop_count.entry(start).or_insert((end, 0, None));
        *count += 1;
    }

    pub fn inc_count_of_func_exec(&mut self) {
        let (count, _) = &mut self.whole_method;
        *count += 1;
    }

    pub fn loop_executed_enough_times(&self, start: usize) -> bool {
        let (_, count, _) = self.loop_count.get(&start).unwrap();
        *count > 7
        // false
    }

    pub fn func_executed_enough_times(&self) -> bool {
        let (count, _) = &self.whole_method;
        *count > 4
        // false
    }

    pub fn get_jit_loop(&mut self, start: usize) -> &mut Option<LoopJITExecInfo> {
        &mut (*self.loop_count.get_mut(&start).unwrap()).2
    }

    pub fn get_jit_func(&mut self) -> &mut Option<FuncJITExecInfo> {
        &mut self.whole_method.1
    }
}
