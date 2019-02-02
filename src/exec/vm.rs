use super::super::class::class::Class;
use super::super::class::classfile::constant::Constant;
use super::super::class::classfile::{method, method::MethodInfo};
use super::super::class::classheap::ClassHeap;
use super::super::gc::gc::GcType;
use super::cfg::CFGMaker;
use super::frame::{AType, Array, Frame, ObjectBody, VariableType};
use super::native_functions;
use super::objectheap::ObjectHeap;
use super::{jit, jit::JIT};
use ansi_term::Colour;
use rustc_hash::FxHashMap;
use std::mem::transmute;

#[macro_export]
macro_rules! fld { ($a:path, $b:expr, $( $arg:ident ),*) => {{
    match $b {
        $a { $($arg, )* .. } => ($(*$arg as usize),*),
        _ => panic!()
    }
}}; }

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

#[derive(Debug, Clone)]
pub struct RuntimeEnvironment {
    pub classheap: GcType<ClassHeap>,
    pub objectheap: GcType<ObjectHeap>,
}

#[derive(Debug)]
pub struct VM {
    pub classheap: GcType<ClassHeap>,
    pub objectheap: GcType<ObjectHeap>,
    pub runtime_env: GcType<RuntimeEnvironment>,
    pub frame_stack: Vec<Frame>,
    pub stack: Vec<u64>,
    pub bp: usize,
    pub jit: JIT,
}

impl VM {
    pub fn new(classheap: GcType<ClassHeap>, objectheap: GcType<ObjectHeap>) -> Self {
        let runtime_env = unsafe { &mut *objectheap }.gc.alloc(RuntimeEnvironment {
            objectheap,
            classheap,
        });
        VM {
            classheap,
            objectheap,
            runtime_env,
            frame_stack: {
                let mut frame_stack = Vec::with_capacity(128);
                frame_stack.push(Frame::new());
                frame_stack
            },
            stack: vec![0; 1024],
            bp: 0,
            jit: unsafe { JIT::new(runtime_env) },
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

        if frame
            .method_info
            .check_access_flags(method::access_flags::ACC_PACC_NATIVE)
        {
            self.run_native_method();
            return 0;
        }

        let jit_info_mgr = unsafe { &mut *frame.class.unwrap() }.get_jit_info_mgr(
            frame.method_info.name_index as usize,
            frame.method_info.descriptor_index as usize,
        );

        let code = unsafe { &*frame.method_info.code.as_ref().unwrap().code };

        macro_rules! loop_jit {
            ($frame:expr, $do_compile:expr, $start:expr, $end:expr, $failed:expr) => {
                if !$do_compile {
                    $failed;
                    continue;
                }

                jit_info_mgr.inc_count_of_loop_exec($start, $end);

                let can_jit = jit_info_mgr.loop_executed_enough_times($start);
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
                Inst::aconst_null => {
                    self.stack[self.bp + frame.sp] = 0;
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {
                    self.stack[self.bp + frame.sp] =
                        (cur_code as i64 - Inst::iconst_0 as i64) as u64;
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dconst_0 | Inst::dconst_1 => {
                    self.stack[self.bp + frame.sp] = d2u(cur_code as f64 - Inst::dconst_0 as f64);
                    frame.sp += 2;
                    frame.pc += 1;
                }
                Inst::dstore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 2];
                    frame.sp -= 2;
                    frame.pc += 2;
                }
                Inst::astore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1];
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::istore => {
                    let index = code[frame.pc as usize + 1] as usize;
                    self.stack[self.bp + index] = self.stack[self.bp + frame.sp - 1];
                    frame.sp -= 1;
                    frame.pc += 2;
                }
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                    self.stack[self.bp + cur_code as usize - Inst::istore_0 as usize] =
                        self.stack[self.bp + frame.sp - 1];
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::iload_0 as usize];
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {
                    self.stack[self.bp + frame.sp] =
                        self.stack[self.bp + cur_code as usize - Inst::dload_0 as usize];
                    frame.sp += 2;
                    frame.pc += 1;
                }
                Inst::baload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 1] as isize;
                    self.stack[self.bp + frame.sp - 2] = unsafe { &*arrayref }.at::<u8>(index);
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iaload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 1] as isize;
                    self.stack[self.bp + frame.sp - 2] = unsafe { &*arrayref }.at::<u32>(index);
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::aaload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 1] as isize;
                    self.stack[self.bp + frame.sp - 2] = unsafe { &*arrayref }.at::<u64>(index);
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::daload => {
                    let arrayref = self.stack[self.bp + frame.sp - 2] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 1] as isize;
                    self.stack[self.bp + frame.sp - 2] = unsafe { &*arrayref }.at::<u64>(index);
                    frame.pc += 1;
                }
                Inst::sipush => {
                    let val = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    self.stack[self.bp + frame.sp] = val as u64;
                    frame.sp += 1;
                    frame.pc += 3;
                }
                Inst::ldc => {
                    let index = code[frame.pc + 1] as usize;
                    let val = match unsafe { &*frame.class.unwrap() }.classfile.constant_pool[index]
                    {
                        Constant::IntegerInfo { i } => i as u64,
                        Constant::FloatInfo { f } => unsafe { transmute::<f32, u32>(f) as u64 },
                        Constant::String { string_index } => unsafe { &mut *frame.class.unwrap() }
                            .get_java_string_utf8_from_const_pool(
                                self.objectheap,
                                string_index as usize,
                            )
                            .unwrap()
                            as u64,
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
                        Constant::DoubleInfo { f } => unsafe { transmute::<f64, u64>(f) },
                        _ => unimplemented!(),
                    };
                    self.stack[self.bp + frame.sp] = val;
                    frame.sp += 2;
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
                    frame.sp += 2;
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
                        self.stack[self.bp + cur_code as usize - Inst::aload_0 as usize];
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dstore_0 | Inst::dstore_1 | Inst::dstore_2 | Inst::dstore_3 => {
                    self.stack[self.bp + (cur_code as usize - Inst::dstore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 2].clone();
                    frame.sp -= 2;
                    frame.pc += 1;
                }
                Inst::astore_0 | Inst::astore_1 | Inst::astore_2 | Inst::astore_3 => {
                    self.stack[self.bp + (cur_code as usize - Inst::astore_0 as usize)] =
                        self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::bastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 3] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 2] as isize;
                    let value = self.stack[self.bp + frame.sp - 1] as u8;
                    unsafe { &mut *arrayref }.store(index, value);
                    frame.sp -= 3;
                    frame.pc += 1;
                }
                Inst::iastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 3] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 2] as isize;
                    let value = self.stack[self.bp + frame.sp - 1] as u32;
                    unsafe { &mut *arrayref }.store(index, value);
                    frame.sp -= 3;
                    frame.pc += 1;
                }
                Inst::aastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 3] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 2] as isize;
                    let value = self.stack[self.bp + frame.sp - 1] as u64;
                    unsafe { &mut *arrayref }.store(index, value);
                    frame.sp -= 3;
                    frame.pc += 1;
                }
                Inst::dastore => {
                    let arrayref = self.stack[self.bp + frame.sp - 4] as GcType<Array>;
                    let index = self.stack[self.bp + frame.sp - 3] as isize;
                    let value = self.stack[self.bp + frame.sp - 2] as u64;
                    unsafe { &mut *arrayref }.store(index, value);
                    frame.sp -= 4;
                    frame.pc += 1;
                }
                Inst::bipush => {
                    self.stack[self.bp + frame.sp] = code[frame.pc + 1] as u64;
                    frame.sp += 1;
                    frame.pc += 2;
                }
                Inst::iadd => {
                    self.stack[self.bp + frame.sp - 2] = (self.stack[self.bp + frame.sp - 2] as i32
                        + self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dadd => {
                    self.stack[self.bp + frame.sp - 4] =
                        d2u(u2d(self.stack[self.bp + frame.sp - 4])
                            + u2d(self.stack[self.bp + frame.sp - 2]));
                    frame.sp -= 2;
                    frame.pc += 1;
                }
                Inst::isub => {
                    self.stack[self.bp + frame.sp - 2] = ((self.stack[self.bp + frame.sp - 2]
                        as i32)
                        - (self.stack[self.bp + frame.sp - 1] as i32))
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dsub => {
                    self.stack[self.bp + frame.sp - 4] =
                        d2u(u2d(self.stack[self.bp + frame.sp - 4])
                            - u2d(self.stack[self.bp + frame.sp - 2]));
                    frame.sp -= 2;
                    frame.pc += 1;
                }
                Inst::imul => {
                    self.stack[self.bp + frame.sp - 2] = ((self.stack[self.bp + frame.sp - 2]
                        as i32)
                        * (self.stack[self.bp + frame.sp - 1] as i32))
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::idiv => {
                    self.stack[self.bp + frame.sp - 2] = ((self.stack[self.bp + frame.sp - 2]
                        as i32)
                        / (self.stack[self.bp + frame.sp - 1] as i32))
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dmul => {
                    self.stack[self.bp + frame.sp - 4] =
                        d2u(u2d(self.stack[self.bp + frame.sp - 4])
                            * u2d(self.stack[self.bp + frame.sp - 2]));
                    frame.sp -= 2;
                    frame.pc += 1;
                }
                Inst::ddiv => {
                    self.stack[self.bp + frame.sp - 4] =
                        d2u(u2d(self.stack[self.bp + frame.sp - 4])
                            / u2d(self.stack[self.bp + frame.sp - 2]));
                    frame.sp -= 2;
                    frame.pc += 1;
                }
                Inst::irem => {
                    self.stack[self.bp + frame.sp - 2] = (self.stack[self.bp + frame.sp - 2] as i32
                        % self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::dneg => {
                    self.stack[self.bp + frame.sp - 2] =
                        d2u(-u2d(self.stack[self.bp + frame.sp - 2]));
                    frame.pc += 1;
                }
                Inst::ishl => {
                    self.stack[self.bp + frame.sp - 2] = ((self.stack[self.bp + frame.sp - 2]
                        as i32)
                        << self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::ishr => {
                    self.stack[self.bp + frame.sp - 2] = (self.stack[self.bp + frame.sp - 2] as i32
                        >> self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iand => {
                    self.stack[self.bp + frame.sp - 2] = (self.stack[self.bp + frame.sp - 2] as i32
                        & self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::ixor => {
                    self.stack[self.bp + frame.sp - 2] = (self.stack[self.bp + frame.sp - 2] as i32
                        ^ self.stack[self.bp + frame.sp - 1] as i32)
                        as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::iinc => {
                    let index = code[frame.pc + 1] as usize;
                    let const_ = code[frame.pc + 2];
                    self.stack[self.bp + index] += const_ as u64;
                    frame.pc += 3;
                }
                Inst::i2d => {
                    self.stack[self.bp + frame.sp - 1] =
                        d2u(self.stack[self.bp + frame.sp - 1] as f64);
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::d2i => {
                    self.stack[self.bp + frame.sp - 2] =
                        u2d(self.stack[self.bp + frame.sp - 2]) as i32 as u64;
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::i2s => {
                    self.stack[self.bp + frame.sp - 1] =
                        (self.stack[self.bp + frame.sp - 1] as i16) as u64;
                    frame.pc += 1;
                }
                Inst::invokestatic => self.run_invoke_static(true),
                Inst::invokespecial => self.run_invoke_static(false),
                Inst::invokevirtual => self.run_invoke_static(false),
                Inst::new => self.run_new(),
                Inst::newarray => self.run_new_array(),
                Inst::anewarray => self.run_new_obj_array(),
                Inst::pop | Inst::pop2 => {
                    frame.sp -= if cur_code == Inst::pop2 { 2 } else { 1 };
                    frame.pc += 1;
                }
                Inst::dup => {
                    self.stack[self.bp + frame.sp] = self.stack[self.bp + frame.sp - 1].clone();
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::dup_x1 => {
                    let val1 = self.stack[self.bp + frame.sp - 1];
                    let val2 = self.stack[self.bp + frame.sp - 2];
                    frame.sp -= 2;
                    self.stack[self.bp + frame.sp + 0] = val1;
                    self.stack[self.bp + frame.sp + 1] = val2;
                    self.stack[self.bp + frame.sp + 2] = val1;
                    frame.sp += 3;
                    frame.pc += 1;
                }
                Inst::dup2_x1 => {
                    // let form2 = match self.stack[self.bp + frame.sp - 2] {
                    //     Variable::Double(_) => true,
                    //     _ => false,
                    // };
                    // if form2 {
                    //     let val1 = self.stack[self.bp + frame.sp - 2];
                    //     let val2 = self.stack[self.bp + frame.sp - 3];
                    //     self.stack[self.bp + frame.sp - 3] = val1;
                    //     self.stack[self.bp + frame.sp - 1] = val2;
                    //     self.stack[self.bp + frame.sp + 0] = val1;
                    // } else {
                    let val1 = self.stack[self.bp + frame.sp - 1];
                    let val2 = self.stack[self.bp + frame.sp - 2];
                    let val3 = self.stack[self.bp + frame.sp - 3];
                    self.stack[self.bp + frame.sp - 3] = val2;
                    self.stack[self.bp + frame.sp - 2] = val1;
                    self.stack[self.bp + frame.sp - 1] = val3;
                    self.stack[self.bp + frame.sp + 0] = val2;
                    self.stack[self.bp + frame.sp + 1] = val1;
                    // }
                    frame.sp += 2;
                    frame.pc += 1;
                }
                Inst::dup2 => {
                    // let form2 = match self.stack[self.bp + frame.sp - 2] {
                    //     Variable::Double(_) => true,
                    //     _ => false,
                    // };
                    // if form2 {
                    //     let val = self.stack[self.bp + frame.sp - 2];
                    //     self.stack[self.bp + frame.sp] = val;
                    // } else {
                    let val1 = self.stack[self.bp + frame.sp - 1];
                    let val2 = self.stack[self.bp + frame.sp - 2];
                    self.stack[self.bp + frame.sp + 0] = val2;
                    self.stack[self.bp + frame.sp + 1] = val1;
                    // }
                    frame.sp += 2;
                    frame.pc += 1;
                }
                Inst::goto => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(frame, dst < frame.pc, dst, frame.pc + 3, frame.pc = dst);
                }
                Inst::dcmpl | Inst::dcmpg => {
                    let val2 = u2d(self.stack[self.bp + frame.sp - 2]);
                    let val1 = u2d(self.stack[self.bp + frame.sp - 4]);
                    frame.sp -= 4;
                    if val1 > val2 {
                        self.stack[self.bp + frame.sp] = 1;
                    } else if val1 == val2 {
                        self.stack[self.bp + frame.sp] = 0;
                    } else if val1 < val2 {
                        self.stack[self.bp + frame.sp] = (0 - 1) as u64;
                    } else if val1.is_nan() || val2.is_nan() {
                        self.stack[self.bp + frame.sp] = if cur_code == Inst::dcmpg {
                            1
                        } else {
                            (0 - 1) as u64
                        };
                    }
                    frame.sp += 1;
                    frame.pc += 1;
                }
                Inst::ifeq => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as i32;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val == 0 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::ifne => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as i32;
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
                Inst::iflt => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as i32;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val < 0 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::ifle => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as i32;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val <= 0 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::ifge => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as i32;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val >= 0 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::ifnull => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as GcType<u64>;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val == 0 as *mut u64 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::ifnonnull => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val = self.stack[self.bp + frame.sp - 1] as GcType<u64>;
                    frame.sp -= 1;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val != 0 as *mut u64 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::if_icmpeq => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1] as i32;
                    let val1 = self.stack[self.bp + frame.sp - 2] as i32;
                    frame.sp -= 2;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(frame, dst < frame.pc, dst, frame.pc + 3, {
                        if val1 == val2 {
                            frame.pc = dst
                        } else {
                            frame.pc += 3;
                        }
                    });
                }
                Inst::if_icmpne => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1] as i32;
                    let val1 = self.stack[self.bp + frame.sp - 2] as i32;
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
                    let val2 = self.stack[self.bp + frame.sp - 1] as i32;
                    let val1 = self.stack[self.bp + frame.sp - 2] as i32;
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
                    let val2 = self.stack[self.bp + frame.sp - 1] as i32;
                    let val1 = self.stack[self.bp + frame.sp - 2] as i32;
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
                Inst::if_icmplt => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1] as i32;
                    let val1 = self.stack[self.bp + frame.sp - 2] as i32;
                    frame.sp -= 2;
                    let dst = (frame.pc as isize + branch as isize) as usize;
                    loop_jit!(
                        frame,
                        dst < frame.pc,
                        dst,
                        frame.pc + 3,
                        if val1 < val2 {
                            frame.pc = dst;
                        } else {
                            frame.pc += 3;
                        }
                    );
                }
                Inst::if_acmpne => {
                    let branch = ((code[frame.pc + 1] as i16) << 8) + code[frame.pc + 2] as i16;
                    let val2 = self.stack[self.bp + frame.sp - 1] as GcType<u64>;
                    let val1 = self.stack[self.bp + frame.sp - 2] as GcType<u64>;
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
                    self.stack[self.bp] = self.stack[self.bp + frame.sp - 1].clone();
                    return Inst::ireturn;
                }
                Inst::areturn => {
                    self.stack[self.bp] = self.stack[self.bp + frame.sp - 1].clone();
                    return Inst::areturn;
                }
                Inst::dreturn => {
                    self.stack[self.bp] = self.stack[self.bp + frame.sp - 2].clone();
                    return Inst::dreturn;
                }
                Inst::return_ => {
                    return Inst::return_;
                }
                Inst::getstatic => self.run_get_static(),
                Inst::putstatic => self.run_put_static(),
                Inst::getfield => self.run_get_field(),
                Inst::putfield => self.run_put_field(),
                Inst::getfield_quick => self.run_get_field_quick(),
                Inst::putfield_quick => self.run_put_field_quick(),
                Inst::getfield2_quick => self.run_get_field2_quick(),
                Inst::putfield2_quick => self.run_put_field2_quick(),
                Inst::monitorenter => {
                    // TODO: Implement
                    frame.sp -= 1;
                    frame.pc += 1;
                }
                Inst::arraylength => {
                    let objectref = self.stack[self.bp + frame.sp - 1];
                    let array = unsafe { &mut *(objectref as GcType<Array>) };
                    self.stack[self.bp + frame.sp - 1] = array.get_length() as u64;
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
            "java/io/PrintStream.println:(Ljava/lang/Object;)V" => {
                println!("{}", self.stack[self.bp + 1]);
            }
            "java/io/PrintStream.println:(I)V" => {
                native_functions::java_io_printstream_println_i_v(
                    self.runtime_env,
                    self.stack[self.bp] as GcType<ObjectBody>,
                    self.stack[self.bp + 1] as i32,
                );
            }
            "java/io/PrintStream.println:(D)V" => {
                println!("{}", u2d(self.stack[self.bp + 1]));
            }
            "java/io/PrintStream.println:(Z)V" => {
                println!(
                    "{}",
                    if self.stack[self.bp + 1] == 0 {
                        false
                    } else {
                        true
                    }
                );
            }
            "java/io/PrintStream.println:(Ljava/lang/String;)V" => {
                native_functions::java_io_printstream_println_string_v(
                    self.runtime_env,
                    self.stack[self.bp + 0] as GcType<ObjectBody>,
                    self.stack[self.bp + 1] as GcType<ObjectBody>,
                );
            }
            "java/io/PrintStream.print:(Ljava/lang/String;)V" => {
                native_functions::java_io_printstream_print_string_v(
                    self.runtime_env,
                    self.stack[self.bp + 0] as GcType<ObjectBody>,
                    self.stack[self.bp + 1] as GcType<ObjectBody>,
                );
            }
            "java/lang/String.valueOf:(I)Ljava/lang/String;" => {
                let i = self.stack[self.bp + 0] as i32;
                self.stack[self.bp + 0] =
                    objectheap.create_string_object(format!("{}", i), self.classheap);
            }
            "java/lang/StringBuilder.append:(Ljava/lang/String;)Ljava/lang/StringBuilder;" => {
                native_functions::java_lang_stringbuilder_append_string_stringbuilder(
                    self.runtime_env,
                    self.stack[self.bp + 0] as GcType<ObjectBody>,
                    self.stack[self.bp + frame.sp - 1] as GcType<ObjectBody>,
                );
            }
            "java/lang/StringBuilder.append:(I)Ljava/lang/StringBuilder;" => {
                native_functions::java_lang_stringbuilder_append_i_stringbuilder(
                    self.runtime_env,
                    self.stack[self.bp + 0] as GcType<ObjectBody>,
                    self.stack[self.bp + frame.sp - 1] as i32,
                );
            }
            "java/lang/StringBuilder.toString:()Ljava/lang/String;" => {
                let s = native_functions::java_lang_stringbuilder_tostring_string(
                    self.runtime_env,
                    self.stack[self.bp + 0] as GcType<ObjectBody>,
                );
                self.stack[self.bp + 0] = s as u64;
            }
            "java/lang/Math.random:()D" => {
                self.stack[self.bp + 0] =
                    d2u(native_functions::java_lang_math_random_d(self.runtime_env))
            }
            "java/lang/Math.sin:(D)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_sin_d_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                ))
            }
            "java/lang/Math.cos:(D)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_cos_d_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                ))
            }
            "java/lang/Math.tan:(D)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_tan_d_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                ))
            }
            "java/lang/Math.sqrt:(D)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_sqrt_d_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                ))
            }
            "java/lang/Math.pow:(DD)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_pow_dd_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                    u2d(self.stack[self.bp + 2]),
                ))
            }
            "java/lang/Math.abs:(D)D" => {
                self.stack[self.bp + 0] = d2u(native_functions::java_lang_math_abs_d_d(
                    self.runtime_env,
                    u2d(self.stack[self.bp + 0]),
                ))
            }
            e => panic!("{:?}", e),
        }
    }

    fn run_get_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);

        let objectref = unsafe { &mut *(self.stack[self.bp + frame.sp - 1] as GcType<ObjectBody>) };

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

        let class = unsafe { &*objectref.class };
        let (id, ty) = *class.get_numbered_field_info(name.as_str()).unwrap();

        assert!(id <= 0xff);

        let code = unsafe { &mut *frame.method_info.code.as_mut().unwrap().code };
        code[frame.pc + 0] = match ty {
            VariableType::Double | VariableType::Long => Inst::getfield2_quick,
            _ => Inst::getfield_quick,
        };
        code[frame.pc + 1] = 0;
        code[frame.pc + 2] = id as u8;
    }

    fn run_put_field(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let frame_class = unsafe { &*frame.class.unwrap() };
        let index = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);

        let name_and_type_index = fld!(
            Constant::FieldrefInfo,
            &frame_class.classfile.constant_pool[index],
            name_and_type_index
        );
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

        let ty = VariableType::parse_type(descriptor.as_str()).unwrap();
        let i = match &ty {
            VariableType::Double | VariableType::Long => 2,
            _ => 1,
        };
        let objectref =
            unsafe { &mut *(self.stack[self.bp + frame.sp - (i + 1)] as GcType<ObjectBody>) };

        let class = unsafe { &*objectref.class };
        let id = class.get_numbered_field_info(name.as_str()).unwrap().0;

        assert!(id <= 0xff);

        let code = unsafe { &mut *frame.method_info.code.as_mut().unwrap().code };
        code[frame.pc + 0] = match ty {
            VariableType::Double | VariableType::Long => Inst::putfield2_quick,
            _ => Inst::putfield_quick,
        };
        code[frame.pc + 1] = 0;
        code[frame.pc + 2] = id as u8;
    }

    fn run_get_field_quick(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let id = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);
        frame.pc += 3;

        let objectref = unsafe { &mut *(self.stack[self.bp + frame.sp - 1] as GcType<ObjectBody>) };
        let value = objectref.variables[id];
        self.stack[self.bp + frame.sp - 1] = value;
    }

    fn run_put_field_quick(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let id = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);
        frame.pc += 3;

        let value = self.stack[self.bp + frame.sp - 1];
        let objectref = unsafe { &mut *(self.stack[self.bp + frame.sp - 2] as GcType<ObjectBody>) };
        frame.sp -= 2;

        objectref.variables[id] = value;
    }

    fn run_get_field2_quick(&mut self) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame = frame!();
        let id = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);
        frame.pc += 3;

        let objectref = unsafe { &mut *(self.stack[self.bp + frame.sp - 1] as GcType<ObjectBody>) };
        let value = objectref.variables[id];
        self.stack[self.bp + frame.sp - 1] = value;
        frame.sp += 1;
    }

    fn run_put_field2_quick(&mut self) {
        let frame = self.frame_stack.last_mut().unwrap();
        let id = frame
            .method_info
            .code
            .as_ref()
            .unwrap()
            .read_u16_from_code(frame.pc + 1);
        frame.pc += 3;

        let value = self.stack[self.bp + frame.sp - 2];
        let objectref = unsafe { &mut *(self.stack[self.bp + frame.sp - 3] as GcType<ObjectBody>) };
        frame.sp -= 3;

        objectref.variables[id] = value;
    }

    fn run_get_static(&mut self) {
        let frame_stack_len = self.frame_stack.len();
        let (frame_class, index) = {
            let frame = &self.frame_stack[frame_stack_len - 1];
            (
                unsafe { &*frame.class.unwrap() },
                frame
                    .method_info
                    .code
                    .as_ref()
                    .unwrap()
                    .read_u16_from_code(frame.pc + 1),
            )
        };

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
        let class = self.load_class(class_name);
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

        let frame = &mut self.frame_stack[frame_stack_len - 1];
        self.stack[self.bp + frame.sp] = object;
        frame.pc += 3;
        frame.sp += 1;
    }

    fn run_put_static(&mut self) {
        let frame_stack_len = self.frame_stack.len();
        let (frame_class, index) = {
            let frame = &self.frame_stack[frame_stack_len - 1];
            (
                unsafe { &*frame.class.unwrap() },
                frame
                    .method_info
                    .code
                    .as_ref()
                    .unwrap()
                    .read_u16_from_code(frame.pc + 1),
            )
        };

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
        let class = self.load_class(class_name);
        let name_index = fld!(
            Constant::NameAndTypeInfo,
            &frame_class.classfile.constant_pool[name_and_type_index],
            name_index
        );
        let name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();

        // TODO: ``descriptor`` will be necessary to verify the field's type.

        let frame = &mut self.frame_stack[frame_stack_len - 1];
        let val = self.stack[self.bp + frame.sp - 1].clone();
        frame.sp -= 1;
        frame.pc += 3;

        unsafe { &mut *class }.put_static_variable(name.as_str(), val)
    }

    fn run_invoke_static(&mut self, is_invoke_static: bool) {
        #[rustfmt::skip]
        macro_rules! frame { () => {{ self.frame_stack.last_mut().unwrap() }}; }

        let frame_class = unsafe { &*frame!().class.unwrap() };
        let mref_index = {
            let frame = frame!();
            frame
                .method_info
                .code
                .as_ref()
                .unwrap()
                .read_u16_from_code(frame.pc + 1)
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
        let class = self.load_class(class_name);
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
            self.run_jit_compiled_func(&exec_method, former_sp, descriptor.as_str(), virtual_class)
        } {
            frame!().sp = sp;
            return;
        }

        self.frame_stack.push(Frame::new());

        let frame = frame!();

        frame.method_info = exec_method.clone();

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
            let max_locals = frame.method_info.code.as_ref().unwrap().max_locals;
            sp_start += max_locals as usize;
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
            frame.sp += if descriptor.ends_with(")D") || descriptor.ends_with(")J") {
                2
            } else {
                1
            };
        }
    }

    unsafe fn run_jit_compiled_func(
        &mut self,
        exec_method: &MethodInfo,
        sp: usize,
        descriptor: &str,
        class: GcType<Class>,
    ) -> Option<usize> {
        let jit_info_mgr = (&mut *class).get_jit_info_mgr(
            exec_method.name_index as usize,
            exec_method.descriptor_index as usize,
        );

        jit_info_mgr.inc_count_of_func_exec();

        if !jit_info_mgr.func_executed_enough_times() {
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
                let code = &*exec_method.code.as_ref().unwrap().code;
                let mut blocks = CFGMaker::new().make(code, 0, code.len());

                match self.jit.compile_func(
                    (
                        exec_method.name_index as usize,
                        exec_method.descriptor_index as usize,
                    ),
                    class,
                    &mut blocks,
                    descriptor,
                    exec_method.check_access_flags(method::access_flags::ACC_PACC_STATIC),
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
        let atype = {
            let atype = frame
                .method_info
                .code
                .as_ref()
                .unwrap()
                .read_u8_from_code(frame.pc + 1);
            AType::to_atype(atype)
        };
        frame.pc += 2;

        let size = self.stack[self.bp + frame.sp - 1] as usize;
        self.stack[self.bp + frame.sp - 1] =
            unsafe { &mut *self.objectheap }.create_array(atype, size);

        unsafe { &mut *self.objectheap }.gc.mark_and_sweep(self);
    }

    fn run_new_obj_array(&mut self) {
        let frame_stack_len = self.frame_stack.len();
        let (frame_class, class_index) = {
            let frame = &self.frame_stack[frame_stack_len - 1];
            let frame_class = unsafe { &*frame.class.unwrap() };
            let class_index = {
                let code = unsafe { &*frame.method_info.code.as_ref().unwrap().code };
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            };
            (frame_class, class_index)
        };

        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index]
            .get_utf8()
            .unwrap();
        let class = self.load_class(class_name);

        let frame = &mut self.frame_stack[frame_stack_len - 1];
        let size = self.stack[self.bp + frame.sp - 1] as usize;
        self.stack[self.bp + frame.sp - 1] =
            unsafe { &mut *self.objectheap }.create_obj_array(class, size);
        frame.pc += 3;

        unsafe { &mut *self.objectheap }.gc.mark_and_sweep(self);
    }

    fn run_new(&mut self) {
        let frame_stack_len = self.frame_stack.len();
        let (frame_class, class_index) = {
            let frame = &self.frame_stack[frame_stack_len - 1];
            let frame_class = unsafe { &*frame.class.unwrap() };
            let class_index = {
                let code = unsafe { &*frame.method_info.code.as_ref().unwrap().code };
                ((code[frame.pc + 1] as usize) << 8) + code[frame.pc + 2] as usize
            };
            (frame_class, class_index)
        };

        let name_index = fld!(
            Constant::ClassInfo,
            &frame_class.classfile.constant_pool[class_index],
            name_index
        );
        let class_name = frame_class.classfile.constant_pool[name_index as usize]
            .get_utf8()
            .unwrap();
        let class = self.load_class(class_name);
        let object = unsafe { &mut *self.objectheap }.create_object(class);

        let frame = &mut self.frame_stack[frame_stack_len - 1];
        self.stack[self.bp + frame.sp] = object;
        frame.pc += 3;
        frame.sp += 1;

        unsafe { &mut *self.objectheap }.gc.mark_and_sweep(self);
    }
}

impl VM {
    pub fn load_class(&mut self, class_name: &str) -> GcType<Class> {
        if let Some(class) = unsafe { &*self.classheap }.get_class(class_name) {
            return class;
        }

        let filename = format!("./examples/{}.class", class_name);
        self.load_class_by_file_name(filename.as_str())
    }

    pub fn load_class_by_file_name(&mut self, file_name: &str) -> GcType<Class> {
        let class_ptr = unsafe { &mut *self.objectheap }.gc.alloc(Class::new());

        unsafe { (*class_ptr).classheap = Some(self.classheap) };

        expect!(
            unsafe { &mut *self.classheap }.load_class(file_name, class_ptr),
            format!("Could not load class file '{}'", file_name)
        );

        let object = unsafe { &mut *self.objectheap }.create_object(class_ptr);

        let cur_sp = self.frame_stack.last().unwrap().sp;
        let save_bp = self.bp;

        self.bp = save_bp + cur_sp;
        self.stack[self.bp] = object;

        // TODO: Support initializer whose descriptor is not '()V'
        if let Some((class, method)) = unsafe { &*class_ptr }.get_method("<init>", "()V") {
            let mut frame = Frame::new();
            frame.class = Some(class);
            frame.method_info = method;
            frame.sp = frame.method_info.code.as_ref().unwrap().max_locals as usize;

            self.frame_stack.push(frame);
            self.bp = save_bp + cur_sp;
            self.run();
            self.frame_stack.pop();
        }

        // Initialization with ``static { ... }``
        if let Some((class, method)) = unsafe { &*class_ptr }.get_method("<clinit>", "()V") {
            let mut frame = Frame::new();
            frame.class = Some(class);
            frame.method_info = method;
            frame.sp = frame.method_info.code.as_ref().unwrap().max_locals as usize;
            self.frame_stack.push(frame);
            self.bp = save_bp + cur_sp;
            self.run();
            self.frame_stack.pop();
        }

        self.bp = save_bp;

        class_ptr
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
            count += 1;
        }
        count += 1;
        i += 1;
    }
    count
}

#[inline]
pub fn d2u(f: f64) -> u64 {
    unsafe { transmute::<f64, u64>(f) }
}

#[inline]
pub fn u2d(u: u64) -> f64 {
    unsafe { transmute::<u64, f64>(u) }
}

#[rustfmt::skip]
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
pub mod Inst {
    pub type Code = u8;
    pub const aconst_null:  u8 = 1;
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
    pub const daload:       u8 = 49;
    pub const aaload:       u8 = 50;
    pub const baload:       u8 = 51;
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
    pub const dastore:      u8 = 82;
    pub const aastore:      u8 = 83;
    pub const bastore:      u8 = 84;
    pub const pop:          u8 = 87;
    pub const pop2:         u8 = 88;
    pub const dup:          u8 = 89;
    pub const dup_x1:       u8 = 90;
    pub const dup2:         u8 = 92;
    pub const dup2_x1:      u8 = 93;
    pub const iadd:         u8 = 96;
    pub const dadd:         u8 = 99;
    pub const isub:         u8 = 100;
    pub const dsub:         u8 = 103;
    pub const imul:         u8 = 104;
    pub const dmul:         u8 = 107;
    pub const idiv:         u8 = 108;
    pub const ddiv:         u8 = 111;
    pub const irem:         u8 = 112;
    pub const dneg:         u8 = 119;
    pub const ishl:         u8 = 120;
    pub const ishr:         u8 = 122;
    pub const iand:         u8 = 126;
    pub const ixor:         u8 = 130;
    pub const iinc:         u8 = 132;
    pub const i2d:          u8 = 135;
    pub const d2i:          u8 = 142;
    pub const i2s:          u8 = 147;
    pub const dcmpl:        u8 = 151;
    pub const dcmpg:        u8 = 152;
    pub const ifeq:         u8 = 153;
    pub const ifne:         u8 = 154;
    pub const iflt:         u8 = 155;
    pub const ifge:         u8 = 156;
    pub const ifle:         u8 = 158;
    pub const if_icmpeq:    u8 = 159;
    pub const if_icmpne:    u8 = 160;
    pub const if_icmpge:    u8 = 162;
    pub const if_icmpgt:    u8 = 163;
    pub const if_icmplt:    u8 = 164;
    pub const if_acmpne:    u8 = 166;
    pub const goto:         u8 = 167;
    pub const ireturn:      u8 = 172;
    pub const dreturn:      u8 = 175;
    pub const areturn:      u8 = 176;
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
    pub const arraylength:  u8 = 190;
    pub const monitorenter: u8 = 194;
    pub const ifnull:       u8 = 198;
    pub const ifnonnull:    u8 = 199;
    // Quick opcodes (faster)
    pub const getfield_quick: u8 = 204;
    pub const putfield_quick: u8 = 205;
    pub const getfield2_quick: u8 = 206;
    pub const putfield2_quick: u8 = 207;
    
    pub fn get_inst_size(inst: Code) -> usize {
        match inst {
            iconst_m1 | iconst_0 | iconst_1 | iconst_2 | iconst_3 | iconst_4 | iconst_5 | dconst_0
                | dconst_1 | istore_0 | istore_1 | istore_2 | istore_3 | iload_0 | iload_1 | iload_2
                | iload_3 | dload_0 | dload_1 | dload_2 | dload_3 | aload_0 | aload_1 | aload_2
                | aload_3 | dstore_0 | dstore_1 | dstore_2 | dstore_3 | astore_0 | astore_1 | astore_2
                | astore_3 | iaload | aaload | daload | baload | iastore | aastore | dastore | bastore
                | iadd | isub | imul | irem | iand | idiv
                | dadd | dsub | dmul | ddiv | dneg | i2d | i2s | pop | pop2 | dcmpl | dcmpg | dup
                | ireturn | dreturn | areturn | return_ | monitorenter | aconst_null | arraylength 
                | ishl | ishr | ixor | dup_x1 | d2i | dup2 | dup2_x1 => 1,
            dstore | astore | istore | ldc | aload | dload | iload | bipush | newarray => 2,
            sipush | ldc2_w | iinc | invokestatic | invokespecial | invokevirtual | new | anewarray 
                | goto | ifeq | iflt | ifne | ifle | ifge | if_icmpne | if_icmpge | if_icmpgt | if_icmpeq | if_acmpne | if_icmplt |
                ifnull | ifnonnull | 
                getstatic | putstatic | getfield | putfield | getfield_quick | putfield_quick | getfield2_quick | putfield2_quick => 3, 
            e => unimplemented!("{}", e),
        }
    }
}
