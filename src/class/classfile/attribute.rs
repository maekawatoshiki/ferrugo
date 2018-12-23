#[derive(Debug, Clone)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    pub info: Attribute,
}

#[derive(Debug, Clone)]
pub enum Attribute {
    Code {
        max_stack: u16,
        max_locals: u16,
        code_length: u32,
        code: Vec<u8>,
        exception_table_length: u16,
        exception_table: Vec<Exception>,
        attributes_count: u16,
        attributes: Vec<AttributeInfo>,
    },
    LineNumberTable {
        line_number_table_length: u16,
        line_number_table: Vec<LineNumber>,
    },
    SourceFile {
        sourcefile_index: u16,
    },
    StackMapTable {
        number_of_entries: u16,
        entries: Vec<StackMapFrame>,
    },
    Signature {
        signature_index: u16,
    },
    Exceptions {
        number_of_exceptions: u16,
        exception_index_table: Vec<u16>,
    },
    Deprecated,
    RuntimeVisibleAnnotations {
        num_annotations: u16,
        annotations: Vec<Annotation>,
    },
    InnerClasses {
        number_of_classes: u16,
        classes: Vec<InnerClassesBody>,
    },
}

#[derive(Debug, Clone)]
pub struct InnerClassesBody {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16,
}

#[derive(Debug, Clone)]
pub struct Exception {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumber {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct StackMapFrame {
    pub frame_type: u8,
    pub body: StackMapFrameBody,
}

#[derive(Debug, Clone)]
pub enum StackMapFrameBody {
    SameFrame,
    SameLocals1StackItemFrame {
        stack: VerificationTypeInfo,
    },
    AppendFrame {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
    },
    ChopFrame {
        offset_delta: u16,
    },
    FullFrame {
        offset_delta: u16,
        number_of_locals: u16,
        locals: Vec<VerificationTypeInfo>,
        number_of_stack_items: u16,
        stack: Vec<VerificationTypeInfo>,
    },
}

#[derive(Debug, Clone)]
pub enum VerificationTypeInfo {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    UninitializedThis,
    Object { cpool_index: u16 },
    Uninitialized,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[derive(Clone, Debug)]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue,
}

#[derive(Clone, Debug)]
pub struct ElementValue {
    // TODO: Implement
}
