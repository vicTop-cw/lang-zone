// ── lz 标准库桥接 shims ──
// #[inline(always)] 零成本强制助手
// 用途：Rust 自由函数常收 &Path/&str/AsRef<T>，
//       lz 侧用 str/String，shim 在逐参强制中包裹
//       这些函数由 codegen 经 include_str! 注入生成的 .rs 文件头部

#![allow(dead_code)]

/// `[checker]` 参数检查站使用的参数结构体
/// 用于可变参数函数接收 args 和 kwargs
pub struct __Params {
    pub args: Vec<Box<dyn std::any::Any>>,
    pub kwargs: std::collections::HashMap<String, Box<dyn std::any::Any>>,
}

/// 将 String 自动转为 &Path
/// lz: fs::read_to_string(path) 其中 path: str/String
/// Rust: std::fs::read_to_string<P: AsRef<Path>>(path: P)
#[inline(always)]
fn __lz_path(s: &str) -> &std::path::Path {
    std::path::Path::new(s)
}

/// 将 String 自动转为 &Path（owned 版本，接受 String）
/// 用于需要 PathBuf 参数但 lz 传 String 的场景
#[inline(always)]
fn __lz_pathbuf(s: String) -> std::path::PathBuf {
    std::path::PathBuf::from(s)
}

/// 将 String 自动转为 &str（解引用）
/// 用于 Rust 期望 &str 但 lz 传 String 的场景
#[inline(always)]
fn __lz_str_ref(s: &String) -> &str {
    s.as_str()
}

/// 将 &String 自动转为 &[u8]（字节切片）
/// 用于 Rust 期望 &[u8] 但 lz 传 str 的场景
#[inline(always)]
fn __lz_bytes(s: &str) -> &[u8] {
    s.as_bytes()
}

/// Duration 毫秒构造
/// lz: thread::sleep(ms:int) → Rust: std::thread::sleep(Duration::from_millis(ms))
#[inline(always)]
fn __lz_duration_ms(ms: i64) -> std::time::Duration {
    std::time::Duration::from_millis(ms as u64)
}

/// Duration 秒构造
#[inline(always)]
fn __lz_duration_secs(secs: f64) -> std::time::Duration {
    std::time::Duration::from_secs_f64(secs)
}

// ═══════════════════════════════════════════════════════════════════
// lz std::reflect — 运行时反射库（Rust 重写自 reflect/ C 反射库）
// 零外部依赖，仅依赖 Rust std。codegen 为每个 lz struct 生成
// __TYPEINFO_* 常量并在 main 前注册。
// ═══════════════════════════════════════════════════════════════════

/// 运行时反射命名空间
pub mod __lz_reflect {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    // ── 类型元信息 ──

    /// 类型分类（对标 C 版 reflect_kind_t）
    #[derive(Debug, Clone, Copy)]
    pub enum TypeKind {
        Int, UInt, Float, Bool, Char, Str, Ptr, Struct(&'static TypeInfo), Unknown,
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

    /// 字段描述符（对标 C 版 reflect_field_t）
    #[derive(Debug, Clone, Copy)]
    pub struct FieldInfo {
        pub name: &'static str,
        pub offset: usize,
        pub kind: TypeKind,
        pub type_info: Option<&'static TypeInfo>,
    }

    /// 类型描述符（对标 C 版 reflect_type_t）
    #[derive(Debug, Clone, Copy)]
    pub struct TypeInfo {
        pub name: &'static str,
        pub size: usize,
        pub fields: &'static [FieldInfo],
    }

    // ── 全局注册表 ──

    static REGISTRY: OnceLock<Mutex<HashMap<&'static str, &'static TypeInfo>>> = OnceLock::new();

    fn registry() -> &'static Mutex<HashMap<&'static str, &'static TypeInfo>> {
        REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// 初始化反射库（惰性）
    pub fn init() { let _ = registry(); }

    /// 注册一个类型到全局注册表。返回 Err(name) 表示重复。
    pub fn register(ti: &'static TypeInfo) -> Result<(), &'static str> {
        let mut map = registry().lock().unwrap();
        if map.contains_key(ti.name) { return Err(ti.name); }
        map.insert(ti.name, ti);
        Ok(())
    }

    /// 按名称查找已注册类型
    pub fn find_type(name: &str) -> Option<&'static TypeInfo> {
        registry().lock().unwrap().get(name).copied()
    }

    /// 已注册类型数量
    pub fn type_count() -> usize {
        registry().lock().unwrap().len()
    }

    /// 清空注册表
    pub fn clear() { registry().lock().unwrap().clear(); }

    // ── 内省 API ──

    pub fn field_count(ti: &TypeInfo) -> usize { ti.fields.len() }
    pub fn field(ti: &TypeInfo, idx: usize) -> Option<&FieldInfo> { ti.fields.get(idx) }
    pub fn find_field(ti: &TypeInfo, name: &str) -> Option<&FieldInfo> {
        ti.fields.iter().find(|f| f.name == name)
    }

    // ── 字段原始字节读写 ──

    /// # Safety
    /// data 必须指向与 type_info 匹配的已初始化对象
    pub unsafe fn get_raw(
        type_info: &TypeInfo, data: *const u8,
        field_name: &str, dst: &mut [u8],
    ) -> Result<(), &'static str> {
        let fi = find_field(type_info, field_name).ok_or("field not found")?;
        let src = data.add(fi.offset);
        let copy_size = dst.len().min(type_info.size.saturating_sub(fi.offset));
        std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), copy_size);
        Ok(())
    }

    /// # Safety
    /// data 必须指向与 type_info 匹配的可变已初始化对象
    pub unsafe fn set_raw(
        type_info: &TypeInfo, data: *mut u8,
        field_name: &str, src: &[u8],
    ) -> Result<(), &'static str> {
        let fi = find_field(type_info, field_name).ok_or("field not found")?;
        let dst = data.add(fi.offset);
        let copy_size = src.len().min(type_info.size.saturating_sub(fi.offset));
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, copy_size);
        Ok(())
    }

    // ── 类型安全字段读写（宏生成）──

    macro_rules! typed_getset {
        ($get:ident, $set:ident, $kind:path, $ty:ty) => {
            pub unsafe fn $get(
                type_info: &TypeInfo, data: *const u8, field_name: &str,
            ) -> Result<$ty, &'static str> {
                let fi = find_field(type_info, field_name).ok_or("field not found")?;
                if !matches!(fi.kind, $kind | TypeKind::Unknown) { return Err("type kind mismatch"); }
                Ok(std::ptr::read_unaligned(data.add(fi.offset) as *const $ty))
            }
            pub unsafe fn $set(
                type_info: &TypeInfo, data: *mut u8, field_name: &str, val: $ty,
            ) -> Result<(), &'static str> {
                let fi = find_field(type_info, field_name).ok_or("field not found")?;
                if !matches!(fi.kind, $kind | TypeKind::Unknown) { return Err("type kind mismatch"); }
                std::ptr::write_unaligned(data.add(fi.offset) as *mut $ty, val);
                Ok(())
            }
        };
    }
    typed_getset!(get_i64, set_i64, TypeKind::Int, i64);
    typed_getset!(get_i32, set_i32, TypeKind::Int, i32);
    typed_getset!(get_i16, set_i16, TypeKind::Int, i16);
    typed_getset!(get_i8,  set_i8,  TypeKind::Int, i8);
    typed_getset!(get_u64, set_u64, TypeKind::UInt, u64);
    typed_getset!(get_u32, set_u32, TypeKind::UInt, u32);
    typed_getset!(get_u16, set_u16, TypeKind::UInt, u16);
    typed_getset!(get_u8,  set_u8,  TypeKind::UInt, u8);
    typed_getset!(get_f64, set_f64, TypeKind::Float, f64);
    typed_getset!(get_f32, set_f32, TypeKind::Float, f32);
    typed_getset!(get_bool, set_bool, TypeKind::Bool, bool);
    typed_getset!(get_char, set_char, TypeKind::Char, char);

    /// 安全读取字符串字段
    pub unsafe fn get_str<'a>(
        type_info: &TypeInfo, data: *const u8, field_name: &str,
    ) -> Result<&'a str, &'static str> {
        let fi = find_field(type_info, field_name).ok_or("field not found")?;
        if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) { return Err("type kind mismatch"); }
        Ok((&*(data.add(fi.offset) as *const String)).as_str())
    }

    /// 安全写入字符串字段
    pub unsafe fn set_str(
        type_info: &TypeInfo, data: *mut u8, field_name: &str, val: String,
    ) -> Result<(), &'static str> {
        let fi = find_field(type_info, field_name).ok_or("field not found")?;
        if !matches!(fi.kind, TypeKind::Str | TypeKind::Unknown) { return Err("type kind mismatch"); }
        std::ptr::write(data.add(fi.offset) as *mut String, val);
        Ok(())
    }

    /// 错误码文本描述
    pub fn status_str(err: &'static str) -> &'static str {
        match err {
            "field not found" => "field not found",
            "type kind mismatch" => "type kind mismatch",
            "type not found" => "type not found",
            _ => err,
        }
    }
}

// 将 __lz_reflect 命名空间的内容提升到当前作用域
pub use __lz_reflect::*;
