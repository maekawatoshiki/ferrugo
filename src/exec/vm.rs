use super::super::class::classfile::attribute::Attribute;
use super::super::class::classfile::constant::Constant;
use super::super::class::classfile::method::MethodInfo;
use super::super::class::classheap::ClassHeap;
use super::super::gc::gc::GcType;
use super::frame::{Frame, Variable};
use super::objectheap::ObjectHeap;

#[derive(Debug)]
pub struct VM {
    pub classheap: Option<GcType<ClassHeap>>,
    pub objectheap: Option<GcType<ObjectHeap>>,
    pub frame_stack: Vec<Frame>,
    pub stack: Vec<Variable>,
    pub bp: usize,
}

impl VM {
    pub fn new() -> Self {
        VM {
            classheap: None,
            objectheap: None,
            frame_stack: {
                let mut frame_stack = Vec::with_capacity(128);
                frame_stack.push(Frame::new());
                frame_stack
            },
            stack: {
                let mut stack = vec![];
                for _ in 0..128 {
                    stack.push(Variable::Int(0));
                }
                stack
            },
            bp: 0,
        }
    }
}

impl VM {
    pub fn run(&mut self) -> Inst::Code {
        macro_rules! frame {
            () => {{
                self.frame_stack.last_mut().unwrap()
            }};
        }

        let code =
            if let Some(Attribute::Code { code, .. }) = frame!().method_info.get_code_attribute() {
                code.clone()
            } else {
                panic!()
            };

        loop {
            let cur_code = code[frame!().pc as usize];

            match cur_code {
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        Variable::Int(cur_code as i32 - Inst::iconst_0 as i32);
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + cur_code as usize - Inst::istore_0 as usize] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::iload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::sipush => {
                    let mut frame = frame!();
                    let val = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    self.stack[self.bp + frame.sp] = Variable::Short(val);
                    frame.sp += 1;
                    frame.pc += 3;
                }
                Inst::ldc => {
                    let mut frame = frame!();
                    let index = code[frame.pc + 1] as usize;
                    let val = match unsafe { &*frame.class.unwrap() }.classfile.constant_pool[index]
                    {
                        Constant::IntegerInfo { i } => Variable::Int(i),
                        Constant::FloatInfo { f } => Variable::Float(f),
                        _ => unimplemented!(),
                    };
                    self.stack[self.bp + frame.sp] = val;
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::aload_0 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::aload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::bipush => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] = Variable::Char(code[frame.pc + 1] as i8);
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::iadd => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            + self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iinc => {
                    let mut frame = frame!();
                    let index = code[frame.pc + 1] as usize;
                    let const_ = code[frame.pc + 2];
                    match self.stack[self.bp + index] {
                        Variable::Char(ref mut n) => *n += const_ as i8,
                        Variable::Short(ref mut n) => *n += const_ as i16,
                        Variable::Int(ref mut n) => *n += const_ as i32,
                        _ => panic!("must be int"),
                    }
                    frame.pc += 3;
                }
                Inst::invokestatic => {
                    self.run_invoke_static(false);
                    frame!().pc += 3;
                }
                Inst::invokevirtual => {
                    self.run_invoke_static(true);
                    frame!().pc += 3;
                }
                Inst::pop => {
                    let mut frame = frame!();
                    // frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dup => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::goto => {
                    let mut frame = frame!();
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    frame.pc = (frame.pc as isize + branch as isize) as usize;
                }
                Inst::if_icmpgt => {
                    let mut frame = frame!();
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val1 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val2 = self.stack[self.bp + frame.sp - 2].get_int();
                    if val1 < val2 {
                        frame.pc = (frame.pc as isize + branch as isize) as usize;
                    } else {
                        frame.pc += 3;
                    }
                    frame.sp -= 2;
                }
                Inst::ireturn => {
                    self.stack[self.bp] =
                        Variable::Int(self.stack[self.bp + frame!().sp - 1].get_int());
                    return Inst::ireturn;
                }
                Inst::return_ => {
                    return Inst::return_;
                }
                Inst::getstatic => {
                    let index = {
                        let frame = frame!();
                        ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
                    };
                    self.run_get_static(index);
                    frame!().pc += 3;
                }
                e => unimplemented!("{}", e),
            }
        }
    }

    fn run_get_static(&mut self, index: usize) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };

        let const_pool = frame_class.classfile.constant_pool[index].clone();
        let (class_index, name_and_type_index) = if let Constant::FieldrefInfo {
            class_index,
            name_and_type_index,
        } = const_pool
        {
            (class_index, name_and_type_index)
        } else {
            panic!()
        };

        let const_pool = frame_class.classfile.constant_pool[class_index as usize].clone();
        let name_index = if let Constant::ClassInfo { name_index } = const_pool {
            name_index
        } else {
            panic!()
        };

        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        let class = unsafe { &*self.classheap.unwrap() }
            .class_map
            .get(class_name)
            .unwrap();
            
        println!("getstatic: {:?}, class {:?}", const_pool, class_name);
    }

    fn run_invoke_static(&mut self, is_invoke_virtual: bool) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let mref_index =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            } else {
                panic!()
            };
        let const_pool = frame_class.classfile.constant_pool[mref_index].clone();

        let (class_index, name_and_type_index) = if let Constant::MethodrefInfo {
            class_index,
            name_and_type_index,
        } = const_pool
        {
            (class_index, name_and_type_index)
        } else {
            panic!()
        };

        let const_pool = unsafe { &*frame!().class.unwrap() }.classfile.constant_pool
            [class_index as usize]
            .clone();

        let name_index = if let Constant::ClassInfo { name_index } = const_pool {
            name_index
        } else {
            panic!()
        };

        let class_name = unsafe { &*frame!().class.unwrap() }.classfile.constant_pool
            [name_index as usize]
            .get_utf8()
            .unwrap();

        let class = unsafe { &*self.classheap.unwrap() }
            .class_map
            .get(class_name)
            .unwrap();

        let const_pool = unsafe { &*frame!().class.unwrap() }.classfile.constant_pool
            [name_and_type_index as usize]
            .clone();

        let mut method = MethodInfo::new();

        if let Constant::NameAndTypeInfo {
            name_index,
            descriptor_index,
        } = const_pool
        {
            method.name_index = name_index;
            method.descriptor_index = descriptor_index;
        }

        method.access_flags = 0;

        let name = unsafe { &*frame!().class.unwrap() }.classfile.constant_pool
            [method.name_index as usize]
            .get_utf8()
            .unwrap();
        let descriptor = unsafe { &*frame!().class.unwrap() }.classfile.constant_pool
            [method.descriptor_index as usize]
            .get_utf8()
            .unwrap();
        println!("{}.{}:{}", class_name, name, descriptor);

        // println!("{:?}", unsafe { &**class });

        let (virtual_class, method2) = unsafe { &**class }.get_method(name, descriptor).unwrap();

        let former_sp = frame!().sp as usize;

        self.frame_stack.push(Frame::new());

        frame!().pc = 0;
        frame!().method_info = method2.clone();

        method.access_flags = frame!().method_info.access_flags;

        frame!().class = if method.access_flags & 0x0020/*=ACC_SUPER*/> 0 {
            Some(unsafe { &*virtual_class }.get_super_class().unwrap())
        } else {
            Some(virtual_class)
        };

        let params_num = {
            // 	//todo: long/double takes 2 stack position
            let mut count = 0usize;
            let mut i = 1;
            while i < descriptor.len() {
                if descriptor.chars().nth(i).unwrap() == 'L' {
                    while descriptor.chars().nth(i).unwrap() != ';' {
                        i += 1
                    }
                }
                if descriptor.chars().nth(i).unwrap() == ')' {
                    break;
                }
                while descriptor.chars().nth(i).unwrap() == 'J'
                    || descriptor.chars().nth(i).unwrap() == 'D'
                {
                    count += 1;
                }
                count += 1;
                i += 1
            }

            if is_invoke_virtual {
                count + 1
            } else {
                count
            }
        };

        let mut discard_stack = params_num;

        println!("native -> {}", frame!().method_info.access_flags & 0x0100);
        if let Some(Attribute::Code { max_locals, .. }) = frame!().method_info.get_code_attribute()
        {
            // TODO: method_info.access_flags & ACC_NATIVE => do not add max_locals
            discard_stack += *max_locals as usize;
        } else {
            panic!()
        };

        frame!().sp = discard_stack;
        self.bp += former_sp - params_num;

        self.run();

        self.bp -= former_sp - params_num;
        self.frame_stack.pop();
    }
}

#[rustfmt::skip]
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
mod Inst {
    pub type Code = u8;
    pub const iconst_m1:    u8 = 2;
    pub const iconst_0:     u8 = 3;
    pub const iconst_1:     u8 = 4;
    pub const iconst_2:     u8 = 5;
    pub const iconst_3:     u8 = 6;
    pub const iconst_4:     u8 = 7;
    pub const iconst_5:     u8 = 8;
    pub const sipush:       u8 = 17;
    pub const ldc:          u8 = 18;
    pub const aload_0:      u8 = 42;
    pub const istore_0:     u8 = 59;
    pub const istore_1:     u8 = 60;
    pub const istore_2:     u8 = 61;
    pub const istore_3:     u8 = 62;
    pub const iload_0:      u8 = 26;
    pub const iload_1:      u8 = 27;
    pub const iload_2:      u8 = 28;
    pub const iload_3:      u8 = 29;
    pub const bipush:       u8 = 16;
    pub const pop:          u8 = 87;
    pub const dup:          u8 = 89;
    pub const iadd:         u8 = 96;
    pub const iinc:         u8 = 132;
    pub const if_icmpgt:    u8 = 163;
    pub const goto:         u8 = 167;
    pub const ireturn:      u8 = 172;
    pub const return_:      u8 = 177;
    pub const getstatic:    u8 = 178;
    pub const invokevirtual:u8 = 182;
    pub const invokestatic: u8 = 184;
}
