extern crate ferrugo;
use ferrugo::class::class::Class;
use ferrugo::class::classfile::attribute::Attribute;
use ferrugo::class::classheap;
use ferrugo::exec::frame::Variable;
use ferrugo::exec::objectheap::ObjectHeap;
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

    run_file(filename);
}

fn run_file(filename: &str) {
    let classheap_ptr = gc::new(classheap::ClassHeap::new());
    let classheap = unsafe { &mut *classheap_ptr };

    macro_rules! try_load_class {
        ($filename:expr) => {{
            let class_ptr = gc::new(Class::new());
            if let None = classheap.load_class($filename, class_ptr) {
                eprintln!(
                    "{}: An error occurred while loading class file",
                    Colour::Red.bold().paint("error"),
                );
                return;
            }
            unsafe { (*class_ptr).classheap = Some(classheap_ptr) };
            class_ptr
        }};
    }

    macro_rules! expect {
        ($expr:expr, $msg:expr) => {{
            match $expr {
                Some(some) => some,
                None => {
                    eprintln!("{}: {}", Colour::Red.bold().paint("error"), $msg);
                    return;
                }
            }
        }};
    }

    let class_ptr = try_load_class!(filename);
    try_load_class!("examples/java/lang/String.class");
    try_load_class!("examples/Print.class");
    // "java/lang/System.class"
    // "java/lang/String.class"
    // "java/lang/Integer.class"

    let (class, method) = expect!(
        unsafe { &*class_ptr }.get_method("main", "([Ljava/lang/String;)V"),
        "Couldn't find method 'main(String[])'"
    );

    let objectheap_ptr = gc::new(ObjectHeap::new());
    let objectheap = unsafe { &mut *objectheap_ptr };

    let object = objectheap.create_object(class_ptr);

    let mut vm = VM::new(classheap, objectheap);
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

    println!("---- exec output begin ----");
    vm.run();
    println!("---- exec output end ------");

    println!("stack trace: {:?}", vm.stack);
}
