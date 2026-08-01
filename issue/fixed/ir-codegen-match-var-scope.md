# 🔴 P1: IR codegen match 臂变量绑定作用域错误

**Bug 标题**: IR 路线 match 臂绑定的变量在臂外不可访问，生成无效 Rust 代码

**严重等级**: 🔴 P1 — 使用 match 绑定的文件 rustc 编译失败
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// mutable_let.lz
def main() =
    let v = match 42:
        1: "a"
        _:
            let v = "matched"
            v
    print(v)  // v 应该在 match 表达式外可用
```

编译: `lang-zone mutable_let.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
// IR codegen 产出
pub fn main() {
    let v: String = {
        return "a".to_string();
    };
    return println!("{:?} {:?}", "matched:".to_string(), v);
    //                                                        ^ E0425: cannot find value `v`
}
```

- `let v = "matched"` 在内部臂块中绑定，但 IR codegen 没有将块最终值赋值给外层的 `v`
- 生成的代码结构是 `let v = match_arm_body;` 没有正确处理臂内 let 绑定与外层变量的关系

## 预期结果

- IR codegen 应该将 match 表达式的结果正确地绑定到外层变量
- 臂内最后执行的表达式值应该流向外层

## 根因

`src/ir/builder.rs` 中 `convert_stmt` 对 `AstStmt::LetMatch` 的处理没有将 match body 的最终表达式值正确地提升到外层。`match` 表达式（AstExpr::Match）目前仅取 `arms.first()` 的 body，完全丢失了多臂逻辑和变量绑定关系。

## 影响范围

涉及文件（共 ~20 个 DEMO 文件 rustc 编译失败）:
- `DEMO/03_variables/mutable_let.lz` — `v`, `second` 变量
- `DEMO/06_control_flow/match.lz` — `r` 变量
- `DEMO/06_control_flow/match_more.lz` — `r` 变量
- `DEMO/07_data_structures/enum.lz` — `r` 变量
- `DEMO/07_data_structures/enum_more.lz`
- `DEMO/03_variables/walrus.lz` — match 表达式变量
- `DEMO/03_variables/walrus_more.lz`
- 多个 99_spec 文件

## 关联 Issue

- `issue/ir-builder-match-arm-first-only.md` — 同一根因，match 仅取第一个臂
