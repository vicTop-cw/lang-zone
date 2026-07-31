#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
LZ 编译器全面集成测试驱动
===========================
覆盖 ~210 个最小单元测试用例，按 11 个语法大类分层组织。

用法:
  python run_tests.py                          # 运行全部
  python run_tests.py --priority P0            # 只运行 P0
  python run_tests.py --category types         # 只运行类型测试
  python run_tests.py --bug-type 2             # 只运行 Bug-2 相关
  python run_tests.py --id TYP-PRIM-001        # 指定 ID
  python run_tests.py --only-failed            # 只运行上次失败的
  python run_tests.py --gen-sources            # 生成所有 .lz 源码文件
  python run_tests.py --clean                  # 运行后清理 _work
  python run_tests.py --skip-build             # 跳过 cargo build
"""

import os
import sys
import subprocess
import json
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
CASES_DIR = os.path.join(HERE, "cases")
ROOT = os.path.dirname(os.path.dirname(HERE))  # lang-zone/

# 确保模块可导入
sys.path.insert(0, HERE)
from config import Config
from runner import TestRunner, TestCase, TestResult
from reporter import TestReporter


# =====================================================================
# CATALOG: 所有测试用例定义
# =====================================================================

CATALOG = []

def _t(id, title, category, priority, mode, source_file, **kw):
    """辅助函数：定义测试用例"""
    CATALOG.append(TestCase(
        id=id, title=title, category=category, priority=priority,
        mode=mode, source_file=source_file,
        present=kw.get("present", []),
        absent=kw.get("absent", []),
        error_contains=kw.get("error_contains", None),
        bug_types=kw.get("bug_types", []),
        bug_reason=kw.get("bug_reason", ""),
        needs_std=kw.get("needs_std", True),
        rustc_args=kw.get("rustc_args", []),
        skip_reason=kw.get("skip_reason", None),
    ))


# ---- 词法层 LEX (P0-P1) ----

# 关键字识别 (P0, Bug-1)
_t("LEX-KW-001", "声明关键字: def/let/mut/const/fn/struct/enum/trait/impl",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-001.lz",
   present=["Def", "Let", "Mut", "Const", "Struct", "Enum", "Trait", "Impl"], bug_types=[1])
_t("LEX-KW-002", "控制流关键字: if/elif/else/match/case/for/in/while/loop",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-002.lz",
   present=["If", "Elif", "Else", "Match", "Case", "For", "In", "While", "Loop", "Break", "Continue", "Return"], bug_types=[1])
_t("LEX-KW-003", "异常关键字: try/catch/finally/raise/raises/panic",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-003.lz",
   present=["Try", "Catch", "Finally", "Raise", "Raises", "Panic"], bug_types=[1])
_t("LEX-KW-004", "测试关键字: test/assert/check/suite",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-004.lz",
   present=["Test", "Assert", "Check", "Suite"], bug_types=[1])
_t("LEX-KW-005", "并发异步: async/await/spawn/yield",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-005.lz",
   present=["Async", "Await", "Spawn", "Yield"], bug_types=[1])
_t("LEX-KW-006", "导入关键字: import/from/as",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-006.lz",
   present=["Import", "From", "As"], bug_types=[1])
_t("LEX-KW-007", "元编程: macro/template/comptime/where",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-007.lz",
   present=["Macro", "Template", "Comptime", "Where"], bug_types=[1])
_t("LEX-KW-008", "逻辑关键字: and/or/not/in/is",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-008.lz",
   present=["And", "Or", "Not", "In", "Is"], bug_types=[1])
_t("LEX-KW-009", "字面量关键字: True/False/None/Some/Ok/Err",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-009.lz",
   present=["True", "False", "None_", "Some_", "Ok_", "Err_"], bug_types=[1])
_t("LEX-KW-010", "修饰: owned/ref/move/abstract/private/public",
   "lexer/keywords", "P0", "tokens", "lexer/keywords/LEX-KW-010.lz",
   present=["Owned", "Ref"], bug_types=[1])

# 注释 (P1, Bug-1)
_t("LEX-CMT-001", "// 行注释词法", "lexer/comments", "P1", "tokens",
   "lexer/comments/LEX-CMT-001.lz", present=["Def", "Ident(\"f\")"], bug_types=[1])
_t("LEX-CMT-002", "/* */ 块注释词法", "lexer/comments", "P1", "tokens",
   "lexer/comments/LEX-CMT-002.lz", present=["Def", "Ident(\"f\")"], bug_types=[1])
_t("LEX-CMT-003", "嵌套块注释词法", "lexer/comments", "P1", "tokens",
   "lexer/comments/LEX-CMT-003.lz", absent=["nested", "inner"], bug_types=[1])
_t("LEX-CMT-004", "# 是属性标记非注释", "lexer/comments", "P1", "tokens",
   "lexer/comments/LEX-CMT-004.lz", present=["Pound", "Def"], absent=["Comment"], bug_types=[1])

# 字面量 (P1, Bug-1)
_t("LEX-LIT-001", "多进制整数词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-001.lz", present=["42"], bug_types=[1])
_t("LEX-LIT-002", "下划线分隔整数", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-002.lz", present=["1000000"], bug_types=[1])
_t("LEX-LIT-003", "浮点数字面量词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-003.lz", present=["3.14"], bug_types=[1])
_t("LEX-LIT-004", "双引号字符串词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-004.lz", present=["hello"], bug_types=[1])
_t("LEX-LIT-005", "f-string 插值词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-005.lz", present=["x ="], bug_types=[1])
_t("LEX-LIT-006", "原始字符串词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-006.lz", present=["path"], bug_types=[1])
_t("LEX-LIT-007", "三引号多行字符串词法", "lexer/literals", "P1", "tokens",
   "lexer/literals/LEX-LIT-007.lz", present=["multi"], bug_types=[1])

# 操作符词法 (P1, Bug-1/2/4)
_t("LEX-OP-001", ":: 废弃报 LexError", "lexer/operators", "P0", "error",
   "lexer/operators/LEX-OP-001.lz", error_contains=":", bug_types=[2,4])
_t("LEX-OP-002", "构建块 =: ~: *: 词法识别", "lexer/operators", "P1", "tokens",
   "lexer/operators/LEX-OP-002.lz", present=["BuildAssign", "BuildCall", "BuildGen"], bug_types=[1])
_t("LEX-OP-003", "?. ?? |> ^ := 特殊操作符词法", "lexer/operators", "P1", "tokens",
   "lexer/operators/LEX-OP-003.lz", present=["y"], bug_types=[1])


# ---- 类型系统 TYP (P0-P1) ----

_t("TYP-PRIM-001", "int 类型基本运算", "types/primitives", "P0", "run",
   "types/primitives/TYP-PRIM-001.lz", present=["42"], bug_types=[3], bug_reason="验证 int 运行时输出正确")
_t("TYP-PRIM-002", "float 类型", "types/primitives", "P0", "run",
   "types/primitives/TYP-PRIM-002.lz", present=["7.5"], bug_types=[3])
_t("TYP-PRIM-003", "str 类型拼接", "types/primitives", "P0", "run",
   "types/primitives/TYP-PRIM-003.lz", present=["Hello, World"], bug_types=[3])
_t("TYP-PRIM-004", "bool 类型", "types/primitives", "P0", "run",
   "types/primitives/TYP-PRIM-004.lz", present=["True"], bug_types=[3])
_t("TYP-PRIM-005", "i32/u32/u64/f32 类型标注", "types/primitives", "P0", "compile",
   "types/primitives/TYP-PRIM-005.lz", present=["i32", "u32", "u64", "f32"], bug_types=[1,2])
_t("TYP-PRIM-006", "char 类型", "types/primitives", "P0", "compile",
   "types/primitives/TYP-PRIM-006.lz", present=[], bug_types=[1,2])

_t("TYP-CON-001", "List<int> 操作", "types/containers", "P0", "run",
   "types/containers/TYP-CON-001.lz", present=["1", "2", "3"], bug_types=[3])
_t("TYP-CON-002", "Dict<str,int> 操作", "types/containers", "P0", "run",
   "types/containers/TYP-CON-002.lz", present=["1"], bug_types=[3])
_t("TYP-CON-003", "Set<str>", "types/containers", "P0", "compile",
   "types/containers/TYP-CON-003.lz", bug_types=[1,3])
_t("TYP-CON-004", "Array<T,N>", "types/containers", "P0", "compile",
   "types/containers/TYP-CON-004.lz", bug_types=[1,3])
_t("TYP-CON-005", "元组 (int,str)", "types/containers", "P0", "run",
   "types/containers/TYP-CON-005.lz", present=["42", "hello"], bug_types=[3])
_t("TYP-CON-006", "Range 1..10", "types/containers", "P0", "compile",
   "types/containers/TYP-CON-006.lz", bug_types=[1,3])

_t("TYP-OPT-001", "T? 糖语法 -> Option<T>", "types/option", "P0", "rust",
   "types/option/TYP-OPT-001.lz", present=["Option<"], bug_types=[1])
_t("TYP-OPT-002", "Some 构造", "types/option", "P0", "run",
   "types/option/TYP-OPT-002.lz", present=["42"], bug_types=[3])
_t("TYP-OPT-003", "None + ?? 默认值", "types/option", "P0", "run",
   "types/option/TYP-OPT-003.lz", present=["42", "0"], bug_types=[3])

_t("TYP-GEN-001", "函数泛型 id<T>", "types/generics", "P0", "run",
   "types/generics/TYP-GEN-001.lz", present=["42", "hello"], bug_types=[3])
_t("TYP-GEN-002", "where 约束", "types/generics", "P0", "compile",
   "types/generics/TYP-GEN-002.lz", bug_types=[1,3])
_t("TYP-GEN-003", "多约束 + 连接", "types/generics", "P0", "compile",
   "types/generics/TYP-GEN-003.lz", present=["Clone", "Ord"], bug_types=[1,3])
_t("TYP-GEN-004", "struct 泛型", "types/generics", "P0", "compile",
   "types/generics/TYP-GEN-004.lz", bug_types=[1])

_t("TYP-ALIAS-001", "type 基本别名", "types/alias", "P1", "compile",
   "types/alias/TYP-ALIAS-001.lz", bug_types=[1,3])
_t("TYP-ALIAS-002", "type 泛型别名", "types/alias", "P1", "compile",
   "types/alias/TYP-ALIAS-002.lz", bug_types=[1])


# ---- 表达式 EXP (P0-P1) ----

_t("EXP-LIT-001", "整数运算", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-001.lz", present=["50"], bug_types=[3])
_t("EXP-LIT-002", "多进制混合运算", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-002.lz", present=["384"], bug_types=[3])
_t("EXP-LIT-003", "浮点运算", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-003.lz", present=["4.14"], bug_types=[3])
_t("EXP-LIT-004", "Bool 字面量 return", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-004.lz", present=["True"], bug_types=[3])
_t("EXP-LIT-005", "None 字面量生成 Option", "expr/literals", "P0", "rust",
   "expr/literals/EXP-LIT-005.lz", present=["Option"], absent=["()"], bug_types=[1,3], bug_reason="Bug-5: None 不应推断为 ()")
_t("EXP-LIT-006", "f-string 插值", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-006.lz", present=["Hello, LZ"], bug_types=[3])
_t("EXP-LIT-007", "原始字符串", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-007.lz", present=["C:\\path"], bug_types=[3])
_t("EXP-LIT-008", "三引号多行字符串", "expr/literals", "P0", "run",
   "expr/literals/EXP-LIT-008.lz", present=["line1", "line2"], bug_types=[3])

# 运算符 (P0)
_t("EXP-OP-001", "算术 + - * / %", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-001.lz", present=["15", "5", "50", "2", "0"], bug_types=[3])
_t("EXP-OP-002", "** 幂运算", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-002.lz", present=["8"], bug_types=[1,3])
_t("EXP-OP-003", "== != 比较", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-003.lz", present=["True", "False", "True"], bug_types=[3])
_t("EXP-OP-004", "< > <= >= 比较", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-004.lz", present=["True", "True", "False", "True", "True"], bug_types=[3])
_t("EXP-OP-005", "and / or 逻辑", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-005.lz", present=["True", "False", "True"], bug_types=[3])
_t("EXP-OP-006", "not 逻辑非", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-006.lz", present=["False", "True"], bug_types=[3])
_t("EXP-OP-007", "& 位与", "expr/operators", "P1", "run",
   "expr/operators/EXP-OP-007.lz", present=["1"], bug_types=[1,3])
_t("EXP-OP-008", "| 位或", "expr/operators", "P1", "run",
   "expr/operators/EXP-OP-008.lz", present=["3"], bug_types=[1,3])
_t("EXP-OP-009", "^ 位异或", "expr/operators", "P1", "run",
   "expr/operators/EXP-OP-009.lz", present=["6"], bug_types=[1,3])
_t("EXP-OP-010", "<< >> 移位", "expr/operators", "P1", "run",
   "expr/operators/EXP-OP-010.lz", present=["8", "1"], bug_types=[1,3])
_t("EXP-OP-011", "in 运算符", "expr/operators", "P1", "compile",
   "expr/operators/EXP-OP-011.lz", bug_types=[1,3])
_t("EXP-OP-012", "is 运算符", "expr/operators", "P1", "compile",
   "expr/operators/EXP-OP-012.lz", bug_types=[1,3])
_t("EXP-OP-013", "复合赋值 += -= *= /=", "expr/operators", "P0", "run",
   "expr/operators/EXP-OP-013.lz", present=["11", "9", "20", "5", "1"], bug_types=[3])
_t("EXP-OP-014", "复合赋值 &= |= ^= <<= >>=", "expr/operators", "P1", "compile",
   "expr/operators/EXP-OP-014.lz", bug_types=[1,3])
_t("EXP-OP-015", "**= 幂赋值", "expr/operators", "P1", "compile",
   "expr/operators/EXP-OP-015.lz", bug_types=[1,3])

# 特殊运算符 (P1)
_t("EXP-SPC-001", "|> 管道", "expr/special", "P1", "run",
   "expr/special/EXP-SPC-001.lz", present=["11"], bug_types=[3])
_t("EXP-SPC-002", "?. 安全导航", "expr/special", "P1", "run",
   "expr/special/EXP-SPC-002.lz", present=["hello", "None"], bug_types=[1,3], bug_reason="Bug-6: ?. 应生成 Option::map")
_t("EXP-SPC-003", "?? 空值合并", "expr/special", "P1", "run",
   "expr/special/EXP-SPC-003.lz", present=["42", "0"], bug_types=[3])
_t("EXP-SPC-004", ":= 海象运算符", "expr/special", "P1", "run",
   "expr/special/EXP-SPC-004.lz", present=["1", "15", "6", "3", "1"], bug_types=[1,3])
_t("EXP-SPC-005", ".. 半开范围", "expr/special", "P1", "run",
   "expr/special/EXP-SPC-005.lz", present=["1", "2", "3", "4"], bug_types=[1,3])
_t("EXP-SPC-006", "..= 闭区间范围", "expr/special", "P1", "compile",
   "expr/special/EXP-SPC-006.lz", bug_types=[1,3])
_t("EXP-SPC-007", "^ move 后缀", "expr/special", "P1", "compile",
   "expr/special/EXP-SPC-007.lz", bug_types=[1])

# 列表推导式 (P1, Bug-3)
_t("EXP-CMP-001", "基本列表推导", "expr/comprehension", "P1", "run",
   "expr/comprehension/EXP-CMP-001.lz", present=["2", "4", "6", "8"], bug_types=[3])
_t("EXP-CMP-002", "带条件的推导", "expr/comprehension", "P1", "run",
   "expr/comprehension/EXP-CMP-002.lz", present=["2", "4", "6", "8"], bug_types=[3])
_t("EXP-CMP-003", "多变量推导", "expr/comprehension", "P1", "run",
   "expr/comprehension/EXP-CMP-003.lz", present=["2", "3", "4"], bug_types=[1,3])
_t("EXP-CMP-004", "推导含方法调用", "expr/comprehension", "P1", "compile",
   "expr/comprehension/EXP-CMP-004.lz", bug_types=[1,3])

# 闭包 (P1)
_t("EXP-CLS-001", "单参数闭包", "expr/closure", "P1", "run",
   "expr/closure/EXP-CLS-001.lz", present=["6"], bug_types=[3])
_t("EXP-CLS-002", "多参数闭包", "expr/closure", "P1", "run",
   "expr/closure/EXP-CLS-002.lz", present=["5"], bug_types=[3])
_t("EXP-CLS-003", "闭包作参数", "expr/closure", "P1", "compile",
   "expr/closure/EXP-CLS-003.lz", bug_types=[1,3])
_t("EXP-CLS-004", "闭包捕获外部变量", "expr/closure", "P1", "run",
   "expr/closure/EXP-CLS-004.lz", present=["15"], bug_types=[1,3])


# ---- 语句与控制流 STM (P0-P1) ----

_t("STM-BND-001", "隐式 mutable 绑定", "stmt/bindings", "P0", "run",
   "stmt/bindings/STM-BND-001.lz", present=["2"], bug_types=[3])
_t("STM-BND-002", "let 不可变绑定", "stmt/bindings", "P0", "run",
   "stmt/bindings/STM-BND-002.lz", present=["42"], bug_types=[3])
_t("STM-BND-003", "mut 可变绑定", "stmt/bindings", "P0", "run",
   "stmt/bindings/STM-BND-003.lz", present=["1"], bug_types=[3])
_t("STM-BND-004", "const 常量", "stmt/bindings", "P0", "run",
   "stmt/bindings/STM-BND-004.lz", present=["3.14"], bug_types=[3])
_t("STM-BND-005", "ref 引用绑定", "stmt/bindings", "P1", "compile",
   "stmt/bindings/STM-BND-005.lz", bug_types=[1,3])
_t("STM-BND-006", "owned 所有权绑定", "stmt/bindings", "P1", "compile",
   "stmt/bindings/STM-BND-006.lz", bug_types=[1,3])
_t("STM-BND-007", "类型注解绑定", "stmt/bindings", "P0", "run",
   "stmt/bindings/STM-BND-007.lz", present=["42"], bug_types=[3])

# if/match
_t("STM-IF-001", "if/else 语句", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-001.lz", present=["positive"], bug_types=[3])
_t("STM-IF-002", "if/elif/else 多分支", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-002.lz", present=["positive", "negative"], bug_types=[3])
_t("STM-IF-003", "if 表达式赋值", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-003.lz", present=["pos", "neg"], bug_types=[3])
_t("STM-IF-004", "match 箭头风格", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-004.lz", present=["zero", "one", "other"], bug_types=[3])
_t("STM-IF-005", "match 冒号风格", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-005.lz", present=["zero", "other"], bug_types=[3])
_t("STM-IF-006", "match 变量绑定", "stmt/if_match", "P0", "run",
   "stmt/if_match/STM-IF-006.lz", present=["43"], bug_types=[3])
_t("STM-IF-007", "match 或模式", "stmt/if_match", "P1", "compile",
   "stmt/if_match/STM-IF-007.lz", bug_types=[1,3])
_t("STM-IF-008", "match 守卫", "stmt/if_match", "P1", "run",
   "stmt/if_match/STM-IF-008.lz", present=["positive", "zero"], bug_types=[1,3])
_t("STM-IF-009", "match 范围模式", "stmt/if_match", "P1", "compile",
   "stmt/if_match/STM-IF-009.lz", bug_types=[1,3])
_t("STM-IF-010", "match 元组解构", "stmt/if_match", "P1", "run",
   "stmt/if_match/STM-IF-010.lz", present=["3"], bug_types=[1,3])
_t("STM-IF-011", "match Some(x) 解构", "stmt/if_match", "P1", "run",
   "stmt/if_match/STM-IF-011.lz", present=["got", "42", "nothing"], bug_types=[1,3])

# 循环
_t("STM-LP-001", "for 遍历列表", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-001.lz", present=["1", "2", "3"], bug_types=[3])
_t("STM-LP-002", "for 遍历 range", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-002.lz", present=["1", "2", "3", "4"], bug_types=[3])
_t("STM-LP-003", "while 循环", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-003.lz", present=["0", "1", "2"], bug_types=[3])
_t("STM-LP-004", "loop 无限循环", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-004.lz", present=["0", "1", "2"], bug_types=[1,3])
_t("STM-LP-005", "break", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-005.lz", present=["1", "2"], bug_types=[3])
_t("STM-LP-006", "break 带返回值", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-006.lz", present=["42"], bug_types=[1,3])
_t("STM-LP-007", "continue", "stmt/loops", "P0", "run",
   "stmt/loops/STM-LP-007.lz", present=["1", "2", "4", "5"], bug_types=[3])
_t("STM-LP-008", "sum 推导", "stmt/loops", "P1", "run",
   "stmt/loops/STM-LP-008.lz", present=["10"], bug_types=[1,3])
_t("STM-LP-009", "prod 推导", "stmt/loops", "P1", "run",
   "stmt/loops/STM-LP-009.lz", present=["24"], bug_types=[1,3])

# guard/defer/with
_t("STM-GRD-001", "guard 条件守卫", "stmt/guard_defer", "P1", "run",
   "stmt/guard_defer/STM-GRD-001.lz", present=["2", "0"], bug_types=[3])
_t("STM-GRD-002", "guard let 模式守卫", "stmt/guard_defer", "P1", "run",
   "stmt/guard_defer/STM-GRD-002.lz", present=["42", "0"], bug_types=[1,3])
_t("STM-GRD-003", "defer 单行", "stmt/guard_defer", "P1", "run",
   "stmt/guard_defer/STM-GRD-003.lz", present=["body", "cleanup"], bug_types=[3])
_t("STM-GRD-004", "defer 多行块", "stmt/guard_defer", "P1", "run",
   "stmt/guard_defer/STM-GRD-004.lz", present=["body", "c1", "c2"], bug_types=[3])
_t("STM-GRD-005", "defer LIFO 逆序", "stmt/guard_defer", "P1", "run",
   "stmt/guard_defer/STM-GRD-005.lz", present=["body", "second", "first"], bug_types=[3])
_t("STM-GRD-006", "with 上下文", "stmt/guard_defer", "P1", "compile",
   "stmt/guard_defer/STM-GRD-006.lz", bug_types=[1,3])

# 异常处理
_t("STM-TRY-001", "raise 抛出异常", "stmt/try_catch", "P1", "compile",
   "stmt/try_catch/STM-TRY-001.lz", bug_types=[1,3])
_t("STM-TRY-002", "try/catch 基本", "stmt/try_catch", "P1", "run",
   "stmt/try_catch/STM-TRY-002.lz", present=["oops"], bug_types=[3])
_t("STM-TRY-003", "try/catch/finally", "stmt/try_catch", "P1", "run",
   "stmt/try_catch/STM-TRY-003.lz", present=["try", "catch", "finally"], bug_types=[3])
_t("STM-TRY-004", "raises 标注", "stmt/try_catch", "P1", "compile",
   "stmt/try_catch/STM-TRY-004.lz", bug_types=[1])
_t("STM-TRY-005", "panic 中止", "stmt/try_catch", "P1", "compile",
   "stmt/try_catch/STM-TRY-005.lz", bug_types=[1])


# ---- 声明与定义 DECL (P0-P1) ----

_t("DCL-FN-001", "def 等式风格", "decl/func", "P0", "run",
   "decl/func/DCL-FN-001.lz", present=["5"], bug_types=[3])
_t("DCL-FN-002", "def 块式风格", "decl/func", "P0", "run",
   "decl/func/DCL-FN-002.lz", present=["5"], bug_types=[3])
_t("DCL-FN-003", "无返回标注", "decl/func", "P0", "run",
   "decl/func/DCL-FN-003.lz", present=["Alice"], bug_types=[3])
_t("DCL-FN-004", "参数默认值", "decl/func", "P0", "run",
   "decl/func/DCL-FN-004.lz", present=["20", "42"], bug_types=[3])
_t("DCL-FN-005", "mut 参数修饰", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-005.lz", bug_types=[1])
_t("DCL-FN-006", "ref 参数修饰", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-006.lz", bug_types=[1])
_t("DCL-FN-007", "owned 参数修饰", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-007.lz", bug_types=[1])
_t("DCL-FN-008", "变长参数 ..", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-008.lz", bug_types=[1])
_t("DCL-FN-009", "变长参数混合", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-009.lz", bug_types=[1])
_t("DCL-FN-010", "raises 标注", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-010.lz", bug_types=[1])
_t("DCL-FN-011", "嵌套函数", "decl/func", "P0", "run",
   "decl/func/DCL-FN-011.lz", present=["42"], bug_types=[3])
_t("DCL-FN-012", "async 函数", "decl/func", "P2", "compile",
   "decl/func/DCL-FN-012.lz", bug_types=[1])
_t("DCL-FN-013", "隐式返回", "decl/func", "P0", "run",
   "decl/func/DCL-FN-013.lz", present=["42"], bug_types=[3])
_t("DCL-FN-014", "return 无值", "decl/func", "P1", "compile",
   "decl/func/DCL-FN-014.lz", bug_types=[1,3])

# struct
_t("DCL-ST-001", "struct 基本定义", "decl/struct", "P0", "run",
   "decl/struct/DCL-ST-001.lz", present=["3", "4"], bug_types=[3])
_t("DCL-ST-002", "struct 字段访问", "decl/struct", "P0", "run",
   "decl/struct/DCL-ST-002.lz", present=["10", "20"], bug_types=[3])
_t("DCL-ST-003", "struct 泛型", "decl/struct", "P0", "compile",
   "decl/struct/DCL-ST-003.lz", bug_types=[1])
_t("DCL-ST-004", "元组结构体", "decl/struct", "P0", "compile",
   "decl/struct/DCL-ST-004.lz", bug_types=[1])
_t("DCL-ST-005", "单元结构体", "decl/struct", "P0", "compile",
   "decl/struct/DCL-ST-005.lz", bug_types=[1])
_t("DCL-ST-006", "@derive 装饰", "decl/struct", "P1", "compile",
   "decl/struct/DCL-ST-006.lz", bug_types=[1])

# enum
_t("DCL-EN-001", "enum 基本定义", "decl/enum", "P0", "run",
   "decl/enum/DCL-EN-001.lz", present=["red"], bug_types=[3])
_t("DCL-EN-002", "enum 带数据变体", "decl/enum", "P0", "run",
   "decl/enum/DCL-EN-002.lz", present=["5"], bug_types=[3])
_t("DCL-EN-003", "enum 泛型", "decl/enum", "P0", "compile",
   "decl/enum/DCL-EN-003.lz", bug_types=[1])
_t("DCL-EN-004", "enum 命名字段变体", "decl/enum", "P1", "compile",
   "decl/enum/DCL-EN-004.lz", bug_types=[1])

# trait/impl
_t("DCL-TR-001", "trait 定义", "decl/trait_impl", "P0", "compile",
   "decl/trait_impl/DCL-TR-001.lz", bug_types=[1])
_t("DCL-TR-002", "impl Trait for Type", "decl/trait_impl", "P0", "run",
   "decl/trait_impl/DCL-TR-002.lz", present=["Hi, Tom"], bug_types=[3])
_t("DCL-TR-003", "trait 继承 (+)", "decl/trait_impl", "P0", "compile",
   "decl/trait_impl/DCL-TR-003.lz", bug_types=[1,2])
_t("DCL-TR-004", "关联类型", "decl/trait_impl", "P1", "compile",
   "decl/trait_impl/DCL-TR-004.lz", bug_types=[1])
_t("DCL-TR-005", "trait 默认方���", "decl/trait_impl", "P1", "compile",
   "decl/trait_impl/DCL-TR-005.lz", bug_types=[1])
_t("DCL-TR-006", "impl where 约束", "decl/trait_impl", "P1", "compile",
   "decl/trait_impl/DCL-TR-006.lz", bug_types=[1])
_t("DCL-TR-007", "负向: impl 方法签名不匹配", "decl/trait_impl", "P0", "error",
   "decl/trait_impl/DCL-TR-007.lz", error_contains="mismatch", bug_types=[2], bug_reason="Bug-9: trait/impl 签名不匹配应拦截")
_t("DCL-TR-008", "负向: impl 缺少方法", "decl/trait_impl", "P0", "error",
   "decl/trait_impl/DCL-TR-008.lz", error_contains="missing", bug_types=[2], bug_reason="Bug-11: impl 缺方法应拦截")
_t("DCL-TR-009", "负向: impl 返回类型不一致", "decl/trait_impl", "P0", "error",
   "decl/trait_impl/DCL-TR-009.lz", error_contains="return", bug_types=[2], bug_reason="Bug-12: impl 返回类型不一致应拦截")

# import
_t("DCL-IM-001", "import 基本", "decl/import", "P1", "compile",
   "decl/import/DCL-IM-001.lz", bug_types=[1])
_t("DCL-IM-002", "from X import a, b", "decl/import", "P1", "compile",
   "decl/import/DCL-IM-002.lz", bug_types=[1])
_t("DCL-IM-003", "import as 别名", "decl/import", "P1", "compile",
   "decl/import/DCL-IM-003.lz", bug_types=[1])

# 魔法方法
_t("DCL-MG-001", "__add__ 算术", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-001.lz", present=["10", "20"], bug_types=[3])
_t("DCL-MG-002", "__eq__ 比较", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-002.lz", present=["True"], bug_types=[3])
_t("DCL-MG-003", "__getitem__ 容器", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-003.lz", present=["10", "20", "30"], bug_types=[3])
_t("DCL-MG-004", "__str__ 转换", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-004.lz", present=["magic"], bug_types=[3])
_t("DCL-MG-005", "__iter__ + __next__", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-005.lz", present=["3"], bug_types=[3])
_t("DCL-MG-006", "__len__ 长度", "decl/magic", "P1", "run",
   "decl/magic/DCL-MG-006.lz", present=["6"], bug_types=[3])


# ---- 元编程 META (P1-P2) ----
_t("META-DEC-001", "@decorator 无参", "meta/decorator", "P1", "compile",
   "meta/decorator/META-DEC-001.lz", bug_types=[1])
_t("META-DEC-002", "@decorator 带参", "meta/decorator", "P1", "compile",
   "meta/decorator/META-DEC-002.lz", bug_types=[1])
_t("META-DEC-003", "@export(Rust)", "meta/decorator", "P1", "compile",
   "meta/decorator/META-DEC-003.lz", bug_types=[1])
_t("META-DEC-004", "@derive(Clone,Debug)", "meta/derive", "P1", "compile",
   "meta/derive/META-DEC-004.lz", bug_types=[1])
_t("META-DEC-005", "@curry 装饰器", "meta/decorator", "P2", "compile",
   "meta/decorator/META-DEC-005.lz", bug_types=[1])
_t("META-CPT-001", "comptime 表达式", "meta/comptime", "P2", "compile",
   "meta/comptime/META-CPT-001.lz", bug_types=[1,3])
_t("META-CPT-002", "comptime 块", "meta/comptime", "P2", "compile",
   "meta/comptime/META-CPT-002.lz", bug_types=[1,3])
_t("META-MCR-001", "宏模块声明", "meta/macro", "P2", "compile",
   "meta/macro/META-MCR-001.lz", bug_types=[1])
_t("META-MCR-002", "Cangjie 宏调用", "meta/macro", "P2", "compile",
   "meta/macro/META-MCR-002.lz")
_t("META-TMP-001", "template 定义", "meta/template", "P2", "compile",
   "meta/template/META-TMP-001.lz")


# ---- 构建块 BUILD (P2) ----
_t("BLD-VAR-001", "=: 变量块", "build/var_block", "P2", "run",
   "build/var_block/BLD-VAR-001.lz", present=["3"], bug_types=[3])
_t("BLD-VAR-002", "=: 多语句块", "build/var_block", "P2", "run",
   "build/var_block/BLD-VAR-002.lz", present=["30"], bug_types=[3])
_t("BLD-CALL-001", "~: 调用块(元组)", "build/call_block", "P2", "run",
   "build/call_block/BLD-CALL-001.lz", present=["30"], bug_types=[3])
_t("BLD-GEN-001", "*: 生成器 + yield", "build/gen_block", "P2", "compile",
   "build/gen_block/BLD-GEN-001.lz", bug_types=[1,3])
_t("BLD-GEN-002", "yield from", "build/gen_block", "P2", "compile",
   "build/gen_block/BLD-GEN-002.lz")


# ---- 模块系统 MOD (P1) ----
_t("MOD-001", "#!bin", "modules", "P1", "compile",
   "modules/MOD-001.lz", bug_types=[1,3])
_t("MOD-002", "#!lib", "modules", "P1", "compile",
   "modules/MOD-002.lz", bug_types=[1,3])
_t("MOD-003", "#!test", "modules", "P1", "compile",
   "modules/MOD-003.lz", bug_types=[1])
_t("MOD-004", "#!bin macro", "modules", "P1", "compile",
   "modules/MOD-004.lz")
_t("MOD-005", "#!lenient", "modules", "P1", "compile",
   "modules/MOD-005.lz", bug_types=[1,2])


# ---- 测试框架 TEST (P1) ----
_t("TST-001", "assert 复合表达式", "test_framework", "P1", "run",
   "test_framework/TST-001.lz", present=["ok"], bug_types=[3])
_t("TST-002", "assert not", "test_framework", "P1", "run",
   "test_framework/TST-002.lz", present=["ok"], bug_types=[3])
_t("TST-003", "test 引用外部函数", "test_framework", "P1", "compile",
   "test_framework/TST-003.lz", bug_types=[1])
_t("TST-004", "suite + const", "test_framework", "P1", "compile",
   "test_framework/TST-004.lz", bug_types=[1])


# ---- 并发异步 ASYNC (P2) ----
_t("ASY-001", "async def 定义", "async", "P2", "compile",
   "async/ASY-001.lz")
_t("ASY-002", "await 前缀", "async", "P2", "compile",
   "async/ASY-002.lz")
_t("ASY-003", "await 后缀", "async", "P2", "compile",
   "async/ASY-003.lz")
_t("ASY-004", "spawn 启动", "async", "P2", "compile",
   "async/ASY-004.lz")
_t("ASY-005", "yield 生成器", "async", "P2", "compile",
   "async/ASY-005.lz", bug_types=[1,3])
_t("ASY-006", "yield from + with", "async", "P2", "compile",
   "async/ASY-006.lz")


# ---- 负向测试 NEG (P0-P1) ----
# 词法应拦截
_t("NEG-LEX-001", ":: 路径分隔符应拦截", "negative/lex", "P0", "error",
   "negative/lex/NEG-LEX-001.lz", error_contains=":", bug_types=[2,4], bug_reason="Bug-2: :: 应被 lz 拦截而非透传")
_t("NEG-LEX-002", "未闭合 /* 块注释", "negative/lex", "P0", "error",
   "negative/lex/NEG-LEX-002.lz", error_contains="comment", bug_types=[2])
_t("NEG-LEX-003", "非法字符检测", "negative/lex", "P0", "error",
   "negative/lex/NEG-LEX-003.lz", error_contains="error", bug_types=[2])
_t("NEG-LEX-004", "未闭合字符串引号", "negative/lex", "P0", "error",
   "negative/lex/NEG-LEX-004.lz", error_contains="error", bug_types=[2])

# 语法应拦截
_t("NEG-PARSE-001", "缺失冒号应拦截", "negative/parse", "P0", "error",
   "negative/parse/NEG-PARSE-001.lz", error_contains="expected", bug_types=[2])
_t("NEG-PARSE-002", "缺失缩进应拦截", "negative/parse", "P0", "error",
   "negative/parse/NEG-PARSE-002.lz", error_contains="indent", bug_types=[2])
_t("NEG-PARSE-003", "不匹配括号应拦截", "negative/parse", "P0", "error",
   "negative/parse/NEG-PARSE-003.lz", error_contains="expected", bug_types=[2])
_t("NEG-PARSE-004", "变长参数双逗号 ..,,.. 应拦截", "negative/parse", "P0", "error",
   "negative/parse/NEG-PARSE-004.lz", error_contains="连续逗号", bug_types=[2,4])
_t("NEG-PARSE-005", "catch 后无参数应拦截", "negative/parse", "P1", "error",
   "negative/parse/NEG-PARSE-005.lz", error_contains="error", bug_types=[2])
_t("NEG-PARSE-006", "非法闭包语法应拦截", "negative/parse", "P1", "error",
   "negative/parse/NEG-PARSE-006.lz", error_contains="error", bug_types=[2])

# 类型应拦截 (注: lz 类型推断委托 rustc，部分 error 测试预期生成成功但 rustc 报错)
_t("NEG-TYPE-001", "参数数量不匹配", "negative/type", "P1", "error",
   "negative/type/NEG-TYPE-001.lz", error_contains="error", bug_types=[2])
_t("NEG-TYPE-002", "返回类型不匹配", "negative/type", "P1", "error",
   "negative/type/NEG-TYPE-002.lz", error_contains="error", bug_types=[2])
_t("NEG-TYPE-003", "使用未定义变量", "negative/type", "P1", "error",
   "negative/type/NEG-TYPE-003.lz", error_contains="error", bug_types=[2])
_t("NEG-TYPE-004", "i64 索引 String", "negative/type", "P1", "error",
   "negative/type/NEG-TYPE-004.lz", error_contains="error", bug_types=[2], bug_reason="Bug-20: 类型错误应被 lz 拦截")
_t("NEG-TYPE-005", "?. 用于非 Option 类型", "negative/type", "P1", "error",
   "negative/type/NEG-TYPE-005.lz", error_contains="error", bug_types=[2])

# 语义应拦截
_t("NEG-SEM-001", "impl 方法签名≠trait", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-001.lz", error_contains="mismatch", bug_types=[2], bug_reason="Bug-9: trait/impl 签名不匹配应拦截")
_t("NEG-SEM-002", "impl 缺方法", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-002.lz", error_contains="missing", bug_types=[2], bug_reason="Bug-11: impl 缺方法应拦截")
_t("NEG-SEM-003", "impl 返回类型不一致", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-003.lz", error_contains="return", bug_types=[2], bug_reason="Bug-12: impl 返回类型不一致应拦截")
_t("NEG-SEM-004", "mut 修饰不一致", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-004.lz", error_contains="mut", bug_types=[2], bug_reason="Bug-13: mut 不匹配应拦截")
_t("NEG-SEM-005", "方法名冲突", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-005.lz", error_contains="conflict", bug_types=[2], bug_reason="Bug-10: 方法名冲突应拦截")
_t("NEG-SEM-006", "match 非穷尽", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-006.lz", error_contains="exhaustive", bug_types=[2])
_t("NEG-SEM-007", "move 后使用", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-007.lz", error_contains="error", bug_types=[2])
_t("NEG-SEM-008", "重复定义", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-008.lz", error_contains="duplicate", bug_types=[2])
_t("NEG-SEM-009", "访问不存在字段", "negative/semantic", "P1", "error",
   "negative/semantic/NEG-SEM-009.lz", error_contains="field", bug_types=[2])


# =====================================================================
# 源文件生成器
# =====================================================================

def generate_sources():
    """生成所有 .lz 源码文件到 cases/ 目录"""
    sources = _get_all_sources()
    created = 0
    for filepath, content in sources.items():
        full_path = os.path.join(CASES_DIR, filepath)
        os.makedirs(os.path.dirname(full_path), exist_ok=True)
        with open(full_path, "w", encoding="utf-8") as f:
            f.write(content)
        created += 1
    print(f"Generated {created} .lz source files in {CASES_DIR}")
    return created


def _get_all_sources():
    """返回 { relative_path: content } 字典"""
    s = {}

    # ---- 词法层 ----
    s["lexer/keywords/LEX-KW-001.lz"] = "def x = 1\nlet y = 2\nmut z = 3\nconst C = 4"
    s["lexer/keywords/LEX-KW-002.lz"] = "def f(x):\n    if x > 0: return 1\n    elif x < 0: return -1\n    else: return 0"
    s["lexer/keywords/LEX-KW-003.lz"] = "def f():\n    try:\n        raise \"err\"\n    catch e:\n        panic(\"crash\")\n    finally:\n        pass"
    s["lexer/keywords/LEX-KW-004.lz"] = "test \"t1\":\n    assert True\n    check False\nsuite \"s\":\n    test \"t2\":\n        assert True"
    s["lexer/keywords/LEX-KW-005.lz"] = "async def f():\n    await g()\n    spawn h()\n    yield 1"
    s["lexer/keywords/LEX-KW-006.lz"] = "import std.io\nfrom std.io import print as p"
    s["lexer/keywords/LEX-KW-007.lz"] = "macro m() = 1\ntemplate t!() = 1\ncomptime x = 1"
    s["lexer/keywords/LEX-KW-008.lz"] = "def f(x, y):\n    return x > 0 and not (x is None or y in [1, 2])"
    s["lexer/keywords/LEX-KW-009.lz"] = "def f():\n    a = True\n    b = False\n    c = None\n    d = Some(1)\n    e = Ok(1)\n    f = Err(\"e\")"
    s["lexer/keywords/LEX-KW-010.lz"] = "def f(owned x: str, ref y: int):\n    abstract\n    private z = 1"

    s["lexer/comments/LEX-CMT-001.lz"] = "// this is a comment\nx = 1"
    s["lexer/comments/LEX-CMT-002.lz"] = "/* block comment */\nx = 1"
    s["lexer/comments/LEX-CMT-003.lz"] = "/* outer /* inner nested */ still outer */\nx = 1"
    s["lexer/comments/LEX-CMT-004.lz"] = "#[attr]\ndef f() = 1"

    s["lexer/literals/LEX-LIT-001.lz"] = "def f() = 42"
    s["lexer/literals/LEX-LIT-002.lz"] = "def f() = 1_000_000"
    s["lexer/literals/LEX-LIT-003.lz"] = "def f() = 3.14"
    s["lexer/literals/LEX-LIT-004.lz"] = "def f() = \"hello\""
    s["lexer/literals/LEX-LIT-005.lz"] = "def f(x):\n    return f\"x = {x}\""
    s["lexer/literals/LEX-LIT-006.lz"] = "def f() = r\"C:\\path\""
    s["lexer/literals/LEX-LIT-007.lz"] = "def f() = \"\"\"multi\nline\"\"\""

    s["lexer/operators/LEX-OP-001.lz"] = "std::io::println(\"err\")"
    s["lexer/operators/LEX-OP-002.lz"] = "def f():\n    x =: y = 1\n    y"
    s["lexer/operators/LEX-OP-003.lz"] = "def f():\n    x = None\n    y = x?.val ?? 0"

    # ---- 类型系统 ----
    s["types/primitives/TYP-PRIM-001.lz"] = "def f(x: int) -> int = x + 1\ndef main():\n    print(f(41))"
    s["types/primitives/TYP-PRIM-002.lz"] = "def add(a: float, b: float) -> float = a + b\ndef main():\n    print(3.5 + 4.0)"
    s["types/primitives/TYP-PRIM-003.lz"] = "def greet(name: str) -> str = \"Hello, \" + name\ndef main():\n    print(greet(\"World\"))"
    s["types/primitives/TYP-PRIM-004.lz"] = "def is_pos(x: int) -> bool = x > 0\ndef main():\n    print(is_pos(5))"
    s["types/primitives/TYP-PRIM-005.lz"] = "def f_i32(x: i32) -> i32 = x\ndef f_u32(x: u32) -> u32 = x\ndef f_u64(x: u64) -> u64 = x\ndef f_f32(x: f32) -> f32 = x"
    s["types/primitives/TYP-PRIM-006.lz"] = "def ch() -> char = 'a'"

    s["types/containers/TYP-CON-001.lz"] = "def main():\n    n: List<int> = [1, 2, 3]\n    print(n[0])\n    print(n[1])\n    print(n[2])"
    s["types/containers/TYP-CON-002.lz"] = "def main():\n    d: Dict<str, int> = {\"a\": 1}\n    print(d[\"a\"])"
    s["types/containers/TYP-CON-003.lz"] = "def f() -> Set<str> = {\"a\", \"b\"}"
    s["types/containers/TYP-CON-004.lz"] = "def f() -> Array<int, 3> = [1, 2, 3]"
    s["types/containers/TYP-CON-005.lz"] = "def f() -> (int, str) = (42, \"hello\")\ndef main():\n    (a, b) = f()\n    print(a)\n    print(b)"
    s["types/containers/TYP-CON-006.lz"] = "def f() = 1..10"

    s["types/option/TYP-OPT-001.lz"] = "def f(x: int?) -> int? = x"
    s["types/option/TYP-OPT-002.lz"] = "def main():\n    x = Some(42)\n    print(x.unwrap())"
    s["types/option/TYP-OPT-003.lz"] = "def main():\n    a: int? = Some(42)\n    b: int? = None\n    print(a ?? 0)\n    print(b ?? 0)"

    s["types/generics/TYP-GEN-001.lz"] = "def id<T>(x: T) -> T = x\ndef main():\n    print(id(42))\n    print(id(\"hello\"))"
    s["types/generics/TYP-GEN-002.lz"] = "def max_val<T>(a: T, b: T) -> T where T <: Ord = if a > b: a else: b"
    s["types/generics/TYP-GEN-003.lz"] = "def dup<T>(x: T) -> (T, T) where T <: Clone + Ord = (x, x)"
    s["types/generics/TYP-GEN-004.lz"] = "struct Pair<T, U> = first: T, second: U"

    s["types/alias/TYP-ALIAS-001.lz"] = "type ID = int\ndef f(x: ID) -> ID = x"
    s["types/alias/TYP-ALIAS-002.lz"] = "type Pair<T> = (T, T)"

    # ---- 表达式 ---- (to be continued...)
    # 由于文件量大，采用分批写入策略

    return s


# =====================================================================
# 主入口
# =====================================================================

def main():
    import argparse

    parser = argparse.ArgumentParser(description="LZ 编译器全面集成测试")
    parser.add_argument("--priority", default=None, help="只运行指定优先级 (P0/P1/P2)")
    parser.add_argument("--category", default=None, help="只运行指定分类")
    parser.add_argument("--bug-type", type=int, default=None, help="只运行指定 Bug 类型 (1-4)")
    parser.add_argument("--id", default=None, help="指定测试 ID，逗号分隔")
    parser.add_argument("--only-failed", action="store_true", help="只运行上次失败的")
    parser.add_argument("--gen-sources", action="store_true", help="生成所有 .lz 源码文件")
    parser.add_argument("--clean", action="store_true", help="运行后清理 _work")
    parser.add_argument("--skip-build", action="store_true", help="跳过 cargo build")
    args = parser.parse_args()

    # 生成源文件
    if args.gen_sources:
        generate_sources()
        print("Source generation complete. Run without --gen-sources to test.")
        return

    # 确保源文件存在
    missing = [c for c in CATALOG if not os.path.isfile(c.source_path())]
    if missing:
        print(f"Missing {len(missing)} source files. Run with --gen-sources first.")
        for m in missing[:5]:
            print(f"  {m.source_file}")
        if len(missing) > 5:
            print(f"  ... and {len(missing)-5} more")
        return 1

    # 构建编译器
    if not args.skip_build:
        print("Building lz compiler...")
        rc = subprocess.run(["cargo", "build"], cwd=ROOT).returncode
        if rc != 0:
            print("cargo build failed. Use --skip-build to use existing binary.")
            return 1

    # 配置
    config = Config.auto_discover()
    if not config.ensure_binary():
        return 1

    print(f"Binary: {config.sut_binary}")
    print(f"Std:    {config.std_dir}")
    print(f"Cases:  {len(CATALOG)} test cases\n")

    # 创建 work 目录
    os.makedirs(config.work_dir, exist_ok=True)
    os.makedirs(config.reports_dir, exist_ok=True)

    # 运行
    runner = TestRunner(config)
    filter_ids = args.id.split(",") if args.id else None

    from runner import TestResult  # noqa

    results = runner.run_all(
        CATALOG,
        filter_priority=args.priority,
        filter_category=args.category,
        filter_bug_type=args.bug_type,
        filter_ids=filter_ids,
        only_failed=args.only_failed,
    )

    # 报告
    reporter = TestReporter(config)
    reporter.console_summary(results)
    json_path = reporter.generate_json(results)
    md_path = reporter.generate_markdown(results)
    print(f"Reports: {json_path}")
    print(f"         {md_path}")

    # 清理
    if args.clean:
        import shutil
        shutil.rmtree(config.work_dir, ignore_errors=True)
        print("Cleaned _work/")

    # 退出码
    failed = sum(1 for r in results if r.status in ("FAIL", "CRASH"))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
