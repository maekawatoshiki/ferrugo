use super::{
    super::{
        class::{class::Class, classfile::constant::Constant},
        gc::gc::GcType,
    },
    cfg::{Block, BrKind},
    frame::{ObjectBody, Variable},
    vm::{load_class, Inst, RuntimeEnvironment},
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

#[derive(Debug, Clone, PartialEq)]
pub enum VariableType {
    Int,
    Void,
    Pointer,
}

trait CastIntoLLVMType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef;
}

impl CastIntoLLVMType for VariableType {
    unsafe fn to_llvmty(&self, ctx: LLVMContextRef) -> LLVMTypeRef {
        match self {
            &VariableType::Int => LLVMInt32TypeInContext(ctx),
            &VariableType::Void => LLVMVoidTypeInContext(ctx),
            &VariableType::Pointer => LLVMPointerType(LLVMInt8TypeInContext(ctx), 0),
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
    pub args_ty: Vec<VariableType>,
    pub ret_ty: Option<VariableType>,
}

impl FuncJITExecInfo {
    pub fn cant_compile() -> Self {
        FuncJITExecInfo {
            func: ptr::null_mut(),
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
            native_functions: {
                let mut map = FxHashMap::default();
                let func_ty = LLVMFunctionType(
                    VariableType::Void.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Int.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    3,
                    0,
                );
                let func = LLVMAddFunction(
                    module,
                    CString::new("java/io/PrintStream.println:(I)V")
                        .unwrap()
                        .as_ptr(),
                    func_ty,
                );
                map.insert("java/io/PrintStream.println:(I)V".to_string(), func);
                let func_ty = LLVMFunctionType(
                    VariableType::Void.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    3,
                    0,
                );
                let func = LLVMAddFunction(
                    module,
                    CString::new("java/io/PrintStream.println:(Ljava/lang/String;)V")
                        .unwrap()
                        .as_ptr(),
                    func_ty,
                );
                map.insert(
                    "java/io/PrintStream.println:(Ljava/lang/String;)V".to_string(),
                    func,
                );
                let name = "java/lang/StringBuilder.append:(I)Ljava/lang/StringBuilder;";
                let func_ty = LLVMFunctionType(
                    VariableType::Pointer.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Int.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    3,
                    0,
                );
                let func = LLVMAddFunction(module, CString::new(name).unwrap().as_ptr(), func_ty);
                map.insert(name.to_string(), func);
                let name =
                    "java/lang/StringBuilder.append:(Ljava/lang/String;)Ljava/lang/StringBuilder;";
                let func_ty = LLVMFunctionType(
                    VariableType::Pointer.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    3,
                    0,
                );
                let func = LLVMAddFunction(module, CString::new(name).unwrap().as_ptr(), func_ty);
                map.insert(name.to_string(), func);
                let name = "java/lang/StringBuilder.toString:()Ljava/lang/String;";
                let func_ty = LLVMFunctionType(
                    VariableType::Pointer.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    2,
                    0,
                );
                let func = LLVMAddFunction(module, CString::new(name).unwrap().as_ptr(), func_ty);
                map.insert(name.to_string(), func);
                let name = "ferrugo_internal_new";
                let func_ty = LLVMFunctionType(
                    VariableType::Pointer.to_llvmty(context),
                    vec![
                        VariableType::Pointer.to_llvmty(context),
                        VariableType::Pointer.to_llvmty(context),
                    ]
                    .as_mut_ptr(),
                    2,
                    0,
                );
                let func = LLVMAddFunction(module, CString::new(name).unwrap().as_ptr(), func_ty);
                map.insert(name.to_string(), func);
                map
            },
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
        let mut local_vars = vec![];

        for i in bp + sp - exec_info.args_ty.len()..bp + sp {
            local_vars.push(match stack[i] {
                Variable::Int(i) => llvm_const_int32(self.context, i as u64),
                _ => return None,
            });
        }

        sp -= exec_info.args_ty.len();

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

        self.add_native_functions(ee);

        let ret_val = llvm::execution_engine::LLVMRunFunction(ee, func, 0, vec![].as_mut_ptr());
        let ret_int = llvm::execution_engine::LLVMGenericValueToInt(ret_val, 0);

        match ret_ty {
            VariableType::Int => {
                stack[bp + sp] = Variable::Int(ret_int as i32);
                sp += 1
            }
            VariableType::Pointer => {}
            VariableType::Void => {}
        }

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
    // TODO
    unsafe fn add_native_functions(&self, ee: llvm::execution_engine::LLVMExecutionEngineRef) {
        for (name, func) in &[
            (
                "java/io/PrintStream.println:(I)V",
                java_io_printstream_println_i_v as *mut libc::c_void,
            ),
            (
                "java/io/PrintStream.println:(Ljava/lang/String;)V",
                java_io_printstream_println_string_v as *mut libc::c_void,
            ),
            (
                "java/lang/StringBuilder.append:(Ljava/lang/String;)Ljava/lang/StringBuilder;",
                java_lang_stringbuilder_append_string_stringbuilder as *mut libc::c_void,
            ),
            (
                "java/lang/StringBuilder.append:(I)Ljava/lang/StringBuilder;",
                java_lang_stringbuilder_append_i_stringbuilder as *mut libc::c_void,
            ),
            (
                "java/lang/StringBuilder.toString:()Ljava/lang/String;",
                java_lang_stringbuilder_tostring_string as *mut libc::c_void,
            ),
            (
                "ferrugo_internal_new",
                ferrugo_internal_new as *mut libc::c_void,
            ),
        ] {
            llvm::execution_engine::LLVMAddGlobalMapping(
                ee,
                *self.native_functions.get(*name).unwrap(),
                *func,
            );
        }
    }
}

impl JIT {
    pub unsafe fn compile_func(
        &mut self,
        (name_index, descriptor_index): (usize, usize),
        class: GcType<Class>,
        blocks: &mut Vec<Block>,
        descriptor: &str,
    ) -> CResult<FuncJITExecInfo> {
        self.cur_class = Some(class);
        self.cur_func_indices = Some((name_index, descriptor_index));

        let (arg_types, ret_ty) = self
            .get_arg_return_ty(descriptor)
            .ok_or(Error::CouldntCompile)?;
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

        self.add_native_functions(ee);

        Ok(FuncJITExecInfo {
            func,
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

        self.add_native_functions(ee);

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
                    stack.push(llvm_const_int32(self.context, num));
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
                        llvm_const_uint32(self.context, 0),
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
                Inst::bipush => {
                    stack.push(llvm_const_uint32(self.context, code[pc + 1] as i8 as u64));
                }
                Inst::sipush => {
                    let val = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    stack.push(llvm_const_uint32(self.context, val as u64));
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
                        Constant::String { string_index } => {
                            let string = cur_class
                                .get_utf8_from_const_pool(string_index as usize)
                                .unwrap()
                                .to_owned();
                            // TODO: Constant string refers to constant pool,
                            // so should not create a new string object.
                            // "aaa" == "aaa" // => true
                            stack.push(
                                (&mut *(&mut *self.runtime_env).objectheap)
                                    .create_string_object(string, cur_class.classheap.unwrap())
                                    .to_llvm_val(self.context),
                            )
                        }
                        _ => return Err(Error::CouldntCompile),
                    };
                }
                Inst::ireturn if !loop_compile => {
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
                    let class = load_class(
                        cur_class.classheap.unwrap(),
                        (&mut *self.runtime_env).objectheap,
                        class_name,
                    );
                    let name_index = fld!(
                        Constant::NameAndTypeInfo,
                        &cur_class.classfile.constant_pool[name_and_type_index],
                        name_index
                    );
                    let name = cur_class.classfile.constant_pool[name_index]
                        .get_utf8()
                        .unwrap();
                    let object = (&*class).get_static_variable(name.as_str()).unwrap();
                    stack.push(object.to_llvm_val(self.context));
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
                    let objectheap = (&mut *self.runtime_env).objectheap;
                    let class = load_class(classheap, objectheap, class_name);
                    let ret = LLVMBuildCall(
                        self.builder,
                        *self.native_functions.get("ferrugo_internal_new").unwrap(),
                        vec![
                            llvm_const_ptr(self.context, self.runtime_env as *mut u64),
                            llvm_const_ptr(self.context, class as *mut u64),
                        ]
                        .as_mut_ptr(),
                        2,
                        CString::new("").unwrap().as_ptr(),
                    );
                    stack.push(ret);
                }
                Inst::dup => {
                    let val = stack.last().clone().unwrap();
                    stack.push(*val);
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
                    let class = load_class(
                        cur_class.classheap.unwrap(),
                        (&mut *self.runtime_env).objectheap,
                        class_name,
                    );
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
                    let mut renv_need = false;
                    let llvm_func = if Some((
                        exec_method.name_index as usize,
                        exec_method.descriptor_index as usize,
                    )) == self.cur_func_indices
                    {
                        self.cur_func.unwrap()
                    } else if let Some(native_func) = {
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

impl Variable {
    pub unsafe fn to_llvm_val(&self, ctx: LLVMContextRef) -> LLVMValueRef {
        match self {
            Variable::Char(c) => LLVMConstInt(LLVMInt16TypeInContext(ctx), *c as u64, 1),
            Variable::Short(i) => LLVMConstInt(LLVMInt16TypeInContext(ctx), *i as u64, 1),
            Variable::Int(i) => LLVMConstInt(LLVMInt32TypeInContext(ctx), *i as u64, 1),
            Variable::Float(f) => LLVMConstReal(LLVMFloatTypeInContext(ctx), *f as f64),
            Variable::Double(f) => LLVMConstReal(LLVMDoubleTypeInContext(ctx), *f),
            Variable::Pointer(p) => {
                let ptr_as_int = LLVMConstInt(LLVMInt64TypeInContext(ctx), *p as u64, 0);
                let const_ptr = LLVMConstIntToPtr(ptr_as_int, VariableType::Pointer.to_llvmty(ctx));
                const_ptr
            }
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

unsafe fn llvm_const_ptr(ctx: LLVMContextRef, p: *mut u64) -> LLVMValueRef {
    let ptr_as_int = LLVMConstInt(LLVMInt64TypeInContext(ctx), p as u64, 0);
    let const_ptr = LLVMConstIntToPtr(ptr_as_int, VariableType::Pointer.to_llvmty(ctx));
    const_ptr
}

#[no_mangle]
pub extern "C" fn java_io_printstream_println_i_v(
    _renv: *mut RuntimeEnvironment,
    _obj: *mut ObjectBody,
    i: i32,
) {
    println!("{}", i);
}

#[no_mangle]
pub extern "C" fn java_io_printstream_println_string_v(
    _renv: *mut RuntimeEnvironment,
    _obj: *mut ObjectBody,
    s: *mut ObjectBody,
) {
    let object_body = unsafe { &mut *s };
    println!("{}", unsafe {
        &*(object_body
            .variables
            .get("str")
            .unwrap()
            .get_pointer::<String>())
    });
}

#[no_mangle]
pub extern "C" fn java_lang_stringbuilder_append_i_stringbuilder(
    renv: *mut RuntimeEnvironment,
    obj: *mut ObjectBody,
    i: i32,
) -> *mut ObjectBody {
    let renv = unsafe { &mut *renv };
    let string_builder = unsafe { &mut *obj };
    let string = unsafe {
        let string = &mut *string_builder
            .variables
            .entry("str".to_string())
            .or_insert((&mut *renv.objectheap).create_string_object("".to_string(), renv.classheap))
            .get_pointer::<ObjectBody>();
        &mut *(string.variables.get_mut("str").unwrap().get_pointer() as GcType<String>)
    };
    string.push_str(format!("{}", i).as_str());
    obj
}

#[no_mangle]
pub extern "C" fn java_lang_stringbuilder_append_string_stringbuilder(
    renv: *mut RuntimeEnvironment,
    obj: *mut ObjectBody,
    s: *mut ObjectBody,
) -> *mut ObjectBody {
    let renv = unsafe { &mut *renv };
    let string_builder = unsafe { &mut *obj };
    let append_str = unsafe {
        let string = &mut *s;
        &*(string.variables.get("str").unwrap().get_pointer::<String>())
    };
    let string = unsafe {
        let string = &mut *string_builder
            .variables
            .entry("str".to_string())
            .or_insert((&mut *renv.objectheap).create_string_object("".to_string(), renv.classheap))
            .get_pointer::<ObjectBody>();
        &mut *(string
            .variables
            .get_mut("str")
            .unwrap()
            .get_pointer::<String>())
    };
    string.push_str(append_str);
    obj
}

#[no_mangle]
pub extern "C" fn java_lang_stringbuilder_tostring_string(
    _renv: *mut RuntimeEnvironment,
    obj: *mut ObjectBody,
) -> *mut ObjectBody {
    let string_builder = unsafe { &mut *obj };
    let s = string_builder.variables.get("str").unwrap().clone();
    s.get_pointer::<ObjectBody>()
}

#[no_mangle]
pub extern "C" fn ferrugo_internal_new(
    renv: *mut RuntimeEnvironment,
    class: *mut Class,
) -> *mut ObjectBody {
    let renv = unsafe { &mut *renv };
    let object = unsafe { &mut *renv.objectheap }.create_object(class);
    object.get_pointer::<ObjectBody>()
}
