#[derive(Debug, Clone)]
pub enum ConstantType {
    Class,
    Fieldref,
    Methodref,
    InterfaceMethodref,
    String,
    Integer,
    Float,
    Long,
    Double,
    NameAndType,
    Utf8,
    MethodHandle,
    MethodType,
    InvokeDynamic,
}

impl ConstantType {
    pub fn value(&self) -> usize {
        match self {
            ConstantType::Class => 7,
            ConstantType::Fieldref => 9,
            ConstantType::Methodref => 10,
            ConstantType::InterfaceMethodref => 11,
            ConstantType::String => 8,
            ConstantType::Integer => 3,
            ConstantType::Float => 4,
            ConstantType::Long => 5,
            ConstantType::Double => 6,
            ConstantType::NameAndType => 12,
            ConstantType::Utf8 => 1,
            ConstantType::MethodHandle => 15,
            ConstantType::MethodType => 16,
            ConstantType::InvokeDynamic => 18,
        }
    }
}

pub fn u8_to_constant_type(val: u8) -> Option<ConstantType> {
    match val {
        7 => Some(ConstantType::Class),
        9 => Some(ConstantType::Fieldref),
        10 => Some(ConstantType::Methodref),
        11 => Some(ConstantType::InterfaceMethodref),
        8 => Some(ConstantType::String),
        3 => Some(ConstantType::Integer),
        4 => Some(ConstantType::Float),
        5 => Some(ConstantType::Long),
        6 => Some(ConstantType::Double),
        12 => Some(ConstantType::NameAndType),
        1 => Some(ConstantType::Utf8),
        15 => Some(ConstantType::MethodHandle),
        16 => Some(ConstantType::MethodType),
        18 => Some(ConstantType::InvokeDynamic),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub enum Constant {
    MethodrefInfo {
        class_index: u16,
        name_and_type_index: u16,
    },
    FieldrefInfo {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodrefInfo {
        class_index: u16,
        name_and_type_index: u16,
    },
    String {
        string_index: u16,
    },
    ClassInfo {
        name_index: u16,
    },
    Utf8 {
        s: String,
    },
    NameAndTypeInfo {
        name_index: u16,
        descriptor_index: u16,
    },
    IntegerInfo {
        i: i32,
    },
    FloatInfo {
        f: f32,
    },
    LongInfo {
        i: i64,
    },
    DoubleInfo {
        f: f64,
    },
    MethodHandleInfo {
        reference_kind: u8,
        reference_index: u16,
    },
    MethodTypeInfo {
        descriptor_index: u16,
    },
    InvokeDynamicInfo {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    None,
}

impl Constant {
    pub fn get_utf8(&self) -> Option<&String> {
        match self {
            Constant::Utf8 { s } => Some(s),
            _ => None,
        }
    }

    pub fn get_class_name_index(&self) -> Option<usize> {
        match self {
            Constant::ClassInfo { name_index } => Some(*name_index as usize),
            _ => None,
        }
    }
}

