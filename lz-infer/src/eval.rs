//! 编译期常量表达式求值
//!
//! 仅处理可在 AST 层面静态求值的简单字面量与算术表达式，
//! 不引入副作用或复杂语义。

use lang_zone::ast::expr::{BinOp, Expr};

/// 对常量表达式做编译期求值，返回其字面量字符串表示。
///
/// 当前支持：
/// - 字面量（int、float、str、bool、None）
/// - 二元运算 `+ - * /`，对 int/float 做简单算术
///
/// 无法求值时返回 `None`。
pub fn eval_const_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::IntLit(n) => Some(n.to_string()),
        Expr::FloatLit(f) => Some(f.to_string()),
        Expr::StrLit(s) => Some(s.clone()),
        Expr::BoolLit(b) => Some(b.to_string()),
        Expr::NoneLit => Some("None".to_string()),

        Expr::Binary { left, op, right } => {
            let lv = eval_const_expr(left)?;
            let rv = eval_const_expr(right)?;

            // 尝试按整数解析
            let li = lv.parse::<i64>();
            let ri = rv.parse::<i64>();

            match op {
                BinOp::Add => match (li, ri) {
                    (Ok(l), Ok(r)) => Some((l + r).to_string()),
                    _ => Some((lv.parse::<f64>().ok()? + rv.parse::<f64>().ok()?).to_string()),
                },
                BinOp::Sub => match (li, ri) {
                    (Ok(l), Ok(r)) => Some((l - r).to_string()),
                    _ => Some((lv.parse::<f64>().ok()? - rv.parse::<f64>().ok()?).to_string()),
                },
                BinOp::Mul => match (li, ri) {
                    (Ok(l), Ok(r)) => Some((l * r).to_string()),
                    _ => Some((lv.parse::<f64>().ok()? * rv.parse::<f64>().ok()?).to_string()),
                },
                BinOp::Div => {
                    let l = lv.parse::<f64>().ok()?;
                    let r = rv.parse::<f64>().ok()?;
                    if r == 0.0 {
                        return None;
                    }
                    Some((l / r).to_string())
                }
                _ => None,
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lang_zone::ast::expr::{BinOp, Expr};

    #[test]
    fn eval_literals() {
        assert_eq!(eval_const_expr(&Expr::IntLit(42)), Some("42".to_string()));
        assert_eq!(eval_const_expr(&Expr::StrLit("hello".into())), Some("hello".to_string()));
        assert_eq!(eval_const_expr(&Expr::BoolLit(true)), Some("true".to_string()));
        assert_eq!(eval_const_expr(&Expr::NoneLit), Some("None".to_string()));
    }

    #[test]
    fn eval_binary_int() {
        let expr = Expr::Binary {
            left: Box::new(Expr::IntLit(1)),
            op: BinOp::Add,
            right: Box::new(Expr::IntLit(2)),
        };
        assert_eq!(eval_const_expr(&expr), Some("3".to_string()));
    }
}
