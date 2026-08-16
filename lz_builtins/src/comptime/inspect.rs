// lz_builtins::comptime::inspect — 编译期专用 API
//
// 这些函数 **仅在 comptime 上下文中可用**，运行时不可调用
// align with Python inspect + LZ 编译期内省

// ══════════════════════════════════════════════════════════════
// 编译期 inspect 数据类型
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParameterInfo>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub kind: ParameterKind,
    pub annotation: Option<String>,
    pub has_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub function: Option<String>,
    pub filename: String,
    pub lineno: i64,
}

// ══════════════════════════════════════════════════════════════
// typeof / type_name — 编译期类型查询
// ══════════════════════════════════════════════════════════════

pub fn type_name<T>() -> &'static str {
    std::any::type_name::<T>()
}

pub fn type_of<T>(_val: &T) -> &'static str {
    std::any::type_name::<T>()
}

pub fn size_of<T>() -> usize {
    std::mem::size_of::<T>()
}

pub fn align_of<T>() -> usize {
    std::mem::align_of::<T>()
}

// ══════════════════════════════════════════════════════════════
// compile_warn!
// ══════════════════════════════════════════════════════════════

pub fn compile_warn(_msg: &str) {}

// ══════════════════════════════════════════════════════════════
// inspect 命名空间 — 编译期由 ComptimeEvaluator 实现
// ══════════════════════════════════════════════════════════════

pub mod inspect {
    use super::*;

    pub fn getmembers() -> Vec<(&'static str, String)> {
        unimplemented!("comptime only")
    }
    pub fn getmodulename() -> String {
        unimplemented!("comptime only")
    }
    pub fn ismodule(_name: &str) -> bool {
        unimplemented!("comptime only")
    }
    pub fn isclass(_name: &str) -> bool {
        unimplemented!("comptime only")
    }
    pub fn isfunction(_name: &str) -> bool {
        unimplemented!("comptime only")
    }
    pub fn ismethod(_cls: &str, _method: &str) -> bool {
        unimplemented!("comptime only")
    }
    pub fn signature(_f: &str) -> super::FunctionSignature {
        unimplemented!("comptime only")
    }
    pub fn getsource(_o: &str) -> String {
        unimplemented!("comptime only")
    }
    pub fn getsourcefile(_o: &str) -> String {
        unimplemented!("comptime only")
    }
    pub fn getsourcelines(_o: &str) -> Vec<String> {
        unimplemented!("comptime only")
    }
    pub fn getdoc(_o: &str) -> Option<String> {
        unimplemented!("comptime only")
    }
    pub fn getcomments(_o: &str) -> Vec<String> {
        unimplemented!("comptime only")
    }
    pub fn currentframe() -> super::FrameInfo {
        unimplemented!("comptime only")
    }
    pub fn getargs(_f: &str) -> Vec<String> {
        unimplemented!("comptime only")
    }
    pub fn getreturntype(_f: &str) -> String {
        unimplemented!("comptime only")
    }
}
