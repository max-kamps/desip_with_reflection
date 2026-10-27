use std::{
    alloc::{Allocator as _, Global, Layout},
    any::TypeId,
    mem::MaybeUninit,
    ptr::{self, DynMetadata},
};

use crate::lex::{JsonError, JsonErrorKind, JsonTokenKind, Lexer};

mod type_ids {
    use std::any::TypeId;

    pub const U8: TypeId = TypeId::of::<u8>();
    pub const U16: TypeId = TypeId::of::<u16>();
    pub const U32: TypeId = TypeId::of::<u32>();
}

#[derive(Debug, Clone, Copy)]
struct Parser {
    root_object: usize,
    objects: &'static [ParseObject],
}

#[derive(Debug, Clone, Copy)]
struct ParseObject {
    fields: &'static [ParseField],
    type_id: TypeId,
}

#[derive(Debug, Clone, Copy)]
struct ParseField {
    name: &'static str,
    offset: usize,
    parse_type: ParseType,
    vtable: DynMetadata<dyn Empty>,
}

trait Empty {}
impl<T> Empty for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseType {
    U8,
    U16,
    U32,
    Object(usize),
}

// Comptime parser construction:

impl Parser {
    #[rustc_comptime]
    pub fn new(type_id: TypeId) -> Parser {
        let mut objects = Vec::new();
        let root_object = ParseObject::make(type_id, &mut objects);
        let objects = objects.const_make_global();

        Parser {
            root_object,
            objects,
        }
    }
}

impl ParseObject {
    #[rustc_comptime]
    fn make(type_id: TypeId, objects: &mut Vec<ParseObject>) -> usize {
        objects.push(ParseObject {
            fields: &[],
            type_id,
        });
        let object_index = objects.len() - 1;

        let mut fields = Vec::new();
        for field_idx in 0..type_id.fields(0) {
            let field_id = type_id.field(0, field_idx);
            let field_type_id = field_id.type_id();
            let parse_type = match field_type_id {
                i if i == type_ids::U8 => ParseType::U8,
                i if i == type_ids::U16 => ParseType::U16,
                i if i == type_ids::U32 => ParseType::U32,
                _ => 'block: {
                    // Check if the nested type has already been added to the objects vector
                    #[allow(clippy::needless_range_loop)] // Enumerate is not const yet
                    for obj_idx in 0..objects.len() {
                        let obj = &objects[obj_idx];
                        if obj.type_id == field_type_id {
                            break 'block ParseType::Object(obj_idx);
                        }
                    }
                    // If not, create a new ParseObject for the nested type
                    let nested_object_index = ParseObject::make(field_type_id, objects);
                    ParseType::Object(nested_object_index)
                }
            };

            let vtable = field_type_id
                .trait_info_of::<dyn Empty>()
                .unwrap()
                .get_vtable();

            fields.push(ParseField {
                name: field_id.name(),
                offset: field_id.offset(),
                parse_type,
                vtable,
            })
        }
        let fields = fields.const_make_global();

        objects[object_index].fields = fields;
        object_index
    }
}

// Runtime parsing:

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

impl Parser {
    pub fn parse_into(&self, lex: &mut Lexer, dest: *mut u8) -> Result<(), JsonError> {
        // FIXME: This assumes the global allocator will be lazy and overprovision memory.
        // It would be nicer to allocate scratch memory on the stack instead of on the heap.
        let scratchpad_layout = Layout::from_size_align(256 * 1024 * 1024, 16).unwrap();
        let mut scratchpad = Global.allocate(scratchpad_layout).unwrap();
        self.parse_object_into(&self.objects[self.root_object], lex, dest, unsafe {
            scratchpad.as_mut()
        })?;
        unsafe { Global.deallocate(scratchpad.cast(), scratchpad_layout) };
        Ok(())
    }

    fn parse_object_into(
        &self,
        object: &ParseObject,
        lex: &mut Lexer,
        dest: *mut u8,
        scratchpad: &mut [u8],
    ) -> Result<(), JsonError> {
        lex.expect_object()?;

        let (field_initialized, scratchpad) =
            scratchpad.split_at_mut(object.fields.len().div_ceil(8));
        let mut field_initialized = BitSet::new(field_initialized);

        let result =
            self.parse_object_into_inner(object, lex, dest, scratchpad, &mut field_initialized);

        if result.is_err() {
            // If parsing fails we need to drop any fields that were already initialized
            for (field_index, field) in object.fields.iter().enumerate() {
                if field_initialized.test(field_index) {
                    let field_ptr = unsafe { dest.add(field.offset) };
                    let wide_pointer: *mut dyn Empty =
                        ptr::from_raw_parts_mut(field_ptr, field.vtable);
                    unsafe { ptr::drop_in_place(wide_pointer) };
                }
            }
        }

        result
    }

    fn parse_object_into_inner(
        &self,
        object: &ParseObject,
        lex: &mut Lexer<'_>,
        dest: *mut u8,
        scratchpad: &mut [u8],
        field_initialized: &mut BitSet<'_>,
    ) -> Result<(), JsonError> {
        loop {
            match lex.next_token()? {
                JsonTokenKind::EndObject => break,
                JsonTokenKind::PendingString => {
                    let name_offset = lex.offset();
                    let name = lex.read_string_wtf8()?;
                    let field_index = object
                        .fields
                        .iter()
                        .position(|field| field.name.as_bytes() == name)
                        .ok_or_else(|| lex.error_at(name_offset, JsonErrorKind::UnknownField))?;
                    let field = object.fields[field_index];
                    let field_ptr = unsafe { dest.add(field.offset) };

                    if field_initialized.test(field_index) {
                        return Err(lex.error_at(name_offset, JsonErrorKind::DuplicateField));
                    }

                    lex.expect_colon()?;

                    match field.parse_type {
                        ParseType::U8 | ParseType::U16 | ParseType::U32 => {
                            lex.expect_number()?;
                            let value_offset = lex.offset();
                            let n = lex.read_integer()?;

                            macro_rules! write_integer {
                                ($ty:ty) => {
                                    unsafe {
                                        field_ptr.cast::<$ty>().write(n.try_into().map_err(
                                            |_| {
                                                lex.error_at(
                                                    value_offset,
                                                    JsonErrorKind::IntegerTooLarge,
                                                )
                                            },
                                        )?)
                                    }
                                };
                            }

                            match field.parse_type {
                                ParseType::U8 => write_integer!(u8),
                                ParseType::U16 => write_integer!(u16),
                                ParseType::U32 => write_integer!(u32),
                                _ => unreachable!(),
                            }
                        }
                        ParseType::Object(i) => {
                            let nested_object = &self.objects[i];
                            self.parse_object_into(nested_object, lex, field_ptr, scratchpad)?;
                        }
                    }

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

        if !field_initialized.all_up_to(object.fields.len()) {
            return Err(lex.error(JsonErrorKind::MissingField));
        }

        Ok(())
    }
}

pub fn parse_slice<T: 'static>(slice: &[u8]) -> Result<T, JsonError> {
    let mut lex: Lexer = Lexer::new(slice);
    let mut t = MaybeUninit::<T>::uninit();
    let parser = &const {
        let type_id = TypeId::of::<T>();
        Parser::new(type_id)
    };
    println!("Parser: {:#?}", parser);
    parser.parse_into(&mut lex, t.as_mut_ptr().cast())?;
    Ok(unsafe { t.assume_init() })
}
