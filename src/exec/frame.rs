use super::super::class::class::Class;
use super::super::class::classfile::method::MethodInfo;
use super::super::gc::gc::GcType;
use rustc_hash::FxHashMap;

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

#[derive(Debug, Clone, Copy)]
pub enum Variable {
    Byte(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    Double(f64),
    Pointer(GcType<u64>),
}

#[derive(Debug, Clone)]
pub struct ObjectBody {
    pub class: Variable,
    pub variables: FxHashMap<String, Variable>,
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
}

#[derive(Debug, Clone)]
pub struct Array {
    pub atype: AType,
    pub elements: Vec<Variable>,
    // TODO: Treat as special. Need a better way.
    pub string: Option<String>,
}

impl Array {
    pub fn get_length(&self) -> usize {
        if let Some(ref string) = self.string {
            string.len()
        } else {
            self.elements.len()
        }
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
            AType::Class(_) => panic!(),
        }
    }
}

impl Variable {
    pub fn get_int(&self) -> i32 {
        match self {
            Variable::Byte(n) => *n as i32,
            Variable::Short(n) => *n as i32,
            Variable::Int(n) => *n,
            _ => panic!("what?"),
        }
    }

    pub fn get_double(&self) -> f64 {
        match self {
            Variable::Double(f) => *f,
            _ => panic!("what?"),
        }
    }

    pub fn get_pointer<T>(&self) -> GcType<T> {
        match self {
            Variable::Pointer(ptr) => *ptr as GcType<T>,
            _ => panic!("hmm"),
        }
    }
}

impl ObjectBody {
    pub fn get_string_mut(&self) -> &mut String {
        unsafe { &mut *(self.variables.get("value").unwrap().get_pointer::<Array>()) }
            .string
            .as_mut()
            .unwrap()
    }
}
