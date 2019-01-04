use super::attribute::{Attribute, AttributeInfo};

#[derive(Clone, Debug)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

impl MethodInfo {
    pub fn new() -> Self {
        MethodInfo {
            access_flags: 0,
            name_index: 0,
            descriptor_index: 0,
            attributes_count: 0,
            attributes: vec![],
        }
    }

    pub fn get_code_attribute(&self) -> Option<&Attribute> {
        for i in 0..self.attributes_count as usize {
            match self.attributes[i].info {
                Attribute::Code { .. } => return Some(&self.attributes[i].info),
                _ => {}
            };
        }
        None
    }

    pub fn check_access_flags(&self, flag: u16) -> bool {
        (self.access_flags & flag) > 0
    }
}

#[rustfmt::skip]
#[allow(dead_code)]
pub mod access_flags {
    pub const ACC_PUBLIC:            u16 = 0x0001;
    pub const ACC_PACC_PRIVATE:      u16 = 0x0002;
    pub const ACC_PACC_PROTECTED:    u16 = 0x0004;
    pub const ACC_PACC_STATIC:       u16 = 0x0008;
    pub const ACC_PACC_FINAL:        u16 = 0x0010;
    pub const ACC_PACC_SYNCHRONIZED: u16 = 0x0020;
    pub const ACC_PACC_BRIDGE:       u16 = 0x0040;
    pub const ACC_PACC_VARARGS:      u16 = 0x0080;
    pub const ACC_PACC_NATIVE:       u16 = 0x0100;
    pub const ACC_PACC_ABSTRACT:     u16 = 0x0400;
    pub const ACC_PACC_STRICT:       u16 = 0x0800;
    pub const ACC_PACC_SYNTHETIC:    u16 = 0x10;
}
