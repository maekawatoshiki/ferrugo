use super::jit::*;
use llvm;
use llvm::{core::*, prelude::*};
use rustc_hash::FxHashMap;
use std::ffi::CString;

#[rustfmt::skip]
pub unsafe fn native_functions(
    module: LLVMModuleRef,
    context: LLVMContextRef,
) -> FxHashMap<String, LLVMValueRef> {
    let mut map = FxHashMap::default();

    macro_rules! parse_ty {
        (void   ) => { VariableType::Void.   to_llvmty(context) };
        (int    ) => { VariableType::Int.    to_llvmty(context) };
        (double ) => { VariableType::Double. to_llvmty(context) };
        (ptr) => { VariableType::Pointer.to_llvmty(context) };
    }
    macro_rules! define_native_function {
        ($ret_ty:ident, [ $($param_ty:ident),* ], $name:expr) => {
            let mut params_ty = vec![$(parse_ty!($param_ty)),*];
            let func_ty = LLVMFunctionType(
                            parse_ty!($ret_ty),
                            params_ty.as_mut_ptr(),
                            params_ty.len() as u32, 0);
            let func = LLVMAddFunction(
                        module,
                        CString::new($name).unwrap().as_ptr(), 
                        func_ty);
            map.insert($name.to_string(), func);
        }
    }

    define_native_function!(void, [ptr, ptr, int], "java/io/PrintStream.println:(I)V");
    define_native_function!(void, [ptr, ptr, ptr], "java/io/PrintStream.println:(Ljava/lang/String;)V");
    define_native_function!(void, [ptr, ptr, ptr], "java/io/PrintStream.print:(Ljava/lang/String;)V");
    define_native_function!(ptr,  [ptr, ptr, int ],"java/lang/StringBuilder.append:(I)Ljava/lang/StringBuilder;");
    define_native_function!(ptr,  [ptr, ptr, ptr], "java/lang/StringBuilder.append:(Ljava/lang/String;)Ljava/lang/StringBuilder;");
    define_native_function!(ptr,  [ptr, ptr],      "java/lang/StringBuilder.toString:()Ljava/lang/String;");
    define_native_function!(ptr,  [ptr, ptr],      "ferrugo_internal_new");

    map
}
