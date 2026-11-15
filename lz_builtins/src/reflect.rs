// lz_builtins::reflect — 运行时反射库
// 类型注册、字段内省、类型安全字段读写
// 零外部依赖，纯 Rust std

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ══════════════════════════════════════════════════════════════
// 类型元信息
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub enum TypeKind {
    Int,
    UInt,
    Float,
    Bool,
    Char,
    Str,
    Ptr,
    Struct(&'static TypeInfo),
    Unknown,
}

impl PartialEq for TypeKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TypeKind::Int, TypeKind::Int) => true,
            (TypeKind::UInt, TypeKind::UInt) => true,
            (TypeKind::Float, TypeKind::Float) => true,
            (TypeKind::Bool, TypeKind::Bool) => true,
            (TypeKind::Char, TypeKind::Char) => true,
            (TypeKind::Str, TypeKind::Str) => true,
            (TypeKind::Ptr, TypeKind::Ptr) => true,
            (TypeKind::Unknown, TypeKind::Unknown) => true,
            (TypeKind::Struct(a), TypeKind::Struct(b)) => std::ptr::eq(*a, *b),
            _ => false,
        }
    }
}
impl Eq for TypeKind {}

#[derive(Debug, Clone, Copy)]
pub struct FieldInfo {
    pub name: &'static str,
    pub offset: usize,
    pub kind: TypeKind,
    pub type_info: Option<&'static TypeInfo>,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    pub name: &'static str,
    pub size: usize,
    pub fields: &'static [FieldInfo],
}

// ══════════════════════════════════════════════════════════════
// 全局注册表
// ══════════════════════════════════════════════════════════════

static REGISTRY: OnceLock<Mutex<HashMap<&'static str, &'static TypeInfo>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<&'static str, &'static TypeInfo>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn init() {
    let _ = registry();
}

pub fn register(ti: &'static TypeInfo) -> Result<(), &'static str> {
    let mut map = registry().lock().unwrap();
    if map.contains_key(ti.name) {
        return Err(ti.name);
    }
    map.insert(ti.name, ti);
    Ok(())
}

pub fn find_type(name: &str) -> Option<&'static TypeInfo> {
    registry().lock().unwrap().get(name).copied()
}

pub fn type_count() -> usize {
    registry().lock().unwrap().len()
}
pub fn clear() {
    registry().lock().unwrap().clear();
}

pub fn field_count(ti: &TypeInfo) -> usize {
    ti.fields.len()
}
pub fn field(ti: &TypeInfo, idx: usize) -> Option<&FieldInfo> {
    ti.fields.get(idx)
}
pub fn find_field<'a>(ti: &'a TypeInfo, name: &str) -> Option<&'a FieldInfo> {
    ti.fields.iter().find(|f| f.name == name)
}

/// # Safety: data must point to initialized object matching type_info
pub unsafe fn get_raw(
    type_info: &TypeInfo,
    data: *const u8,
    field_name: &str,
    dst: &mut [u8],
) -> Result<(), &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    let src = data.add(fi.offset);
    let copy_size = dst.len().min(type_info.size.saturating_sub(fi.offset));
    std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), copy_size);
    Ok(())
}

/// # Safety: data must point to mutable initialized object matching type_info
pub unsafe fn set_raw(
    type_info: &TypeInfo,
    data: *mut u8,
    field_name: &str,
    src: &[u8],
) -> Result<(), &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    let dst = data.add(fi.offset);
    let copy_size = src.len().min(type_info.size.saturating_sub(fi.offset));
    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, copy_size);
    Ok(())
}

macro_rules! typed_getset {
    ($get:ident, $set:ident, $kind:path, $ty:ty) => {
        pub unsafe fn $get(
            type_info: &TypeInfo,
            data: *const u8,
            field_name: &str,
        ) -> Result<$ty, &'static str> {
            let fi = find_field(type_info, field_name).ok_or("field not found")?;
            if !matches!(fi.kind, $kind | TypeKind::Unknown) {
                return Err("type kind mismatch");
            }
            Ok(std::ptr::read_unaligned(data.add(fi.offset) as *const $ty))
        }
        pub unsafe fn $set(
            type_info: &TypeInfo,
            data: *mut u8,
            field_name: &str,
            val: $ty,
        ) -> Result<(), &'static str> {
            let fi = find_field(type_info, field_name).ok_or("field not found")?;
            if !matches!(fi.kind, $kind | TypeKind::Unknown) {
                return Err("type kind mismatch");
            }
            std::ptr::write_unaligned(data.add(fi.offset) as *mut $ty, val);
            Ok(())
        }
    };
}

typed_getset!(get_i64, set_i64, TypeKind::Int, i64);
typed_getset!(get_i32, set_i32, TypeKind::Int, i32);
typed_getset!(get_i16, set_i16, TypeKind::Int, i16);
typed_getset!(get_i8, set_i8, TypeKind::Int, i8);
typed_getset!(get_u64, set_u64, TypeKind::UInt, u64);
typed_getset!(get_u32, set_u32, TypeKind::UInt, u32);
typed_getset!(get_u16, set_u16, TypeKind::UInt, u16);
typed_getset!(get_u8, set_u8, TypeKind::UInt, u8);
typed_getset!(get_f64, set_f64, TypeKind::Float, f64);
typed_getset!(get_f32, set_f32, TypeKind::Float, f32);
typed_getset!(get_bool, set_bool, TypeKind::Bool, bool);
typed_getset!(get_char, set_char, TypeKind::Char, char);

/// 安全读取字符串字段
pub unsafe fn get_str<'a>(
    type_info: &TypeInfo,
    data: *const u8,
    field_name: &str,
) -> Result<&'a str, &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) {
        return Err("type kind mismatch");
    }
    Ok((&*(data.add(fi.offset) as *const String)).as_str())
}

/// 安全写入字符串字段
pub unsafe fn set_str(
    type_info: &TypeInfo,
    data: *mut u8,
    field_name: &str,
    val: String,
) -> Result<(), &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) {
        return Err("type kind mismatch");
    }
    std::ptr::write(data.add(fi.offset) as *mut String, val);
    Ok(())
}

pub fn status_str(err: &'static str) -> &'static str {
    err
}
