#![allow(internal_features)]
#![feature(rustc_attrs)]
// Lang features:
#![feature(decl_macro)]
#![feature(type_info)]
#![feature(const_trait_impl)]
// Lib features:
#![feature(const_default)]
#![feature(const_convert)]
#![feature(const_cmp)]
#![feature(const_heap)]
#![feature(const_destruct)]
#![feature(const_index)]
#![feature(const_iter)]
#![feature(const_try)]
#![feature(int_from_ascii)]

mod const_format;
mod lex;
mod parser;

use crate::const_format::{const_panic, full_type_name, id};
pub use crate::lex::{JsonError, JsonErrorKind, Lexer};
pub use crate::parser::bytecode::Program;
use crate::parser::construct::program_for_type_id;
use std::mem::MaybeUninit;

pub fn desip_slice<T: 'static>(slice: &[u8], dest: &mut MaybeUninit<T>) -> Result<(), JsonError> {
    let mut lex: Lexer = Lexer::new(slice);
    let parser = &const {
        match program_for_type_id(id![T]) {
            Ok(parser) => parser,
            Err(e) => const_panic!(
                "Failed to construct parser for type `{}`. Error: {}",
                full_type_name(id![T]),
                e
            ),
        }
    };
    parser.parse_into(&mut lex, dest.as_mut_ptr().cast())?;
    Ok(())
}

pub fn deserialize_slice<T: 'static>(slice: &[u8]) -> Result<T, JsonError> {
    let mut t = MaybeUninit::<T>::uninit();
    desip_slice(slice, &mut t)?;
    Ok(unsafe { t.assume_init() })
}
