use super::attribute::AttributeInfo;
use super::constant::Constant;
use super::field::FieldInfo;
use super::method::MethodInfo;

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub magic: u32,
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool_count: u16,
    pub constant_pool: Vec<Constant>,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces_count: u16,
    pub interfaces: Vec<Constant>,
    pub fields_count: u16,
    pub fields: Vec<FieldInfo>,
    pub methods_count: u16,
    pub methods: Vec<MethodInfo>,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

impl ClassFile {
    pub fn new() -> Self {
        ClassFile {
            magic: 0,
            minor_version: 0,
            major_version: 0,
            constant_pool_count: 0,
            constant_pool: vec![],
            access_flags: 0,
            this_class: 0,
            super_class: 0,
            interfaces_count: 0,
            interfaces: vec![],
            fields_count: 0,
            fields: vec![],
            methods_count: 0,
            methods: vec![],
            attributes_count: 0,
            attributes: vec![],
        }
    }
}
