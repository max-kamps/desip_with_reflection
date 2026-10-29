use crate::const_format::{Buffer, TypeIdNamePolyfill as _, const_format, full_type_name, id};
use crate::parser::bytecode::{Command, Opcode, Program};
use std::any::TypeId;
use std::mem::type_info::TypeKind;
use std::num::{Saturating, Wrapping};

#[rustc_comptime]
pub fn program_for_type_id(ty: TypeId) -> Result<Program, &'static str> {
    let mut program = Program::new();

    // For better error reporting, tracks the path from the root to the current type
    let mut path = Buffer::new();
    path.extend_from_str(ty.name());

    let mut added: Vec<(TypeId, Opcode)> = Vec::new();
    program.root = program_add_type(&mut program, ty, &mut path, &mut added)?;

    Ok(program.const_make_global())
}

const fn get_type_opcode(added: &[(TypeId, Opcode)], ty: TypeId) -> Option<Opcode> {
    // added
    //     .iter()
    //     .find(|(added_ty, _)| *added_ty == ty)
    //     .map(const |(_, opcode)| *opcode)
    for i in 0..added.len() {
        let (added_ty, opcode) = added[i];
        if added_ty == ty {
            return Some(opcode);
        }
    }
    None
}

trait Transparent {}
impl<N> Transparent for Saturating<N> {}
impl<N> Transparent for Wrapping<N> {}

#[rustc_comptime]
fn program_add_type(
    program: &mut Program<Vec<u32>>,
    ty: TypeId,
    path: &mut Buffer,
    added: &mut Vec<(TypeId, Opcode)>,
) -> Result<Opcode, &'static str> {
    Ok(match ty.info().kind {
        TypeKind::Struct if ty.trait_info_of::<dyn Transparent>().is_some() => {
            let inner_ty = ty.field(0, 0).type_id();
            program_add_type(program, inner_ty, path, added)?
        }

        // Scalars, primitive
        TypeKind::Other if ty == id![!] => Command::Never.as_scalar_opcode().unwrap(),
        TypeKind::Tuple if ty == id![()] => Command::Unit.as_scalar_opcode().unwrap(),
        TypeKind::Bool if ty == id![bool] => Command::Bool.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![u8] => Command::U8.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![u16] => Command::U16.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![u32] => Command::U32.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![u64] => Command::U64.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![u128] => Command::U128.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![usize] => Command::Usize.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![i8] => Command::I8.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![i16] => Command::I16.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![i32] => Command::I32.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![i64] => Command::I64.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![i128] => Command::I128.as_scalar_opcode().unwrap(),
        TypeKind::Int if ty == id![isize] => Command::Isize.as_scalar_opcode().unwrap(),
        // TypeKind::Char if id == id![char] => Command::Char.as_scalar_opcode().unwrap(),
        TypeKind::Float if ty == id![f32] => Command::F32.as_scalar_opcode().unwrap(),
        TypeKind::Float if ty == id![f64] => Command::F64.as_scalar_opcode().unwrap(),

        // Scalars, non-primitive
        TypeKind::Struct if ty == id![String] => Command::String.as_scalar_opcode().unwrap(),
        TypeKind::Struct if ty == id![Box<str>] => Command::BoxStr.as_scalar_opcode().unwrap(),

        // Compound types
        TypeKind::Array => 'block: {
            if let Some(existing) = get_type_opcode(added, ty) {
                break 'block existing;
            }

            let elem_ty = ty.element_ty().unwrap();
            let len = ty.array_len() as u32;
            let elem_size = elem_ty.size().unwrap() as u32;

            let mut partial = program.push_array(len, elem_size);
            added.push((ty, partial.opcode()));

            let saved_path_len = path.len();
            path.extend_from_str("[0]");
            let elem_opcode = program_add_type(program, elem_ty, path, added)?;
            path.truncate(saved_path_len);

            program.fill_array_elem(&mut partial, elem_opcode);

            partial.opcode()
        }

        TypeKind::Struct => 'block: {
            if let Some(existing) = get_type_opcode(added, ty) {
                break 'block existing;
            }

            let field_count = ty.fields(0) as u32;

            let mut partial = program.push_object(field_count);
            added.push((ty, partial.opcode()));
            for field_idx in 0..field_count {
                let field_id = ty.field(0, field_idx as usize);
                let name = field_id.name();
                let field_ty = field_id.type_id();
                let offset = field_id.offset() as u32;

                let saved_path_len = path.len();
                path.push(b'.');
                path.extend_from_str(name);
                let field_opcode = program_add_type(program, field_ty, path, added)?;
                path.truncate(saved_path_len);

                program.fill_field(&mut partial, name, offset, field_opcode);
            }

            partial.opcode()
        }
        _other => {
            return Err(const_format!(
                "`{}` has unsupported type `{}`",
                path,
                full_type_name(ty),
            ));
        }
    })
}
