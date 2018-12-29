use super::super::class::class::Class;
use super::super::class::classfile::attribute::Attribute;
use super::super::class::classfile::constant::Constant;
use super::super::class::classfile::method::MethodInfo;
use super::super::class::classheap::ClassHeap;
use super::super::gc::{gc, gc::GcType};
use super::frame::{Frame, Object, Variable};
use super::objectheap::ObjectHeap;
use ansi_term::Colour;

#[derive(Debug)]
pub struct VM {
    pub classheap: GcType<ClassHeap>,
    pub objectheap: GcType<ObjectHeap>,
    pub frame_stack: Vec<Frame>,
    pub stack: Vec<Variable>,
    pub bp: usize,
}

impl VM {
    pub fn new(classheap: GcType<ClassHeap>, objectheap: GcType<ObjectHeap>) -> Self {
        VM {
            classheap,
            objectheap,
            frame_stack: {
                let mut frame_stack = Vec::with_capacity(128);
                frame_stack.push(Frame::new());
                frame_stack
            },
            stack: {
                let mut stack = vec![];
                for _ in 0..1024 {
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

        if frame!().method_info.access_flags & 0x0100 > 0 {
            // ACC_NATIVE
            self.run_native_method();
            return 0;
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
                Inst::dconst_0 | Inst::dconst_1 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        Variable::Double((cur_code as i64 - Inst::dconst_0 as i64) as f64);
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dstore => {
                    let mut frame = frame!();
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::istore => {
                    let mut frame = frame!();
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 2;
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
                Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::dload_0 as usize].clone();
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
                        Constant::String { string_index } => {
                            let string = unsafe { &*frame.class.unwrap() }
                                .get_utf8_from_const_pool(string_index as usize)
                                .unwrap()
                                .to_owned();
                            // TODO: Constant string refers to constant pool,
                            // so should not create a new string object.
                            // "aaa" == "aaa" // => true
                            Variable::Object(
                                unsafe { &mut *self.objectheap }
                                    .create_string_object(string, self.classheap),
                            )
                        }
                        _ => unimplemented!(),
                    };
                    self.stack[self.bp + frame.sp] = val;
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::ldc2_w => {
                    let mut frame = frame!();
                    let index = ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize;
                    let val = match unsafe { &*frame.class.unwrap() }.classfile.constant_pool[index]
                    {
                        Constant::DoubleInfo { f } => Variable::Double(f),
                        _ => unimplemented!(),
                    };
                    self.stack[self.bp + frame.sp] = val;
                    frame.sp += 1;
                    frame.pc += 3;
                }
                Inst::dload => {
                    let mut frame = frame!();
                    let index = code[frame.pc + 1] as usize;
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + index].clone();
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::iload => {
                    let mut frame = frame!();
                    let index = code[frame.pc + 1] as usize;
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + index].clone();
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::aload_0 | Inst::aload_1 | Inst::aload_2 | Inst::aload_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::aload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dstore_0 | Inst::dstore_1 | Inst::dstore_2 | Inst::dstore_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + (cur_code as usize - Inst::dstore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::astore_0 | Inst::astore_1 | Inst::astore_2 | Inst::astore_3 => {
                    let mut frame = frame!();
                    self.stack[self.bp + (cur_code as usize - Inst::astore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
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
                Inst::dadd => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            + self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::isub => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            - self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dsub => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            - self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dmul => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            * self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::ddiv => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            / self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dneg => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 1] =
                        Variable::Double(-self.stack[self.bp + frame.sp - 1].get_double());
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
                Inst::i2d => {
                    let mut frame = frame!();
                    self.stack[self.bp + frame.sp - 1] =
                        Variable::Double(self.stack[self.bp + frame.sp - 1].get_int() as f64);
                    frame.pc += 1;
                }
                Inst::invokestatic => {
                    self.run_invoke_static(true);
                    frame!().pc += 3;
                }
                Inst::invokespecial => {
                    self.run_invoke_static(false);
                    frame!().pc += 3;
                }
                Inst::invokevirtual => {
                    self.run_invoke_static(false);
                    frame!().pc += 3;
                }
                Inst::new => {
                    self.run_new();
                    frame!().pc += 3;
                }
                Inst::pop | Inst::pop2 => {
                    let mut frame = frame!();
                    frame.sp -= 1;
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
                Inst::dcmpl => {
                    let mut frame = frame!();
                    let val2 = self.stack[self.bp + frame.sp - 1].get_double();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_double();
                    frame.sp -= 2;
                    if val1 > val2 {
                        self.stack[self.bp + frame.sp] = Variable::Int(1);
                    } else if val1 == val2 {
                        self.stack[self.bp + frame.sp] = Variable::Int(0);
                    } else if val1 < val2 {
                        self.stack[self.bp + frame.sp] = Variable::Int(-1);
                    } else if val1.is_nan() || val2.is_nan() {
                        self.stack[self.bp + frame.sp] = Variable::Int(-1);
                    }
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::ifeq => {
                    let mut frame = frame!();
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1].get_int();
                    frame.sp -= 1;
                    if val == 0 {
                        frame.pc = (frame.pc as isize + branch as isize) as usize;
                    } else {
                        frame.pc += 3;
                    }
                }
                Inst::if_icmpge => {
                    let mut frame = frame!();
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_int();
                    if val1 >= val2 {
                        frame.pc = (frame.pc as isize + branch as isize) as usize;
                    } else {
                        frame.pc += 3;
                    }
                    frame.sp -= 2;
                }
                Inst::if_icmpgt => {
                    let mut frame = frame!();
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_int();
                    if val1 > val2 {
                        frame.pc = (frame.pc as isize + branch as isize) as usize;
                    } else {
                        frame.pc += 3;
                    }
                    frame.sp -= 2;
                }
                // Inst::ifnonnull => {
                //     let mut frame = frame!();
                //     let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                //     let val = self.stack[self.bp + frame.sp - 1].clone();
                //     if true {
                //         frame.pc = (frame.pc as isize + branch as isize) as usize;
                //     } else {
                //         frame.pc += 3;
                //     }
                //     frame.sp -= 1;
                // }
                Inst::ireturn => {
                    self.stack[self.bp] = self.stack[self.bp + frame!().sp - 1].clone();
                    return Inst::ireturn;
                }
                Inst::dreturn => {
                    self.stack[self.bp] = self.stack[self.bp + frame!().sp - 1].clone();
                    return Inst::dreturn;
                }
                Inst::return_ => {
                    return Inst::return_;
                }
                Inst::getstatic => self.run_get_static(),
                Inst::putstatic => self.run_put_static(),
                Inst::getfield => self.run_get_field(),
                Inst::putfield => self.run_put_field(),
                Inst::monitorenter => {
                    // TODO: Implement
                    let mut frame = frame!();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                e => unimplemented!("{}", e),
            }
        }
    }

    fn run_native_method(&mut self) {
        let frame = self.frame_stack.last_mut().unwrap();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let class_name = frame_class.get_name().unwrap();
        let method_name = frame_class
            .get_utf8_from_const_pool(frame.method_info.name_index as usize)
            .unwrap();
        let descriptor = frame_class
            .get_utf8_from_const_pool(frame.method_info.descriptor_index as usize)
            .unwrap();
        let signature = format!("{}.{}:{}", class_name, method_name, descriptor);

        match signature.as_str() {
            "java/io/PrintStream.println:(I)V" => {
                println!("{}", self.stack[self.bp + 1].get_int());
            }
            "java/io/PrintStream.println:(D)V" => {
                println!("{}", self.stack[self.bp + 1].get_double());
            }
            "java/io/PrintStream.println:(Ljava/lang/String;)V" => {
                let object_body = match &self.stack[self.bp + 1] {
                    Variable::Object(object) => unsafe {
                        &mut *(*self.objectheap).get_object(object.heap_id).unwrap()
                    },
                    _ => panic!(),
                };
                println!("{}", unsafe {
                    &*(object_body.variables.get("str").unwrap().get_pointer() as GcType<String>)
                });
            }
            // static
            "java/lang/String.valueOf:(I)Ljava/lang/String;" => {
                let i = self.stack[self.bp + 0].get_int();
                self.stack[self.bp + 0] = Variable::Object(
                    unsafe { &mut *self.objectheap }
                        .create_string_object(format!("{}", i), self.classheap),
                );
            }
            e => panic!("{:?}", e),
        }
    }

    fn run_get_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            } else {
                panic!()
            };
        frame.pc += 3;

        let objectref =
            if let Variable::Object(Object { heap_id }) = self.stack[self.bp + frame.sp - 1] {
                unsafe { &*self.objectheap }.get_object(heap_id).unwrap()
            } else {
                panic!()
            };
        frame.sp -= 1;

        let const_pool = &frame_class.classfile.constant_pool[index];
        let name_and_type_index = if let Constant::FieldrefInfo {
            name_and_type_index,
            ..
        } = const_pool
        {
            *name_and_type_index as usize
        } else {
            panic!()
        };

        let const_pool = &frame_class.classfile.constant_pool[name_and_type_index];

        let name_index = if let Constant::NameAndTypeInfo {
            name_index,..
            // descriptor_index,
        } = const_pool
        {
            *name_index as usize
        }else {panic!()};

        let name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();

        let value = unsafe { &mut *objectref }.variables.get(name).unwrap();

        self.stack[self.bp + frame.sp] = value.clone();
        frame.sp += 1;
    }

    fn run_put_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            } else {
                panic!()
            };
        frame.pc += 3;

        let objectref =
            if let Variable::Object(Object { heap_id }) = self.stack[self.bp + frame.sp - 2] {
                unsafe { &*self.objectheap }.get_object(heap_id).unwrap()
            } else {
                panic!()
            };
        let value = self.stack[self.bp + frame.sp - 1].clone();
        frame.sp -= 2;

        let const_pool = &frame_class.classfile.constant_pool[index];
        let name_and_type_index = if let Constant::FieldrefInfo {
            name_and_type_index,
            ..
        } = const_pool
        {
            *name_and_type_index as usize
        } else {
            panic!()
        };

        let const_pool = &frame_class.classfile.constant_pool[name_and_type_index];

        let name_index = if let Constant::NameAndTypeInfo {
            name_index,..
            // descriptor_index,
        } = const_pool
        {
            *name_index as usize
        }else {panic!()};

        let name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();

        unsafe { &mut *objectref }
            .variables
            .insert(name.clone(), value);
    }

    fn run_get_static(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let code =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                code.clone()
            } else {
                panic!()
            };
        let index = ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize;
        frame.pc += 3;

        let const_pool = frame_class.classfile.constant_pool[index].clone();
        let (class_index, name_and_type_index) = if let Constant::FieldrefInfo {
            class_index,
            name_and_type_index,
        } = const_pool
        {
            (class_index as usize, name_and_type_index as usize)
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

        let class = load_class(self.classheap, self.objectheap, class_name);

        let const_pool = frame_class.classfile.constant_pool[name_and_type_index].clone();

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

        let name = frame_class.classfile.constant_pool[method.name_index as usize]
            .get_utf8()
            .unwrap();
        // TODO: ``descriptor`` will be necessary to verify the field's type.
        // let descriptor = frame_class.classfile.constant_pool[method.descriptor_index as usize]
        //     .get_utf8()
        //     .unwrap();

        let object = unsafe { &*class }
            .get_static_variable(name.as_str())
            .unwrap();

        self.stack[self.bp + frame.sp] = object;
        frame.sp += 1;
    }

    fn run_put_static(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let code =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                code.clone()
            } else {
                panic!()
            };
        let index = ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize;
        frame.pc += 3;

        let const_pool = frame_class.classfile.constant_pool[index].clone();
        let (class_index, name_and_type_index) = if let Constant::FieldrefInfo {
            class_index,
            name_and_type_index,
        } = const_pool
        {
            (class_index as usize, name_and_type_index as usize)
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

        let class = *unsafe { &*self.classheap }
            .class_map
            .get(class_name)
            .unwrap();

        let const_pool = frame_class.classfile.constant_pool[name_and_type_index].clone();

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

        let name = frame_class.classfile.constant_pool[method.name_index as usize]
            .get_utf8()
            .unwrap();
        // TODO: ``descriptor`` will be necessary to verify the field's type.
        // let descriptor = frame_class.classfile.constant_pool[method.descriptor_index as usize]
        //     .get_utf8()
        //     .unwrap();

        let val = self.stack[self.bp + frame.sp - 1].clone();
        frame.sp -= 1;

        unsafe { &mut *class }.put_static_variable(name.as_str(), val)
    }

    fn run_invoke_static(&mut self, is_invoke_static: bool) {
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

        let const_pool = frame_class.classfile.constant_pool[class_index as usize].clone();

        let name_index = if let Constant::ClassInfo { name_index } = const_pool {
            name_index
        } else {
            panic!()
        };

        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        let class = load_class(self.classheap, self.objectheap, class_name);

        let const_pool = frame_class.classfile.constant_pool[name_and_type_index as usize].clone();

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

        let name = frame_class.classfile.constant_pool[method.name_index as usize]
            .get_utf8()
            .unwrap();
        let descriptor = frame_class.classfile.constant_pool[method.descriptor_index as usize]
            .get_utf8()
            .unwrap();

        // println!("invoke: {}.{}:{}", class_name, name, descriptor);

        let (virtual_class, method2) = unsafe { &*class }.get_method(name, descriptor).unwrap();

        let former_sp = frame.sp as usize;

        self.frame_stack.push(Frame::new());

        let frame = frame!();

        frame.method_info = method2;

        method.access_flags = frame.method_info.access_flags;

        frame.class = if method.access_flags & 0x0020/*=ACC_SUPER*/> 0 {
            Some(unsafe { &*virtual_class }.get_super_class().unwrap())
        } else {
            Some(virtual_class)
        };

        let params_num = {
            // TODO: long/double takes 2 stack position
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
                if descriptor.chars().nth(i).unwrap() == 'J'
                    || descriptor.chars().nth(i).unwrap() == 'D'
                {
                    // count += 1;
                }
                count += 1;
                i += 1;
            }

            if is_invoke_static {
                count
            } else {
                count + 1
            }
        };

        let mut sp_start = params_num;
        if frame.method_info.access_flags & 0x0100 > 0 {
            // method_info.access_flags & ACC_NATIVE => do not add max_locals
        } else {
            if let Some(Attribute::Code { max_locals, .. }) = frame.method_info.get_code_attribute()
            {
                sp_start += *max_locals as usize;
            } else {
                panic!()
            };
        }

        frame.sp = sp_start;
        let bp_offset = former_sp - params_num;
        self.bp += bp_offset;

        self.run();

        self.bp -= bp_offset;
        self.frame_stack.pop();

        let mut frame = frame!();
        frame.sp -= params_num;
        if !descriptor.ends_with(")V") {
            // Returns a value
            frame.sp += 1;
        }
    }

    fn run_new(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let class_index =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            } else {
                panic!()
            };
        let const_pool = frame_class.classfile.constant_pool[class_index].clone();

        let name_index = if let Constant::ClassInfo { name_index } = const_pool {
            name_index
        } else {
            panic!()
        };

        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        // println!("> {}", class_name);

        let class = load_class(self.classheap, self.objectheap, class_name);

        let object = unsafe { &mut *self.objectheap }.create_object(class);

        self.stack[self.bp + frame.sp] = Variable::Object(object);

        frame.sp += 1;
    }
}

macro_rules! expect {
    ($expr:expr, $msg:expr) => {{
        match $expr {
            Some(some) => some,
            None => {
                eprintln!("{}: {}", Colour::Red.bold().paint("error"), $msg);
                ::std::process::abort();
            }
        }
    }};
}

pub fn load_class_with_filename(
    classheap: GcType<ClassHeap>,
    objectheap: GcType<ObjectHeap>,
    filename: &str,
) -> GcType<Class> {
    let class_ptr = gc::new(Class::new());

    expect!(
        unsafe { &mut *classheap }.load_class(filename, class_ptr),
        "loading class file is failed"
    );

    unsafe { (*class_ptr).classheap = Some(classheap) };

    let mut vm = VM::new(classheap, objectheap);
    let object = unsafe { &mut *objectheap }.create_object(class_ptr);
    let (class, method) = expect!(
        unsafe { &*class_ptr }.get_method("<init>", "()V"),
        "Couldn't find <init>"
    );
    vm.stack[0] = Variable::Object(object);
    vm.frame_stack[0].class = Some(class);
    vm.frame_stack[0].method_info = method;
    vm.frame_stack[0].sp = if let Some(Attribute::Code { max_locals, .. }) =
        vm.frame_stack[0].method_info.get_code_attribute()
    {
        *max_locals as usize
    } else {
        panic!()
    };

    vm.run();

    if let Some((class, method)) = unsafe { &*class_ptr }.get_method("<clinit>", "()V") {
        vm.bp = 0;
        vm.frame_stack[0].pc = 0;
        vm.frame_stack[0].class = Some(class);
        vm.frame_stack[0].method_info = method;
        vm.frame_stack[0].sp = if let Some(Attribute::Code { max_locals, .. }) =
            vm.frame_stack[0].method_info.get_code_attribute()
        {
            *max_locals as usize
        } else {
            panic!()
        };

        vm.run();
    }

    class_ptr
}

pub fn load_class(
    classheap: GcType<ClassHeap>,
    objectheap: GcType<ObjectHeap>,
    class_name: &str,
) -> GcType<Class> {
    if let Some(class) = unsafe { &*classheap }.get_class(class_name) {
        return class;
    }

    let filename = format!("./examples/{}.class", class_name);
    load_class_with_filename(classheap, objectheap, filename.as_str())
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
    pub const dconst_0:     u8 = 14;
    pub const dconst_1:     u8 = 15;
    pub const bipush:       u8 = 16;
    pub const sipush:       u8 = 17;
    pub const ldc:          u8 = 18;
    pub const ldc2_w:       u8 = 20;
    pub const iload:        u8 = 21;
    pub const dload:        u8 = 24;
    pub const aload_0:      u8 = 42;
    pub const aload_1:      u8 = 43;
    pub const aload_2:      u8 = 44;
    pub const aload_3:      u8 = 45;
    pub const istore:       u8 = 54;
    pub const istore_0:     u8 = 59;
    pub const istore_1:     u8 = 60;
    pub const istore_2:     u8 = 61;
    pub const istore_3:     u8 = 62;
    pub const iload_0:      u8 = 26;
    pub const iload_1:      u8 = 27;
    pub const iload_2:      u8 = 28;
    pub const iload_3:      u8 = 29;
    pub const dload_0:      u8 = 38;
    pub const dload_1:      u8 = 39;
    pub const dload_2:      u8 = 40;
    pub const dload_3:      u8 = 41;
    pub const dstore:       u8 = 57;
    pub const dstore_0:     u8 = 71;
    pub const dstore_1:     u8 = 72;
    pub const dstore_2:     u8 = 73;
    pub const dstore_3:     u8 = 74;
    pub const astore_0:     u8 = 75;
    pub const astore_1:     u8 = 76;
    pub const astore_2:     u8 = 77;
    pub const astore_3:     u8 = 78;
    pub const pop:          u8 = 87;
    pub const pop2:         u8 = 88;
    pub const dup:          u8 = 89;
    pub const iadd:         u8 = 96;
    pub const dadd:         u8 = 99;
    pub const isub:         u8 = 100;
    pub const dsub:         u8 = 103;
    pub const dmul:         u8 = 107;
    pub const ddiv:         u8 = 111;
    pub const dneg:         u8 = 119;
    pub const iinc:         u8 = 132;
    pub const i2d:          u8 = 135;
    pub const dcmpl:        u8 = 151;
    pub const ifeq:         u8 = 153;
    pub const if_icmpge:    u8 = 162;
    pub const if_icmpgt:    u8 = 163;
    pub const goto:         u8 = 167;
    pub const ireturn:      u8 = 172;
    pub const dreturn:      u8 = 175;
    pub const return_:      u8 = 177;
    pub const getstatic:    u8 = 178;
    pub const putstatic:    u8 = 179;
    pub const getfield:     u8 = 180;
    pub const putfield:     u8 = 181;
    pub const invokevirtual:u8 = 182;
    pub const invokespecial:u8 = 183;
    pub const invokestatic: u8 = 184;
    pub const new:          u8 = 187;
    pub const monitorenter: u8 = 194;
    // pub const ifnonnull:    u8 = 199;
}
