#![allow(internal_features)]
#![feature(rustc_attrs)]
#![feature(type_info)]
#![feature(const_heap)]
#![feature(const_trait_impl)]
#![feature(const_iter)]
#![feature(const_cmp)]
#![feature(const_index)]
#![feature(int_from_ascii)]
#![feature(ptr_metadata)]

mod lex;
mod parse;

pub use lex::{JsonError, JsonErrorKind};
pub use parse::parse_slice;
