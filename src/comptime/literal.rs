// Lang-Zone 编译器 — comptime/literal.rs
// （由 comptime/mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl ComptimeValue {
    /// 将编译期值内联为 Rust 字面量。Type / Inspect 等不可内联，返回 Err。
    pub fn to_rust_literal(&self) -> Result<String, String> {
        match self {
            ComptimeValue::Int(i) => Ok(i.to_string()),
            ComptimeValue::Float(f) => Ok(if f.is_nan() {
                "f64::NAN".into()
            } else if f.is_infinite() && *f > 0.0 {
                "f64::INFINITY".into()
            } else if f.is_infinite() {
                "f64::NEG_INFINITY".into()
            } else {
                format!("{f:?}")
            }),
            ComptimeValue::Bool(b) => Ok(b.to_string()),
            ComptimeValue::Str(s) => Ok(format!("{:?}", s)),
            ComptimeValue::None => Ok("()".into()),
            ComptimeValue::List(xs) => {
                let items: Result<Vec<_>, _> = xs.iter().map(|x| x.to_rust_literal()).collect();
                Ok(format!("vec![{}]", items?.join(", ")))
            }
            ComptimeValue::Tuple(xs) => {
                let items: Result<Vec<_>, _> = xs.iter().map(|x| x.to_rust_literal()).collect();
                let inner = items?.join(", ");
                Ok(if xs.len() == 1 {
                    format!("({inner},)")
                } else {
                    format!("({inner})")
                })
            }
            ComptimeValue::Map(_) => Err("Map 类型不能直接内联为字面量".into()),
            ComptimeValue::Type(_) => Err("Type 值不能内联为运行代码".into()),
            ComptimeValue::Inspect(_) => Err("Inspect 对象不能内联为运行代码".into()),
        }
    }

    /// 布尔判定（用于条件控制流）
    pub fn truthy(&self) -> bool {
        match self {
            ComptimeValue::Int(i) => *i != 0,
            ComptimeValue::Float(f) => *f != 0.0 && !f.is_nan(),
            ComptimeValue::Bool(b) => *b,
            ComptimeValue::Str(s) => !s.is_empty(),
            ComptimeValue::None => false,
            ComptimeValue::List(xs) => !xs.is_empty(),
            ComptimeValue::Tuple(xs) => !xs.is_empty(),
            ComptimeValue::Map(m) => !m.is_empty(),
            ComptimeValue::Type(_) => true,
            ComptimeValue::Inspect(_) => true,
        }
    }
}
