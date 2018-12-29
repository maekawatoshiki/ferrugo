#![feature(box_patterns)]
#![feature(if_while_or_patterns)]
#![feature(repeat_generic_slice)]

#[macro_use]
pub mod macros;
pub mod class;
pub mod exec;
pub mod gc;

extern crate rustc_hash;
