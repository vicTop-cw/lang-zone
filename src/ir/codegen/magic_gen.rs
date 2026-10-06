// Lang-Zone 编译器 — ir/codegen/magic_gen.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl CodeGen {
    /// 扩展 trait（ListExt/StrExt 等）方法名映射：与调用点 MethodCall 的
    /// `slice → lz_slice` 映射保持一致，否则 trait 声明是 slice、调用是
    /// lz_slice → E0599（list.lz 复现）
    pub(crate) fn ext_trait_method_name(&self, name: &str, _ext_name: &str) -> String {
        match name {
            "slice" => "lz_slice".to_string(),
            _ => name.to_string(),
        }
    }

    /// 计算重载函数的 mangled 名称。仅当函数名有多个重载签名时返回 mangled 名，
    /// 否则返回原名。用于函数定义处。
    pub(crate) fn mangled_fn_name(&self, name: String, sig: &[IrType]) -> String {
        // 清理测试名：空格 → 下划线，确保生成合法的 Rust 标识符
        let name = name.replace(' ', "_");
        if let Some(sigs) = self.overload_sigs.get(&name) {
            if sigs.len() > 1 {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return format!("{}__{}", name, suffix.join("_"));
            }
        }
        name
    }

    /// 根据实参 IR 类型匹配重载签名，返回选择的 mangled 函数名。
    /// 找不到匹配时返回 None（调用方保留原名）。
    /// 分派原则（03d §2.7）：先用不带 `..` 的固定签名匹配；无命中时
    /// 再按声明顺序对带 `..` 的变长签名做兜底匹配（只看显式参数）。
    pub(crate) fn match_overload(
        &self,
        name: &str,
        sigs: &[Vec<IrType>],
        args: &[Expr],
    ) -> Option<String> {
        // 参数类型兼容：实参类型与签名参数类型匹配（含 Any 通配）
        let compatible = |arg_ty: &IrType, param_ty: &IrType| -> bool {
            if matches!(param_ty, IrType::Any) {
                return true;
            }
            if matches!(arg_ty, IrType::Any) {
                return true;
            }
            arg_ty == param_ty
        };
        let variadic_flags = self.overload_variadic.get(name);
        let explicit_sigs = self.overload_explicit.get(name);
        // 阶段 1：固定签名（无 `..`）精确匹配
        for (i, sig) in sigs.iter().enumerate() {
            let is_var = variadic_flags.map_or(false, |v| v.get(i).copied().unwrap_or(false));
            if is_var {
                continue;
            }
            if sig.len() == args.len()
                && args
                    .iter()
                    .zip(sig.iter())
                    .all(|(a, p)| compatible(&a.ty, p))
            {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return Some(format!("{}__{}", name, suffix.join("_")));
            }
        }
        // 阶段 2：变长签名（带 `..`）兜底：显式参数全部兼容且数量不超出即可
        for (i, sig) in sigs.iter().enumerate() {
            let is_var = variadic_flags.map_or(false, |v| v.get(i).copied().unwrap_or(false));
            if !is_var {
                continue;
            }
            let explicit = explicit_sigs
                .and_then(|e| e.get(i))
                .cloned()
                .unwrap_or_default();
            if args.len() < explicit.len() {
                continue;
            }
            if explicit
                .iter()
                .zip(args.iter())
                .all(|(p, a)| compatible(&a.ty, p))
            {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return Some(format!("{}__{}", name, suffix.join("_")));
            }
        }
        None
    }

    /// 将 IrType 编码为 mangled 后缀（简短稳定编码）
    pub(crate) fn type_mangle_suffix(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".to_string(),
            IrType::F64 => "f64".to_string(),
            IrType::Bool => "bool".to_string(),
            IrType::Str => "String".to_string(),
            IrType::Named { path, args } => {
                if args.is_empty() {
                    path.replace("::", "_")
                } else {
                    let inner: Vec<String> =
                        args.iter().map(|a| self.type_mangle_suffix(a)).collect();
                    format!("{}_{}", path.replace("::", "_"), inner.join("_"))
                }
            }
            other => format!("{:?}", other).replace(['<', '>', ' ', '(', ')', ',', '{', '}'], "_"),
        }
    }

    /// 自动为魔法方法生成对应的 Rust trait impl（联动补全，对齐设计 §9.1）。
    /// 仅在 inherent impl（trait_==None）且非枚举/外部类型时由 emit_impl 调用。
    ///
    /// 关键：每个魔法方法自带 `where` 约束（如 `def __str__(ref self) -> str
    /// where T: Display`），其生成的 trait impl 必须**携带同名 `where` 子句**，
    /// 否则 impl 块的泛型上下文缺少该约束，调用 `__str__()` 时触发 E0277
    /// （box.lz `impl<T: Clone + Debug> Display for Box<T>` 缺 `T: Display`）。
    pub(crate) fn gen_magic_trait_impls(
        &mut self,
        for_ty: &str,
        generics: &str,
        methods: &[FnDef],
    ) {
        // __eq__ → std::cmp::PartialEq（struct 体内 / impl 块共用）。
        // 委托 __eq__ 生成 impl，并携带 __eq__ 的 where 约束（如 T: Eq）。
        // 第二参数为 ref 时直接传 other（&Self）；值为参数时需 (*other).clone()。
        if let Some(eq_m) = methods.iter().find(|m| m.name == "__eq__") {
            let eq_where: String = eq_m
                .where_clause
                .iter()
                .map(|(tp, bounds)| {
                    let bs: Vec<String> = bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                    format!("{}: {}", tp, bs.join(" + "))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let where_str = if eq_where.is_empty() {
                String::new()
            } else {
                format!(" where {}", eq_where)
            };
            self.emit_line(&format!(
                "impl{} std::cmp::PartialEq for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn eq(&self, other: &Self) -> bool {");
            self.indent += 1;
            let eq_takes_ref = eq_m.params.get(1).map_or(false, |p| {
                p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
            });
            if eq_takes_ref {
                self.emit_line("self.__eq__(other)");
            } else {
                self.emit_line("self.__eq__((*other).clone())");
            }
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __str__ → std::fmt::Display
        if let Some(sm) = methods.iter().find(|m| m.name == "__str__") {
            let where_str = self.magic_impl_where_str(sm);
            self.emit_line(&format!(
                "impl{} std::fmt::Display for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
            self.indent += 1;
            self.emit_line("write!(f, \"{}\", self.__str__())");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __repr__ → std::fmt::Debug
        if let Some(rm) = methods.iter().find(|m| m.name == "__repr__") {
            let where_str = self.magic_impl_where_str(rm);
            self.emit_line(&format!(
                "impl{} std::fmt::Debug for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
            self.indent += 1;
            self.emit_line("write!(f, \"{}\", self.__repr__())");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __next__ → std::iter::Iterator（仅当返回 Option<T> 时生成；否则 next 签名不匹配）
        // 同时检查 __next__ 和 next（LZ 源码用 next，IR 可能映射为 __next__）
        if let Some(nm) = methods
            .iter()
            .find(|m| m.name == "__next__" || m.name == "next")
        {
            if let IrType::Option(inner) = &nm.ret_ty {
                let where_str = self.magic_impl_where_str(nm);
                let item_ty = self.rust_type(inner);
                self.emit_line(&format!(
                    "impl{} Iterator for {}{} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                // 本模块自定义了 `trait Iterator`（会生成 pub trait Iterator 遮蔽 std）时，
                // 该 impl 解析到本地 trait（无 Item 关联类型），补 type Item 反报
                // E0437 type Item is not a member of trait Iterator（lib_iterator 复现）。
                // 仅当模块未声明 Iterator trait（impl 落 std::iter::Iterator）时补 Item。
                if !self.trait_names.contains("Iterator") {
                    self.emit_line(&format!("type Item = {};", item_ty));
                }
                self.emit_line(&format!("fn next(&mut self) -> Option<{}> {{", item_ty));
                self.indent += 1;
                // 调用对应的方法：如果是 __next__ 则调用 self.__next__()，否则调用结构体的 next 方法
                // 注意：self 已经是 &mut self，直接传 self 即可
                if nm.name == "__next__" {
                    self.emit_line("self.__next__()");
                } else {
                    // 使用 struct_name::next(self) 调用结构体的 next 方法
                    self.emit_line(&format!("{}::next(self)", for_ty));
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __rev__ → std::iter::DoubleEndedIterator（06d §九）：仅当 __next__ 共存时
        // 生成（DoubleEndedIterator 继承 Iterator，需先有 Iterator impl）。
        // `def __rev__(ref self) -> Option<T>` →
        // impl DoubleEndedIterator for SelfTy { fn next_back(&mut self) -> Option<Self::Item> { self.__rev__() } }
        // 注意：不在 let 链中写 `&& let`（Rust 2021 不支持），改用 match 解构
        if methods.iter().any(|m| m.name == "__next__") {
            if let Some(rm) = methods.iter().find(|m| m.name == "__rev__") {
                if let IrType::Option(_inner) = &rm.ret_ty {
                    let where_str = self.magic_impl_where_str(rm);
                    self.emit_line(&format!(
                        "impl{} std::iter::DoubleEndedIterator for {} {} {{",
                        generics, for_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line("fn next_back(&mut self) -> Option<Self::Item> {");
                    self.indent += 1;
                    self.emit_line("self.__rev__()");
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __iter__ → std::iter::IntoIterator（仅当返回命名迭代器类型时生成；
        // 返回 () 等非法类型会报 "() is not an iterator"，见 duck_nested.lz）
        // __iter__ → std::iter::IntoIterator（06d §九）：
        // `def __iter__(self) -> IterTy` → impl IntoIterator。
        // 当返回 Vec<T> 时，IntoIter 不能是 Vec<T>（Vec 不满足 Iterator），
        // 应映射为 std::vec::IntoIter<T> 并在 into_iter 体调用 .into_iter()
        // 包装（p41 探针暴露：`Vec<i64>` is not an iterator）
        if let Some(im) = methods.iter().find(|m| m.name == "__iter__") {
            if let IrType::Named { .. } = &im.ret_ty {
                let where_str = self.magic_impl_where_str(im);
                let (iter_ty, body_expr) = match &im.ret_ty {
                    IrType::Named { path, args } if path == "Vec" && args.len() == 1 => {
                        let elem = self.rust_type(&args[0]);
                        (
                            format!("std::vec::IntoIter<{}>", elem),
                            "self.__iter__().into_iter()".to_string(),
                        )
                    }
                    IrType::Named { path, args } if path == "List" && args.len() == 1 => {
                        let elem = self.rust_type(&args[0]);
                        (
                            format!("std::vec::IntoIter<{}>", elem),
                            "self.__iter__().into_iter()".to_string(),
                        )
                    }
                    _ => {
                        let ty = self.rust_type(&im.ret_ty);
                        (ty, "self.__iter__()".to_string())
                    }
                };
                self.emit_line(&format!(
                    "impl{} std::iter::IntoIterator for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type IntoIter = {};", iter_ty));
                self.emit_line(&format!(
                    "type Item = <{} as std::iter::Iterator>::Item;",
                    iter_ty
                ));
                self.emit_line("fn into_iter(self) -> Self::IntoIter {");
                self.indent += 1;
                self.emit_line(&body_expr);
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 复合赋值族 → std::ops::*Assign（06d §四，对齐 __iadd__ 先例）：
        // __iadd__→AddAssign、__isub__→SubAssign、__imul__→MulAssign、__idiv__→DivAssign
        //（Rhs 取 other 参数类型；self 固有方法为 `mut self`，AddAssign 的
        // add_assign(&mut self, rhs) 经 auto-mut 调用即可）
        for (magic, trait_path, trait_method) in &[
            ("__iadd__", "std::ops::AddAssign", "add_assign"),
            ("__isub__", "std::ops::SubAssign", "sub_assign"),
            ("__imul__", "std::ops::MulAssign", "mul_assign"),
            ("__idiv__", "std::ops::DivAssign", "div_assign"),
        ] {
            if let Some(am) = methods.iter().find(|m| m.name == *magic) {
                let where_str = self.magic_impl_where_str(am);
                let rhs_ty = am
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| "()".to_string());
                self.emit_line(&format!(
                    "impl{} {}<{}> for {} {} {{",
                    generics, trait_path, rhs_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "fn {}(&mut self, other: {}) {{",
                    trait_method, rhs_ty
                ));
                self.indent += 1;
                self.emit_line(&format!("self.{}(other)", magic));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 一元 __neg__ → std::ops::Neg / __not__ → std::ops::Not：
        // 调用点已直派 __neg__/__not__（builder Unary 分派），trait impl 供
        // 泛型/库代码路径使用
        if let Some(nm) = methods.iter().find(|m| m.name == "__neg__") {
            if !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Neg for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn neg(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__neg__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
        if let Some(nm) = methods.iter().find(|m| m.name == "__not__") {
            if !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Not for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn not(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__not__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
        // 位非 ~a：__invert__ 复用 std::ops::Not impl（注册表 Invert kind 同 trait；
        // Rust 无独立 BitNot trait，Not 即位非/逻辑非统一入口）
        let has_not = methods.iter().any(|m| m.name == "__not__");
        if let Some(nm) = methods.iter().find(|m| m.name == "__invert__") {
            if !has_not && !matches!(nm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(nm);
                let out_ty = self.rust_type(&nm.ret_ty);
                self.emit_line(&format!(
                    "impl{} std::ops::Not for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", out_ty));
                self.emit_line(&format!("fn not(self) -> {} {{", out_ty));
                self.indent += 1;
                self.emit_line("self.__invert__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __cmp__ → std::cmp::Ord（06d §五）：全序比较委托 __cmp__。
        // Ord 要求 Eq + PartialOrd 超trait——仅在 __eq__/__lt__ 同时定义
        // （PartialEq/PartialOrd impl 已生成）时才生成，否则 E0277。
        // LZ 的 __cmp__ 返回 int（-1/0/1），映射为 Ordering::Less/Equal/Greater
        let has_eq_impl = methods.iter().any(|m| m.name == "__eq__");
        let has_lt_impl = methods.iter().any(|m| m.name == "__lt__");
        if let Some(cm) = methods.iter().find(|m| m.name == "__cmp__") {
            if has_eq_impl && has_lt_impl {
                // Rust 的 Eq 是独立标记 trait（Ord/Hash 超trait 要求 Eq），
                // 仅 PartialEq 不满足——需一并生成空 Eq 标记 impl
                let eq_m = methods.iter().find(|m| m.name == "__eq__");
                let eq_where = eq_m
                    .map(|m| self.magic_impl_where_str(m))
                    .unwrap_or_default();
                self.emit_line(&format!(
                    "impl{} std::cmp::Eq for {} {} {{}}",
                    generics, for_ty, eq_where
                ));
                let where_str = self.magic_impl_where_str(cm);
                self.emit_line(&format!(
                    "impl{} std::cmp::Ord for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line("fn cmp(&self, other: &Self) -> std::cmp::Ordering {");
                self.indent += 1;
                // __cmp__ 第二参数 ref 直接传，owned 需解引用克隆
                let takes_ref = cm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let arg = if takes_ref {
                    "other".to_string()
                } else {
                    "(*other).clone()".to_string()
                };
                self.emit_line(&format!(
                    "let c = self.__cmp__({}); if c < 0 {{ std::cmp::Ordering::Less }} else if c == 0 {{ std::cmp::Ordering::Equal }} else {{ std::cmp::Ordering::Greater }}",
                    arg
                ));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __hash__ → std::hash::Hash（06d §五）：哈希委托 __hash__（返回 int）
        if let Some(hm) = methods.iter().find(|m| m.name == "__hash__") {
            let where_str = self.magic_impl_where_str(hm);
            self.emit_line(&format!(
                "impl{} std::hash::Hash for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn hash<H: std::hash::Hasher>(&self, state: &mut H) {");
            self.indent += 1;
            self.emit_line("self.__hash__().hash(state)");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __drop__ → std::ops::Drop（06d §十）：析构委托 __drop__（&mut self）。
        // 注意：Drop impl 的泛型参数须与 struct 定义**逐字一致**（E0367）——
        // generics 串可能带 bounds（`<T: Clone + Debug>`），加上即与无约束的
        // struct 定义不匹配；裸泛型内调 self.__drop__() 又会因方法约束报 E0599
        //（box.lz）。故仅方法与泛型串均无约束时生成（保守回退）
        if let Some(dm) = methods.iter().find(|m| m.name == "__drop__") {
            if dm.where_clause.is_empty() && !generics.contains(':') {
                self.emit_line(&format!(
                    "impl{} std::ops::Drop for {} {{",
                    generics, for_ty
                ));
                self.indent += 1;
                self.emit_line("fn drop(&mut self) {");
                self.indent += 1;
                self.emit_line("self.__drop__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __clone__ → std::clone::Clone 委托（06d §十）：struct 定义处已去掉
        // derive(Clone)（E0119），此处生成手动 impl，使 `a.clone()` 走用户的
        // 自定义克隆逻辑（如引用计数 +1）而非逐字段复制
        if let Some(cm) = methods.iter().find(|m| m.name == "__clone__") {
            let where_str = self.magic_impl_where_str(cm);
            self.emit_line(&format!(
                "impl{} std::clone::Clone for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn clone(&self) -> Self {");
            self.indent += 1;
            self.emit_line("self.__clone__()");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __default__ → std::default::Default（06d §十）：默认值委托 __default__
        //（静态方法，无 self，返回 Self）
        if let Some(fm) = methods.iter().find(|m| m.name == "__default__") {
            if !matches!(fm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(fm);
                self.emit_line(&format!(
                    "impl{} std::default::Default for {} {} {{",
                    generics, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line("fn default() -> Self {");
                self.indent += 1;
                self.emit_line("Self::__default__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __int__/__float__ 缺口魔法（06d §十六）：编译器自动生成
        // impl From<SelfTy> for i64 / impl From<SelfTy> for f64，
        // 使 int(x)/float(x) 可经 .into() 或调用点直派转换
        for (magic, std_ty) in &[("__int__", "i64"), ("__float__", "f64")] {
            if let Some(im) = methods.iter().find(|m| m.name == *magic) {
                if !matches!(im.ret_ty, IrType::Unit) {
                    let where_str = self.magic_impl_where_str(im);
                    self.emit_line(&format!(
                        "impl{} std::convert::From<{}> for {} {} {{",
                        generics, for_ty, std_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line(&format!("fn from(value: {}) -> Self {{", for_ty));
                    self.indent += 1;
                    self.emit_line(&format!("value.{}()", magic));
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __implicit_copy__ → lz_builtins::ImplicitCopy（06d §十四 Mojo 风格）：
        // `def __implicit_copy__(self) -> Self` →
        // impl ImplicitCopy for SelfTy { fn __implicit_copy__(&self) -> Self { Self::__implicit_copy__(&self) } }
        if let Some(ic) = methods.iter().find(|m| m.name == "__implicit_copy__") {
            let where_str = self.magic_impl_where_str(ic);
            self.emit_line(&format!(
                "impl{} ImplicitCopy for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn __implicit_copy__(&self) -> Self {");
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_copy__(&self)", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __implicit_default__ → lz_builtins::ImplicitDefault（06d §十四）：
        // `def __implicit_default__() -> Self`（静态方法，无 self 参数）→
        // impl ImplicitDefault for SelfTy { fn __implicit_default__() -> Self { Self::__implicit_default__() } }
        if methods.iter().any(|m| m.name == "__implicit_default__") {
            let where_str =
                if let Some(im) = methods.iter().find(|m| m.name == "__implicit_default__") {
                    self.magic_impl_where_str(im)
                } else {
                    String::new()
                };
            self.emit_line(&format!(
                "impl{} ImplicitDefault for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn __implicit_default__() -> Self {");
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_default__()", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __implicit_from__ → ImplicitFrom blanket（06d §十五 隐式策略）：
        // 用户在 impl 块定义 `def __implicit_from__(raw: SrcTy) -> Self` →
        // impl ImplicitFrom<SrcTy> for SelfTy，委托用户方法（区别于 struct
        // 声明式 implicit_froms 的内建首字段构造，见 emit StructDef 处）
        if let Some(im) = methods.iter().find(|m| m.name == "__implicit_from__") {
            if let Some(p0) = im.params.first() {
                let src_ty = self.rust_type(&p0.ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} ImplicitFrom<{}> for {} {} {{",
                    generics, src_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "fn __implicit_from__(value: {}) -> Self {{",
                    src_ty
                ));
                self.indent += 1;
                self.emit_line(&format!("{}::__implicit_from__(value)", for_ty));
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __implicit_to__ → lz_builtins::ImplicitInto（06d §十四）：
        // `def __implicit_to__(self) -> TgtTy` →
        // impl ImplicitInto<TgtTy> for SelfTy { fn __implicit_into__(&self) -> TgtTy { Self::__implicit_to__(self) } }
        if let Some(it) = methods.iter().find(|m| m.name == "__implicit_to__") {
            let tgt_ty = self.rust_type(&it.ret_ty);
            let where_str = self.magic_impl_where_str(it);
            self.emit_line(&format!(
                "impl{} ImplicitInto<{}> for {} {} {{",
                generics, tgt_ty, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("fn __implicit_into__(&self) -> {} {{", tgt_ty));
            self.indent += 1;
            self.emit_line(&format!("{}::__implicit_to__(&self)", for_ty));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __from__ → std::convert::From（隐式转换第一环，06d §十四）：
        // `def __from__(raw: SrcTy) -> Self`（静态方法，无 self 参数）→
        // impl From<SrcTy> for SelfTy { fn from(value: SrcTy) -> Self { Self::__from__(value) } }
        // 返回 Unit / 无参数时无 trait 对应物，跳过
        if let Some(fm) = methods.iter().find(|m| m.name == "__from__") {
            if let (Some(p0), false) = (fm.params.first(), matches!(fm.ret_ty, IrType::Unit)) {
                let src_ty = self.rust_type(&p0.ty);
                let where_str = self.magic_impl_where_str(fm);
                self.emit_line(&format!(
                    "impl{} std::convert::From<{}> for {} {} {{",
                    generics, src_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("fn from(value: {}) -> Self {{", src_ty));
                self.indent += 1;
                self.emit_line("Self::__from__(value)");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __into__ → std::convert::Into（From→Into 链第二环，06d §十四）：
        // `def __into__(self) -> TargetTy` →
        // impl Into<TargetTy> for SelfTy { fn into(self) -> TargetTy { self.__into__() } }
        // （Rust 已有 From→Into blanket，但用户只定义 __into__ 时无 From，需显式 Into impl）
        if let Some(im) = methods.iter().find(|m| m.name == "__into__") {
            if !matches!(im.ret_ty, IrType::Unit) {
                let tgt_ty = self.rust_type(&im.ret_ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} std::convert::Into<{}> for {} {} {{",
                    generics, tgt_ty, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("fn into(self) -> {} {{", tgt_ty));
                self.indent += 1;
                self.emit_line("self.__into__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __try_from__ → std::convert::TryFrom（06d §六）：
        // `def __try_from__(raw: SrcTy) -> Result<Self, E>` →
        // impl TryFrom<SrcTy> for SelfTy { type Error = E;
        //   fn try_from(value: SrcTy) -> Result<Self, Self::Error> { Self::__try_from__(value) } }
        if let Some(fm) = methods.iter().find(|m| m.name == "__try_from__") {
            if let (Some(p0), Some(err_ty)) = (fm.params.first(), Self::try_from_err_ty(&fm.ret_ty))
            {
                if !matches!(fm.ret_ty, IrType::Unit) {
                    let src_ty = self.rust_type(&p0.ty);
                    let err_s = self.rust_type(&err_ty);
                    let where_str = self.magic_impl_where_str(fm);
                    self.emit_line(&format!(
                        "impl{} std::convert::TryFrom<{}> for {} {} {{",
                        generics, src_ty, for_ty, where_str
                    ));
                    self.indent += 1;
                    self.emit_line(&format!("type Error = {};", err_s));
                    self.emit_line(&format!(
                        "fn try_from(value: {}) -> Result<Self, Self::Error> {{",
                        src_ty
                    ));
                    self.indent += 1;
                    self.emit_line("Self::__try_from__(value)");
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
        }

        // __try_into__ → std::convert::TryInto（06d §六）：
        // `def __try_into__(self) -> Result<TargetTy, E>` →
        // impl TryInto<TargetTy> for SelfTy { type Error = E;
        //   fn try_into(self) -> Result<TargetTy, Self::Error> { self.__try_into__() } }
        if let Some(im) = methods.iter().find(|m| m.name == "__try_into__") {
            if let Some((tgt, err_ty)) = Self::try_into_tgt_err(&im.ret_ty) {
                let tgt_s = self.rust_type(&tgt);
                let err_s = self.rust_type(&err_ty);
                let where_str = self.magic_impl_where_str(im);
                self.emit_line(&format!(
                    "impl{} std::convert::TryInto<{}> for {} {} {{",
                    generics, tgt_s, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Error = {};", err_s));
                self.emit_line(&format!(
                    "fn try_into(self) -> Result<{}, Self::Error> {{",
                    tgt_s
                ));
                self.indent += 1;
                self.emit_line("self.__try_into__()");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // 比较族 __eq__ + __lt__ → std::cmp::PartialOrd：
        // `<` `>` 等运算符调用点已直派 __lt__/__gt__（本文件 comparison 分派），
        // 但泛型场景（Vec<V> 排序、sort()）需要 PartialOrd impl。由
        // __eq__/__lt__ 推导 partial_cmp（Equal/Less/Greater），两者都定义才生成
        //（PartialOrd: PartialEq 超trait，缺 __eq__ 时 impl 会 E0277）
        if let (Some(eq_m), Some(lt_m)) = (
            methods.iter().find(|m| m.name == "__eq__"),
            methods.iter().find(|m| m.name == "__lt__"),
        ) {
            let where_str = self.magic_impl_where_str(lt_m);
            self.emit_line(&format!(
                "impl{} std::cmp::PartialOrd for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line("fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {");
            self.indent += 1;
            // __eq__/__lt__ 第二参数可能为 ref（box.lz `ref other`）或 owned：
            // ref 直接传 other（&Self），owned 需 (*other).clone() 解引用克隆
            let arg_of = |m: &FnDef| -> String {
                let takes_ref = m.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                if takes_ref {
                    "other".to_string()
                } else {
                    "(*other).clone()".to_string()
                }
            };
            let eq_arg = arg_of(eq_m);
            let lt_arg = arg_of(lt_m);
            self.emit_line(&format!(
                "if self.__eq__({}) {{ Some(std::cmp::Ordering::Equal) }} else if self.__lt__({}) {{ Some(std::cmp::Ordering::Less) }} else {{ Some(std::cmp::Ordering::Greater) }}",
                eq_arg, lt_arg
            ));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // 算术/位运算族（注册表签名固定型）→ std::ops trait impl：
        // __add__→Add、__sub__→Sub、__mul__→Mul、__div__→Div、__rem__→Rem、
        // __bitand__→BitAnd、__bitor__→BitOr、__bitxor__→BitXor、__shl__→Shl、__shr__→Shr
        // 价值：`+=` 未定义 __iadd__ 时脱糖为 `a = a + b`，需要 Add impl 才能编译。
        // Rust 的 Add::add 接收 owned self：用户 `ref self` 时经 auto-ref 调用
        // &self 版本；rhs 参数 ref 时取 &rhs（与 __eq__ 的 owned/ref 处理一致）。
        for (magic, trait_path, trait_method) in &[
            ("__add__", "std::ops::Add", "add"),
            ("__sub__", "std::ops::Sub", "sub"),
            ("__mul__", "std::ops::Mul", "mul"),
            ("__div__", "std::ops::Div", "div"),
            ("__rem__", "std::ops::Rem", "rem"),
            ("__bitand__", "std::ops::BitAnd", "bitand"),
            ("__bitor__", "std::ops::BitOr", "bitor"),
            ("__bitxor__", "std::ops::BitXor", "bitxor"),
            ("__shl__", "std::ops::Shl", "shl"),
            ("__shr__", "std::ops::Shr", "shr"),
        ] {
            if let Some(mm) = methods.iter().find(|m| m.name == *magic) {
                // 输出类型：无返回注解（Unit）的算术魔法方法没有 trait 对应物，跳过
                if matches!(mm.ret_ty, IrType::Unit) {
                    continue;
                }
                let where_str = self.magic_impl_where_str(mm);
                let rhs_is_ref = mm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let rhs_ty = mm
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| for_ty.to_string());
                let output_ty = self.rust_type(&mm.ret_ty);
                // Rhs 泛型参数：ref 参数（&V）时 impl Add<&V>，调用传 &rhs
                let rhs_arg = if rhs_is_ref {
                    format!("&{}", rhs_ty)
                } else {
                    rhs_ty.clone()
                };
                self.emit_line(&format!(
                    "impl{} {}<{}> for {} {} {{",
                    generics, trait_path, rhs_arg, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!(
                    "type Output = {};",
                    if *magic == "__shl__" || *magic == "__shr__" {
                        // Shl/Shr 的 Output 可为任意类型，但 trait 默认 ()；显式声明
                        output_ty.clone()
                    } else {
                        output_ty.clone()
                    }
                ));
                self.emit_line(&format!(
                    "fn {}(self, rhs: {}) -> {} {{",
                    trait_method, rhs_arg, output_ty
                ));
                self.indent += 1;
                if rhs_is_ref {
                    self.emit_line(&format!("self.{}(&rhs)", magic));
                } else {
                    self.emit_line(&format!("self.{}(rhs)", magic));
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __pow__ —— 幂 `**` → lz_builtins::LzPow（std::ops 无对应 trait，LZ 自定义）：
        // 模式与算术运算符一致（委托 self.__pow__(rhs)）。数值 `**` 走内建 .pow()；
        // 用户 struct 定义 __pow__ 时生成 impl LzPow<Rhs>，使泛型场景可约束幂运算（06d §一）。
        if let Some(pm) = methods.iter().find(|m| m.name == "__pow__") {
            if !matches!(pm.ret_ty, IrType::Unit) {
                let where_str = self.magic_impl_where_str(pm);
                let rhs_is_ref = pm.params.get(1).map_or(false, |p| {
                    p.is_ref || matches!(&p.ty, IrType::Ref(_) | IrType::MutRef(_))
                });
                let rhs_ty = pm
                    .params
                    .get(1)
                    .map(|p| self.rust_type(&p.ty))
                    .unwrap_or_else(|| for_ty.to_string());
                let output_ty = self.rust_type(&pm.ret_ty);
                let rhs_arg = if rhs_is_ref {
                    format!("&{}", rhs_ty)
                } else {
                    rhs_ty.clone()
                };
                self.emit_line(&format!(
                    "impl{} lz_builtins::LzPow<{}> for {} {} {{",
                    generics, rhs_arg, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", output_ty));
                // trait 方法名 `pow` 与固有魔法名 `__pow__` 不同（仿 std::ops::Add::add），
                // impl 体内 `self.__pow__(rhs)` 唯一解析到固有方法，无同名递归（仿 Add 循环）。
                self.emit_line(&format!(
                    "fn pow(self, rhs: {}) -> {} {{",
                    rhs_arg, output_ty
                ));
                self.indent += 1;
                if rhs_is_ref {
                    self.emit_line("self.__pow__(&rhs)");
                } else {
                    self.emit_line("self.__pow__(rhs)");
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }

        // __buildparams__ → lz_builtins::BuildParams trait impl（06d §十三 构建块协议）：
        // `def __buildparams__(ref self) -> ArgsTuple` → impl BuildParams for Struct，
        // 使 struct 实例可作为构建块载荷（~: / *:）
        if let Some(bm) = methods.iter().find(|m| m.name == "__buildparams__") {
            let where_str = self.magic_impl_where_str(bm);
            let args_ty = self.rust_type(&bm.ret_ty);
            self.emit_line(&format!(
                "impl{} lz_builtins::BuildParams for {} {} {{",
                generics, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("type Args = {};", args_ty));
            self.emit_line("fn into_args(&self) -> Self::Args {");
            self.indent += 1;
            self.emit_line("self.__buildparams__()");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __guarded_pred__ + __guarded_action__ → lz_builtins::GuardedStrategy impl（06d §十 守卫策略）：
        // 两者配对生成：pred 判定是否执行兜底行为，action 执行兜底行为。
        let guarded_pred = methods.iter().find(|m| m.name == "__guarded_pred__");
        let guarded_action = methods.iter().find(|m| m.name == "__guarded_action__");
        if let (Some(gp), Some(ga)) = (guarded_pred, guarded_action) {
            // 提取 Input 类型（__guarded_pred__ 的非 self 参数类型）
            let input_ty = gp
                .params
                .iter()
                .find(|p| p.name != "self" && p.name != "self_")
                .map(|p| self.rust_type(&p.ty))
                .unwrap_or_else(|| "()".to_string());
            let output_ty = self.rust_type(&ga.ret_ty);
            let where_str = self.magic_impl_where_str(ga);

            self.emit_line(&format!(
                "impl{} lz_builtins::GuardedStrategy<{}> for {} {} {{",
                generics, input_ty, for_ty, where_str
            ));
            self.indent += 1;
            self.emit_line(&format!("type Output = {};", output_ty));
            self.emit_line(&format!("fn pred(&self, input: &{}) -> bool {{", input_ty));
            self.indent += 1;
            self.emit_line(&format!("self.__guarded_pred__(*input)"));
            self.indent -= 1;
            self.emit_line("}");
            self.emit_line(&format!(
                "fn action(self, input: {}) -> Self::Output {{",
                input_ty
            ));
            self.indent += 1;
            self.emit_line(&format!("self.__guarded_action__(input)"));
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // __call__ → Callable trait impl（06d §十一 可调用性）：
        // struct 实例作为闭包传给高阶函数。
        // 使用 lz_builtins::Callable<Args> trait（stable Rust 兼容）。
        if let Some(cm) = methods.iter().find(|m| m.name == "__call__") {
            let arg_types: Vec<String> = cm
                .params
                .iter()
                .filter(|p| p.name != "self" && p.name != "self_")
                .map(|p| self.rust_type(&p.ty))
                .collect();

            if !arg_types.is_empty() {
                let args_tuple = if arg_types.len() == 1 {
                    arg_types[0].clone()
                } else {
                    format!("({})", arg_types.join(", "))
                };
                let ret_ty = self.rust_type(&cm.ret_ty);
                let where_str = self.magic_impl_where_str(cm);
                let call_args: Vec<String> = if arg_types.len() == 1 {
                    cm.params
                        .iter()
                        .filter(|p| p.name != "self" && p.name != "self_")
                        .enumerate()
                        .map(|(i, _)| format!("args.{}", i))
                        .collect()
                } else {
                    cm.params
                        .iter()
                        .filter(|p| p.name != "self" && p.name != "self_")
                        .enumerate()
                        .map(|(i, _)| format!("args.0.{}", i))
                        .collect()
                };

                // 检测 inherent __call__ 的 self 是否为 mut（→ Rust &mut self）。
                // 若是，Callable trait 的 &self __call__ 无法直接调用 inherent 的
                // &mut self 方法，需在体内部克隆出可变副本来转发。
                let self_mut = cm
                    .params
                    .iter()
                    .any(|p| (p.name == "self" || p.name == "self_") && p.is_mut);
                // 全限定 inherent 调用所需的类型路径（含泛型时转 `Foo::<T>` 形式，
                // 避免 `Foo<T>::__call__` 非法语法）。for_ty 已含泛型后缀。
                let ty_path = if generics.is_empty() {
                    for_ty.to_string()
                } else {
                    let base = for_ty.strip_suffix(generics).unwrap_or(for_ty);
                    format!("{}::{}", base, generics)
                };

                self.emit_line(&format!(
                    "impl{} lz_builtins::Callable<({},)> for {} {} {{",
                    generics, args_tuple, for_ty, where_str
                ));
                self.indent += 1;
                self.emit_line(&format!("type Output = {};", ret_ty));
                self.emit_line(&format!(
                    "fn __call__(&self, args: ({},)) -> Self::Output {{",
                    args_tuple
                ));
                self.indent += 1;
                if self_mut {
                    // inherent __call__ 为 mut self（&mut self）：经 &self 的 Callable
                    // 无法调用 &mut self 方法，克隆可变副本后用「全限定 inherent 调用」
                    // 转发（Rust 方法解析会先承诺 &T 接收者，同名 trait 方法会抢走
                    // 解析，故必须显式 `Type::__call__(&mut copy, ..)` 绕过歧义）。
                    // Clone / 泛型 turbofish 由 struct derive 与 generics 保证。
                    self.emit_line("let mut __call_self = self.clone();");
                    self.emit_line(&format!(
                        "{}::__call__(&mut __call_self, {})",
                        ty_path,
                        call_args.join(", ")
                    ));
                } else {
                    self.emit_line(&format!("self.__call__({})", call_args.join(", ")));
                }
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
        }
    }

    pub(crate) fn magic_impl_where_str(&self, m: &FnDef) -> String {
        let w: String = m
            .where_clause
            .iter()
            .map(|(tp, bounds)| {
                let bs: Vec<String> = bounds.iter().map(|b| self.gen_trait_bound(b)).collect();
                format!("{}: {}", tp, bs.join(" + "))
            })
            .collect::<Vec<_>>()
            .join(", ");
        if w.is_empty() {
            String::new()
        } else {
            format!(" where {}", w)
        }
    }

    /// 判断表达式是否为 _KwArg（关键字参数）

    /// 魔法方法 → Rust 降级映射
    pub(crate) fn gen_magic_call(&self, kind: &MagicKind, args: &[Expr]) -> String {
        let gen_args = |a: &[Expr]| -> Vec<String> { a.iter().map(|e| self.gen_expr(e)).collect() };
        let args_s = gen_args(args);
        match kind {
            MagicKind::Call => {
                // __call__ → receiver(args...)
                if args_s.is_empty() {
                    "()".into()
                } else {
                    format!("{}({})", args_s[0], args_s[1..].join(", "))
                }
            }
            MagicKind::GetItem => {
                if args_s.len() >= 2 {
                    format!("{}[{}]", args_s[0], args_s[1])
                } else {
                    "()".into()
                }
            }
            MagicKind::SetItem => {
                if args_s.len() >= 3 {
                    format!("{}[{}] = {}", args_s[0], args_s[1], args_s[2])
                } else {
                    "()".into()
                }
            }
            MagicKind::Iter | MagicKind::IntoIter => {
                if args_s.is_empty() {
                    "().into_iter()".into()
                } else {
                    format!("{}.into_iter()", args_s[0])
                }
            }
            MagicKind::Next => {
                if args_s.is_empty() {
                    "None".into()
                } else {
                    format!("{}.next()", args_s[0])
                }
            }
            MagicKind::Display => {
                if args_s.is_empty() {
                    "\"\"".into()
                } else {
                    format!("{}.to_string()", args_s[0])
                }
            }
            MagicKind::Eq => {
                if args_s.len() >= 2 {
                    format!("{} == {}", args_s[0], args_s[1])
                } else {
                    "true".into()
                }
            }
            MagicKind::Cmp => {
                if args_s.len() >= 2 {
                    format!("{}.cmp(&{})", args_s[0], args_s[1])
                } else {
                    "std::cmp::Ordering::Equal".into()
                }
            }
            MagicKind::Drop => {
                if args_s.is_empty() {
                    "()".into()
                } else {
                    format!("drop({})", args_s[0])
                }
            }
            MagicKind::Add => {
                if args_s.len() >= 2 {
                    format!("{} + {}", args_s[0], args_s[1])
                } else {
                    args_s.first().cloned().unwrap_or_default()
                }
            }
            MagicKind::Sub => {
                if args_s.len() >= 2 {
                    format!("{} - {}", args_s[0], args_s[1])
                } else {
                    format!("-{}", args_s.first().cloned().unwrap_or_default())
                }
            }
            MagicKind::Mul => {
                if args_s.len() >= 2 {
                    format!("{} * {}", args_s[0], args_s[1])
                } else {
                    args_s.first().cloned().unwrap_or_default()
                }
            }
            MagicKind::Neg => {
                if args_s.is_empty() {
                    "0".into()
                } else {
                    format!("-{}", args_s[0])
                }
            }
            MagicKind::Not_ => {
                if args_s.is_empty() {
                    "false".into()
                } else {
                    format!("!{}", args_s[0])
                }
            }
            MagicKind::Len => {
                if args_s.is_empty() {
                    "0".into()
                } else {
                    format!("{}.len()", args_s[0])
                }
            }
            MagicKind::Rev => {
                if args_s.is_empty() {
                    "().into_iter().rev()".into()
                } else {
                    format!("{}.into_iter().rev()", args_s[0])
                }
            }
            MagicKind::SizeHint => {
                if args_s.is_empty() {
                    "(0, None)".into()
                } else {
                    format!("{}.size_hint()", args_s[0])
                }
            }
            MagicKind::IterStrategy => args_s.first().cloned().unwrap_or_else(|| "()".into()),
            MagicKind::UnpackBuildCall => args_s.first().cloned().unwrap_or_else(|| "()".into()),
        }
    }
}
