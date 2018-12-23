extern crate ferrugo;
use ferrugo::class::class::Class;
use ferrugo::class::classfile::attribute::Attribute;
use ferrugo::class::classheap;
use ferrugo::exec::objectheap::ObjectHeap;
use ferrugo::exec::vm::VM;
use ferrugo::exec::frame::Variable;
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
    let class2_ptr = gc::new(Class::new());
    let class3_ptr = gc::new(Class::new());

    if let None = classheap.load_class(filename, class1_ptr) {
        eprintln!(
            "{}: An error occurred while loading class file",
            Colour::Red.bold().paint("error"),
        );
        return;
    }
    unsafe { (*class1_ptr).classheap = Some(classheap_ptr) };
    classheap
        .load_class("java/io/PrintStream.class", class2_ptr)
        .unwrap();
    unsafe { (*class2_ptr).classheap = Some(classheap_ptr) };
    classheap
        .load_class("java/lang/System.class", class3_ptr)
        .unwrap();
    unsafe { (*class3_ptr).classheap = Some(classheap_ptr) };

    let (class, method) = unsafe { &*class1_ptr }
        .get_method("main", "([Ljava/lang/String;)V")
        .unwrap();

    let objectheap_ptr = gc::new(ObjectHeap::new());
    let objectheap = unsafe { &mut *objectheap_ptr};

    let object = objectheap.create_object(class1_ptr);

    let mut vm = VM::new();
    vm.classheap = Some(classheap_ptr);
    vm.objectheap = Some(objectheap_ptr);
    vm.stack[0] = Variable::Object(object);
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
