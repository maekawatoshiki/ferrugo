use super::super::class::class::Class;
use super::super::class::classfile::method::MethodInfo;
use super::super::gc::gc::GcType;
use std::ptr;

#[derive(Debug, Clone)]
pub struct Frame {
    pub class: Option<GcType<Class>>,
    pub method_info: MethodInfo,
    pub pc: usize,
    pub sp: usize,
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            class: None,
            method_info: MethodInfo::new(),
            pc: 0,
            sp: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuickInfo {
    pub method: MethodInfo,
    pub class: GcType<Class>,
    pub params_num: usize,
    pub ret_ty: VariableType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableType {
    Void,
    Int,
    Pointer,
    Double,
    Long,
}

#[derive(Debug, Clone)]
pub struct ObjectBody {
    pub class: GcType<Class>,
    pub variables: Vec<u64>,
}

#[derive(Debug, Clone)]
pub enum AType {
    Boolean,
    Char,
    Float,
    Double,
    Byte,
    Short,
    Int,
    Long,
    Class(GcType<Class>),
    Multi {
        element_type: Box<AType>,
        dimensions: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Array {
    pub atype: AType,
    pub elements: Vec<u8>,
    // TODO: Treat as special. Need a better way.
    pub string: Option<String>,
}

impl Array {
    pub fn new(atype: AType, len: usize, string: Option<String>) -> Array {
        Array {
            elements: unsafe {
                let actual_len = len * atype.size_in_byte();
                let mut vec: Vec<u8> = Vec::with_capacity(actual_len);
                vec.set_len(actual_len);
                vec
            },
            atype,
            string,
        }
    }

    pub fn get_length(&self) -> usize {
        if let Some(ref string) = self.string {
            string.len()
        } else {
            self.elements.len() / self.atype.size_in_byte()
        }
    }

    pub fn at<T: Into<u64>>(&self, index: isize) -> u64 {
        unsafe { ptr::read((self.elements.as_ptr() as *const T).offset(index)).into() }
    }

    pub fn store<T>(&mut self, index: isize, val: T) {
        unsafe { ptr::write((self.elements.as_mut_ptr() as *mut T).offset(index), val) }
    }
}

impl AType {
    // https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-6.html#jvms-6.5.sastore
    pub fn to_atype(n: usize) -> AType {
        match n {
            4 => AType::Boolean,
            5 => AType::Char,
            6 => AType::Float,
            7 => AType::Double,
            8 => AType::Byte,
            9 => AType::Short,
            10 => AType::Int,
            11 => AType::Long,
            _ => panic!(),
        }
    }

    pub fn to_num(&self) -> usize {
        match self {
            AType::Boolean => 4,
            AType::Char => 5,
            AType::Float => 6,
            AType::Double => 7,
            AType::Byte => 8,
            AType::Short => 9,
            AType::Int => 10,
            AType::Long => 11,
            _ => panic!(),
        }
    }

    pub fn size_in_byte(&self) -> usize {
        match self {
            AType::Boolean => 1,
            AType::Char => 2,
            AType::Float => 4,
            AType::Double => 8,
            AType::Byte => 1,
            AType::Short => 2,
            AType::Int => 4,
            AType::Long => 8,
            // TODO: This seems to be correct only on 64-bit platforms
            AType::Class(_) => 8,
            AType::Multi { .. } => 8,
        }
    }
}

impl ObjectBody {
    pub fn get_string_mut(&mut self) -> &mut String {
        unsafe { &mut *(self.variables[0] as GcType<Array>) }
            .string
            .as_mut()
            .unwrap()
    }
}

impl VariableType {
    pub fn parse_return_type(descriptor: &str) -> Option<VariableType> {
        // https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
        let mut i = 1;

        while i < descriptor.len() {
            let c = descriptor.chars().nth(i).unwrap();
            i += 1;
            if c == ')' {
                break;
            }
        }

        let c = descriptor.chars().nth(i).unwrap();
        let ty = match c {
            'L' => {
                while descriptor.chars().nth(i).unwrap() != ';' {
                    i += 1
                }
                VariableType::Pointer
            }
            '[' => VariableType::Pointer, // TODO
            'V' => VariableType::Void,
            'I' => VariableType::Int,
            'Z' => VariableType::Int,
            'J' => VariableType::Long,
            'D' => VariableType::Double,
            _ => return None,
        };

        Some(ty)
    }

    pub fn parse_type(descriptor: &str) -> Option<VariableType> {
        let c = descriptor.chars().nth(0).unwrap();
        let ty = match c {
            'L' => VariableType::Pointer,
            '[' => VariableType::Pointer,
            'V' => VariableType::Void,
            'I' => VariableType::Int,
            'Z' => VariableType::Int,
            'J' => VariableType::Long,
            'D' => VariableType::Double,
            e => panic!("{:?}", e),
        };
        Some(ty)
    }
}
