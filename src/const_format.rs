use std::{any::TypeId, mem::type_info::TypeKind};

pub macro id($ty:ty) {{ ::core::any::TypeId::of::<$ty>() }}

// FIXME
pub const trait TypeIdNamePolyfill {
    fn name(&self) -> &'static str;
}

const impl TypeIdNamePolyfill for TypeId {
    fn name(&self) -> &'static str {
        "<TODO type name>"
    }
}

/// Like a Vec<u8>, but with all methods const
pub struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub const fn len(&self) -> usize {
        self.data.len()
    }

    pub const fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub const fn push(&mut self, b: u8) {
        self.data.push(b);
    }

    pub const fn truncate(&mut self, n: usize) {
        let len = self.data.len();
        if n > len {
            panic!("Cannot drop more bytes than the buffer contains");
        }
        unsafe { self.data.set_len(len - n) };
    }

    pub const fn extend_from_slice(&mut self, slice: &[u8]) {
        for i in 0..slice.len() {
            self.data.push(slice[i]);
        }
    }

    pub const fn extend_from_str(&mut self, s: &str) {
        for i in 0..s.len() {
            self.data.push(s.as_bytes()[i]);
        }
    }

    pub const fn const_make_global(self) -> &'static [u8] {
        self.data.const_make_global()
    }

    pub const fn const_make_global_str(self) -> &'static str {
        match std::str::from_utf8(self.const_make_global()) {
            Ok(s) => s,
            Err(_) => panic!("Invalid UTF-8 in const_make_global_str"),
        }
    }
}

pub macro const_format($fmt:literal $(, $arg:expr)* $(,)?) {{
    let fmt = str::as_bytes($fmt);
    let mut buf = Buffer::new();
    let args = nested_tuples!($($arg),*);
    WriteConstFormatArgs::const_write_format_args(&args, &mut buf, fmt);
    buf.const_make_global_str()
}}

pub macro const_panic($fmt:literal $(, $arg:expr)* $(,)?) {{
    panic!("{}", const_format!($fmt $(, $arg)*));
}}

macro nested_tuples {
    () => { () },
    ($a:expr $(, $($rest:tt)*)?) => { ($a, nested_tuples!($($($rest)*)?)) },
}

pub const trait WriteConstFormatArgs {
    fn const_write_format_args(&self, buf: &mut Buffer, fmt: &[u8]);
}

const impl<T, U> WriteConstFormatArgs for (T, U)
where
    T: const ConstFormat,
    U: const WriteConstFormatArgs,
{
    fn const_write_format_args(&self, buf: &mut Buffer, fmt: &[u8]) {
        let (head, tail) = self;
        let mut i = 0;
        while i < fmt.len() {
            let c = fmt[i];
            if c == b'{' {
                if i + 1 >= fmt.len() || fmt[i + 1] != b'}' {
                    panic!("Unmatched '{{' in format string");
                }
                head.const_fmt(buf);
                i += 2;
                break;
            } else {
                buf.push(c);
                i += 1;
            }
        }
        tail.const_write_format_args(buf, &fmt[i..]);
    }
}

const impl WriteConstFormatArgs for () {
    fn const_write_format_args(&self, _buf: &mut Buffer, fmt: &[u8]) {
        let mut i = 0;
        while i < fmt.len() {
            let c = fmt[i];
            if c == b'{' {
                if i + 1 >= fmt.len() || fmt[i + 1] != b'}' {
                    panic!("Unmatched '{{' in format string");
                }
                panic!("Not enough arguments for format string");
            } else {
                _buf.push(c);
                i += 1;
            }
        }
    }
}

pub const trait ConstFormat {
    fn const_fmt(&self, buf: &mut Buffer);
}

const impl ConstFormat for &'static str {
    fn const_fmt(&self, buf: &mut Buffer) {
        buf.extend_from_str(self);
    }
}

const impl ConstFormat for Buffer {
    fn const_fmt(&self, buf: &mut Buffer) {
        buf.extend_from_slice(self.as_slice());
    }
}

const impl ConstFormat for u128 {
    fn const_fmt(&self, buf: &mut Buffer) {
        let mut n = *self;
        if n == 0 {
            buf.push(b'0');
            return;
        }
        while n > 0 {
            let digit = (n % 10) as u8;
            buf.push(b'0' + digit);
            n /= 10;
        }
    }
}

const impl ConstFormat for i128 {
    fn const_fmt(&self, buf: &mut Buffer) {
        let n = *self;
        if n < 0 {
            buf.push(b'-');
            ((-n) as u128).const_fmt(buf);
        } else {
            (n as u128).const_fmt(buf);
        }
    }
}

macro impl_const_fmt_int($(
    $as_ty:ty: ($($ty:ty),* $(,)?), $(,)?
)*) {
    $($(
        const impl ConstFormat for $ty {
            fn const_fmt(&self, buf: &mut Buffer) {
                (*self as $as_ty).const_fmt(buf);
            }
        }
    )*)*
}

impl_const_fmt_int!(
    u128: (u8, u16, u32, u64, usize),
    i128: (i8, i16, i32, i64, isize),
);

const impl<T> ConstFormat for &T
where
    T: const ConstFormat,
{
    fn const_fmt(&self, buf: &mut Buffer) {
        (**self).const_fmt(buf);
    }
}

const impl<T> ConstFormat for &mut T
where
    T: const ConstFormat,
{
    fn const_fmt(&self, buf: &mut Buffer) {
        (**self).const_fmt(buf);
    }
}

#[rustc_comptime]
pub fn full_type_name(ty: TypeId) -> &'static str {
    match ty.info().kind {
        TypeKind::Other if ty == id![!] => "!",
        TypeKind::Bool => "bool",
        TypeKind::Int if ty == id![u8] => "u8",
        TypeKind::Int if ty == id![u16] => "u16",
        TypeKind::Int if ty == id![u32] => "u32",
        TypeKind::Int if ty == id![u64] => "u64",
        TypeKind::Int if ty == id![u128] => "u128",
        TypeKind::Int if ty == id![usize] => "usize",
        TypeKind::Int if ty == id![i8] => "i8",
        TypeKind::Int if ty == id![i16] => "i16",
        TypeKind::Int if ty == id![i32] => "i32",
        TypeKind::Int if ty == id![i64] => "i64",
        TypeKind::Int if ty == id![i128] => "i128",
        TypeKind::Int if ty == id![isize] => "isize",
        TypeKind::Int => "{unknown integer type}",
        TypeKind::Float if ty == id![f32] => "f32",
        TypeKind::Float if ty == id![f64] => "f64",
        TypeKind::Float => "{unknown float type}",
        TypeKind::Char => "char",
        TypeKind::Str(_) => "str",
        TypeKind::Tuple if ty.fields(0) == 0 => "()",
        TypeKind::Tuple if ty.fields(0) == 1 => {
            const_format!("({},)", full_type_name(ty.field(0, 0).type_id()))
        }
        TypeKind::Tuple => {
            let mut buf = Buffer::new();
            buf.push(b'(');
            for i in 0..ty.fields(0) {
                if i > 0 {
                    buf.push(b',');
                }
                buf.extend_from_str(full_type_name(ty.field(0, i).type_id()));
            }
            buf.push(b')');
            buf.const_make_global_str()
        }
        TypeKind::Array => {
            let elem_ty = ty.element_ty().unwrap();
            let len = ty.array_len();
            const_format!("[{}; {}]", full_type_name(elem_ty), len)
        }
        TypeKind::Slice => {
            let elem_ty = ty.element_ty().unwrap();
            const_format!("[{}]", full_type_name(elem_ty))
        }
        TypeKind::Struct | TypeKind::Enum | TypeKind::Union => {
            let name = ty.name();
            // TODO generic arguments
            name
        }
        TypeKind::Reference => {
            let elem_ty = ty.points_to().unwrap();
            let is_mut = ty.points_mutably();
            const_format!(
                "&{}{}",
                if is_mut { "mut " } else { "" },
                full_type_name(elem_ty)
            )
        }
        TypeKind::Pointer => {
            let elem_ty = ty.points_to().unwrap();
            let is_mut = ty.points_mutably();
            const_format!(
                "*{}{}",
                if is_mut { "mut " } else { "const " },
                full_type_name(elem_ty)
            )
        }
        TypeKind::FnPtr => {
            let _sig = ty.function_ptr().unwrap();
            // TODO
            "{function pointer}"
        }
        TypeKind::DynTrait(dyn_trait) => {
            let mut buf = Buffer::new();
            buf.extend_from_str("dyn ");
            for i in 0..dyn_trait.predicates.len() {
                if i > 0 {
                    buf.push(b'+');
                }
                let pred = &dyn_trait.predicates[i];
                let trait_id = pred.trait_ty.ty;
                buf.extend_from_str(trait_id.name());
            }
            buf.const_make_global_str()
        }
        TypeKind::Other | _ => "{unknown type}",
    }
}
