#[derive(Debug, Clone)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    pub info: Attribute,
}

#[derive(Debug, Clone)]
pub enum Attribute {
    Code {
        attribute_name_index: u16,
        attribute_length: u16,
        max_stack: u16,
        max_locals: u16,
        code_length: u32,
        code: Vec<u8>,
        exception_table_length: u16,
        exception_table: Vec<Exception>,
        attributes_count: u16,
        attributes: Vec<AttributeInfo>,
    },
}

#[derive(Debug, Clone)]
pub struct Exception {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}
