use super::cfg::{Block, BrKind};
use super::vm::Inst;
use libc;
use llvm;
use llvm::core::*;
use llvm::prelude::*;
use rustc_hash::FxHashMap;
use std::ffi::CString;
use std::ptr;

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

#[derive(Debug)]
pub struct JIT {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    cur_func: Option<LLVMValueRef>,
    env: FxHashMap<usize, LLVMValueRef>,
    bblocks: FxHashMap<usize, LLVMBasicBlockRef>,
    phi_stack: FxHashMap<usize, Vec<(LLVMBasicBlockRef, Vec<LLVMValueRef>)>>, // destination, (basic block, stack)
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

        JIT {
            context,
            module,
            builder,
            cur_func: None,
            env: FxHashMap::default(),
            bblocks: FxHashMap::default(),
            phi_stack: FxHashMap::default(),
        }
    }
}

impl JIT {
    pub unsafe fn compile(&mut self, blocks: &Vec<Block>) {
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

        let func = LLVMAddFunction(
            self.module,
            CString::new("jvm-jited-func").unwrap().as_ptr(),
            func_ty,
        );

        self.cur_func = Some(func);

        let bb_entry = LLVMAppendBasicBlockInContext(
            self.context,
            func,
            CString::new("entry").unwrap().as_ptr(),
        );

        LLVMPositionBuilderAtEnd(self.builder, bb_entry);

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

        for block in blocks {
            if block.start > 0 {
                self.bblocks.insert(
                    block.start,
                    LLVMAppendBasicBlock(func, CString::new("").unwrap().as_ptr()),
                );
            }
        }

        let a = self.compile_block(blocks, 0);
        fn find(pc: usize, blocks: &Vec<Block>) -> usize {
            for (i, block) in blocks.iter().enumerate() {
                if block.start == pc {
                    return i;
                }
            }
            panic!()
        }
        let a = self.compile_block(blocks, find(a, blocks));
        println!("{}", a);

        self.env.clear();
        self.bblocks.clear();

        LLVMDumpModule(self.module);
    }

    unsafe fn compile_block(
        &mut self,
        blocks: &Vec<Block>,
        idx: usize,
        // stack: Vec<LLVMValueRef>,
    ) -> usize {
        let block = &blocks[idx];

        if block.start > 0 {
            let bb = *self.bblocks.get(&block.start).unwrap();
            LLVMPositionBuilderAtEnd(self.builder, bb);
        }

        let mut aastack = vec![];
        if let Some(stacks) = self.phi_stack.get(&block.start) {
            {
                let bb = stacks[0].0;
                for val in &stacks[0].1 {
                    let phi = LLVMBuildPhi(
                        self.builder,
                        LLVMTypeOf(*val),
                        CString::new("").unwrap().as_ptr(),
                    );
                    LLVMAddIncoming(
                        phi,
                        vec![*val].as_mut_slice().as_mut_ptr(),
                        vec![bb].as_mut_slice().as_mut_ptr(),
                        1,
                    );
                    aastack.push(phi);
                }
            }
            for stack in &stacks[1..] {
                let bb = stack.0;
                let mut i = 0;
                for val in &stack.1 {
                    let phi = aastack[i];
                    LLVMAddIncoming(
                        phi,
                        vec![*val].as_mut_slice().as_mut_ptr(),
                        vec![bb].as_mut_slice().as_mut_ptr(),
                        1,
                    );
                    i += 1;
                }
            }
        }

        let stack1 = self.compile_bytecode(&block, aastack);

        fn find(pc: usize, blocks: &Vec<Block>) -> usize {
            for (i, block) in blocks.iter().enumerate() {
                if block.start == pc {
                    return i;
                }
            }
            panic!()
        }

        match &block.kind {
            BrKind::ConditionalJmp { destinations } => {
                let mut d = 0;
                for dst in destinations {
                    let i = find(*dst, blocks);
                    d = self.compile_block(blocks, i);
                }
                d
            }
            BrKind::UnconditionalJmp { destination } => {
                let bb = *self.bblocks.get(&block.start).unwrap();
                self.phi_stack
                    .entry(*destination)
                    .or_insert(vec![])
                    .push((bb, stack1));
                *destination
            }
            BrKind::JmpRequired { destination } => {
                let bb = *self.bblocks.get(&block.start).unwrap();
                LLVMBuildBr(self.builder, bb);
                self.phi_stack
                    .entry(*destination)
                    .or_insert(vec![])
                    .push((bb, stack1));
                *destination
            }
            _ => 0,
        }
    }

    // 1: istore_1
    //     2: iload_1
    //     3: iconst_3
    //     4: if_icmpne     11
    //     7: iconst_4
    //     8: goto          12
    //     11: iconst_5
    //     12: istore_1
    //     13: return
    unsafe fn compile_bytecode(
        &mut self,
        block: &Block,
        mut stack: Vec<LLVMValueRef>,
    ) -> Vec<LLVMValueRef> {
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
                    let num = (cur_code - Inst::iconst_0) as i64 as u64;
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
                Inst::if_icmpne => {
                    let val2 = stack.pop().unwrap();
                    let val1 = stack.pop().unwrap();
                    let cond_val = LLVMBuildICmp(
                        self.builder,
                        llvm::LLVMIntPredicate::LLVMIntNE,
                        val1,
                        val2,
                        CString::new("ine").unwrap().as_ptr(),
                    );

                    let destinations = block.kind.get_conditional_jump_destinations();
                    let bb_then = *self.bblocks.get(&destinations[0]).unwrap();
                    let bb_else = *self.bblocks.get(&destinations[1]).unwrap();

                    LLVMBuildCondBr(self.builder, cond_val, bb_then, bb_else);
                }
                Inst::goto => {
                    let destination = block.kind.get_unconditional_jump_destination();
                    let bb_goto = *self.bblocks.get(&destination).unwrap();
                    LLVMBuildBr(self.builder, bb_goto);
                }
                Inst::return_ => {}
                e => unimplemented!("{}", e),
            }

            pc += Inst::get_inst_size(cur_code);
        }

        stack
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
                    _ => {}
                }
                pc += Inst::get_inst_size(cur_code);
            }
        }

        vars
    }
}
