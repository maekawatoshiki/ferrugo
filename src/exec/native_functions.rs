use super::super::class::class::Class;
use super::jit::*;
use super::{frame::ObjectBody, vm::RuntimeEnvironment};
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
        (dbl)     => { VariableType::Double. to_llvmty(context) };
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
    define_native_function!(dbl,  [ptr],           "java/lang/Math.random:()D");
    define_native_function!(ptr,  [ptr, ptr],      "ferrugo_internal_new");

    map
}

pub unsafe fn add_native_functions(
    native_functions: &FxHashMap<String, LLVMValueRef>,
    ee: llvm::execution_engine::LLVMExecutionEngineRef,
) {
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
            "java/io/PrintStream.print:(Ljava/lang/String;)V",
            java_io_printstream_print_string_v as *mut libc::c_void,
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
            "java/lang/Math.random:()D",
            java_lang_math_random_d as *mut libc::c_void,
        ),
        (
            "ferrugo_internal_new",
            ferrugo_internal_new as *mut libc::c_void,
        ),
    ] {
        llvm::execution_engine::LLVMAddGlobalMapping(
            ee,
            *native_functions.get(*name).unwrap(),
            *func,
        );
    }
}

// Builtin Native Functions

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
    let string = unsafe { &mut *s };
    println!("{}", string.get_string_mut());
}

#[no_mangle]
pub extern "C" fn java_io_printstream_print_string_v(
    _renv: *mut RuntimeEnvironment,
    _obj: *mut ObjectBody,
    s: *mut ObjectBody,
) {
    print!("{}", unsafe { &mut *s }.get_string_mut());
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
        string.get_string_mut()
    };
    string.push_str(format!("{}", i).as_str());
    obj
}

#[no_mangle]
pub extern "C" fn java_lang_stringbuilder_append_string_stringbuilder(
    _renv: *mut RuntimeEnvironment,
    obj: *mut ObjectBody,
    s: *mut ObjectBody,
) -> *mut ObjectBody {
    let string_builder = unsafe { &mut *obj };
    let append_str = unsafe { (&mut *s).get_string_mut() };
    let string = unsafe {
        let string = &mut *string_builder
            .variables
            .get("str")
            .unwrap()
            .get_pointer::<ObjectBody>();
        string.get_string_mut()
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

#[no_mangle]
pub extern "C" fn java_lang_math_random_d(_renv: *mut RuntimeEnvironment) -> f64 {
    use rand::random;
    random::<f64>()
}
