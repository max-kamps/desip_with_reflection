use crate::const_format::const_panic;
use crate::lex::{JsonError, JsonErrorKind, JsonTokenKind, Lexer};
use std::alloc::{Allocator as _, Global, Layout};
use std::any::TypeId;
use std::mem::MaybeUninit;
use std::mem::type_info::TypeKind;
use std::ptr::{self, DynMetadata};

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
    Never,
    Unit,
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    // Char,
    F32,
    F64,
    Object(usize),
}

const impl TryFrom<TypeId> for ParseType {
    type Error = ();

    fn try_from(type_id: TypeId) -> Result<Self, Self::Error> {
        match type_id {
            i if i == TypeId::of::<!>() => Ok(ParseType::Never),
            i if i == TypeId::of::<()>() => Ok(ParseType::Unit),
            i if i == TypeId::of::<bool>() => Ok(ParseType::Bool),
            i if i == TypeId::of::<u8>() => Ok(ParseType::U8),
            i if i == TypeId::of::<u16>() => Ok(ParseType::U16),
            i if i == TypeId::of::<u32>() => Ok(ParseType::U32),
            i if i == TypeId::of::<u64>() => Ok(ParseType::U64),
            i if i == TypeId::of::<u128>() => Ok(ParseType::U128),
            i if i == TypeId::of::<usize>() => Ok(ParseType::USize),
            i if i == TypeId::of::<i8>() => Ok(ParseType::I8),
            i if i == TypeId::of::<i16>() => Ok(ParseType::I16),
            i if i == TypeId::of::<i32>() => Ok(ParseType::I32),
            i if i == TypeId::of::<i64>() => Ok(ParseType::I64),
            i if i == TypeId::of::<i128>() => Ok(ParseType::I128),
            i if i == TypeId::of::<isize>() => Ok(ParseType::ISize),
            // i if i == TypeId::of::<char>() => Ok(ParseType::Char),
            i if i == TypeId::of::<f32>() => Ok(ParseType::F32),
            i if i == TypeId::of::<f64>() => Ok(ParseType::F64),
            _ => Err(()),
        }
    }
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
            let name = field_id.name();
            let offset = field_id.offset();
            let parse_type = match ParseType::try_from(field_type_id) {
                Ok(t) => t,
                Err(_) => match field_type_id.info().kind {
                    TypeKind::Struct => 'block: {
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
                    other => {
                        const_panic!(
                            "Field {} of type {} has unsupported type {}",
                            name,
                            "<TODO type name>",
                            "<TODO field type name>"
                        );
                    }
                },
            };

            let vtable = field_type_id
                .trait_info_of::<dyn Empty>()
                .unwrap()
                .get_vtable();

            fields.push(ParseField {
                name,
                offset,
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
                        ParseType::Never => {
                            return Err(lex.error_at(lex.offset(), JsonErrorKind::UnexpectedValue));
                        }
                        ParseType::Unit => {
                            lex.expect_null()?;
                        }
                        ParseType::Bool => {
                            let ptr = field_ptr.cast::<bool>();
                            match lex.next_token()? {
                                JsonTokenKind::True => unsafe { ptr.write(true) },
                                JsonTokenKind::False => unsafe { ptr.write(false) },
                                _ => return Err(lex.error(JsonErrorKind::ExpectedBoolean)),
                            }
                        }
                        ParseType::U8
                        | ParseType::U16
                        | ParseType::U32
                        | ParseType::U64
                        | ParseType::U128
                        | ParseType::USize
                        | ParseType::I8
                        | ParseType::I16
                        | ParseType::I32
                        | ParseType::I64
                        | ParseType::I128
                        | ParseType::ISize
                        | ParseType::F32
                        | ParseType::F64 => {
                            lex.expect_number()?;
                            let value_offset = lex.offset();
                            let n_str = lex.read_number_raw()?;
                            // Safety: read_number_raw only returns valid JSON numbers, which are
                            // all ASCII and thus valid UTF-8.
                            let n_str = unsafe { str::from_utf8_unchecked(n_str) };

                            macro_rules! write_number {
                                ($ty:ty) => {
                                    unsafe {
                                        let ptr = field_ptr.cast::<$ty>();
                                        let n: $ty = n_str.parse().map_err(|_| {
                                            lex.error_at(value_offset, JsonErrorKind::InvalidNumber)
                                        })?;
                                        ptr.write(n);
                                    }
                                };
                            }

                            match field.parse_type {
                                ParseType::U8 => write_number!(u8),
                                ParseType::U16 => write_number!(u16),
                                ParseType::U32 => write_number!(u32),
                                ParseType::U64 => write_number!(u64),
                                ParseType::U128 => write_number!(u128),
                                ParseType::USize => write_number!(usize),
                                ParseType::I8 => write_number!(i8),
                                ParseType::I16 => write_number!(i16),
                                ParseType::I32 => write_number!(i32),
                                ParseType::I64 => write_number!(i64),
                                ParseType::I128 => write_number!(i128),
                                ParseType::ISize => write_number!(isize),
                                ParseType::F32 => write_number!(f32),
                                ParseType::F64 => write_number!(f64),
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
