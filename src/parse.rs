use std::{any::TypeId, mem::MaybeUninit};

use crate::lex::{JsonError, JsonErrorKind, JsonTokenKind, Lexer};

mod type_ids {
    use std::any::TypeId;

    pub const U8: TypeId = TypeId::of::<u8>();
    pub const U16: TypeId = TypeId::of::<u16>();
    pub const U32: TypeId = TypeId::of::<u32>();
}

#[derive(Debug, Clone, Copy)]
struct Parser {
    required_seen_bits: u16,
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
}

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
            required_seen_bits: 0,
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
            let parse_type = match field_id.type_id() {
                i if i == type_ids::U8 => ParseType::U8,
                i if i == type_ids::U16 => ParseType::U16,
                i if i == type_ids::U32 => ParseType::U32,
                _ => 'block: {
                    let nested_type_id = field_id.type_id();
                    // Check if the nested type has already been added to the objects vector
                    for obj_idx in 0..objects.len() {
                        let obj = &objects[obj_idx];
                        if obj.type_id == nested_type_id {
                            break 'block ParseType::Object(obj_idx);
                        }
                    }
                    // If not, create a new ParseObject for the nested type
                    let nested_object_index = ParseObject::make(nested_type_id, objects);
                    ParseType::Object(nested_object_index)
                }
            };
            fields.push(ParseField {
                name: field_id.name(),
                offset: field_id.offset(),
                parse_type,
            })
        }
        let fields = fields.const_make_global();

        objects[object_index].fields = fields;
        object_index
    }
}

// Runtime parsing:

impl Parser {
    pub fn parse_into(&self, lex: &mut Lexer, dest: *mut u8) -> Result<(), JsonError> {
        self.parse_object_into(&self.objects[self.root_object], lex, dest)
    }

    fn parse_object_into(
        &self,
        object: &ParseObject,
        lex: &mut Lexer,
        dest: *mut u8,
    ) -> Result<(), JsonError> {
        lex.expect_object()?;

        loop {
            match lex.next_token()? {
                JsonTokenKind::EndObject => break,
                JsonTokenKind::PendingString => {
                    let name_offset = lex.offset();
                    let name = lex.read_string_wtf8()?;
                    let field = object
                        .field_with_name(name.as_ref())
                        .ok_or_else(|| lex.error_at(name_offset, JsonErrorKind::UnknownField))?;
                    let field_ptr = unsafe { dest.add(field.offset) };

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
                            self.parse_object_into(nested_object, lex, field_ptr)?;
                        }
                    }
                }
                _ => return Err(lex.error(JsonErrorKind::ExpectedFieldName)),
            }

            match lex.next_token()? {
                JsonTokenKind::EndObject => break,
                JsonTokenKind::Comma => continue,
                _ => return Err(lex.error(JsonErrorKind::ExpectedCommaOrEndObject)),
            }
        }

        Ok(())
    }
}

impl ParseObject {
    fn field_with_name(&self, name: &[u8]) -> Option<&ParseField> {
        self.fields
            .iter()
            .find(|field| field.name.as_bytes() == name)
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
