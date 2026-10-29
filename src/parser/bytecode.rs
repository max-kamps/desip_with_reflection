use crate::const_format::const_panic;
use std::{
    fmt::{self, Debug},
    slice,
};

// This could be replaced with AsRef<[T]> once that trait is const
pub const trait AsSlice<T> {
    fn as_ref(&self) -> &[T];
}

const impl AsSlice<u32> for &'static [u32] {
    fn as_ref(&self) -> &[u32] {
        self
    }
}

const impl AsSlice<u32> for Vec<u32> {
    fn as_ref(&self) -> &[u32] {
        self
    }
}

/// List of commands required to deserialize a type.
///
/// Generic over the command storage, so we can have `Program<&'static [Opcode]>` for compile-time
/// generated programs, and `Program<Vec<Opcode>>` for either runtime-generated programs, or programs
/// still in the process of being generated.
#[derive(Clone, Copy)]
pub struct Program<Commands = &'static [u32]> {
    pub root: Opcode,
    pub commands: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Opcode(u32);

impl Opcode {
    const fn category(&self) -> u32 {
        self.0 >> 28
    }

    const fn argument(&self) -> u32 {
        self.0 & 0x0FFF_FFFF
    }

    pub const INVALID: Self = Opcode(0);

    const CAT_SCALAR: u32 = 0; // Must be 0, so that scalar types encode to just their argument
    const CAT_OBJECT: u32 = 1;
    const CAT_FIXED_ARRAY: u32 = 2;

    // 0 is reserved to be invalid, so we can catch uninitialized opcodes
    const SCALAR_NEVER: Self = Opcode(1);
    const SCALAR_UNIT: Self = Opcode(2);
    const SCALAR_BOOL: Self = Opcode(3);
    const SCALAR_U8: Self = Opcode(4);
    const SCALAR_U16: Self = Opcode(5);
    const SCALAR_U32: Self = Opcode(6);
    const SCALAR_U64: Self = Opcode(7);
    const SCALAR_U128: Self = Opcode(8);
    const SCALAR_USIZE: Self = Opcode(9);
    const SCALAR_I8: Self = Opcode(10);
    const SCALAR_I16: Self = Opcode(11);
    const SCALAR_I32: Self = Opcode(12);
    const SCALAR_I64: Self = Opcode(13);
    const SCALAR_I128: Self = Opcode(14);
    const SCALAR_ISIZE: Self = Opcode(15);
    const SCALAR_F32: Self = Opcode(16);
    const SCALAR_F64: Self = Opcode(17);
    const SCALAR_STRING: Self = Opcode(18);
    const SCALAR_BOX_STR: Self = Opcode(19);

    const fn new(category: u32, argument: u32) -> Self {
        assert!(category <= 0xF, "Category must be 4 bits");
        assert!(argument <= 0x0FFF_FFFF, "Argument must be 28 bits");
        Opcode((category << 28) | argument)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct StringIndex(u32);

#[rustfmt::skip]
#[derive(Debug, Clone, Copy)]
pub enum Command<'prog> {
    Never, Unit,
    Bool,
    U8, U16, U32, U64, U128, Usize,
    I8, I16, I32, I64, I128, Isize,
    F32, F64,
    String, BoxStr,

    FixedArray {
        len: u32,
        elem_size: u32,
        elem_ty: Opcode,
    },
    Object {
        fields: &'prog [Field],
    },
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Field {
    pub name: StringIndex,
    pub offset: u32,
    pub ty: Opcode,
}

impl Command<'_> {
    pub const fn as_scalar_opcode(self) -> Option<Opcode> {
        match self {
            Command::Never => Some(Opcode::SCALAR_NEVER),
            Command::Unit => Some(Opcode::SCALAR_UNIT),
            Command::Bool => Some(Opcode::SCALAR_BOOL),
            Command::U8 => Some(Opcode::SCALAR_U8),
            Command::U16 => Some(Opcode::SCALAR_U16),
            Command::U32 => Some(Opcode::SCALAR_U32),
            Command::U64 => Some(Opcode::SCALAR_U64),
            Command::U128 => Some(Opcode::SCALAR_U128),
            Command::Usize => Some(Opcode::SCALAR_USIZE),
            Command::I8 => Some(Opcode::SCALAR_I8),
            Command::I16 => Some(Opcode::SCALAR_I16),
            Command::I32 => Some(Opcode::SCALAR_I32),
            Command::I64 => Some(Opcode::SCALAR_I64),
            Command::I128 => Some(Opcode::SCALAR_I128),
            Command::Isize => Some(Opcode::SCALAR_ISIZE),
            Command::F32 => Some(Opcode::SCALAR_F32),
            Command::F64 => Some(Opcode::SCALAR_F64),
            Command::String => Some(Opcode::SCALAR_STRING),
            Command::BoxStr => Some(Opcode::SCALAR_BOX_STR),
            _ => None,
        }
    }
}

// Can't have `[const]` bounds on inherent impls yet, so they're on every method :/
impl<Commands> Program<Commands> {
    pub const fn len(&self) -> u32
    where
        Commands: [const] AsSlice<u32>,
    {
        self.commands.as_ref().len() as u32
    }

    pub const fn is_empty(&self) -> bool
    where
        Commands: [const] AsSlice<u32>,
    {
        self.len() == 0
    }

    const fn slice<T>(&self, start: u32, len: u32) -> &[T]
    where
        Commands: [const] AsSlice<u32>,
    {
        assert!(align_of::<T>() <= align_of::<u32>());
        let start = start as usize;
        let u32_len = (len as usize * size_of::<T>()).div_ceil(size_of::<u32>());
        let u32_slice = &self.commands.as_ref()[start..start + u32_len];
        unsafe { slice::from_raw_parts(u32_slice.as_ptr().cast(), len as usize) }
    }

    const fn array<T, const N: usize>(&self, start: u32) -> [T; N]
    where
        Commands: [const] AsSlice<u32>,
        T: Copy,
    {
        match self.slice::<T>(start, N as u32).try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!(),
        }
    }

    pub const fn get_command(&self, opcode: Opcode) -> Command<'_>
    where
        Commands: [const] AsSlice<u32>,
    {
        let argument = opcode.argument();
        match opcode.category() {
            Opcode::CAT_SCALAR => match opcode {
                Opcode::SCALAR_NEVER => Command::Never,
                Opcode::SCALAR_UNIT => Command::Unit,
                Opcode::SCALAR_BOOL => Command::Bool,
                Opcode::SCALAR_U8 => Command::U8,
                Opcode::SCALAR_U16 => Command::U16,
                Opcode::SCALAR_U32 => Command::U32,
                Opcode::SCALAR_U64 => Command::U64,
                Opcode::SCALAR_U128 => Command::U128,
                Opcode::SCALAR_USIZE => Command::Usize,
                Opcode::SCALAR_I8 => Command::I8,
                Opcode::SCALAR_I16 => Command::I16,
                Opcode::SCALAR_I32 => Command::I32,
                Opcode::SCALAR_I64 => Command::I64,
                Opcode::SCALAR_I128 => Command::I128,
                Opcode::SCALAR_ISIZE => Command::Isize,
                Opcode::SCALAR_F32 => Command::F32,
                Opcode::SCALAR_F64 => Command::F64,
                Opcode::SCALAR_STRING => Command::String,
                Opcode::SCALAR_BOX_STR => Command::BoxStr,
                _ => const_panic!("Invalid well-known command index: {}", argument),
            },
            Opcode::CAT_OBJECT => {
                let [field_count] = self.array(argument);
                let fields = self.slice::<Field>(argument + 1, field_count);
                Command::Object { fields }
            }
            Opcode::CAT_FIXED_ARRAY => {
                let [len, elem_size, elem_ty] = self.array(argument);
                let elem_ty = Opcode(elem_ty);
                Command::FixedArray {
                    len,
                    elem_size,
                    elem_ty,
                }
            }
            other => const_panic!("Invalid command category: {}", other),
        }
    }

    pub const fn get_string(&self, index: StringIndex) -> &[u8]
    where
        Commands: [const] AsSlice<u32>,
    {
        let [len] = self.array(index.0);
        let bytes = self.slice(index.0 + 1, len);
        match std::str::from_utf8(bytes) {
            Ok(s) => s.as_bytes(),
            Err(_) => panic!("Invalid UTF-8 in string"),
        }
    }
}

impl<Commands: AsSlice<u32>> Debug for Program<Commands> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn debug_opcode<Commands: AsSlice<u32>>(
            program: &Program<Commands>,
            opcode: Opcode,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let command = program.get_command(opcode);
            match command {
                Command::Never => write!(f, "!"),
                Command::Unit => write!(f, "()"),
                Command::Bool => write!(f, "bool"),
                Command::U8 => write!(f, "u8"),
                Command::U16 => write!(f, "u16"),
                Command::U32 => write!(f, "u32"),
                Command::U64 => write!(f, "u64"),
                Command::U128 => write!(f, "u128"),
                Command::Usize => write!(f, "usize"),
                Command::I8 => write!(f, "i8"),
                Command::I16 => write!(f, "i16"),
                Command::I32 => write!(f, "i32"),
                Command::I64 => write!(f, "i64"),
                Command::I128 => write!(f, "i128"),
                Command::Isize => write!(f, "isize"),
                Command::F32 => write!(f, "f32"),
                Command::F64 => write!(f, "f64"),
                Command::String => write!(f, "String"),
                Command::BoxStr => write!(f, "Box<str>"),
                Command::FixedArray {
                    len,
                    elem_size,
                    elem_ty,
                } => {
                    write!(f, "[")?;
                    debug_opcode(program, elem_ty, f)?;
                    write!(f, "; {}] /* element size: {} */", len, elem_size)
                }
                Command::Object { fields } => {
                    write!(f, "{{")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "/* offset: {} */", field.offset)?;
                        let name = program.get_string(field.name);
                        let name_str = std::str::from_utf8(name).unwrap_or("<invalid utf-8>");
                        write!(f, " {:?}: ", name_str)?;
                        debug_opcode(program, field.ty, f)?;
                    }
                    write!(f, "}}")
                }
            }
        }

        write!(
            f,
            "Program {{ ({:x}){:x?} = ",
            self.root.0,
            self.commands.as_ref()
        )?;
        debug_opcode(self, self.root, f)?;
        write!(f, " }}")?;

        Ok(())
    }
}

const impl Default for Program<Vec<u32>> {
    fn default() -> Self {
        Program {
            root: Opcode::INVALID,
            commands: Vec::new(),
        }
    }
}

impl Program<Vec<u32>> {
    pub const fn new() -> Self {
        Self::default()
    }

    pub const fn const_make_global(self) -> Program<&'static [u32]> {
        Program {
            root: self.root,
            commands: self.commands.const_make_global(),
        }
    }

    pub const fn push_scalar(&mut self, opcode: Opcode) {
        if !opcode.category() == Opcode::CAT_SCALAR {
            const_panic!("push_scalar called with non-scalar opcode");
        }
        self.commands.push(opcode.0);
    }

    const fn push_string(&mut self, s: &str) -> StringIndex {
        let start = self.len();
        self.commands.push(s.len() as u32);
        // Would be nicer to write this with chunks, but that's not const yet :(
        for i in 0..(s.len() / 4) {
            let offset = i * 4;
            let chunk: [u8; 4] = match s.as_bytes()[offset..offset + 4].try_into() {
                Ok(chunk) => chunk,
                Err(_) => unreachable!(),
            };
            let chunk_u32 = u32::from_ne_bytes(chunk);
            self.commands.push(chunk_u32);
        }

        let rest_len = s.len() % 4;
        if rest_len != 0 {
            let offset = s.len() / 4 * 4;
            let mut chunk: [u8; 4] = [0; 4];
            chunk[0..rest_len].copy_from_slice(&s.as_bytes()[offset..]);
            let chunk_u32 = u32::from_ne_bytes(chunk);
            self.commands.push(chunk_u32);
        }
        StringIndex(start)
    }
}

pub struct PartialObject {
    start_offset: u32,
    current_offset: u32,
    remaining: u32,
}

impl PartialObject {
    pub const fn opcode(&self) -> Opcode {
        Opcode::new(Opcode::CAT_OBJECT, self.start_offset)
    }
}

const impl Drop for PartialObject {
    fn drop(&mut self) {
        if self.remaining != 0 {
            panic!("PartialObject dropped with remaining unfilled fields",);
        }
    }
}

impl Program<Vec<u32>> {
    pub const fn push_object(&mut self, field_count: u32) -> PartialObject {
        let start = self.len();
        self.commands.push(field_count);
        for _ in 0..field_count * 3 {
            self.commands.push(0);
        }
        PartialObject {
            start_offset: start,
            current_offset: start + 1,
            remaining: field_count,
        }
    }

    pub const fn fill_field(
        &mut self,
        partial: &mut PartialObject,
        name: &str,
        offset: u32,
        ty: Opcode,
    ) {
        let field_index = partial.current_offset;
        let name_index = self.push_string(name);
        self.commands[field_index as usize] = name_index.0;
        self.commands[field_index as usize + 1] = offset;
        self.commands[field_index as usize + 2] = ty.0;
        partial.current_offset += 3;
        partial.remaining -= 1;
    }
}

pub struct PartialArray {
    start_offset: u32,
    completed: bool,
}

impl PartialArray {
    pub const fn opcode(&self) -> Opcode {
        Opcode::new(Opcode::CAT_FIXED_ARRAY, self.start_offset)
    }
}

const impl Drop for PartialArray {
    fn drop(&mut self) {
        if !self.completed {
            panic!("PartialArray dropped without being completed");
        }
    }
}

impl Program<Vec<u32>> {
    pub const fn push_array(&mut self, len: u32, elem_size: u32) -> PartialArray {
        let start = self.len();
        self.commands.push(len);
        self.commands.push(elem_size);
        self.commands.push(0);
        PartialArray {
            start_offset: start,
            completed: true,
        }
    }

    pub const fn fill_array_elem(&mut self, partial: &mut PartialArray, elem: Opcode) {
        let elem_index = partial.start_offset + 2;
        self.commands[elem_index as usize] = elem.0;
        partial.completed = true;
    }
}
