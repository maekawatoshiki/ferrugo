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

#[derive(Debug, Clone)]
pub enum Variable {
    Char(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    Double(f64),
    Object(GcType<ObjectBody>),
    Pointer(GcType<u64>),
}

#[derive(Debug, Clone)]
pub struct ObjectBody {
    pub class: Variable,
    pub variables: FxHashMap<String, Variable>,
}

impl Variable {
    pub fn get_int(&self) -> i32 {
        match self {
            Variable::Char(n) => *n as i32,
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

    pub fn get_pointer(&self) -> GcType<u64> {
        match self {
            Variable::Pointer(ptr) => *ptr,
            _ => panic!("hmm"),
        }
    }

    pub fn get_object(&self) -> GcType<ObjectBody> {
        match self {
            Variable::Object(body) => *body,
            _ => panic!("huh"),
        }
    }
}
