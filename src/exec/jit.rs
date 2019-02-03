use super::{
    super::{
        class::{class::Class, classfile::constant::Constant},
        gc::gc::GcType,
    },
    cfg::{Block, BrKind},
    frame::VariableType,
    native_functions,
    vm::{d2u, u2d, Inst, RuntimeEnvironment},
};
use libc;
use llvm;
use llvm::{core::*, prelude::*};
use rand::random;
use rustc_hash::FxHashMap;
use std::{ffi::CString, mem::transmute, ptr};

pub type CResult<T> = Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    CouldntCompile,
    General,
}

pub trait CastIntoLLVMType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef;
}

impl CastIntoLLVMType for VariableType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef {
        match self {
            &VariableType::Int => LLVMInt32TypeInContext(ctx),
            &VariableType::Double => LLVMDoubleTypeInContext(ctx),
            &VariableType::Void => LLVMVoidTypeInContext(ctx),
            &VariableType::Pointer => LLVMPointerType(LLVMInt8TypeInContext(ctx), 0),
            &VariableType::Long => unimplemented!(),
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
    pub func: LLVMValueRef,
    pub cant_compile: bool,
    pub params_ty: Vec<VariableType>,
    pub params_len: usize,
    pub ret_ty: Option<VariableType>,
}

impl FuncJITExecInfo {
    pub fn cant_compile() -> Self {
        FuncJITExecInfo {
            func: ptr::null_mut(),
            cant_compile: true,
            params_ty: vec![],
            params_len: 0,
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
    cur_func_indices: Option<(usize, usize)>,
    env: FxHashMap<usize, LLVMValueRef>,
    bblocks: FxHashMap<usize, BasicBlockInfo>,
    phi_stack: FxHashMap<usize, Vec<PhiStack>>, // destination,
    native_functions: FxHashMap<String, LLVMValueRef>,
    runtime_env: GcType<RuntimeEnvironment>,
}

impl JIT {
    pub unsafe fn new(runtime_env: GcType<RuntimeEnvironment>) -> Self {
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
            cur_func_indices: None,
            env: FxHashMap::default(),
            bblocks: FxHashMap::default(),
            phi_stack: FxHashMap::default(),
            runtime_env,
            native_functions: native_functions::native_functions(module, context),
        }
    }
}

impl JIT {
    pub unsafe fn run_func(
        &self,
        stack: &mut Vec<u64>,
        bp: usize,
        mut sp: usize,
        exec_info: &FuncJITExecInfo,
    ) -> Option<usize> {
        let mut local_vars = vec![];
        let mut params_ty_iter = exec_info.params_ty.iter();

        let mut i = bp + sp - exec_info.params_len;
        while i < bp + sp {
            let val = stack[i];
            local_vars.push(match params_ty_iter.next().unwrap() {
                VariableType::Int => llvm_const_int32(self.context, val),
                VariableType::Double => {
                    i += 1;
                    llvm_const_double(self.context, u2d(val))
                }
                VariableType::Pointer => llvm_const_ptr(self.context, val as *mut u64),
                _ => return None,
            });
            i += 1;
        }

        sp -= exec_info.params_len;

        let ret_ty = exec_info.ret_ty.clone().unwrap();
        let func_ret_ty = ret_ty.to_llvmty(self.context);
        let func_ty = LLVMFunctionType(func_ret_ty, vec![].as_mut_ptr(), 0, 0);
        let func_name = format!("ferrugo-jit-func-executer-{}", random::<u32>());
        let func = LLVMAddFunction(
            self.module,
            CString::new(func_name.as_str()).unwrap().as_ptr(),
            func_ty,
        );
        let bb_entry = LLVMAppendBasicBlockInContext(
            self.context,
            func,
            CString::new("entry").unwrap().as_ptr(),
        );
        LLVMPositionBuilderAtEnd(self.builder, bb_entry);
        let val = LLVMBuildCall(
            self.builder,
            exec_info.func,
            local_vars.as_mut_ptr(),
            local_vars.len() as u32,
            CString::new("").unwrap().as_ptr(),
        );
        LLVMBuildRet(self.builder, val);
        // when_debug!(LLVMDumpValue(func));
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

        native_functions::add_native_functions(&self.native_functions, ee);

        let ret_val = llvm::execution_engine::LLVMRunFunction(ee, func, 0, vec![].as_mut_ptr());
        let ret_int = llvm::execution_engine::LLVMGenericValueToInt(ret_val, 0);

        match ret_ty {
            VariableType::Int => {
                stack[bp + sp] = ret_int as i32 as u64;
                sp += 1
            }
            VariableType::Double => {
                stack[bp + sp] = ret_int as u64;
                sp += 2
            }
            VariableType::Pointer => {
                stack[bp + sp] = ret_int as u64;
                sp += 1
            }
            _ => {}
        }

        LLVMDeleteFunction(func);

        Some(sp)
    }

    pub unsafe fn run_loop(
        &self,
        stack: &mut Vec<u64>,
        bp: usize,
        exec_info: &LoopJITExecInfo,
    ) -> Option<usize> {
        let mut raw_local_vars = vec![];

        for (offset, ty) in &exec_info.local_variables {
            let val = stack[bp + offset];
            raw_local_vars.push(match ty {
                VariableType::Int => Box::into_raw(Box::new(val as i32)) as *mut libc::c_void,
                VariableType::Double => Box::into_raw(Box::new(u2d(val))) as *mut libc::c_void,
                VariableType::Pointer => Box::into_raw(Box::new(val as u64)) as *mut libc::c_void,
                _ => return None,
            });
        }

        let pc = transmute::<u64, fn(*mut *mut libc::c_void) -> i32>(exec_info.func)(
            raw_local_vars.as_mut_slice().as_mut_ptr(),
        );

        for (i, (offset, ty)) in exec_info.local_variables.iter().enumerate() {
            stack[bp + offset] = match ty {
                VariableType::Int => *(raw_local_vars[i] as *mut i32) as u64,
                VariableType::Double => d2u(*(raw_local_vars[i] as *mut f64)),
                VariableType::Pointer => *(raw_local_vars[i] as *mut u64),
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
        descriptor: &str,
        static_method: bool,
    ) -> CResult<FuncJITExecInfo> {
        self.cur_class = Some(class);
        self.cur_func_indices = Some((name_index, descriptor_index));

        let (arg_types, ret_ty) = {
            let (mut arg_types, ret_ty) = self
                .get_arg_return_ty(descriptor)
                .ok_or(Error::CouldntCompile)?;
            if !static_method {
                arg_types.insert(0, VariableType::Pointer);
            }
            (arg_types, ret_ty)
        };
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

        let mut var_id = 0;
        for (i, ty) in arg_types.iter().enumerate() {
            LLVMBuildStore(
                self.builder,
                LLVMGetParam(func, i as u32),
                self.declare_local_var(var_id, &ty),
            );
            var_id += match ty {
                // TODO: VariableType::Long
                VariableType::Double => 2,
                _ => 1,
            };
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

        when_debug!(LLVMDumpValue(func));

        if let Err(e) = compiling_error {
            LLVMDeleteFunction(func);
            dprintln!("JIT: compiling error");
            return Err(e);
        }

        llvm::analysis::LLVMVerifyFunction(
            func,
            llvm::analysis::LLVMVerifierFailureAction::LLVMAbortProcessAction,
        );

        LLVMRunPassManager(self.pass_mgr, self.module);

        Ok(FuncJITExecInfo {
            func,
            cant_compile: false,
            params_len: arg_types.iter().fold(0, |acc, ty| {
                acc + match ty {
                    VariableType::Double => 2,
                    _ => 1,
                }
            }),
            params_ty: arg_types.clone(),
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
                vec![llvm_const_uint64(self.context, i as u64)]
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
                llvm_const_int32(self.context, last_block.code_end_position() as u64),
            );
        }

        for (pos, bb) in &self.bblocks {
            if let BasicBlockInfo::Unpositioned(bb) = *bb {
                if cur_bb_has_no_terminator(self.builder) {
                    LLVMBuildBr(self.builder, bb);
                }
                LLVMPositionBuilderAtEnd(self.builder, bb);
                if cur_bb_has_no_terminator(self.builder) {
                    LLVMBuildRet(self.builder, llvm_const_int32(self.context, *pos as u64));
                }
            }
        }

        self.env.clear();
        self.bblocks.clear();
        self.phi_stack.clear();

        when_debug!(LLVMDumpModule(self.module));

        if let Err(e) = compiling_error {
            LLVMDeleteFunction(func);
            dprintln!("JIT: compiling error");
            return Err(e);
        }

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

        native_functions::add_native_functions(&self.native_functions, ee);

        let func_raw = llvm::execution_engine::LLVMGetFunctionAddress(
            ee,
            CString::new(func_name.as_str()).unwrap().as_ptr(),
        );

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
            // Firstly, build llvm's phi which needs a type of all conceivable values.
            let src_bb = phi_stacks[0].src_bb;
            for val in &phi_stacks[0].stack {
                let phi = LLVMBuildPhi(
                    self.builder,
                    LLVMTypeOf(*val),
                    CString::new("").unwrap().as_ptr(),
                );
                LLVMAddIncoming(phi, vec![*val].as_mut_ptr(), vec![src_bb].as_mut_ptr(), 1);
                stack.push(phi);
            }

            for phi_stack in &phi_stacks[1..] {
                let src_bb = phi_stack.src_bb;
                for (i, val) in (&phi_stack.stack).iter().enumerate() {
                    let phi = stack[init_size + i];
                    LLVMAddIncoming(phi, vec![*val].as_mut_ptr(), vec![src_bb].as_mut_ptr(), 1);
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
                Ok(d)
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
                if cur_bb_has_no_terminator(self.builder) {
                    let bb = self
                        .get_basic_block(destination)
                        .set_positioned()
                        .retrieve();
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
                    stack.push(llvm_const_int32(self.context, num));
                }
                Inst::dconst_0 | Inst::dconst_1 => {
                    let num = cur_code as f64 - Inst::dconst_0 as f64;
                    stack.push(llvm_const_double(self.context, num));
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
                Inst::dstore => {
                    let index = code[pc as usize + 1] as usize;
                    let val = stack.pop().unwrap();
                    LLVMBuildStore(
                        self.builder,
                        val,
                        self.declare_local_var(index, &VariableType::Double),
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
                Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {
                    let name = (cur_code - Inst::dload_0) as usize;
                    let var = self.declare_local_var(name, &VariableType::Double);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ));
                }
                Inst::dload => {
                    let index = code[pc + 1] as usize;
                    let var = self.declare_local_var(index, &VariableType::Double);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ))
                }
                Inst::aload_0 | Inst::aload_1 | Inst::aload_2 | Inst::aload_3 => {
                    let index = (cur_code - Inst::aload_0) as usize;
                    let var = self.declare_local_var(index, &VariableType::Pointer);
                    stack.push(LLVMBuildLoad(
                        self.builder,
                        var,
                        CString::new("").unwrap().as_ptr(),
                    ))
                }
                Inst::if_icmpne | Inst::if_icmpge | Inst::if_icmpgt | Inst::if_icmpeq => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::if_icmpeq => llvm::LLVMIntPredicate::LLVMIntEQ,
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
                Inst::ifne | Inst::ifeq | Inst::ifle | Inst::ifge => {
                    let val = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        match cur_code {
                            Inst::ifeq => llvm::LLVMIntPredicate::LLVMIntEQ,
                            Inst::ifne => llvm::LLVMIntPredicate::LLVMIntNE,
                            Inst::ifge => llvm::LLVMIntPredicate::LLVMIntSGE,
                            Inst::ifle => llvm::LLVMIntPredicate::LLVMIntSLE,
                            _ => unreachable!(),
                        },
                        val,
                        llvm_const_int32(self.context, 0),
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
                        llvm_const_uint32(self.context, const_ as u64),
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
                Inst::dadd => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildFAdd(
                        self.builder,
                        val1,
                        val2,
                        CString::new("dadd").unwrap().as_ptr(),
                    ));
                }
                Inst::dsub => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildFSub(
                        self.builder,
                        val1,
                        val2,
                        CString::new("dsub").unwrap().as_ptr(),
                    ));
                }
                Inst::dmul => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildFMul(
                        self.builder,
                        val1,
                        val2,
                        CString::new("dmul").unwrap().as_ptr(),
                    ));
                }
                Inst::iand => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildAnd(
                        self.builder,
                        val1,
                        val2,
                        CString::new("iand").unwrap().as_ptr(),
                    ));
                }
                Inst::ixor => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildXor(
                        self.builder,
                        val1,
                        val2,
                        CString::new("ixor").unwrap().as_ptr(),
                    ));
                }
                Inst::ishl => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildShl(
                        self.builder,
                        val1,
                        val2,
                        CString::new("ishl").unwrap().as_ptr(),
                    ));
                }
                Inst::ishr => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    stack.push(LLVMBuildAShr(
                        self.builder,
                        val1,
                        val2,
                        CString::new("ishr").unwrap().as_ptr(),
                    ));
                }
                Inst::dcmpl | Inst::dcmpg => self.gen_dcmp(&mut stack)?,
                Inst::bipush => {
                    stack.push(llvm_const_int32(self.context, code[pc + 1] as i8 as u64));
                }
                Inst::sipush => {
                    let val = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    stack.push(llvm_const_int32(self.context, val as u64));
                }
                Inst::ldc => {
                    let cur_class = &mut *self.cur_class.unwrap();
                    let index = code[pc + 1] as usize;
                    match cur_class.classfile.constant_pool[index] {
                        Constant::IntegerInfo { i } => {
                            stack.push(llvm_const_int32(self.context, i as u64))
                        }
                        Constant::FloatInfo { f } => stack.push(LLVMConstReal(
                            LLVMFloatTypeInContext(self.context),
                            f as f64,
                        )),
                        Constant::String { string_index } => stack.push({
                            let string_object = (&mut *self.cur_class.unwrap())
                                .get_java_string_utf8_from_const_pool(
                                    (&mut *self.runtime_env).objectheap,
                                    string_index as usize,
                                )
                                .unwrap();
                            llvm_const_ptr(self.context, string_object as GcType<u64>)
                        }),
                        _ => return Err(Error::CouldntCompile),
                    };
                }
                Inst::ldc2_w => {
                    let cur_class = &mut *self.cur_class.unwrap();
                    let index = ((code[pc + 1] as usize) << 8) + code[pc + 2] as usize;
                    match cur_class.classfile.constant_pool[index] {
                        Constant::DoubleInfo { f } => {
                            stack.push(llvm_const_double(self.context, f))
                        }
                        _ => return Err(Error::CouldntCompile),
                    };
                }
                Inst::baload => {
                    let index = stack.pop().unwrap();
                    let arrayref = stack.pop().unwrap();
                    let val = self.call_function(
                        *self
                            .native_functions
                            .get("ferrugo_internal_baload")
                            .unwrap(),
                        vec![
                            llvm_const_ptr(self.context, self.runtime_env as *mut u64),
                            arrayref,
                            index,
                        ],
                    );
                    stack.push(val);
                }
                Inst::bastore => {
                    let val = stack.pop().unwrap();
                    let index = stack.pop().unwrap();
                    let arrayref = stack.pop().unwrap();
                    self.call_function(
                        *self
                            .native_functions
                            .get("ferrugo_internal_bastore")
                            .unwrap(),
                        vec![
                            llvm_const_ptr(self.context, self.runtime_env as *mut u64),
                            arrayref,
                            index,
                            val,
                        ],
                    );
                }
                Inst::ireturn | Inst::dreturn | Inst::areturn if !loop_compile => {
                    let val = stack.pop().unwrap();
                    LLVMBuildRet(self.builder, val);
                }
                Inst::return_ if !loop_compile => {
                    LLVMBuildRetVoid(self.builder);
                }
                Inst::getstatic => {
                    // TODO: The following code should be a method.
                    let cur_class = &mut *self.cur_class.unwrap();
                    let mref_index = ((code[pc + 1] as usize) << 8) + code[pc + 2] as usize;
                    let (class_index, name_and_type_index) = fld!(
                        Constant::FieldrefInfo,
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
                    let class = (&*cur_class.classheap.unwrap())
                        .get_class(class_name)
                        .unwrap();
                    let name_index = fld!(
                        Constant::NameAndTypeInfo,
                        &cur_class.classfile.constant_pool[name_and_type_index],
                        name_index
                    );
                    let name = cur_class.classfile.constant_pool[name_index]
                        .get_utf8()
                        .unwrap();
                    let object = (&*class).get_static_variable(name.as_str()).unwrap();
                    stack.push(llvm_const_ptr(self.context, object as GcType<u64>));
                }
                Inst::new => {
                    let cur_class = &mut *self.cur_class.unwrap();
                    let class_index = ((code[pc + 1] as usize) << 8) + code[pc + 2] as usize;
                    let name_index = fld!(
                        Constant::ClassInfo,
                        &cur_class.classfile.constant_pool[class_index],
                        name_index
                    );
                    let class_name = cur_class.classfile.constant_pool[name_index as usize]
                        .get_utf8()
                        .unwrap();
                    let classheap = (&mut *self.runtime_env).classheap;
                    let class = (&*classheap).get_class(class_name).unwrap();
                    let ret = self.call_function(
                        *self.native_functions.get("ferrugo_internal_new").unwrap(),
                        vec![
                            llvm_const_ptr(self.context, self.runtime_env as *mut u64),
                            llvm_const_ptr(self.context, class as *mut u64),
                        ],
                    );
                    stack.push(ret);
                }
                Inst::pop => {
                    stack.pop().unwrap();
                }
                Inst::dup => {
                    let val = stack.last().clone().unwrap();
                    stack.push(*val);
                }
                Inst::i2d => {
                    let val = stack.pop().unwrap();
                    stack.push(LLVMBuildSIToFP(
                        self.builder,
                        val,
                        VariableType::Double.to_llvmty(self.context),
                        CString::new("i2d").unwrap().as_ptr(),
                    ));
                }
                Inst::d2i => {
                    let val = stack.pop().unwrap();
                    stack.push(LLVMBuildFPToSI(
                        self.builder,
                        val,
                        VariableType::Int.to_llvmty(self.context),
                        CString::new("d2i").unwrap().as_ptr(),
                    ));
                }
                Inst::invokespecial => {}
                Inst::invokestatic | Inst::invokevirtual => {
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
                    let class = (&*cur_class.classheap.unwrap())
                        .get_class(class_name)
                        .unwrap();
                    let (name_index, descriptor_index) = fld!(
                        Constant::NameAndTypeInfo,
                        &cur_class.classfile.constant_pool[name_and_type_index],
                        name_index,
                        descriptor_index
                    );
                    let jit_info_mgr = (&mut *class).get_jit_info_mgr(name_index, descriptor_index);
                    let jit_func = jit_info_mgr.get_jit_func();
                    let mut renv_need = false;
                    let llvm_func = if Some((name_index as usize, descriptor_index as usize))
                        == self.cur_func_indices
                    {
                        self.cur_func.unwrap()
                    } else if let Some(native_func) = {
                        let name = cur_class.classfile.constant_pool[name_index]
                            .get_utf8()
                            .unwrap();
                        let descriptor = cur_class.classfile.constant_pool[descriptor_index]
                            .get_utf8()
                            .unwrap();
                        let signature = format!("{}.{}:{}", class_name, name, descriptor);
                        self.native_functions.get(signature.as_str())
                    } {
                        renv_need = true;
                        *native_func
                    } else {
                        if jit_func.is_none() {
                            return Err(Error::CouldntCompile);
                        }

                        let exec_info = jit_func.clone().unwrap();
                        if exec_info.cant_compile {
                            return Err(Error::CouldntCompile);
                        }

                        exec_info.func
                    };

                    let mut args = vec![];
                    let args_count = LLVMCountParams(llvm_func) - if renv_need { 1 } else { 0 };
                    for _ in 0..args_count {
                        args.push(stack.pop().unwrap());
                    }
                    if renv_need {
                        args.push(llvm_const_ptr(self.context, self.runtime_env as *mut u64));
                    }
                    args.reverse();

                    let ret = self.call_function(llvm_func, args);

                    if LLVMGetTypeKind(LLVMGetElementType(LLVMGetReturnType(LLVMTypeOf(llvm_func))))
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

    #[rustfmt::skip]
    unsafe fn gen_dcmp(&mut self, stack: &mut Vec<LLVMValueRef>) -> CResult<()> {
        let func = self.cur_func.unwrap();
        let v2 = stack.pop().unwrap();
        let v1 = stack.pop().unwrap();
        let bb_merge = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());

        let cond1 = LLVMBuildFCmp(self.builder, llvm::LLVMRealPredicate::LLVMRealOGT, v1, v2, CString::new("").unwrap().as_ptr());
        let bb_then1 = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        LLVMBuildCondBr(self.builder, cond1, bb_then1, bb_else);
        LLVMPositionBuilderAtEnd(self.builder, bb_then1);
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        let cond2 = LLVMBuildFCmp(self.builder, llvm::LLVMRealPredicate::LLVMRealOEQ, v1, v2, CString::new("").unwrap().as_ptr());
        let bb_then2 = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        LLVMBuildCondBr(self.builder, cond2, bb_then2, bb_else);
        LLVMPositionBuilderAtEnd(self.builder, bb_then2);
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        let cond3 = LLVMBuildFCmp(self.builder, llvm::LLVMRealPredicate::LLVMRealOLT, v1, v2, CString::new("").unwrap().as_ptr());
        let bb_then3 = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        let bb_else = LLVMAppendBasicBlockInContext(self.context, func, CString::new("").unwrap().as_ptr());
        LLVMBuildCondBr(self.builder, cond3, bb_then3, bb_else);
        LLVMPositionBuilderAtEnd(self.builder, bb_then3);
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_else);
        LLVMBuildBr(self.builder, bb_merge);

        LLVMPositionBuilderAtEnd(self.builder, bb_merge);
        let phi = LLVMBuildPhi(self.builder, VariableType::Int.to_llvmty(self.context), CString::new("").unwrap().as_ptr());
        LLVMAddIncoming(phi, vec![llvm_const_int32(self.context, 1)].as_mut_ptr(), vec![bb_then1].as_mut_ptr(), 1);
        LLVMAddIncoming(phi, vec![llvm_const_int32(self.context, 0)].as_mut_ptr(), vec![bb_then2].as_mut_ptr(), 1);
        LLVMAddIncoming(phi, vec![llvm_const_int32(self.context, (0-1) as u64)].as_mut_ptr(), vec![bb_then3].as_mut_ptr(), 1);
        LLVMAddIncoming(phi, vec![llvm_const_int32(self.context, 0)].as_mut_ptr(), vec![bb_else].as_mut_ptr(), 1); // TODO

        stack.push(phi);

        Ok(())
    }

    unsafe fn call_function(
        &self,
        callee: LLVMValueRef,
        mut args: Vec<LLVMValueRef>,
    ) -> LLVMValueRef {
        LLVMBuildCall(
            self.builder,
            callee,
            args.as_mut_ptr(),
            args.len() as u32,
            CString::new("").unwrap().as_ptr(),
        )
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
                    Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {
                        vars.insert((cur_code - Inst::dload_0) as usize, VariableType::Double);
                    }
                    Inst::aload_0 | Inst::aload_1 | Inst::aload_2 | Inst::aload_3 => {
                        vars.insert((cur_code - Inst::aload_0) as usize, VariableType::Pointer);
                    }
                    Inst::astore_0 | Inst::astore_1 | Inst::astore_2 | Inst::astore_3 => {
                        vars.insert((cur_code - Inst::astore_0) as usize, VariableType::Pointer);
                    }
                    Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {
                        vars.insert((cur_code - Inst::istore_0) as usize, VariableType::Int);
                    }
                    Inst::dstore_0 | Inst::dstore_1 | Inst::dstore_2 | Inst::dstore_3 => {
                        vars.insert((cur_code - Inst::dstore_0) as usize, VariableType::Double);
                    }
                    Inst::istore | Inst::iload => {
                        let index = block.code[pc + 1] as usize;
                        vars.insert(index, VariableType::Int);
                    }
                    Inst::dstore | Inst::dload => {
                        let index = block.code[pc + 1] as usize;
                        vars.insert(index, VariableType::Double);
                    }
                    // TODO: Add
                    _ => {}
                }
                pc += Inst::get_inst_size(cur_code);
            }
        }

        vars
    }

    fn get_arg_return_ty(&self, descriptor: &str) -> Option<(Vec<VariableType>, VariableType)> {
        // https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
        let mut i = 1;
        let mut args_ty = vec![];
        let mut ret_ty = None;
        let mut args = true;
        while i < descriptor.len() {
            let c = descriptor.chars().nth(i).unwrap();
            let ty = match c {
                'L' => {
                    while descriptor.chars().nth(i).unwrap() != ';' {
                        i += 1
                    }
                    VariableType::Pointer
                }
                'I' => VariableType::Int,
                'Z' => VariableType::Int,
                'D' => VariableType::Double,
                ')' => {
                    args = false;
                    i += 1;
                    continue;
                }
                _ => return None,
            };
            if args {
                args_ty.push(ty)
            } else {
                ret_ty = Some(ty);
            }
            i += 1;
        }
        Some((args_ty, ret_ty.unwrap()))
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

unsafe fn llvm_const_int32(ctx: LLVMContextRef, n: u64) -> LLVMValueRef {
    LLVMConstInt(LLVMInt32TypeInContext(ctx), n, 1)
}

unsafe fn llvm_const_uint32(ctx: LLVMContextRef, n: u64) -> LLVMValueRef {
    LLVMConstInt(LLVMInt32TypeInContext(ctx), n, 0)
}

unsafe fn llvm_const_uint64(ctx: LLVMContextRef, n: u64) -> LLVMValueRef {
    LLVMConstInt(LLVMInt64TypeInContext(ctx), n, 0)
}

unsafe fn llvm_const_double(ctx: LLVMContextRef, f: f64) -> LLVMValueRef {
    LLVMConstReal(LLVMDoubleTypeInContext(ctx), f)
}

unsafe fn llvm_const_ptr(ctx: LLVMContextRef, p: *mut u64) -> LLVMValueRef {
    let ptr_as_int = LLVMConstInt(LLVMInt64TypeInContext(ctx), p as u64, 0);
    let const_ptr = LLVMConstIntToPtr(ptr_as_int, VariableType::Pointer.to_llvmty(ctx));
    const_ptr
}
