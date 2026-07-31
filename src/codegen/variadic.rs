// Lang-Zong 编译器 — codegen/variadic.rs
// 上下文感知 variadic 后端选择：Any vs JSON

use crate::ast::Function;

/// variadic 后端类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariadicBackend {
    /// Box<dyn Any> 通用后端
    Any,
    /// serde_json::Value JSON 后端
    Json,
}

/// 检测函数是否应该使用 JSON 后端
///
/// 优先级：
///   P0: 函数有 @json 装饰器
///   P1: 模块导入 serde_json（TODO）
///   P2: 函数体包含 JSON 操作（TODO）
pub fn detect_variadic_backend(f: &Function) -> VariadicBackend {
    // P0: @json 装饰器
    for d in &f.decorators {
        if d.name == "json" {
            return VariadicBackend::Json;
        }
    }
    VariadicBackend::Any
}

/// 生成 variadic 参数值的包装表达式
///   Any 后端：Box::new(val)
///   Json 后端：serde_json::json!(val)
pub fn variadic_wrap(val_str: &str, backend: VariadicBackend) -> String {
    match backend {
        VariadicBackend::Any => format!("Box::new({})", val_str),
        VariadicBackend::Json => format!("serde_json::json!({})", val_str),
    }
}
