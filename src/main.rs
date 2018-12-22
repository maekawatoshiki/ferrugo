extern crate ferrugo;
use ferrugo::class::class::Class;
use ferrugo::class::classfile::attribute::Attribute;
use ferrugo::class::classheap;
use ferrugo::exec::{frame::Frame, vm::VM};
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

    // println!("{:?}", classheap);

    let mut frame_stack = vec![Frame::new(), Frame::new()];

    let (class, method) = unsafe { &*class1_ptr }.get_method("Entry", "()I").unwrap();

    // println!("{:?}", method);
    frame_stack[0].class = Some(class);
    frame_stack[0].method_info = method;
    frame_stack[0].init_stack();
    frame_stack[0].sp = if let Some(Attribute::Code { max_locals, .. }) =
        frame_stack[0].method_info.get_code_attribute()
    {
        *max_locals
    } else {
        panic!()
    };

    let vm = VM::new();
    vm.run(&mut frame_stack);
    println!("stack top: {:?}", frame_stack[0].stack[0]);
}
