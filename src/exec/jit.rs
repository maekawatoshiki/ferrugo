use super::cfg::Block;
use super::vm::Inst;
use libc;
use llvm;
use llvm::core::*;
use llvm::prelude::*;
use rustc_hash::FxHashMap;
use std::ffi::CString;

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
            LLVMModuleCreateWithNameInContext(CString::new("rapidus").unwrap().as_ptr(), context);
        let builder = LLVMCreateBuilderInContext(context);

        JIT {
            context,
            module,
            builder,
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
                name,
                LLVMBuildPointerCast(
                    self.builder,
                    local_var_val,
                    LLVMPointerType(ty.to_llvmty(self.context), 0),
                    CString::new("").unwrap().as_ptr(),
                ),
            );
        }

        assert!(blocks.len() > 0);
        self.compile_block(blocks, 0);
    }

    fn compile_block(&mut self, blocks: &Vec<Block>, idx: usize) {}

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
        // 1: istore_1
        //     2: iload_1
        //     3: iconst_3
        //     4: if_icmpne     11
        //     7: iconst_4
        //     8: goto          12
        //     11: iconst_5
        //     12: istore_1
        //     13: return
    }
}
