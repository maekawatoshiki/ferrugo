use super::super::class::class::Class;
use super::super::gc::gc::GcType;
use super::jit::*;
use super::{
    frame::{ObjectBody, VariableType},
    vm::RuntimeEnvironment,
};
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
    define_native_function!(dbl,  [ptr, dbl],      "java/lang/Math.sqrt:(D)D");
    define_native_function!(dbl,  [ptr, dbl],      "java/lang/Math.sin:(D)D");
    define_native_function!(dbl,  [ptr, dbl],      "java/lang/Math.cos:(D)D");
    define_native_function!(dbl,  [ptr, dbl],      "java/lang/Math.tan:(D)D");
    define_native_function!(dbl,  [ptr, dbl, dbl], "java/lang/Math.pow:(DD)D");
    define_native_function!(dbl,  [ptr, dbl],      "java/lang/Math.abs:(D)D");
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
            "java/lang/Math.sin:(D)D",
            java_lang_math_sin_d_d as *mut libc::c_void,
        ),
        (
            "java/lang/Math.pow:(DD)D",
            java_lang_math_pow_dd_d as *mut libc::c_void,
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
    unsafe {
        let string = &mut string_builder.variables[0];
        let mut string2 = (&mut *(*string as GcType<ObjectBody>))
            .get_string_mut()
            .clone();
        string2.push_str(format!("{}", i).as_str());
        *string = (&mut *renv.objectheap).create_string_object(string2.to_string(), renv.classheap);
    }
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
    let append_str = unsafe { (&mut *s).get_string_mut() };
    unsafe {
        let string = &mut string_builder.variables[0];
        let mut string2 = (&mut *(*string as GcType<ObjectBody>))
            .get_string_mut()
            .clone();
        string2.push_str(append_str);
        *string = (&mut *renv.objectheap).create_string_object(string2.to_string(), renv.classheap);
    }
    obj
}

#[no_mangle]
pub extern "C" fn java_lang_stringbuilder_tostring_string(
    _renv: *mut RuntimeEnvironment,
    obj: *mut ObjectBody,
) -> *mut ObjectBody {
    let string_builder = unsafe { &mut *obj };
    let s = string_builder.variables[0];
    s as GcType<ObjectBody>
}

#[no_mangle]
pub extern "C" fn ferrugo_internal_new(
    renv: *mut RuntimeEnvironment,
    class: *mut Class,
) -> *mut ObjectBody {
    let renv = unsafe { &mut *renv };
    let object = unsafe { &mut *renv.objectheap }.create_object(class);
    object as GcType<ObjectBody>
}

#[no_mangle]
pub extern "C" fn java_lang_math_random_d(_renv: *mut RuntimeEnvironment) -> f64 {
    use rand::random;
    random::<f64>()
}
#[no_mangle]
pub extern "C" fn java_lang_math_sqrt_d_d(_renv: *mut RuntimeEnvironment, x: f64) -> f64 {
    x.sqrt()
}
#[no_mangle]
pub extern "C" fn java_lang_math_sin_d_d(_renv: *mut RuntimeEnvironment, x: f64) -> f64 {
    x.sin()
}
#[no_mangle]
pub extern "C" fn java_lang_math_cos_d_d(_renv: *mut RuntimeEnvironment, x: f64) -> f64 {
    x.cos()
}
#[no_mangle]
pub extern "C" fn java_lang_math_tan_d_d(_renv: *mut RuntimeEnvironment, x: f64) -> f64 {
    x.tan()
}
#[no_mangle]
pub extern "C" fn java_lang_math_abs_d_d(_renv: *mut RuntimeEnvironment, x: f64) -> f64 {
    x.abs()
}
#[no_mangle]
pub extern "C" fn java_lang_math_pow_dd_d(_renv: *mut RuntimeEnvironment, x: f64, y: f64) -> f64 {
    x.powf(y)
}
