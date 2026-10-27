#![allow(internal_features)]
#![feature(rustc_attrs)]
#![feature(type_info)]
#![feature(const_trait_impl)]
#![feature(const_destruct)]
#![feature(const_heap)]
#![feature(const_iter)]
#![feature(const_cmp)]
#![feature(const_index)]
#![feature(const_convert)]
#![feature(int_from_ascii)]
#![feature(ptr_metadata)]
#![feature(decl_macro)]

mod const_format;
mod lex;
mod parse;

pub use lex::{JsonError, JsonErrorKind};
pub use parse::parse_slice;
