//! 缂栬瘧鏈熷父閲忚〃杈惧紡姹傚€?
//!
//! 浠呭鐞嗗彲鍦?AST 灞傞潰闈欐€佹眰鍊肩殑绠€鍗曞瓧闈㈤噺涓庣畻鏈〃杈惧紡锛?
//! 涓嶅紩鍏ュ壇浣滅敤鎴栧鏉傝涔夈€?

use lang_zone::ast::{BinOp, Expr};

/// 瀵瑰父閲忚〃杈惧紡鍋氱紪璇戞湡姹傚€硷紝杩斿洖鍏跺瓧闈㈤噺瀛楃涓茶〃绀恒€?
///
/// 褰撳墠鏀寔锛?
/// - 瀛楅潰閲忥紙int銆乫loat銆乻tr銆乥ool銆丯one锛?
/// - 浜屽厓杩愮畻 `+ - * /`锛屽 int/float 鍋氱畝鍗曠畻鏈?
///
/// 鏃犳硶姹傚€兼椂杩斿洖 `None`銆?
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

            // 灏濊瘯鎸夋暣鏁拌В鏋?
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
    use lang_zone::ast::{BinOp, Expr};

    #[test]
    fn eval_literals() {
        assert_eq!(eval_const_expr(&Expr::IntLit(42)), Some("42".to_string()));
        assert_eq!(
            eval_const_expr(&Expr::StrLit("hello".into())),
            Some("hello".to_string())
        );
        assert_eq!(
            eval_const_expr(&Expr::BoolLit(true)),
            Some("true".to_string())
        );
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
