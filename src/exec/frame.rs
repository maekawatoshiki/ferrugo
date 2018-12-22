use super::super::class::class::Class;
use super::super::class::classfile::method::MethodInfo;
use super::super::gc::gc::GcType;

#[derive(Debug, Clone)]
pub struct Frame {
    pub class: Option<GcType<Class>>,
    pub method_info: MethodInfo,
    pub pc: u32,
    pub sp: u16,
    pub stack: Vec<Variable>,
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            class: None,
            method_info: MethodInfo::new(),
            pc: 0,
            sp: 0,
            stack: vec![],
        }
    }

    pub fn init_stack(&mut self) {
        for _ in 0..100 {
            self.stack.push(Variable::Int(1));
        }
    }
}

#[derive(Debug, Clone)]
pub enum Variable {
    Char(u8),
    Short(u16),
    Int(u32),
    Float(f32),
}
