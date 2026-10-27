#![allow(internal_features)]
#![feature(rustc_attrs)]
#![feature(type_info)]
#![feature(const_heap)]
#![feature(const_trait_impl)]
#![feature(const_type_name)]
#![feature(const_iter)]
#![feature(const_cmp)]
#![feature(const_index)]
#![feature(cow_is_borrowed)]
#![feature(int_from_ascii)]

mod lex;
mod parse;

pub use parse::parse_slice;
