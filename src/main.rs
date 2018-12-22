extern crate ferrugo;
use ferrugo::class::class::Class;
use ferrugo::class::classfile::attribute::Attribute;
use ferrugo::class::classheap;
use ferrugo::exec::vm::VM;
use ferrugo::gc::gc;

extern crate clap;
use clap::{App, Arg};

extern crate ansi_term;
use ansi_term::Colour;

const VERSION_STR: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    let app = App::new("Ferrugo")
        .version(VERSION_STR)
        .author("uint256_t")
        .about("A JVM Implementation written in Rust")
        .arg(Arg::with_name("file").help("Input file name").index(1));
    let app_matches = app.clone().get_matches();

    let filename = match app_matches.value_of("file") {
        Some(filename) => filename,
        None => return,
    };

    let classheap_ptr = gc::new(classheap::ClassHeap::new());
    let classheap = unsafe { &mut *classheap_ptr };

    let class1_ptr = gc::new(Class::new());
    let _class2_ptr = gc::new(Class::new());

    if let None = classheap.load_class(filename, class1_ptr) {
        eprintln!(
            "{}: An error occurred while loading class file",
            Colour::Red.bold().paint("error"),
        );
        return;
    }
    unsafe { (*class1_ptr).classheap = Some(classheap_ptr) };
    // classheap
    //     .load_class("java/lang/Object.class", class2_ptr)
    //     .unwrap();

    let (class, method) = unsafe { &*class1_ptr }
        .get_method("main", "([Ljava/lang/String;)V")
        .unwrap();

    let mut vm = VM::new();
    vm.classheap = Some(classheap_ptr);
    vm.frame_stack[0].class = Some(class);
    vm.frame_stack[0].method_info = method;
    vm.frame_stack[0].sp = if let Some(Attribute::Code { max_locals, .. }) =
        vm.frame_stack[0].method_info.get_code_attribute()
    {
        *max_locals as usize
    } else {
        panic!()
    };
    vm.run();
    println!("stack top: {:?}", vm.stack);
}
