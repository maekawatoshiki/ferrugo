use super::super::class::class::Class;
use super::super::class::classfile::constant::Constant;
use super::super::gc::gc::GcType;
use super::cfg::{Block, BrKind};
use super::frame::Variable;
use super::objectheap::ObjectHeap;
use super::vm::{load_class, Inst};
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
    Void,
}

trait CastIntoLLVMType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef;
}

impl CastIntoLLVMType for VariableType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef {
        match self {
            &VariableType::Int => LLVMInt32TypeInContext(ctx),
            &VariableType::Void => LLVMVoidTypeInContext(ctx),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoopJITExecInfo {
    pub local_variables: FxHashMap<usize, VariableType>,
    pub func: u64,
    pub cant_compile: bool,
}

impl LoopJITExecInfo {
    pub fn cant_compile() -> Self {
        LoopJITExecInfo {
            local_variables: FxHashMap::default(),
            func: 0,
            cant_compile: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FuncJITExecInfo {
    pub func: u64,
    pub cant_compile: bool,
    pub args_ty: Vec<VariableType>,
    pub ret_ty: Option<VariableType>,
}

impl FuncJITExecInfo {
    pub fn cant_compile() -> Self {
        FuncJITExecInfo {
            func: 0,
            cant_compile: true,
            args_ty: vec![],
            ret_ty: None,
        }
    }
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
    cur_class: Option<GcType<Class>>,
    objectheap: GcType<ObjectHeap>,
    cur_func_indices: Option<(usize, usize)>,
    env: FxHashMap<usize, LLVMValueRef>,
    bblocks: FxHashMap<usize, BasicBlockInfo>,
    phi_stack: FxHashMap<usize, Vec<PhiStack>>, // destination,
    raw_func_addr_to_llvm_val: FxHashMap<u64, LLVMValueRef>,
}

impl JIT {
    pub unsafe fn new(objectheap: GcType<ObjectHeap>) -> Self {
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
            cur_class: None,
            objectheap: objectheap,
            cur_func_indices: None,
            env: FxHashMap::default(),
            bblocks: FxHashMap::default(),
            phi_stack: FxHashMap::default(),
            raw_func_addr_to_llvm_val: FxHashMap::default(),
        }
    }
}

impl JIT {
    pub unsafe fn run_func(
        &self,
        stack: &mut Vec<Variable>,
        bp: usize,
        mut sp: usize,
        exec_info: &FuncJITExecInfo,
    ) -> Option<usize> {
        let mut raw_local_vars = vec![];

        for i in bp + sp - exec_info.args_ty.len()..bp + sp {
            raw_local_vars.push(match stack[i] {
                Variable::Int(i) => i,
                _ => return None,
            });
        }

        let ret = match exec_info.args_ty.len() {
            0 => transmute::<u64, fn() -> u64>(exec_info.func)(),
            1 => transmute::<u64, fn(i32) -> u64>(exec_info.func)(raw_local_vars[0]),
            2 => transmute::<u64, fn(i32, i32) -> u64>(exec_info.func)(
                raw_local_vars[0],
                raw_local_vars[1],
            ),
            3 => transmute::<u64, fn(i32, i32, i32) -> u64>(exec_info.func)(
                raw_local_vars[0],
                raw_local_vars[1],
                raw_local_vars[2],
            ),
            4 => transmute::<u64, fn(i32, i32, i32, i32) -> u64>(exec_info.func)(
                raw_local_vars[0],
                raw_local_vars[1],
                raw_local_vars[2],
                raw_local_vars[3],
            ),
            _ => unimplemented!(),
        };

        sp -= exec_info.args_ty.len();

        let ret_ty = exec_info.ret_ty.clone().unwrap();
        match ret_ty {
            VariableType::Void => {}
            VariableType::Int => {
                stack[bp + sp] = Variable::Int(ret as i32);
                sp += 1
            }
        };

        Some(sp)
    }

    pub unsafe fn run_loop(
        &self,
        stack: &mut Vec<Variable>,
        bp: usize,
        exec_info: &LoopJITExecInfo,
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
                _ => return None,
            };
            Box::from_raw(raw_local_vars[i]);
        }

        Some(pc as usize)
    }
}

impl JIT {
    pub unsafe fn compile_func(
        &mut self,
        (name_index, descriptor_index): (usize, usize),
        class: GcType<Class>,
        blocks: &mut Vec<Block>,
        arg_types: &Vec<VariableType>,
    ) -> CResult<FuncJITExecInfo> {
        self.cur_class = Some(class);
        self.cur_func_indices = Some((name_index, descriptor_index));

        let ret_ty = self.infer_return_type(blocks)?;
        let func_ret_ty = ret_ty.to_llvmty(self.context);
        let func_ty = LLVMFunctionType(
            func_ret_ty,
            arg_types
                .iter()
                .map(|ty| ty.to_llvmty(self.context))
                .collect::<Vec<_>>()
                .as_mut_ptr(),
            arg_types.len() as u32,
            0,
        );

        let func_name = format!("ferrugo-jit-func-{}", random::<u32>());

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

        let env = FxHashMap::default();
        self.env = env;

        for (i, ty) in arg_types.iter().enumerate() {
            LLVMBuildStore(
                self.builder,
                LLVMGetParam(func, i as u32),
                self.declare_local_var(i, &ty),
            );
        }

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
        let mut compiling_error = Ok(0);

        for i in 0..blocks.len() {
            if let Err(e) = self.compile_block(blocks, i, vec![], false) {
                compiling_error = Err(e);
                break;
            }
        }

        let last_block = blocks.last().unwrap();
        let bb_last = (*self.bblocks.get(&last_block.start).unwrap()).retrieve();
        LLVMPositionBuilderAtEnd(self.builder, bb_last);
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildRet(self.builder, LLVMConstNull(func_ret_ty));
        }

        let mut iter_bb = LLVMGetFirstBasicBlock(func);
        while iter_bb != ptr::null_mut() {
            if LLVMIsATerminatorInst(LLVMGetLastInstruction(iter_bb)) == ptr::null_mut() {
                let terminator_builder = LLVMCreateBuilderInContext(self.context);
                LLVMPositionBuilderAtEnd(terminator_builder, iter_bb);
                LLVMBuildRet(terminator_builder, LLVMConstNull(func_ret_ty));
            }
            iter_bb = LLVMGetNextBasicBlock(iter_bb);
        }

        self.env.clear();
        self.bblocks.clear();
        self.phi_stack.clear();
        self.cur_func = None;
        self.cur_func_indices = None;

        llvm::analysis::LLVMVerifyFunction(
            func,
            llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction,
        );

        if let Err(e) = compiling_error {
            return Err(e);
        }

        when_debug!(LLVMDumpValue(func));

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

        self.raw_func_addr_to_llvm_val.insert(func_raw, func);

        Ok(FuncJITExecInfo {
            func: func_raw,
            cant_compile: false,
            args_ty: arg_types.clone(),
            ret_ty: Some(ret_ty),
        })
    }

    pub unsafe fn compile_loop(
        &mut self,
        class: GcType<Class>,
        blocks: &mut Vec<Block>,
    ) -> CResult<LoopJITExecInfo> {
        self.cur_class = Some(class);

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

        let mut compiling_error = Ok(0);
        for i in 0..blocks.len() {
            if let Err(e) = self.compile_block(blocks, i, vec![], true) {
                compiling_error = Err(e);
                break;
            }
        }

        let last_block = blocks.last().unwrap();
        let bb_last = (*self.bblocks.get(&last_block.start).unwrap()).retrieve();
        if cur_bb_has_no_terminator(self.builder) {
            LLVMBuildBr(self.builder, bb_last);
        }
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
                if cur_bb_has_no_terminator(self.builder) {
                    LLVMBuildBr(self.builder, bb);
                }
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

        llvm::analysis::LLVMVerifyFunction(
            func,
            llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction,
        );

        if let Err(e) = compiling_error {
            return Err(e);
        }

        when_debug!(LLVMDumpModule(self.module));

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

        self.raw_func_addr_to_llvm_val.insert(func_raw, func);

        Ok(LoopJITExecInfo {
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
        loop_compile: bool,
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
        let stack = self.compile_bytecode(block!(), phi_stack, loop_compile)?;

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
                        d = self.compile_block(blocks, i, stack.clone(), loop_compile)?;
                    } else {
                        continue;
                    };
                    // TODO: All ``d`` must be the same
                }
                match find(d, blocks) {
                    Some(i) => self.compile_block(blocks, i, vec![], loop_compile),
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
        loop_compile: bool,
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
                        self.declare_local_var(name, &VariableType::Int),
                    );
                }
                Inst::istore => {
                    let index = code[pc as usize + 1] as usize;
                    let val = stack.pop().unwrap();
                    LLVMBuildStore(
                        self.builder,
                        val,
                        self.declare_local_var(index, &VariableType::Int),
                    );
                }
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {
                    let name = (cur_code - Inst::iload_0) as usize;
                    let var = self.declare_local_var(name, &VariableType::Int);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ));
                }
                Inst::iload => {
                    let index = code[pc + 1] as usize;
                    let var = self.declare_local_var(index, &VariableType::Int);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ))
                }
                Inst::if_icmpne | Inst::if_icmpge | Inst::if_icmpgt => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::if_icmpne => llvm::LLVMIntPredicate::LLVMIntNE,
                            Inst::if_icmpge => llvm::LLVMIntPredicate::LLVMIntSGE,
                            Inst::if_icmpgt => llvm::LLVMIntPredicate::LLVMIntSGT,
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
                Inst::ifne | Inst::ifeq => {
                    let val = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::ifeq => llvm::LLVMIntPredicate::LLVMIntEQ,
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
                    let var_ref = self.declare_local_var(index, &VariableType::Int);
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
                Inst::isub => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildSub(
                        self.builder,
                        val1,
                        val2,
                        CString::new("isub").unwrap().as_ptr(),
                    ));
                }
                Inst::imul => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildMul(
                        self.builder,
                        val1,
                        val2,
                        CString::new("imul").unwrap().as_ptr(),
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
                Inst::ldc => {
                    let cur_class = &mut *self.cur_class.unwrap();
                    let index = code[pc + 1] as usize;
                    match cur_class.classfile.constant_pool[index] {
                        Constant::IntegerInfo { i } => stack.push(LLVMConstInt(
                            LLVMInt32TypeInContext(self.context),
                            i as u64,
                            0,
                        )),
                        Constant::FloatInfo { f } => stack.push(LLVMConstReal(
                            LLVMFloatTypeInContext(self.context),
                            f as f64,
                        )),
                        _ => unimplemented!(),
                    };
                }
                Inst::ireturn if !loop_compile => {
                    let val = stack.pop().unwrap();
                    LLVMBuildRet(self.builder, val);
                }
                Inst::return_ if !loop_compile => {
                    LLVMBuildRetVoid(self.builder);
                }
                Inst::invokestatic => {
                    // TODO: The following code should be a method.
                    let cur_class = &mut *self.cur_class.unwrap();
                    let mref_index = ((code[pc + 1] as usize) << 8) + code[pc + 2] as usize;
                    let (class_index, name_and_type_index) = fld!(
                        Constant::MethodrefInfo,
                        &cur_class.classfile.constant_pool[mref_index],
                        class_index,
                        name_and_type_index
                    );
                    let name_index = fld!(
                        Constant::ClassInfo,
                        &cur_class.classfile.constant_pool[class_index],
                        name_index
                    );
                    let class_name = cur_class.classfile.constant_pool[name_index as usize]
                        .get_utf8()
                        .unwrap();
                    let class =
                        load_class(cur_class.classheap.unwrap(), self.objectheap, class_name);
                    let (name_index, descriptor_index) = fld!(
                        Constant::NameAndTypeInfo,
                        &cur_class.classfile.constant_pool[name_and_type_index],
                        name_index,
                        descriptor_index
                    );
                    let name = cur_class.classfile.constant_pool[name_index]
                        .get_utf8()
                        .unwrap();
                    let descriptor = cur_class.classfile.constant_pool[descriptor_index]
                        .get_utf8()
                        .unwrap();
                    let (_virtual_class, exec_method) =
                        (&*class).get_method(name, descriptor).unwrap();

                    let jit_info_mgr = (&mut *class).get_jit_info_mgr(name_index, descriptor_index);
                    let jit_func = jit_info_mgr.get_jit_func();
                    let llvm_func = if Some((
                        exec_method.name_index as usize,
                        exec_method.descriptor_index as usize,
                    )) == self.cur_func_indices
                    {
                        self.cur_func.unwrap()
                    } else {
                        if jit_func.is_none() {
                            return Err(Error::CouldntCompile);
                        }

                        let exec_info = jit_func.clone().unwrap();
                        if exec_info.cant_compile {
                            return Err(Error::CouldntCompile);
                        }
                        if exec_info.func == 0 {
                            return Err(Error::CouldntCompile);
                        }

                        *self.raw_func_addr_to_llvm_val.get(&exec_info.func).unwrap()
                    };

                    let mut args = vec![];
                    for _ in 0..LLVMCountParams(llvm_func) {
                        args.push(stack.pop().unwrap());
                    }
                    args.reverse();

                    let ret = LLVMBuildCall(
                        self.builder,
                        llvm_func,
                        args.as_mut_slice().as_mut_ptr(),
                        args.len() as u32,
                        CString::new("").unwrap().as_ptr(),
                    );
                    if LLVMGetTypeKind(LLVMGetReturnType(LLVMTypeOf(llvm_func)))
                        != llvm::LLVMTypeKind::LLVMVoidTypeKind
                    {
                        stack.push(ret);
                    }
                }
                e => {
                    dprintln!("***JIT: unimplemented instruction: {}***", e);
                    return Err(Error::CouldntCompile);
                }
            }

            pc += Inst::get_inst_size(cur_code);
        }

        Ok(stack)
    }

    unsafe fn declare_local_var(&mut self, name: usize, ty: &VariableType) -> LLVMValueRef {
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
                    Inst::istore | Inst::iload => {
                        let index = block.code[pc + 1] as usize;
                        vars.insert(index, VariableType::Int);
                    }
                    // TODO: Add
                    _ => {}
                }
                pc += Inst::get_inst_size(cur_code);
            }
        }

        vars
    }

    fn infer_return_type(&mut self, blocks: &Vec<Block>) -> CResult<VariableType> {
        for block in blocks {
            let mut pc = 0;
            while pc < block.code.len() {
                let cur_code = block.code[pc];
                match cur_code {
                    Inst::return_ => return Ok(VariableType::Void),
                    Inst::ireturn => return Ok(VariableType::Int),
                    // TODO: Add
                    _ => {}
                }
                pc += Inst::get_inst_size(cur_code);
            }
        }
        Err(Error::CouldntCompile)
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
