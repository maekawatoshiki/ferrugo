use super::super::class::class::Class;
use super::super::class::classfile::attribute::Attribute;
use super::super::class::classfile::constant::Constant;
use super::super::class::classfile::{method, method::MethodInfo};
use super::super::class::classheap::ClassHeap;
use super::super::gc::{gc, gc::GcType};
use super::cfg::CFGMaker;
use super::frame::{AType, Array, Frame, ObjectBody, Variable};
use super::objectheap::ObjectHeap;
use super::{jit, jit::VariableType, jit::JIT};
use ansi_term::Colour;
use rustc_hash::FxHashMap;

#[macro_export]
macro_rules! fld { ($a:path, $b:expr, $( $arg:ident ),*) => {{
    match $b {
        $a { $($arg, )* .. } => ($(*$arg as usize),*),
        _ => panic!()
    }
}}; }

#[derive(Debug)]
pub struct VM {
    pub classheap: GcType<ClassHeap>,
    pub objectheap: GcType<ObjectHeap>,
    pub frame_stack: Vec<Frame>,
    pub stack: Vec<Variable>,
    pub bp: usize,
    pub jit: JIT,
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
            jit: unsafe { JIT::new(objectheap) },
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

        let frame = frame!();

        if frame.method_info.access_flags & 0x0100 > 0 {
            // ACC_NATIVE
            self.run_native_method();
            return 0;
        }

        let jit_info_mgr = unsafe { &mut *frame.class.unwrap() }.get_jit_info_mgr(
            frame.method_info.name_index as usize,
            frame.method_info.descriptor_index as usize,
        );

        let code =
            if let Some(Attribute::Code { code, .. }) = frame.method_info.get_code_attribute() {
                code.clone()
            } else {
                panic!()
            };

        macro_rules! loop_jit {
            ($frame:expr, $do_compile:expr, $start:expr, $end:expr, $failed:expr) => {
                if !$do_compile {
                    $failed;
                    continue;
                }

                let can_jit = jit_info_mgr.inc_count_of_loop_exec($start, $end);

                if !can_jit {
                    $failed;
                    continue;
                }

                let jit_func = jit_info_mgr.get_jit_loop($start);
                let exec_info = match jit_func {
                    Some(exec_info) => {
                        if exec_info.cant_compile {
                            $failed;
                            continue;
                        }
                        exec_info.clone()
                    }
                    none => unsafe {
                        let mut blocks = CFGMaker::new().make(&code, $start, $end);
                        let class = $frame.class.unwrap();
                        match self.jit.compile_loop(class, &mut blocks) {
                            Ok(exec_info) => {
                                *none = Some(exec_info.clone());
                                exec_info
                            }
                            Err(_) => {
                                *none = Some(jit::LoopJITExecInfo {
                                    local_variables: FxHashMap::default(),
                                    func: 0,
                                    cant_compile: true,
                                });
                                $failed;
                                continue;
                            }
                        }
                    },
                };

                $frame.pc = unsafe {
                    self.jit
                        .run_loop(&mut self.stack, self.bp, &exec_info)
                        .unwrap()
                };
            };
        }

        loop {
            let frame = frame!();
            let cur_code = code[frame.pc as usize];

            match cur_code {
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {
                    self.stack[self.bp + frame.sp] =
                        Variable::Int(cur_code as i32 - Inst::iconst_0 as i32);
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dconst_0 | Inst::dconst_1 => {
                    self.stack[self.bp + frame.sp] =
                        Variable::Double((cur_code as i64 - Inst::dconst_0 as i64) as f64);
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dstore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::astore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::istore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                    self.stack[self.bp + cur_code as usize - Inst::istore_0 as usize] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::iload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::dload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::iaload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2].get_pointer::<Array>();
                    let index = self.stack[self.bp + frame.sp - 1].get_int() as usize;
                    self.stack[self.bp + frame.sp - 2] =
                        unsafe { &*arrayref }.elements[index].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::aaload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2].get_pointer::<Array>();
                    let index = self.stack[self.bp + frame.sp - 1].get_int() as usize;
                    self.stack[self.bp + frame.sp - 2] =
                        unsafe { &*arrayref }.elements[index].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::sipush => {
                    let val = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    self.stack[self.bp + frame.sp] = Variable::Short(val);
                    frame.sp += 1;
                    frame.pc += 3;
                }
                Inst::ldc => {
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
                            unsafe { &mut *self.objectheap }
                                .create_string_object(string, self.classheap)
                        }
                        _ => unimplemented!(),
                    };
                    self.stack[self.bp + frame.sp] = val;
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::ldc2_w => {
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
                Inst::aload => {
                    let index = code[frame.pc + 1] as usize;
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + index].clone();
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::dload => {
                    let index = code[frame.pc + 1] as usize;
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + index].clone();
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::iload => {
                    let index = code[frame.pc + 1] as usize;
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + index].clone();
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::aload_0 | Inst::aload_1 | Inst::aload_2 | Inst::aload_3 => {
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::aload_0 as usize].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dstore_0 | Inst::dstore_1 | Inst::dstore_2 | Inst::dstore_3 => {
                    self.stack[self.bp + (cur_code as usize - Inst::dstore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::astore_0 | Inst::astore_1 | Inst::astore_2 | Inst::astore_3 => {
                    self.stack[self.bp + (cur_code as usize - Inst::astore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 3].get_pointer::<Array>();
                    let index = self.stack[self.bp + frame.sp - 2].get_int() as usize;
                    let value = self.stack[self.bp + frame.sp - 1].clone();
                    unsafe { &mut *arrayref }.elements[index] = value;
                    frame.sp -= 3;
                    frame.pc += 1;
                }
                Inst::aastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 3].get_pointer::<Array>();
                    let index = self.stack[self.bp + frame.sp - 2].get_int() as usize;
                    let value = self.stack[self.bp + frame.sp - 1].clone();
                    unsafe { &mut *arrayref }.elements[index] = value;
                    frame.sp -= 3;
                    frame.pc += 1;
                }
                Inst::bipush => {
                    self.stack[self.bp + frame.sp] = Variable::Char(code[frame.pc + 1] as i8);
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::iadd => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            + self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dadd => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            + self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::isub => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            - self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dsub => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            - self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::imul => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            * self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dmul => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            * self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::ddiv => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Double(
                        self.stack[self.bp + frame.sp - 2].get_double()
                            / self.stack[self.bp + frame.sp - 1].get_double(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::irem => {
                    self.stack[self.bp + frame.sp - 2] = Variable::Int(
                        self.stack[self.bp + frame.sp - 2].get_int()
                            % self.stack[self.bp + frame.sp - 1].get_int(),
                    );
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dneg => {
                    self.stack[self.bp + frame.sp - 1] =
                        Variable::Double(-self.stack[self.bp + frame.sp - 1].get_double());
                    frame.pc += 1;
                }
                Inst::iinc => {
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
                    self.stack[self.bp + frame.sp - 1] =
                        Variable::Double(self.stack[self.bp + frame.sp - 1].get_int() as f64);
                    frame.pc += 1;
                }
                Inst::i2s => {
                    self.stack[self.bp + frame.sp - 1] =
                        Variable::Short(self.stack[self.bp + frame.sp - 1].get_int() as i16);
                    frame.pc += 1;
                }
                Inst::invokestatic => self.run_invoke_static(true),
                Inst::invokespecial => self.run_invoke_static(false),
                Inst::invokevirtual => self.run_invoke_static(false),
                Inst::new => self.run_new(),
                Inst::newarray => self.run_new_array(),
                Inst::anewarray => self.run_new_obj_array(),
                Inst::pop | Inst::pop2 => {
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dup => {
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::goto => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(frame, dst < frame.pc, dst, frame.pc + 3, frame.pc = dst);
                }
                Inst::dcmpl => {
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
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1].get_int();
                    frame.sp -= 1;
                    if val == 0 {
                        frame.pc = (frame.pc as isize + branch as isize) as usize;
                    } else {
                        frame.pc += 3;
                    }
                }
                Inst::ifne => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1].get_int();
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val != 0 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::if_icmpne => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_int();
                    frame.sp -= 2;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(frame, dst < frame.pc, dst, frame.pc + 3, {
                        if val1 != val2 {
                            frame.pc = dst
                        } else {
                            frame.pc += 3;
                        }
                    });
                }
                Inst::if_icmpge => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_int();
                    frame.sp -= 2;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val1 >= val2 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::if_icmpgt => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1].get_int();
                    let val1 = self.stack[self.bp + frame.sp - 2].get_int();
                    frame.sp -= 2;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val1 > val2 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                // Inst::ifnonnull => {
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
        let objectheap = unsafe { &mut *self.objectheap };

        match signature.as_str() {
            "java/io/PrintStream.println:(I)V" => {
                jit::java_io_printstream_println_i_v(
                    self.stack[self.bp].get_pointer::<u64>(),
                    self.stack[self.bp + 1].get_int(),
                );
            }
            "java/io/PrintStream.println:(D)V" => {
                println!("{}", self.stack[self.bp + 1].get_double());
            }
            "java/io/PrintStream.println:(Ljava/lang/String;)V" => {
                let object_body =
                    unsafe { &mut *self.stack[self.bp + 1].get_pointer::<ObjectBody>() };
                println!("{}", unsafe {
                    &*(object_body
                        .variables
                        .get("str")
                        .unwrap()
                        .get_pointer::<String>())
                });
            }
            // static
            "java/lang/String.valueOf:(I)Ljava/lang/String;" => {
                let i = self.stack[self.bp + 0].get_int();
                self.stack[self.bp + 0] =
                    objectheap.create_string_object(format!("{}", i), self.classheap);
            }
            "java/lang/StringBuilder.append:(Ljava/lang/String;)Ljava/lang/StringBuilder;" => {
                let string_builder =
                    unsafe { &mut *self.stack[self.bp + 0].get_pointer::<ObjectBody>() };
                let append_str = unsafe {
                    let string =
                        &mut *self.stack[self.bp + frame.sp - 1].get_pointer::<ObjectBody>();
                    &*(string.variables.get("str").unwrap().get_pointer::<String>())
                };
                let string = {
                    unsafe {
                        let string = &mut *string_builder
                            .variables
                            .entry("str".to_string())
                            .or_insert(
                                objectheap.create_string_object("".to_string(), self.classheap),
                            )
                            .get_pointer::<ObjectBody>();
                        &mut *(string
                            .variables
                            .get_mut("str")
                            .unwrap()
                            .get_pointer::<String>())
                    }
                };
                string.push_str(append_str);
            }
            "java/lang/StringBuilder.append:(I)Ljava/lang/StringBuilder;" => {
                let string_builder =
                    unsafe { &mut *self.stack[self.bp + 0].get_pointer::<ObjectBody>() };
                let append_int = self.stack[self.bp + frame.sp - 1].get_int();
                let string = {
                    unsafe {
                        let string = &mut *string_builder
                            .variables
                            .entry("str".to_string())
                            .or_insert(
                                objectheap.create_string_object("".to_string(), self.classheap),
                            )
                            .get_pointer::<ObjectBody>();
                        &mut *(string.variables.get_mut("str").unwrap().get_pointer()
                            as GcType<String>)
                    }
                };
                string.push_str(format!("{}", append_int).as_str());
            }
            "java/lang/StringBuilder.toString:()Ljava/lang/String;" => {
                let string_builder =
                    unsafe { &mut *self.stack[self.bp + 0].get_pointer::<ObjectBody>() };
                let s = string_builder.variables.get("str").unwrap().clone();
                self.stack[self.bp + 0] = s;
            }
            e => panic!("{:?}", e),
        }
    }

    fn run_get_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let objectref =
            unsafe { &mut *self.stack[self.bp + frame.sp - 1].get_pointer::<ObjectBody>() };
        frame.sp -= 1;

        let name_and_type_index = fld!(
            Constant::FieldrefInfo,
            &frame_class.classfile.constant_pool[index],
            name_and_type_index
        );
        let name_index = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index
        );
        let name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();

        let value = objectref.variables.get(name).unwrap();

        self.stack[self.bp + frame.sp] = value.clone();
        frame.sp += 1;
    }

    fn run_put_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let objectref =
            unsafe { &mut *self.stack[self.bp + frame.sp - 2].get_pointer::<ObjectBody>() };
        let value = self.stack[self.bp + frame.sp - 1].clone();
        frame.sp -= 2;

        let name_and_type_index = fld!(
            Constant::FieldrefInfo,
            &frame_class.classfile.constant_pool[index],
            name_and_type_index
        );
        let name_index = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index
        );
        let name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();

        objectref.variables.insert(name.clone(), value);
    }

    fn run_get_static(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let (class_index, name_and_type_index) = fld!(
            Constant::FieldrefInfo,
            &frame_class.classfile.constant_pool[index],
            class_index,
            name_and_type_index
        );
        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();
        let class = load_class(self.classheap, self.objectheap, class_name);
        let name_index = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index
        );
        let name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        // TODO: ``descriptor`` will be necessary to verify the field's type.

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
        let index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let (class_index, name_and_type_index) = fld!(
            Constant::FieldrefInfo,
            &frame_class.classfile.constant_pool[index],
            class_index,
            name_and_type_index
        );
        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();
        let class = load_class(self.classheap, self.objectheap, class_name);
        let name_index = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index
        );
        let name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        // TODO: ``descriptor`` will be necessary to verify the field's type.

        let val = self.stack[self.bp + frame.sp - 1].clone();
        frame.sp -= 1;

        unsafe { &mut *class }.put_static_variable(name.as_str(), val)
    }

    fn run_invoke_static(&mut self, is_invoke_static: bool) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame_class = unsafe { &*frame!().class.unwrap() };
        let mref_index = {
            let frame = frame!();
            match frame.method_info.get_code_attribute() {
                Some(Attribute::Code { code, .. }) => {
                    ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
                }
                _ => panic!(),
            }
        };
        frame!().pc += 3;

        let (class_index, name_and_type_index) = fld!(
            Constant::MethodrefInfo,
            &frame_class.classfile.constant_pool[mref_index],
            class_index,
            name_and_type_index
        );
        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();
        let class = load_class(self.classheap, self.objectheap, class_name);
        let (name_index, descriptor_index) = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index,
            descriptor_index
        );

        let name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();
        let descriptor = frame_class.classfile.constant_pool[descriptor_index]
            .get_utf8()
            .unwrap();
        let (virtual_class, exec_method) = unsafe { &*class }.get_method(name, descriptor).unwrap();
        let params_num = count_params(descriptor.as_str()) + if is_invoke_static { 0 } else { 1 };
        let former_sp = frame!().sp as usize;

        if let Some(sp) = unsafe {
            self.run_jit_compiled_func(&exec_method, former_sp, params_num, virtual_class)
        } {
            frame!().sp = sp;
            return;
        }

        self.frame_stack.push(Frame::new());

        let frame = frame!();

        frame.method_info = exec_method;

        // https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.1
        // > The ACC_SUPER flag exists for backward compatibility with code compiled by older
        // > compilers for the Java programming language. In Oracle’s JDK prior to release 1.0.2, the
        // > compiler generated ClassFile access_flags in which the flag now representing ACC_SUPER
        // > had no assigned meaning, and Oracle's Java Virtual Machine implementation ignored the
        // > flag if it was set.
        frame.class = Some(virtual_class);

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

    unsafe fn run_jit_compiled_func(
        &mut self,
        exec_method: &MethodInfo,
        sp: usize,
        params_num: usize,
        class: GcType<Class>,
    ) -> Option<usize> {
        let jit_info_mgr = (&mut *class).get_jit_info_mgr(
            exec_method.name_index as usize,
            exec_method.descriptor_index as usize,
        );

        if !jit_info_mgr.inc_count_of_func_exec() {
            return None;
        }

        if exec_method.check_access_flags(method::access_flags::ACC_PACC_NATIVE) {
            return None;
        }

        let jit_func = jit_info_mgr.get_jit_func();
        let exec_info = match jit_func {
            Some(exec_info) if exec_info.cant_compile => return None,
            Some(exec_info) => exec_info.clone(),
            none => {
                let code = match exec_method.get_code_attribute() {
                    Some(Attribute::Code { code, .. }) => code,
                    _ => panic!(),
                };
                let mut blocks = CFGMaker::new().make(&code, 0, code.len());
                let mut arg_types = vec![];

                for i in self.bp + sp - params_num..self.bp + sp {
                    match self.stack[i] {
                        Variable::Char(_) | Variable::Short(_) | Variable::Int(_) => {
                            arg_types.push(VariableType::Int)
                        }
                        _ => {
                            *none = Some(jit::FuncJITExecInfo::cant_compile());
                            return None;
                        }
                    }
                }

                match self.jit.compile_func(
                    (
                        exec_method.name_index as usize,
                        exec_method.descriptor_index as usize,
                    ),
                    class,
                    &mut blocks,
                    &arg_types,
                ) {
                    Ok(exec_info) => {
                        *none = Some(exec_info.clone());
                        exec_info.clone()
                    }
                    Err(_) => {
                        *none = Some(jit::FuncJITExecInfo::cant_compile());
                        return None;
                    }
                }
            }
        };

        if let Some(sp) = self.jit.run_func(&mut self.stack, self.bp, sp, &exec_info) {
            return Some(sp);
        }

        None
    }

    fn run_new_array(&mut self) {
        let frame = self.frame_stack.last_mut().unwrap();
        let atype = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                let atype = code[frame.pc + 1] as usize;
                AType::to_atype(atype)
            }
            _ => panic!(),
        };
        frame.pc += 2;

        let size = self.stack[self.bp + frame.sp - 1].get_int() as usize;
        self.stack[self.bp + frame.sp - 1] =
            unsafe { &mut *self.objectheap }.create_array(atype, size);
    }

    fn run_new_obj_array(&mut self) {
        let frame = self.frame_stack.last_mut().unwrap();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let class_index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();
        let class = load_class(self.classheap, self.objectheap, class_name);

        let size = self.stack[self.bp + frame.sp - 1].get_int() as usize;
        self.stack[self.bp + frame.sp - 1] =
            unsafe { &mut *self.objectheap }.create_obj_array(class, size);
    }

    fn run_new(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let class_index = match frame.method_info.get_code_attribute() {
            Some(Attribute::Code { code, .. }) => {
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            }
            _ => panic!(),
        };
        frame.pc += 3;

        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();
        let class = load_class(self.classheap, self.objectheap, class_name);
        let object = unsafe { &mut *self.objectheap }.create_object(class);

        self.stack[self.bp + frame.sp] = object;
        frame.sp += 1;
    }
}

fn count_params(descriptor: &str) -> usize {
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
        if descriptor.chars().nth(i).unwrap() == 'J' || descriptor.chars().nth(i).unwrap() == 'D' {
            // count += 1;
        }
        count += 1;
        i += 1;
    }
    count
}

macro_rules! expect {
    ($expr:expr, $msg:expr) => {{
        match $expr {
            Some(some) => some,
            None => {
                eprintln!("{}: {}", Colour::Red.bold().paint("error"), $msg);
                ::std::process::exit(-1);
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
        format!("Couldn't load file '{}'", filename)
    );

    unsafe { (*class_ptr).classheap = Some(classheap) };

    let mut vm = VM::new(classheap, objectheap);
    let object = unsafe { &mut *objectheap }.create_object(class_ptr);
    let (class, method) = expect!(
        unsafe { &*class_ptr }.get_method("<init>", "()V"),
        "Couldn't find <init>"
    );
    vm.stack[0] = object;
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
pub mod Inst {
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
    pub const aload:        u8 = 25;
    pub const iload_0:      u8 = 26;
    pub const iload_1:      u8 = 27;
    pub const iload_2:      u8 = 28;
    pub const iload_3:      u8 = 29;
    pub const dload_0:      u8 = 38;
    pub const dload_1:      u8 = 39;
    pub const dload_2:      u8 = 40;
    pub const dload_3:      u8 = 41;
    pub const iaload:       u8 = 46;
    pub const aaload:       u8 = 50;
    pub const dstore:       u8 = 57;
    pub const astore:       u8 = 58;
    pub const dstore_0:     u8 = 71;
    pub const dstore_1:     u8 = 72;
    pub const dstore_2:     u8 = 73;
    pub const dstore_3:     u8 = 74;
    pub const astore_0:     u8 = 75;
    pub const astore_1:     u8 = 76;
    pub const astore_2:     u8 = 77;
    pub const astore_3:     u8 = 78;
    pub const iastore:      u8 = 79;
    pub const aastore:      u8 = 83;
    pub const pop:          u8 = 87;
    pub const pop2:         u8 = 88;
    pub const dup:          u8 = 89;
    pub const iadd:         u8 = 96;
    pub const dadd:         u8 = 99;
    pub const isub:         u8 = 100;
    pub const dsub:         u8 = 103;
    pub const imul:         u8 = 104;
    pub const dmul:         u8 = 107;
    pub const ddiv:         u8 = 111;
    pub const irem:         u8 = 112;
    pub const dneg:         u8 = 119;
    pub const iinc:         u8 = 132;
    pub const i2d:          u8 = 135;
    pub const i2s:          u8 = 147;
    pub const dcmpl:        u8 = 151;
    pub const ifeq:         u8 = 153;
    pub const ifne:         u8 = 154;
    pub const if_icmpne:    u8 = 160;
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
    pub const newarray:     u8 = 188;
    pub const anewarray:    u8 = 189;
    pub const monitorenter: u8 = 194;
    // pub const ifnonnull:    u8 = 199;

    pub fn get_inst_size(inst: Code) -> usize {
        match inst {
            iconst_m1
                | iconst_0
                | iconst_1
                | iconst_2
                | iconst_3
                | iconst_4
                | iconst_5 => 1,
                dconst_0 | dconst_1 => 1,
                dstore =>2, 
                astore =>2, 
                istore =>2, 
                istore_0 | istore_1 | istore_2 | istore_3 => 1,
                iload_0 | iload_1 | iload_2 | iload_3 => 1,
                dload_0 | dload_1 | dload_2 | dload_3 => 1,
                iaload =>1, 
                aaload =>1, 
                sipush => 3,
                ldc => 2,
                ldc2_w => 3,
                aload =>2, 
                dload =>2, 
                iload =>2, 
                aload_0 | aload_1 | aload_2 | aload_3 => 1,
                dstore_0 | dstore_1 | dstore_2 | dstore_3 => 1,
                astore_0 | astore_1 | astore_2 | astore_3 => 1,
                iastore => 1,
                aastore => 1,
                bipush => 2,
                iadd =>1, 
                dadd =>1, 
                isub =>1, 
                dsub =>1, 
                imul =>1, 
                dmul =>1, 
                ddiv =>1, 
                irem =>1, 
                dneg => 1,
                iinc => 3,
                i2d => 1,
                i2s => 1,
                invokestatic => 3,
                invokespecial => 3,
                invokevirtual => 3,
                new => 3,
                newarray => 2,
                anewarray => 3,
                pop | pop2 => 1,
                dup => 1,
                goto => 3,
                dcmpl => 1,
                ifeq => 3,
                ifne => 3,
                if_icmpne =>3, 
                if_icmpge =>3, 
                if_icmpgt =>3, 
                ireturn =>1, 
                dreturn =>1, 
                return_ =>1, 
                getstatic =>3, 
                putstatic =>3, 
                getfield =>3, 
                putfield =>3, 
                monitorenter => 1,
                e => unimplemented!("{}", e),
        }
    }
}
