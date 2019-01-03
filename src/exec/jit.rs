use super::cfg::{Block, BrKind};
use super::frame::Variable;
use super::vm::Inst;
use libc;
use llvm;
use llvm::core::*;
use llvm::prelude::*;
use rand::random;
use rustc_hash::FxHashMap;
use std::ffi::CString;
use std::mem::transmute;
use std::ptr;

pub type CResult<T> = Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    CouldntCompile,
    General,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariableType {
    Int,
}

trait CastIntoLLVMType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef;
}

impl CastIntoLLVMType for VariableType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef {
        match self {
            &VariableType::Int => LLVMInt32TypeInContext(ctx),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JITExecInfo {
    pub local_variables: FxHashMap<usize, VariableType>,
    pub func: u64,
    pub cant_compile: bool,
}

#[derive(Debug, Clone)]
pub enum BasicBlockInfo {
    Positioned(LLVMBasicBlockRef),
    Unpositioned(LLVMBasicBlockRef),
}

#[derive(Debug)]
pub struct PhiStack {
    src_bb: LLVMBasicBlockRef,
    stack: Vec<LLVMValueRef>,
}

#[derive(Debug)]
pub struct JIT {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    pass_mgr: LLVMPassManagerRef,
    cur_func: Option<LLVMValueRef>,
    env: FxHashMap<usize, LLVMValueRef>,
    bblocks: FxHashMap<usize, BasicBlockInfo>,
    phi_stack: FxHashMap<usize, Vec<PhiStack>>, // destination,
}

impl JIT {
    pub unsafe fn new() -> Self {
        llvm::target::LLVM_InitializeNativeTarget();
        llvm::target::LLVM_InitializeNativeAsmPrinter();
        llvm::target::LLVM_InitializeNativeAsmParser();
        llvm::target::LLVM_InitializeAllTargetMCs();
        llvm::execution_engine::LLVMLinkInMCJIT();

        let context = LLVMContextCreate();
        let module =
            LLVMModuleCreateWithNameInContext(CString::new("ferrugo").unwrap().as_ptr(), context);
        let builder = LLVMCreateBuilderInContext(context);
        let pass_mgr = LLVMCreatePassManager();

        llvm::transforms::scalar::LLVMAddReassociatePass(pass_mgr);
        llvm::transforms::scalar::LLVMAddGVNPass(pass_mgr);
        llvm::transforms::scalar::LLVMAddInstructionCombiningPass(pass_mgr);
        llvm::transforms::scalar::LLVMAddPromoteMemoryToRegisterPass(pass_mgr);
        llvm::transforms::scalar::LLVMAddTailCallEliminationPass(pass_mgr);
        llvm::transforms::scalar::LLVMAddJumpThreadingPass(pass_mgr);

        JIT {
            context,
            module,
            builder,
            pass_mgr,
            cur_func: None,
            env: FxHashMap::default(),
            bblocks: FxHashMap::default(),
            phi_stack: FxHashMap::default(),
        }
    }
}

impl JIT {
    pub unsafe fn run_loop(
        &self,
        stack: &mut Vec<Variable>,
        bp: usize,
        exec_info: &JITExecInfo,
    ) -> Option<usize> {
        let mut raw_local_vars = vec![];

        for (offset, _ty) in &exec_info.local_variables {
            raw_local_vars.push(match stack[bp + offset] {
                Variable::Int(i) => Box::into_raw(Box::new(i)) as *mut libc::c_void,
                _ => return None,
            });
        }

        let pc = transmute::<u64, fn(*mut *mut libc::c_void) -> i32>(exec_info.func)(
            raw_local_vars.as_mut_slice().as_mut_ptr(),
        );

        for (i, (offset, ty)) in exec_info.local_variables.iter().enumerate() {
            stack[bp + offset] = match ty {
                VariableType::Int => Variable::Int(*(raw_local_vars[i] as *mut i32)),
            };
            Box::from_raw(raw_local_vars[i]);
        }

        Some(pc as usize)
    }
}

impl JIT {
    pub unsafe fn compile(&mut self, blocks: &mut Vec<Block>) -> CResult<JITExecInfo> {
        let local_vars = self.count_local_variables(blocks);

        let func_ret_ty = LLVMInt32TypeInContext(self.context);
        let func_ty = LLVMFunctionType(
            func_ret_ty,
            vec![LLVMPointerType(
                LLVMPointerType(LLVMInt8TypeInContext(self.context), 0),
                0,
            )]
            .as_mut_slice()
            .as_mut_ptr(),
            1,
            0,
        );

        let func_name = format!("ferrugo-jit-loop-{}", random::<u32>());

        let func = LLVMAddFunction(
            self.module,
            CString::new(func_name.as_str()).unwrap().as_ptr(),
            func_ty,
        );

        self.cur_func = Some(func);

        let bb_entry = LLVMAppendBasicBlockInContext(
            self.context,
            func,
            CString::new("entry").unwrap().as_ptr(),
        );

        LLVMPositionBuilderAtEnd(self.builder, bb_entry);

        self.bblocks.insert(0, BasicBlockInfo::Positioned(bb_entry));

        let mut env = FxHashMap::default();
        let arg_0 = LLVMGetParam(func, 0);

        for (i, (name, ty)) in local_vars.iter().enumerate() {
            let local_var_ref = LLVMBuildGEP(
                self.builder,
                arg_0,
                vec![LLVMConstInt(
                    LLVMInt32TypeInContext(self.context),
                    i as u64,
                    0,
                )]
                .as_mut_slice()
                .as_mut_ptr(),
                1,
                CString::new("").unwrap().as_ptr(),
            );

            let local_var_val = LLVMBuildLoad(
                self.builder,
                local_var_ref,
                CString::new("").unwrap().as_ptr(),
            );

            env.insert(
                *name,
                LLVMBuildPointerCast(
                    self.builder,
                    local_var_val,
                    LLVMPointerType(ty.to_llvmty(self.context), 0),
                    CString::new("").unwrap().as_ptr(),
                ),
            );
        }

        self.env = env;

        assert!(blocks.len() > 0);

        for block in &*blocks {
            if block.start > 0 {
                self.bblocks.insert(
                    block.start,
                    BasicBlockInfo::Unpositioned(LLVMAppendBasicBlock(
                        func,
                        CString::new("").unwrap().as_ptr(),
                    )),
                );
            }
        }

        LLVMBuildBr(
            self.builder,
            self.get_basic_block(blocks[0].start).retrieve(),
        );

        for i in 0..blocks.len() {
            self.compile_block(blocks, i, vec![])?;
        }

        let last_block = blocks.last().unwrap();
        let bb_last = (*self.bblocks.get(&last_block.start).unwrap()).retrieve();
        LLVMPositionBuilderAtEnd(self.builder, bb_last);
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildRet(
                self.builder,
                LLVMConstInt(
                    LLVMInt32TypeInContext(self.context),
                    last_block.code_end_position() as u64,
                    0,
                ),
            );
        }

        for (pos, bb) in &self.bblocks {
            if let BasicBlockInfo::Unpositioned(bb) = *bb {
                LLVMPositionBuilderAtEnd(self.builder, bb);
                if cur_bb_has_no_terminator(self.builder) {
                    LLVMBuildRet(
                        self.builder,
                        LLVMConstInt(LLVMInt32TypeInContext(self.context), *pos as u64, 0),
                    );
                }
            }
        }

        self.env.clear();
        self.bblocks.clear();
        self.phi_stack.clear();

        when_debug!(LLVMDumpModule(self.module));
        llvm::analysis::LLVMVerifyFunction(
            func,
            llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction,
        );

        LLVMRunPassManager(self.pass_mgr, self.module);

        // TODO: Is this REALLY right way???
        let mut ee = 0 as llvm::execution_engine::LLVMExecutionEngineRef;
        let mut error = 0 as *mut i8;
        if llvm::execution_engine::LLVMCreateExecutionEngineForModule(
            &mut ee,
            self.module,
            &mut error,
        ) != 0
        {
            panic!("llvm error: failed to initialize execute engine")
        }

        let func_raw = llvm::execution_engine::LLVMGetFunctionAddress(
            ee,
            CString::new(func_name.as_str()).unwrap().as_ptr(),
        );

        Ok(JITExecInfo {
            local_variables: local_vars,
            func: func_raw,
            cant_compile: false,
        })
    }

    unsafe fn build_phi_stack(
        &mut self,
        start: usize,
        mut stack: Vec<LLVMValueRef>,
    ) -> Vec<LLVMValueRef> {
        let init_size = stack.len();

        if let Some(phi_stacks) = self.phi_stack.get(&start) {
            // Firstly, build llvm's phi which needs a type of all conceivavle values.
            let src_bb = phi_stacks[0].src_bb;
            for val in &phi_stacks[0].stack {
                let phi = LLVMBuildPhi(
                    self.builder,
                    LLVMTypeOf(*val),
                    CString::new("").unwrap().as_ptr(),
                );
                LLVMAddIncoming(
                    phi,
                    vec![*val].as_mut_slice().as_mut_ptr(),
                    vec![src_bb].as_mut_slice().as_mut_ptr(),
                    1,
                );
                stack.push(phi);
            }

            for phi_stack in &phi_stacks[1..] {
                let src_bb = phi_stack.src_bb;
                for (i, val) in (&phi_stack.stack).iter().enumerate() {
                    let phi = stack[init_size + i];
                    LLVMAddIncoming(
                        phi,
                        vec![*val].as_mut_slice().as_mut_ptr(),
                        vec![src_bb].as_mut_slice().as_mut_ptr(),
                        1,
                    );
                }
            }
        }

        stack
    }

    unsafe fn compile_block(
        &mut self,
        blocks: &mut Vec<Block>,
        idx: usize,
        init_stack: Vec<LLVMValueRef>,
    ) -> CResult<usize> {
        #[rustfmt::skip]
        macro_rules! block { () => {{ &mut blocks[idx] }}; };

        if block!().generated {
            return Ok(0);
        }

        block!().generated = true;

        let bb = self.bblocks.get_mut(&block!().start).unwrap();
        LLVMPositionBuilderAtEnd(self.builder, bb.set_positioned().retrieve());

        let phi_stack = self.build_phi_stack(block!().start, init_stack);
        let stack = self.compile_bytecode(block!(), phi_stack)?;

        fn find(pc: usize, blocks: &Vec<Block>) -> Option<usize> {
            for (i, block) in blocks.iter().enumerate() {
                if block.start == pc {
                    return Some(i);
                }
            }
            None
        }

        match block!().kind.clone() {
            BrKind::ConditionalJmp { destinations } => {
                let mut d = 0;
                for dst in destinations {
                    if let Some(i) = find(dst, blocks) {
                        d = self.compile_block(blocks, i, stack.clone())?;
                    } else {
                        continue;
                    };
                    // TODO: All ``d`` must be the same
                }
                match find(d, blocks) {
                    Some(i) => self.compile_block(blocks, i, vec![]),
                    None => Ok(0),
                }
            }
            BrKind::UnconditionalJmp { destination } => {
                let src_bb = self.get_basic_block(block!().start).retrieve();
                self.phi_stack
                    .entry(destination)
                    .or_insert(vec![])
                    .push(PhiStack { src_bb, stack });
                Ok(destination)
            }
            BrKind::JmpRequired { destination } => {
                let src_bb = self.get_basic_block(block!().start).retrieve();
                let bb = self
                    .get_basic_block(destination)
                    .set_positioned()
                    .retrieve();

                if cur_bb_has_no_terminator(self.builder) {
                    LLVMBuildBr(self.builder, bb);
                }

                self.phi_stack
                    .entry(destination)
                    .or_insert(vec![])
                    .push(PhiStack { src_bb, stack });
                Ok(destination)
            }
            _ => Ok(0),
        }
    }

    unsafe fn compile_bytecode(
        &mut self,
        block: &Block,
        mut stack: Vec<LLVMValueRef>,
    ) -> CResult<Vec<LLVMValueRef>> {
        let code = &block.code;
        let mut pc = 0;

        while pc < code.len() {
            let cur_code = code[pc];

            match cur_code {
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {
                    let num = (cur_code as i64 - Inst::iconst_0 as i64) as u64;
                    stack.push(LLVMConstInt(LLVMInt32TypeInContext(self.context), num, 1));
                }
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                    let name = (cur_code - Inst::istore_0) as usize;
                    let val = stack.pop().unwrap();
                    LLVMBuildStore(
                        self.builder,
                        val,
                        self.declare_local_var(name, VariableType::Int),
                    );
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    let name = (cur_code - Inst::iload_0) as usize;
                    let var = self.declare_local_var(name, VariableType::Int);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ));
                }
                Inst::if_icmpne | Inst::if_icmpge => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::if_icmpne => llvm::LLVMIntPredicate::LLVMIntNE,
                            Inst::if_icmpge => llvm::LLVMIntPredicate::LLVMIntSGE,
                            _ => unreachable!(),
                        },
                        val1,
                        val2,
                        CString::new("icmp").unwrap().as_ptr(),
                    );
                    let destinations = block.kind.get_conditional_jump_destinations();
                    let bb_then = self.get_basic_block(destinations[0]).retrieve();
                    let bb_else = self.get_basic_block(destinations[1]).retrieve();

                    LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);
                }
                Inst::ifne => {
                    let val = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::ifne => llvm::LLVMIntPredicate::LLVMIntNE,
                            _ => unreachable!(),
                        },
                        val,
                        LLVMConstInt(LLVMInt32TypeInContext(self.context), 0, 0),
                        CString::new("icmp").unwrap().as_ptr(),
                    );
                    let destinations = block.kind.get_conditional_jump_destinations();
                    let bb_then = self.get_basic_block(destinations[0]).retrieve();
                    let bb_else = self.get_basic_block(destinations[1]).retrieve();

                    LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);
                }
                Inst::goto => {
                    let destination = block.kind.get_unconditional_jump_destination();
                    let bb_goto = self.get_basic_block(destination).retrieve();
                    if cur_bb_has_no_terminator(self.builder) {
                        LLVMBuildBr(self.builder, bb_goto);
                    }
                }
                Inst::iinc => {
                    let index = code[pc + 1] as usize;
                    let const_ = code[pc + 2];
                    let var_ref = self.declare_local_var(index, VariableType::Int);
                    let var_val =
                        LLVMBuildLoad(self.builder, var_ref, CString::new("").unwrap().as_ptr());
                    let inc = LLVMBuildAdd(
                        self.builder,
                        var_val,
                        LLVMConstInt(LLVMInt32TypeInContext(self.context), const_ as u64, 0),
                        CString::new("iinc").unwrap().as_ptr(),
                    );
                    LLVMBuildStore(self.builder, inc, var_ref);
                }
                Inst::iadd => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildAdd(
                        self.builder,
                        val1,
                        val2,
                        CString::new("iadd").unwrap().as_ptr(),
                    ));
                }
                Inst::irem => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildSRem(
                        self.builder,
                        val1,
                        val2,
                        CString::new("irem").unwrap().as_ptr(),
                    ));
                }
                Inst::bipush => {
                    stack.push(LLVMConstInt(
                        LLVMInt32TypeInContext(self.context),
                        code[pc + 1] as i8 as u64,
                        0,
                    ));
                }
                Inst::sipush => {
                    let val = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    stack.push(LLVMConstInt(
                        LLVMInt32TypeInContext(self.context),
                        val as u64,
                        0,
                    ));
                }
                Inst::return_ => {}
                e => {
                    dprintln!("***JIT: unimplemented instruction: {}***", e);
                    return Err(Error::CouldntCompile);
                }
            }

            pc += Inst::get_inst_size(cur_code);
        }

        Ok(stack)
    }

    unsafe fn declare_local_var(&mut self, name: usize, ty: VariableType) -> LLVMValueRef {
        if let Some(v) = self.env.get(&name) {
            return *v;
        }

        let func = self.cur_func.unwrap();
        let builder = LLVMCreateBuilderInContext(self.context);
        let entry_bb = LLVMGetEntryBasicBlock(func);
        let first_inst = LLVMGetFirstInstruction(entry_bb);
        // A variable is always declared at the first point of entry block
        if first_inst == ptr::null_mut() {
            LLVMPositionBuilderAtEnd(builder, entry_bb);
        } else {
            LLVMPositionBuilderBefore(builder, first_inst);
        }

        let var = LLVMBuildAlloca(
            builder,
            ty.to_llvmty(self.context),
            CString::new("").unwrap().as_ptr(),
        );

        self.env.insert(name, var);
        var
    }

    fn count_local_variables(&mut self, blocks: &Vec<Block>) -> FxHashMap<usize, VariableType> {
        let mut vars = FxHashMap::default();

        for block in blocks {
            let mut pc = 0;
            while pc < block.code.len() {
                let cur_code = block.code[pc];
                match cur_code {
                    Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                        vars.insert((cur_code - Inst::iload_0) as usize, VariableType::Int);
                    }
                    Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                        vars.insert((cur_code - Inst::istore_0) as usize, VariableType::Int);
                    }
                    // TODO: Add
                    _ => {}
                }
                pc += Inst::get_inst_size(cur_code);
            }
        }

        vars
    }

    unsafe fn get_basic_block(&mut self, pc: usize) -> &mut BasicBlockInfo {
        let func = self.cur_func.unwrap();
        self.bblocks.entry(pc).or_insert_with(|| {
            BasicBlockInfo::Unpositioned(LLVMAppendBasicBlock(
                func,
                CString::new("").unwrap().as_ptr(),
            ))
        })
    }
}

unsafe fn cur_bb_has_no_terminator(builder: LLVMBuilderRef) -> bool {
    LLVMIsATerminatorInst(LLVMGetLastInstruction(LLVMGetInsertBlock(builder))) == ptr::null_mut()
}

impl BasicBlockInfo {
    pub fn retrieve(&self) -> LLVMBasicBlockRef {
        match self {
            BasicBlockInfo::Positioned(bb) | BasicBlockInfo::Unpositioned(bb) => *bb,
        }
    }

    pub fn set_positioned(&mut self) -> &Self {
        match self {
            BasicBlockInfo::Unpositioned(bb) => *self = BasicBlockInfo::Positioned(*bb),
            _ => {}
        };
        self
    }

    pub fn is_positioned(&self) -> bool {
        match self {
            BasicBlockInfo::Positioned(_) => true,
            _ => false,
        }
    }
}
