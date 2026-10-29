use crate::lex::{JsonTokenKind, Lexer};
use crate::parser::bytecode::{Command, Field, Opcode, Program};
use crate::{JsonError, JsonErrorKind};
use std::alloc::{Allocator as _, Global, Layout};

/// A vector of bits, needed to track which fields of an object have been initialized
struct BitSet<'a> {
    data: &'a mut [u8],
}

impl<'a> BitSet<'a> {
    fn new(data: &'a mut [u8]) -> Self {
        data.fill(0);
        BitSet { data }
    }

    fn set(&mut self, index: usize) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        self.data[byte_index] |= 1 << bit_index;
    }

    fn test(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;
        (self.data[byte_index] & (1 << bit_index)) != 0
    }

    fn all_up_to(&self, index: usize) -> bool {
        let full_bytes = index / 8;
        for &b in &self.data[0..full_bytes] {
            if b != 0xFF {
                return false;
            }
        }
        let last_b = self.data[full_bytes];
        for i in 0..(index % 8) {
            if (last_b & (1 << i)) == 0 {
                return false;
            }
        }
        true
    }
}

impl Program {
    pub fn parse_into(&self, lex: &mut Lexer, dest: *mut u8) -> Result<(), JsonError> {
        // Allocate 256 MiB of scratchpad memory.
        // FIXME: This assumes the global allocator will be lazy and overprovision memory.
        // If this is false, it would suck to allocate 256 MiB of memory for every parse.
        // It might be nicer to allocate scratch memory on the stack instead of on the heap?
        let scratchpad_layout = Layout::from_size_align(256 * 1024 * 1024, 16).unwrap();
        let mut scratchpad = Global.allocate(scratchpad_layout).unwrap();
        let result = self.execute_opcode(self.root, lex, dest, unsafe { scratchpad.as_mut() });
        unsafe { Global.deallocate(scratchpad.cast(), scratchpad_layout) };
        result
    }

    fn execute_opcode(
        &self,
        opcode: Opcode,
        lex: &mut Lexer,
        dest: *mut u8,
        scratchpad: &mut [u8],
    ) -> Result<(), JsonError> {
        let command = self.get_command(opcode);
        match command {
            Command::Never => {
                return Err(lex.error_at(lex.offset(), JsonErrorKind::UnexpectedValue));
            }
            Command::Unit => {
                lex.expect_null()?;
            }
            Command::Bool => {
                let ptr = dest.cast::<bool>();
                match lex.next_token()? {
                    JsonTokenKind::True => unsafe { ptr.write(true) },
                    JsonTokenKind::False => unsafe { ptr.write(false) },
                    _ => return Err(lex.error(JsonErrorKind::ExpectedBoolean)),
                }
            }
            Command::U8
            | Command::U16
            | Command::U32
            | Command::U64
            | Command::U128
            | Command::Usize
            | Command::I8
            | Command::I16
            | Command::I32
            | Command::I64
            | Command::I128
            | Command::Isize
            | Command::F32
            | Command::F64 => {
                lex.expect_number()?;
                let value_offset = lex.offset();
                let n_str = lex.read_number_raw()?;
                // Safety: read_number_raw only returns valid JSON numbers, which are valid UTF-8
                let n_str = unsafe { str::from_utf8_unchecked(n_str) };

                macro write_number($ty:ty) {
                    unsafe {
                        let ptr = dest.cast::<$ty>();
                        let n: $ty = n_str.parse().map_err(|_| {
                            lex.error_at(value_offset, JsonErrorKind::InvalidNumber)
                        })?;
                        ptr.write(n);
                    }
                }

                match command {
                    Command::U8 => write_number!(u8),
                    Command::U16 => write_number!(u16),
                    Command::U32 => write_number!(u32),
                    Command::U64 => write_number!(u64),
                    Command::U128 => write_number!(u128),
                    Command::Usize => write_number!(usize),
                    Command::I8 => write_number!(i8),
                    Command::I16 => write_number!(i16),
                    Command::I32 => write_number!(i32),
                    Command::I64 => write_number!(i64),
                    Command::I128 => write_number!(i128),
                    Command::Isize => write_number!(isize),
                    Command::F32 => write_number!(f32),
                    Command::F64 => write_number!(f64),
                    _ => unreachable!(),
                }
            }

            Command::String => {
                lex.expect_string()?;
                let ptr = dest.cast::<String>();
                let cow = lex.read_string_utf8()?;
                let owned = cow.into_owned();
                unsafe { ptr.write(owned) };
            }

            Command::Object { fields } => {
                self.parse_object(fields, lex, dest, scratchpad)?;
            }

            Command::FixedArray {
                len,
                elem_size,
                elem_ty,
            } => {
                self.parse_fixed_array(len, elem_size, elem_ty, lex, dest, scratchpad)?;
            }
        }
        Ok(())
    }

    fn parse_object(
        &self,
        fields: &[Field],
        lex: &mut Lexer,
        dest: *mut u8,
        scratchpad: &mut [u8],
    ) -> Result<(), JsonError> {
        lex.expect_object()?;

        let (field_initialized, scratchpad) = scratchpad.split_at_mut(fields.len().div_ceil(8));
        let mut field_initialized = BitSet::new(field_initialized);

        // Closure to avoid accidentally early-returning
        let result = (|| {
            loop {
                match lex.next_token()? {
                    JsonTokenKind::EndObject => break,
                    JsonTokenKind::PendingString => {
                        let name_offset = lex.offset();
                        let name = lex.read_string_wtf8()?;
                        let field_index = fields
                            .iter()
                            .position(|field| self.get_string(field.name) == name)
                            .ok_or_else(|| {
                                lex.error_at(name_offset, JsonErrorKind::UnknownField)
                            })?;
                        let field = fields[field_index];
                        if field_initialized.test(field_index) {
                            return Err(lex.error_at(name_offset, JsonErrorKind::DuplicateField));
                        }

                        lex.expect_colon()?;

                        let field_ptr = unsafe { dest.add(field.offset as usize) };
                        self.execute_opcode(field.ty, lex, field_ptr, scratchpad)?;

                        field_initialized.set(field_index);
                    }
                    _ => return Err(lex.error(JsonErrorKind::ExpectedFieldName)),
                }

                match lex.next_token()? {
                    JsonTokenKind::EndObject => break,
                    JsonTokenKind::Comma => continue,
                    _ => return Err(lex.error(JsonErrorKind::ExpectedCommaOrEndObject)),
                }
            }

            if !field_initialized.all_up_to(fields.len()) {
                return Err(lex.error(JsonErrorKind::MissingField));
            }

            Ok(())
        })();

        if result.is_err() {
            // If parsing fails we need to drop any fields that were already initialized
            for (field_index, field) in fields.iter().enumerate() {
                if field_initialized.test(field_index) {
                    let field_ptr = unsafe { dest.add(field.offset as usize) };
                    self.drop_field(field.ty, field_ptr);
                }
            }
        }

        result
    }

    fn parse_fixed_array(
        &self,
        len: u32,
        elem_size: u32,
        elem_ty: Opcode,
        lex: &mut crate::Lexer<'_>,
        dest: *mut u8,
        scratchpad: &mut [u8],
    ) -> Result<(), JsonError> {
        lex.expect_array()?;

        let initialized_elems = 0;

        // Closure to avoid accidentally early-returning
        let result = (|| {
            let mut i = 0;
            loop {
                let elem_ptr = unsafe { dest.add(i as usize * elem_size as usize) };
                self.execute_opcode(elem_ty, lex, elem_ptr, scratchpad)?;
                match lex.next_token()? {
                    JsonTokenKind::EndArray => {
                        if i + 1 != len {
                            return Err(lex.error(JsonErrorKind::UnexpectedEndOfArray));
                        }
                        break;
                    }
                    JsonTokenKind::Comma => {
                        i += 1;
                        if i >= len {
                            return Err(lex.error(JsonErrorKind::UnexpectedValue));
                        }
                    }
                    _ => return Err(lex.error(JsonErrorKind::ExpectedCommaOrEndArray)),
                }
            }
            Ok(())
        })();

        if result.is_err() {
            // If parsing fails we need to drop any elements that were already initialized
            for i in 0..initialized_elems {
                let elem_ptr = unsafe { dest.add(i as usize * elem_size as usize) };
                self.drop_field(elem_ty, elem_ptr);
            }
        }

        result
    }

    fn drop_field(&self, opcode: Opcode, field_ptr: *mut u8) {
        let command = self.get_command(opcode);
        match command {
            Command::Never => {}
            Command::Unit => {}
            Command::Bool
            | Command::U8
            | Command::U16
            | Command::U32
            | Command::U64
            | Command::U128
            | Command::Usize
            | Command::I8
            | Command::I16
            | Command::I32
            | Command::I64
            | Command::I128
            | Command::Isize
            | Command::F32
            | Command::F64 => {}
            Command::String => {
                let ptr = field_ptr.cast::<String>();
                unsafe { ptr.drop_in_place() };
            }
            Command::Object { fields } => {
                for field in fields {
                    let field_ptr = unsafe { field_ptr.add(field.offset as usize) };
                    self.drop_field(field.ty, field_ptr);
                }
            }
            Command::FixedArray {
                len,
                elem_size,
                elem_ty,
            } => {
                for i in 0..len {
                    let elem_ptr = unsafe { field_ptr.add(i as usize * elem_size as usize) };
                    self.drop_field(elem_ty, elem_ptr);
                }
            }
        }
    }
}
