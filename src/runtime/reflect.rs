// ── lz std::reflect — Rust 运行时反射库 ──
// 重写自 reflect/ C 反射库。零外部依赖，仅依赖 Rust std。
//
// 功能：
//   1. 类型注册（TypeInfo / FieldInfo）— 在 init 期注册结构体元信息
//   2. 运行时查询（find_type）— 按名称查找已注册类型
//   3. 字段内省（field_count / field / find_field）— 枚举类型字段
//   4. 低级字段读写（get_field_raw / set_field_raw）— 通过偏移量访问
//   5. 类型安全字段读写（get_field_i64 / set_field_i64 等）— 带 kind 校验
//
// 使用方式：
//   lz 编译时，codegen 为每个 struct 生成 __TYPE_INFO_MyStruct 常量，
//   并在程序入口前调用 __lz_reflect_register() 注册。
//   lz 代码通过 `import std::reflect` 使用反射 API。

#![allow(dead_code, unused_variables)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ═══════════════════════════════════════════════════════════════════
// 类型元信息
// ═══════════════════════════════════════════════════════════════════

/// 类型分类（对标 C 版 reflect_kind_t）
#[derive(Debug, Clone, Copy)]
pub enum TypeKind {
    /// 有符号整数（i8/i16/i32/i64）
    Int,
    /// 无符号整数（u8/u16/u32/u64）
    UInt,
    /// 浮点数（f32/f64）
    Float,
    /// 布尔（bool）
    Bool,
    /// 字符（char）
    Char,
    /// 字符串（String / &str）
    Str,
    /// 指针 / 引用
    Ptr,
    /// 已注册结构体（指向 TypeInfo）
    Struct(&'static TypeInfo),
    /// 未知 / 显式标记
    Unknown,
}

// 手动实现 — Struct 变体用指针身份比较（TypeInfo 为 Copy + 'static，指针地址稳定）
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

/// 字段描述符（对标 C 版 reflect_field_t）
#[derive(Debug, Clone, Copy)]
pub struct FieldInfo {
    /// 字段名
    pub name: &'static str,
    /// 字段在结构体中的字节偏移量
    pub offset: usize,
    /// 字段类型
    pub kind: TypeKind,
    /// 对于 Struct 字段，指向嵌套类型的 TypeInfo（可选）
    pub type_info: Option<&'static TypeInfo>,
}

/// 类型描述符（对标 C 版 reflect_type_t）
#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    /// 类型名称
    pub name: &'static str,
    /// sizeof(type)
    pub size: usize,
    /// 字段列表
    pub fields: &'static [FieldInfo],
}

// ═══════════════════════════════════════════════════════════════════
// 全局类型注册表
// ═══════════════════════════════════════════════════════════════════

static REGISTRY: OnceLock<Mutex<HashMap<&'static str, &'static TypeInfo>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<&'static str, &'static TypeInfo>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 初始化反射库（惰性，可多次调用）
pub fn init() {
    let _ = registry();
}

/// 注册一个类型到全局注册表
///
/// 返回 `Ok(())` 成功；返回 `Err(name)` 表示类型名已存在（拒绝重复注册）
pub fn register(ti: &'static TypeInfo) -> Result<(), &'static str> {
    let mut map = registry().lock().unwrap();
    if map.contains_key(ti.name) {
        return Err(ti.name);
    }
    map.insert(ti.name, ti);
    Ok(())
}

/// 按名称查找已注册的类型信息
pub fn find_type(name: &str) -> Option<&'static TypeInfo> {
    let map = registry().lock().unwrap();
    map.get(name).copied()
}

/// 返回当前已注册类型数量
pub fn type_count() -> usize {
    let map = registry().lock().unwrap();
    map.len()
}

/// 清空注册表（用于测试或热重载）
pub fn clear() {
    let mut map = registry().lock().unwrap();
    map.clear();
}

// ═══════════════════════════════════════════════════════════════════
// 内省 API
// ═══════════════════════════════════════════════════════════════════

/// 返回类型的字段数
pub fn field_count(ti: &TypeInfo) -> usize {
    ti.fields.len()
}

/// 按索引取字段描述符（索引越界返回 None）
pub fn field(ti: &TypeInfo, idx: usize) -> Option<&FieldInfo> {
    ti.fields.get(idx)
}

/// 按名称查找字段描述符
pub fn find_field<'a>(ti: &'a TypeInfo, name: &str) -> Option<&'a FieldInfo> {
    ti.fields.iter().find(|f| f.name == name)
}

// ═══════════════════════════════════════════════════════════════════
// 字段读写 — 原始字节（对标 C 版 reflect_get_raw / reflect_set_raw）
// ═══════════════════════════════════════════════════════════════════

/// 从对象读取一个字段的原始字节
///
/// # Safety
/// `data` 必须指向一个与 `type_info` 匹配的已初始化对象，且字段真实大小 ≤ dst.len()
pub unsafe fn get_field_raw(
    type_info: &TypeInfo,
    data: *const u8,
    field_name: &str,
    dst: &mut [u8],
) -> Result<(), &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    let src = data.add(fi.offset);
    // 以 TypeInfo 中声明的 size 为准（可能小于 dst）
    let copy_size = dst.len().min(type_info.size.saturating_sub(fi.offset));
    std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), copy_size);
    Ok(())
}

/// 向对象的字段写入原始字节
///
/// # Safety
/// `data` 必须指向一个与 `type_info` 匹配的可变已初始化对象
pub unsafe fn set_field_raw(
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

// ═══════════════════════════════════════════════════════════════════
// 类型安全字段读写（对标 C 版 typed getter/setter）
// ═══════════════════════════════════════════════════════════════════

macro_rules! typed_field_getset {
    ($get_name:ident, $set_name:ident, $kind:path, $ty:ty) => {
        /// 安全读取整数字段
        ///
        /// # Safety
        /// `data` 必须指向一个与 `type_info` 匹配的已初始化对象
        pub unsafe fn $get_name(
            type_info: &TypeInfo,
            data: *const u8,
            field_name: &str,
        ) -> Result<$ty, &'static str> {
            let fi = find_field(type_info, field_name).ok_or("field not found")?;
            // kind 为 Unknown 时不校验（兼容未分类类型）
            if !matches!(fi.kind, $kind | TypeKind::Unknown) {
                return Err("type kind mismatch");
            }
            let ptr = data.add(fi.offset) as *const $ty;
            Ok(std::ptr::read_unaligned(ptr))
        }

        /// 安全写入整数字段
        ///
        /// # Safety
        /// `data` 必须指向一个与 `type_info` 匹配的可变已初始化对象
        pub unsafe fn $set_name(
            type_info: &TypeInfo,
            data: *mut u8,
            field_name: &str,
            val: $ty,
        ) -> Result<(), &'static str> {
            let fi = find_field(type_info, field_name).ok_or("field not found")?;
            if !matches!(fi.kind, $kind | TypeKind::Unknown) {
                return Err("type kind mismatch");
            }
            let ptr = data.add(fi.offset) as *mut $ty;
            std::ptr::write_unaligned(ptr, val);
            Ok(())
        }
    };
}

typed_field_getset!(get_field_i64, set_field_i64, TypeKind::Int, i64);
typed_field_getset!(get_field_i32, set_field_i32, TypeKind::Int, i32);
typed_field_getset!(get_field_i16, set_field_i16, TypeKind::Int, i16);
typed_field_getset!(get_field_i8,  set_field_i8,  TypeKind::Int, i8);
typed_field_getset!(get_field_u64, set_field_u64, TypeKind::UInt, u64);
typed_field_getset!(get_field_u32, set_field_u32, TypeKind::UInt, u32);
typed_field_getset!(get_field_u16, set_field_u16, TypeKind::UInt, u16);
typed_field_getset!(get_field_u8,  set_field_u8,  TypeKind::UInt, u8);
typed_field_getset!(get_field_f64, set_field_f64, TypeKind::Float, f64);
typed_field_getset!(get_field_f32, set_field_f32, TypeKind::Float, f32);
typed_field_getset!(get_field_bool, set_field_bool, TypeKind::Bool, bool);
typed_field_getset!(get_field_char, set_field_char, TypeKind::Char, char);

/// 获取字符串字段（&str/String 均存为 String，读取为 &str）
///
/// # Safety
/// `data` 必须指向一个与 `type_info` 匹配的已初始化对象
pub unsafe fn get_field_str<'a>(
    type_info: &TypeInfo,
    data: *const u8,
    field_name: &str,
) -> Result<&'a str, &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) {
        return Err("type kind mismatch");
    }
    let ptr = data.add(fi.offset) as *const String;
    Ok((&*ptr).as_str())
}

/// 设置字符串字段
///
/// # Safety
/// `data` 必须指向一个与 `type_info` 匹配的可变已初始化对象
pub unsafe fn set_field_str(
    type_info: &TypeInfo,
    data: *mut u8,
    field_name: &str,
    val: String,
) -> Result<(), &'static str> {
    let fi = find_field(type_info, field_name).ok_or("field not found")?;
    if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) {
        return Err("type kind mismatch");
    }
    let ptr = data.add(fi.offset) as *mut String;
    std::ptr::write(ptr, val);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 错误码文本描述（对标 C 版 reflect_status_str）
// ═══════════════════════════════════════════════════════════════════

pub fn status_str(err: &'static str) -> &'static str {
    match err {
        "field not found" => "字段未找到",
        "type kind mismatch" => "类型不匹配",
        "type not found" => "类型未找到",
        "duplicate registration" => "重复注册",
        _ => err,
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Point {
        x: i64,
        y: i64,
        label: String,
    }

    const POINT_FIELDS: &[FieldInfo] = &[
        FieldInfo { name: "x", offset: 0, kind: TypeKind::Int, type_info: None },
        FieldInfo { name: "y", offset: 8, kind: TypeKind::Int, type_info: None },
        FieldInfo { name: "label", offset: 16, kind: TypeKind::Str, type_info: None },
    ];

    const POINT_TYPE: &TypeInfo = &TypeInfo {
        name: "Point",
        size: std::mem::size_of::<Point>(),
        fields: POINT_FIELDS,
    };

    #[test]
    fn test_register_and_find() {
        clear();
        assert!(register(POINT_TYPE).is_ok());
        let found = find_type("Point");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Point");
        assert_eq!(found.unwrap().size, std::mem::size_of::<Point>());
    }

    #[test]
    fn test_duplicate_registration_rejected() {
        clear();
        assert!(register(POINT_TYPE).is_ok());
        assert!(register(POINT_TYPE).is_err());
    }

    #[test]
    fn test_introspection() {
        assert_eq!(field_count(POINT_TYPE), 3);
        let f = field(POINT_TYPE, 1).unwrap();
        assert_eq!(f.name, "y");
        assert_eq!(f.offset, 8);

        let f = find_field(POINT_TYPE, "label").unwrap();
        assert_eq!(f.kind, TypeKind::Str);
    }

    #[test]
    fn test_field_not_found() {
        let f = find_field(POINT_TYPE, "nonexistent");
        assert!(f.is_none());
    }

    #[test]
    fn test_type_safe_field_access() {
        let mut p = Point { x: 42, y: 100, label: "hello".into() };
        let data = &mut p as *mut Point as *mut u8;

        unsafe {
            // read i64 fields
            let x = get_field_i64(POINT_TYPE, data as *const u8, "x").unwrap();
            assert_eq!(x, 42);

            // write i64 field
            set_field_i64(POINT_TYPE, data, "x", 99).unwrap();
            assert_eq!(p.x, 99);

            // read string field
            let s = get_field_str(POINT_TYPE, data as *const u8, "label").unwrap();
            assert_eq!(s, "hello");

            // write string field
            set_field_str(POINT_TYPE, data, "label", "world".into()).unwrap();
            assert_eq!(p.label, "world");
        }
    }

    #[test]
    fn test_kind_mismatch_rejected() {
        let mut p = Point { x: 0, y: 0, label: "".into() };
        let data = &mut p as *mut Point as *mut u8;

        unsafe {
            // trying to get a String field as i64 should fail
            let r = get_field_i64(POINT_TYPE, data as *const u8, "label");
            assert!(r.is_err());

            // trying to set a String field as bool should fail
            let r = set_field_bool(POINT_TYPE, data, "label", true);
            assert!(r.is_err());
        }
    }

    #[test]
    fn test_raw_field_access() {
        let mut p = Point { x: 42, y: 100, label: "test".into() };
        let data = &mut p as *mut Point as *mut u8;

        unsafe {
            // read raw bytes for x
            let mut buf = [0u8; 8];
            get_field_raw(POINT_TYPE, data as *const u8, "x", &mut buf).unwrap();
            let val = i64::from_ne_bytes(buf);
            assert_eq!(val, 42);

            // write raw bytes for y
            let new_y = 200i64.to_ne_bytes();
            set_field_raw(POINT_TYPE, data, "y", &new_y).unwrap();
            assert_eq!(p.y, 200);
        }
    }

    #[test]
    fn test_init_find_and_clear() {
        clear();
        assert!(!find_type("Point").is_some() || type_count() == 0);
    }
}
