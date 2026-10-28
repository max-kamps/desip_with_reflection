#![allow(internal_features)]
#![feature(rustc_attrs)]
// Lang features:
#![feature(decl_macro)]
#![feature(type_info)]
#![feature(const_trait_impl)]
// Lib features:
#![feature(const_cmp)]
#![feature(const_heap)]
#![feature(const_destruct)]
#![feature(const_index)]
#![feature(const_iter)]
#![feature(const_try)]
#![feature(int_from_ascii)]
#![feature(ptr_metadata)]

mod const_format;
mod lex;
mod parse;

pub use lex::{JsonError, JsonErrorKind};
pub use parse::parse_slice;
