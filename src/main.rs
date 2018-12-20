extern crate ferrugo;
use ferrugo::classfile::read;

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

    let mut reader = match read::ClassFileReader::new(filename) {
        Some(reader) => reader,
        None => {
            eprintln!(
                "{}: Couldn't open file '{}'",
                Colour::Red.bold().paint("error"),
                filename
            );
            return;
        }
    };

    reader.read();
}
