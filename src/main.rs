#[macro_use]
extern crate ferrugo;
use ferrugo::class::{class::Class, classheap};
use ferrugo::exec::objectheap::ObjectHeap;
use ferrugo::exec::vm::VM;

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
        .arg(Arg::with_name("file").help("Input file name").index(1))
        .arg(
            Arg::with_name("dump")
                .help("Dumps methods in the specified classfile")
                .short("d")
                .long("dump"),
        );
    let app_matches = app.clone().get_matches();

    let filename = match app_matches.value_of("file") {
        Some(filename) => filename,
        None => return,
    };

    if app_matches.is_present("dump") {
        show_methods(filename);
        return;
    }

    run_file(filename);
}

fn run_file(filename: &str) {
    #[rustfmt::skip]
    macro_rules! expect { ($expr:expr, $msg:expr) => {{ match $expr {
        Some(some) => some,
        None => { eprintln!("{}: {}", Colour::Red.bold().paint("error"), $msg); return }
    } }}; }

    let objectheap_ptr = Box::into_raw(Box::new(ObjectHeap::new()));
    let objectheap = unsafe { &mut *objectheap_ptr };

    let classheap_ptr = objectheap.gc.alloc(classheap::ClassHeap::new());
    let classheap = unsafe { &mut *classheap_ptr };

    let mut vm = VM::new(classheap, objectheap);
    vm.load_class("java/lang/String");

    let class_ptr = vm.load_class_by_file_name(filename);

    let (class, method) = expect!(
        unsafe { &*class_ptr }.get_method("main", "([Ljava/lang/String;)V"),
        "Couldn't find method 'main(String[])'"
    );

    let object = objectheap.create_object(class_ptr);

    vm.stack[0] = object;
    vm.frame_stack[0].class = Some(class);
    vm.frame_stack[0].method_info = method;
    vm.frame_stack[0].sp = vm.frame_stack[0]
        .method_info
        .code
        .as_ref()
        .unwrap()
        .max_locals as usize;

    dprintln!("---- exec output begin ----");
    vm.run();
    dprintln!("---- exec output end ------");

    dprintln!("stack trace: {:?}", &vm.stack[0..128]);

    unsafe {
        Box::from_raw(objectheap_ptr);
    }
}

fn show_methods(filename: &str) {
    let classheap_ptr = Box::into_raw(Box::new(classheap::ClassHeap::new()));
    let mut class = Class::new();
    class.classheap = Some(classheap_ptr);
    class.load_classfile(filename);

    for i in 0..class.classfile.methods_count as usize {
        let method = &class.classfile.methods[i];
        let name = class.classfile.constant_pool[(method.name_index) as usize]
            .get_utf8()
            .unwrap();
        let descriptor = class.classfile.constant_pool[(method.descriptor_index) as usize]
            .get_utf8()
            .unwrap();

        println!("Method:");
        println!("  Name: {}", name);
        println!("  Descriptor: {}", descriptor);
        println!("  Code: ");
        method.code.as_ref().unwrap().dump_bytecode();
    }

    unsafe { Box::from_raw(classheap_ptr) };
}

#[test]
fn run_example() {
    run_file("examples/Hello.class");
    run_file("examples/MillerRabin.class");
    run_file("examples/BigInt.class");
    run_file("examples/EratosthenesSieve.class");
}

#[test]
fn read_classfiles() {
    use ferrugo::class::classfile::read::ClassFileReader;
    use std::fs;
    let paths = fs::read_dir("./examples/test").unwrap();
    for filename in paths {
        ClassFileReader::new(filename.unwrap().path().to_str().unwrap())
            .unwrap()
            .read()
            .unwrap();
    }
}
