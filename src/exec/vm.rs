use super::super::class::classfile::attribute::Attribute;
use super::frame::{Frame, Variable};
#[derive(Debug)]
pub struct VM {}

impl VM {
    pub fn new() -> Self {
        VM {}
    }
}

impl VM {
    pub fn run(&self, frame_stack: &mut Vec<Frame>) -> Inst::Code {
        let mut frame = &mut frame_stack[0];
        let base_pc = frame.pc;
        let (_code_length, code) = if let Some(Attribute::Code {
            code, code_length, ..
        }) = frame.method_info.get_code_attribute()
        {
            (code_length, code)
        } else {
            panic!()
        };

        loop {
            let cur_code = code[(base_pc + frame.pc) as usize];
            match cur_code {
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {
                    frame.sp += 1;
                    frame.stack[frame.sp as usize] =
                        Variable::Int(cur_code as i32 - Inst::iconst_0 as i32);
                    frame.pc += 1;
                }
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                    frame.stack[cur_code as usize - Inst::istore_0 as usize] =
                        frame.stack[frame.sp as usize].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    frame.sp += 1;
                    frame.stack[frame.sp as usize] =
                        frame.stack[cur_code as usize - Inst::iload_0 as usize].clone();
                    frame.pc += 1;
                }
                Inst::bipush => {
                    frame.sp += 1;
                    frame.stack[frame.sp as usize] =
                        Variable::Char(code[(base_pc + frame.pc + 1) as usize] as i8);
                    frame.pc += 2;
                }
                Inst::iadd => {
                    frame.stack[frame.sp as usize - 1] = Variable::Int(
                        frame.stack[frame.sp as usize - 1].get_int()
                            + frame.stack[frame.sp as usize].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::ireturn => {
                    frame.stack[0] = Variable::Int(frame.stack[frame.sp as usize].get_int());
                    return Inst::ireturn;
                }
                _ => unimplemented!(),
            }
        }
    }
}

#[rustfmt::skip]
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
mod Inst {
    pub type Code = u8;
    pub const iconst_m1: u8 = 2;
    pub const iconst_0:  u8 = 3;
    pub const iconst_1:  u8 = 4;
    pub const iconst_2:  u8 = 5;
    pub const iconst_3:  u8 = 6;
    pub const iconst_4:  u8 = 7;
    pub const iconst_5:  u8 = 8;
    pub const istore_0:  u8 = 59;
    pub const istore_1:  u8 = 60;
    pub const istore_2:  u8 = 61;
    pub const istore_3:  u8 = 62;
    pub const iload_0:   u8 = 26;
    pub const iload_1:   u8 = 27;
    pub const iload_2:   u8 = 28;
    pub const iload_3:   u8 = 29;
    pub const bipush:    u8 = 16;
    pub const iadd:      u8 = 96;
    pub const ireturn:   u8 = 172;
}
