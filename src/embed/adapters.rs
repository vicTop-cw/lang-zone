// Lang-Zone 编译器 — embed/adapters.rs
// 每门语言一个 adapter：摘源码 / 判 main / 摘顶层函数 / 生成分发入口 / 建库与运行命令。
// 新增语言只写这里，不写第二套 codegen —— 这是「统一成同样的抽象」的落点。

use super::{ArtifactKind, CommandSpec, EmbedLang, ForeignFn, ForeignParam};

/// Source 档（纯转译为 Rust 源码 / 原样内嵌）的语言 —— 本抽象**不接管**它们，
/// 它们必须继续走今天的逐字行为（tests/bridge_embed.rs 与 tests/bridge_e2e.rs 钉着）。
const SOURCE_LANGS: [&str; 3] = ["tnr", "rust", "rs"];

pub fn source_langs() -> &'static [&'static str] {
    &SOURCE_LANGS
}

pub fn is_source_lang(lang: &str) -> bool {
    SOURCE_LANGS.iter().any(|l| *l == lang)
}

/// 取 adapter：已知语言各有专属规则，其余一律落到通用 Cli adapter
pub fn adapter(lang: &str) -> Box<dyn EmbedLang + 'static> {
    match lang {
        "mojo" => Box::new(MojoLang),
        "python" | "py" => Box::new(PythonLang { canon: "python" }),
        "c" => Box::new(CLane),
        "scala" => Box::new(ScalaLang),
        other => Box::new(GenericLang {
            canon: other.to_string(),
        }),
    }
}

// ──────────────── 共用小工具 ────────────────

/// 只认**顶格**（零缩进）的定义行 —— 类/结构体内的方法不参与摘取（缩进即作用域）
fn toplevel_lines(src: &str) -> Vec<String> {
    src.lines()
        .filter(|l| !l.is_empty() && !l.starts_with(' ') && !l.starts_with('\t'))
        .map(|l| l.to_string())
        .collect()
}

/// 按顶层逗号切分参数（忽略 [] <> () 内的逗号）
fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur = String::new();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// 值是否含会打乱 argv 请求行的字符（换行已由分发入口转义，这里拦 NUL 与空串）
fn value_is_suspicious(t: &str) -> bool {
    t.contains('\0')
}

fn map_ty(table: &[(&str, &str)], ty: &str) -> Option<String> {
    table
        .iter()
        .find(|(k, _)| *k == ty)
        .map(|(_, v)| v.to_string())
}

/// 去掉 Mojo/C 的类型标注修饰词，取出裸类型名
fn strip_type_decor(ty: &str) -> String {
    let mut t = ty.trim().to_string();
    for pre in ["borrowed ", "consuming ", "owned ", "mut ", "const ", "unsigned ", "long "] {
        if t.starts_with(pre) {
            t = t[pre.len()..].to_string();
        }
    }
    t = t.replace(' ', "");
    while t.ends_with('*') {
        t.pop();
    }
    t
}

/// 一行文本里是否出现 `return <expr>`（关键字后必须有非空白内容；裸 `return` 不算）
fn line_has_value_return(s: &str) -> bool {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("return") {
        // 排除 `returns` 这类同前缀标识符
        if rest.is_empty() || rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return false;
        }
        return !rest.trim().is_empty();
    }
    false
}

/// 顶层定义的**函数体**里是否出现**带值的** `return <expr>`。
///
/// 为什么要有这个判据（T0r49 §一「文档没写、实测补上的那一刀」）：返回注解缺失时旧实现既不进
/// unsupported 也不报错 —— 函数被当成 Unit，垫片发成 `pub fn f(..)`（无返回值），
/// **调用成功、值被丢掉、没有任何回执**。`def f(): pass` 这类真 Unit 不受影响：
/// 只认「return 后面还有东西」的行。
///
/// 体 = 定义行之后的连续缩进行（允许空行），遇到第一条顶格非空行结束 —— 与「缩进即作用域」
/// 的摘取口径（`toplevel_lines`）一致；注释行（`#` 起头）不参与判定。
/// `inline_tail` = 定义行自身冒号之后的部分（覆盖 `def f(n: int): return [n]` 这种单行体，
/// 探针 p1 报的正是这一形态）。
fn body_has_value_return(lines: &[&str], def_idx: usize, inline_tail: &str) -> bool {
    // 定义行冒号之后的部分（`: return [n]` ⇒ 去掉前导 `:` 再判，探针 p1 报的正是这一形态）
    if line_has_value_return(inline_tail.trim().trim_start_matches(':').trim()) {
        return true;
    }
    let mut i = def_idx + 1;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        if !l.starts_with(' ') && !l.starts_with('\t') {
            break; // 回到顶格 ⇒ 出了这个函数的体
        }
        let t = l.trim();
        if t.starts_with('#') {
            i += 1;
            continue;
        }
        if line_has_value_return(t) {
            return true;
        }
        i += 1;
    }
    false
}

/// 摘取函数的公共文案：无返回注解却丢弃返回值（判据文本，MANIFEST/回执/测试按此串认）
pub const DISCARDED_RETURN: &str = "无返回注解且函数体有 return <expr> ⇒ 结果会被静默丢弃";

/// 无返回注解（`ret == None`）但体内有带值的 return ⇒ 置 unsupported
/// （已有更高优先级的点名原因时不覆盖 —— 保留先报的那条，不做二次归因）。
fn mark_discarded_return(
    ret: &Option<String>,
    lines: &[&str],
    def_idx: usize,
    inline_tail: &str,
    unsupported: &mut Option<String>,
) {
    if ret.is_none()
        && unsupported.is_none()
        && body_has_value_return(lines, def_idx, inline_tail)
    {
        *unsupported = Some(DISCARDED_RETURN.to_string());
    }
}

// ──────────────── Python ────────────────

const PY_TY: [(&str, &str); 17] = [
    ("int", "int"),
    ("float", "float"),
    ("str", "str"),
    ("bool", "bool"),
    ("List<int>", "list<int>"),
    ("List<float>", "list<float>"),
    ("List<str>", "list<str>"),
    ("Dict<str,int>", "dict<str,int>"),
    ("Dict<str,float>", "dict<str,float>"),
    ("Dict<str,str>", "dict<str,str>"),
    ("Dict<int,int>", "dict<int,int>"),
    ("Tuple<int,int>", "tuple<int,int>"),
    ("Tuple<int,str>", "tuple<int,str>"),
    ("Tuple<str,str>", "tuple<str,str>"),
    ("Tuple<int,int,int>", "tuple<int,int,int>"),
    ("Tuple<float,float>", "tuple<float,float>"),
    ("Tuple<bool,bool>", "tuple<bool,bool>"),
];

pub struct PythonLang {
    pub canon: &'static str,
}

impl EmbedLang for PythonLang {
    fn lang(&self) -> &str {
        self.canon
    }
    fn src_ext(&self) -> &str {
        "py"
    }

    fn has_main(&self, src: &str) -> bool {
        toplevel_lines(src).iter().any(|l| {
            let t = l.trim();
            t.starts_with("if __name__")
                || t.starts_with("def main(")
                || t.starts_with("async def main(")
        })
    }

    fn methods(&self, src: &str) -> Vec<ForeignFn> {
        let mut out = Vec::new();
        // 带下标遍历：判「返回值是否被丢弃」要看**定义行之后**的体，只留顶格行的话体就没了
        let all: Vec<&str> = src.lines().collect();
        for (idx, l) in all.iter().enumerate() {
            let l = *l;
            if l.is_empty() || l.starts_with(' ') || l.starts_with('\t') {
                continue;
            }
            let t = l.trim();
            let rest = if let Some(r) = t.strip_prefix("def ") {
                r
            } else if let Some(r) = t.strip_prefix("async def ") {
                r
            } else {
                continue;
            };
            let open = match rest.find('(') {
                Some(i) => i,
                None => continue,
            };
            let close = match rest.rfind(')') {
                Some(i) if i > open => i,
                _ => continue,
            };
            let name = rest[..open].trim().to_string();
            if name.is_empty() || name.starts_with('_') {
                continue;
            }
            let params = parse_annotated_params(&rest[open + 1..close], &PY_TY);
            let ret = match t[close + 1..].find("->") {
                Some(_) => {
                    let raw = t[close + 1..].split("->").nth(1).unwrap_or("");
                    let raw = raw.split('#').next().unwrap_or("").trim().trim_end_matches(':');
                    let m = map_ty(&PY_TY, raw);
                    if raw.is_empty() {
                        None
                    } else {
                        Some(m.unwrap_or_else(|| format!("?{}", raw)))
                    }
                }
                None => None,
            };
            let mut unsupported = None;
            if let Some(ref r) = ret {
                if r.starts_with('?') || value_is_suspicious(r) {
                    unsupported = Some(format!("返回类型 {} 不在闭集 int/float/str/bool/list<int>/list<float>/list<str>/dict<str,int>/dict<str,float>/dict<str,str>/dict<int,int> 内", r));
                }
            }
            for p in &params {
                if p.lz_type.starts_with('?') {
                    unsupported = Some(format!("参数 {} 的类型 {} 不在闭集内", p.name, p.lz_type));
                }
            }
            if params.iter().any(|p| p.name.is_empty()) {
                unsupported = Some("存在无名字形参".to_string());
            }
            if rest[open + 1..close].contains("**") {
                unsupported = Some("**kwargs 形参无法按 key=value 序列化".to_string());
            }
            // 无返回注解 + 体内带值 return ⇒ 静默丢弃，点名（T0r49 §一）
            // inline_tail 用 rest 下标（与 close 同尺）：`def f(n: int): return [n]` 的单行体就在这里。
            // 注：上面 ret 的解析用的是既有的 `t[close + 1..]`（带 `def ` 前缀的偏移，既有行为，不动它）。
            mark_discarded_return(&ret, &all, idx, &rest[close + 1..], &mut unsupported);
            out.push(ForeignFn {
                name,
                params,
                ret,
                unsupported,
            });
        }
        out
    }

    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)> {
        vec![(format!("{}.py", artifact), src.to_string())]
    }

    fn dispatch_file(&self, artifact: &str, methods: &[&ForeignFn]) -> Option<(String, String)> {
        // argv 里所有值都是字符串；按摘出的签名把形参转回声明类型，
        // 否则 python 侧 `"2" + "3" == "23"`（这条真在门禁里红过一次）
        let mut table = String::new();
        for m in methods.iter().filter(|m| m.callable()) {
            let items: Vec<String> = m
                .params
                .iter()
                .map(|p| {
                let conv = match p.lz_type.as_str() {
                    "int" => "int",
                    "float" => "float",
                    "bool" => "__lz_bool",
                    "list<int>" => "__lz_json_list_int",
                    "list<float>" => "__lz_json_list_float",
                    "list<str>" => "__lz_json_list_str",
                        "dict<str,int>" => "__lz_json_dict_str_int",
                        "dict<str,float>" => "__lz_json_dict_str_float",
                        "dict<str,str>" => "__lz_json_dict_str_str",
                        "dict<int,int>" => "__lz_json_dict_int_int",
                        "tuple<int,int>" => "__lz_json_tuple_int_int",
                    "tuple<int,str>" => "__lz_json_tuple_int_str",
                    "tuple<str,str>" => "__lz_json_tuple_str_str",
                    "tuple<int,int,int>" => "__lz_json_tuple_int_int_int",
                    "tuple<float,float>" => "__lz_json_tuple_float_float",
                    "tuple<bool,bool>" => "__lz_json_tuple_bool_bool",
                    _ => "str",
                };
                    format!("(\"{}\", {})", p.name, conv)
                })
                .collect();
            // 尾逗号必须有：单参数时 `(("x", float))` 只是那一个二元组本身，
            // 不是「参数列表」⇒ 解包时报 not enough values（双参数恰好蒙对，只有一条用例绿）
            table.push_str(&format!("    \"{}\": ({},),\n", m.name, items.join(", ")));
        }
        let names: Vec<String> = methods
            .iter()
            .filter(|m| m.callable())
            .map(|m| format!("        \"{}\",", m.name))
            .collect();
        // 为每个 callable 方法生成独立 arm（if/elif 链），每个 arm 内部有自己的 try/except
        let mut arms = String::new();
        for (i, m) in methods.iter().filter(|m| m.callable()).enumerate() {
            let ret_is_list = matches!(m.ret.as_deref(), Some("list<int>") | Some("list<float>") | Some("list<str>") | Some("dict<str,int>") | Some("dict<str,float>") | Some("dict<str,str>") | Some("dict<int,int>") | Some("tuple<int,int>") | Some("tuple<int,str>") | Some("tuple<str,str>") | Some("tuple<int,int,int>") | Some("tuple<float,float>") | Some("tuple<bool,bool>"));
            let branch = if i == 0 { "if" } else { "elif" };
            if ret_is_list {
                arms.push_str(&format!(
                    "    {} method == \"{}\":\n        try:\n            r = fn(**params)\n            _out_ok_json(\"\" if r is None else r)\n        except Exception as e:\n            _out(\"ERR\", type(e).__name__ + \": \" + str(e))\n        return\n",
                    branch, m.name
                ));
            } else {
                arms.push_str(&format!(
                    "    {} method == \"{}\":\n        try:\n            r = fn(**params)\n            _out(\"OK\", \"\" if r is None else r)\n        except Exception as e:\n            _out(\"ERR\", type(e).__name__ + \": \" + str(e))\n        return\n",
                    branch, m.name
                ));
            }
        }
        let body = format!(
            "# -*- coding: utf-8 -*-\n# {artifact} 的 CLI 分发入口 —— 由 lang-zone 生成，勿手改\n# 请求：argv = <method> [key=value ...]    响应：单行 `OK:<值>` / `ERR:<原因>`\nimport importlib\nimport sys\nimport json\n\nARTIFACT = {art_lit}\n\nKNOWN = (\n{names}\n)\n\nCOERCE = {{\n{table}}}\n\n\ndef __lz_bool(s):\n    return s in (\"true\", \"True\", \"1\")\n\n\ndef __lz_json_list_int(s):\n    return json.loads(s)\n\n\ndef __lz_json_list_float(s):\n    return json.loads(s)\n\n\ndef __lz_json_list_str(s):\n    return json.loads(s)\n\n\ndef __lz_json_dict_str_int(s):\n    return json.loads(s)\n\n\ndef __lz_json_dict_str_float(s):\n    return json.loads(s)\n\n\ndef __lz_json_dict_str_str(s):\n    return json.loads(s)\n\n\ndef __lz_json_dict_int_int(s):\n    d = json.loads(s)\n    return {{int(k): v for k, v in d.items()}}\n\n\ndef __lz_json_tuple_int_int(s):\n    return tuple(json.loads(s))\n\n\ndef __lz_json_tuple_int_str(s):\n    return tuple(json.loads(s))\n\n\ndef __lz_json_tuple_str_str(s):\n    return tuple(json.loads(s))\n\n\ndef __lz_json_tuple_int_int_int(s):\n    return tuple(json.loads(s))\n\n\ndef __lz_json_tuple_float_float(s):\n    return tuple(json.loads(s))\n\n\ndef __lz_json_tuple_bool_bool(s):\n    return tuple(json.loads(s))\n\n\ndef _out(code, text):\n    one = str(text).replace(chr(92), chr(92) + chr(92)).replace(chr(10), chr(92) + \"n\")\n    sys.stdout.write(\"%s:%s\\n\" % (code, one))\n    sys.stdout.flush()\n\n\ndef _out_ok_json(text):\n    one = json.dumps(text).replace(chr(92), chr(92) + chr(92)).replace(chr(10), chr(92) + \"n\")\n    sys.stdout.write(\"OK:%s\\n\" % one)\n    sys.stdout.flush()\n\n\ndef main():\n    argv = sys.argv[1:]\n    if not argv:\n        _out(\"ERR\", \"missing method\")\n        return\n    method = argv[0]\n    if method not in KNOWN:\n        _out(\"ERR\", \"no such method: \" + method)\n        return\n    params = {{}}\n    for tok in argv[1:]:\n        k, sep, v = tok.partition(\"=\")\n        if sep:\n            params[k] = v\n    for name, conv in COERCE.get(method, ()): \n        if name in params:\n            params[name] = conv(params[name])\n    mod = importlib.import_module(ARTIFACT)\n    fn = getattr(mod, method, None)\n    if fn is None:\n        _out(\"ERR\", \"not callable: \" + method)\n        return\n{arms}    _out(\"ERR\", \"no such method: \" + method)\n\n\nmain()\n",
            artifact = artifact,
            art_lit = format!("\"{}\"", artifact),
            names = names.join("\n"),
            table = table,
            arms = arms,
        );
        Some((format!("__lz_dispatch_{}.py", artifact), body))
    }

    fn build_cmd(&self, _kind: ArtifactKind, _entry: &str) -> Option<CommandSpec> {
        None // 解释型：建库步骤为空 —— 不是省略，是这档语言没有这一步
    }

    fn run_cmd(&self, kind: ArtifactKind, entry: &str) -> CommandSpec {
        match kind {
            ArtifactKind::Executable => CommandSpec::new("python", &["-B", entry]),
            ArtifactKind::Library => CommandSpec::new(
                "python",
                &[
                    "-B",
                    &format!(
                        "__lz_dispatch_{}.py",
                        entry.trim_end_matches(".py")
                    ),
                ],
            ),
        }
    }
}

fn parse_annotated_params(raw: &str, table: &[(&str, &str)]) -> Vec<ForeignParam> {
    let mut out = Vec::new();
    for part in split_top_commas(raw) {
        if part == "self" || part.starts_with("self:") || part.starts_with("*") {
            out.push(ForeignParam {
                name: String::new(),
                lz_type: format!("?{}", part),
            });
            continue;
        }
        let core = part.split('#').next().unwrap_or("").trim();
        let (n, ty) = match core.find(':') {
            Some(i) => (core[..i].trim(), core[i + 1..].trim()),
            None => (core, ""),
        };
        let ty = ty.split('=').next().unwrap_or("").trim();
        let lz = if ty.is_empty() {
            format!("?未标注")
        } else {
            map_ty(table, &strip_type_decor(ty)).unwrap_or_else(|| format!("?{}", ty))
        };
        out.push(ForeignParam {
            name: n.to_string(),
            lz_type: lz,
        });
    }
    out
}

// ──────────────── Mojo ────────────────

const MOJO_TY: [(&str, &str); 10] = [
    ("Int", "int"),
    ("Int64", "int"),
    ("Int32", "int"),
    ("UInt", "int"),
    ("UInt64", "int"),
    ("Float64", "float"),
    ("Float32", "float"),
    ("String", "str"),
    ("StringLiteral", "str"),
    ("Bool", "bool"),
];

pub struct MojoLang;

impl EmbedLang for MojoLang {
    fn lang(&self) -> &str {
        "mojo"
    }
    fn src_ext(&self) -> &str {
        "mojo"
    }

    fn has_main(&self, src: &str) -> bool {
        toplevel_lines(src)
            .iter()
            .any(|l| l.starts_with("fn main") || l.starts_with("@main"))
    }

    fn methods(&self, src: &str) -> Vec<ForeignFn> {
        fn_from_toplevel(src, &MOJO_TY, "fn ")
    }

    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)> {
        vec![(format!("{}.mojo", artifact), src.to_string())]
    }

    /// Mojo 无反射 ⇒ 分发入口按摘出的函数名生成**静态派发链**。
    /// 本机无 mojo 工具链时这段产物**未经编译验证**：MANIFEST 记 `dispatch_verified=no`，
    /// 报告与缺陷台账必须原样转述这一条，不许当成功交付。
    fn dispatch_file(&self, artifact: &str, methods: &[&ForeignFn]) -> Option<(String, String)> {
        let mut arms = String::new();
        for m in methods.iter().filter(|m| m.callable()) {
            if m.params.iter().any(|p| p.name.is_empty()) {
                continue;
            }
            let args: Vec<String> = m
                .params
                .iter()
                .map(|p| match p.lz_type.as_str() {
                    "int" => format!("Int64(__lz_arg(args, \"{}\"))", p.name),
                    "float" => format!("Float64(__lz_arg(args, \"{}\"))", p.name),
                    "bool" => format!("(__lz_arg(args, \"{}\") == \"true\")", p.name),
                    _ => format!("String(__lz_arg(args, \"{}\"))", p.name),
                })
                .collect();
            let call = format!("{}({})", m.name, args.join(", "));
            let body = match m.ret.as_deref() {
                Some("int") => format!("__lz_out(String({} as Int))", call),
                Some("float") => format!("__lz_out(String({} as Float))", call),
                Some("bool") => format!("__lz_out(String({}) as String)", call),
                None => format!("{}\n        __lz_out(\"\")", call),
                _ => format!("__lz_out_ok_str({})", call),
            };
            arms.push_str(&format!(
                "    if method == \"{}\":\n        {}\n        return\n",
                m.name, body
            ));
        }
        let body = format!(
            "// {artifact} 的 CLI 分发入口 —— 由 lang-zone 生成，勿手改\n// 请求：argv = <method> [key=value ...]    响应：单行 `OK:<值>` / `ERR:<原因>`\n// 状态：本机无 mojo 工具链 ⇒ 本文件未经编译验证（MANIFEST: dispatch_verified=no）\nfrom sys import args\n\nfn __lz_arg(all: List[String], key: String) -> String:\n    for i in range(len(all)):\n        let one = all[i]\n        if one.startswith(key + \"=\"):\n            return one[str(len(key) + 1 :)]\n    return \"\"\n\nfn __lz_out(s: String):\n    print(\"OK:\" + s)\n\nfn __lz_out_ok_str(s: String):\n    print(\"OK:\" + s)\n\nfn __lz_fail(s: String):\n    print(\"ERR:\" + s)\n\nfn main() raises:\n    if len(args) < 2:\n        __lz_fail(\"missing method\")\n        return\n    let method = String(args[1])\n{arms}    __lz_fail(\"no such method: \" + method)\n",
            artifact = artifact,
            arms = arms,
        );
        Some((format!("__lz_dispatch_{}.mojo", artifact), body))
    }

    fn build_cmd(&self, kind: ArtifactKind, entry: &str) -> Option<CommandSpec> {
        match kind {
            ArtifactKind::Executable => Some(CommandSpec::new("mojo", &["build", entry])),
            // 库产物：源目录即包（同语言程序 `import <artifact>` 直接可用），无建库步
            ArtifactKind::Library => None,
        }
    }

    fn run_cmd(&self, kind: ArtifactKind, entry: &str) -> CommandSpec {
        match kind {
            ArtifactKind::Executable => CommandSpec::new("mojo", &["run", entry]),
            ArtifactKind::Library => CommandSpec::new(
                "mojo",
                &[
                    "run",
                    &format!(
                        "__lz_dispatch_{}.mojo",
                        entry.trim_end_matches(".mojo")
                    ),
                ],
            ),
        }
    }
}

/// `fn name(params) -> Type` 形状（Mojo / Scala 共用）
fn fn_from_toplevel(src: &str, table: &[(&str, &str)], kw: &str) -> Vec<ForeignFn> {
    let mut out = Vec::new();
    let all: Vec<&str> = src.lines().collect();
    for (idx, l) in all.iter().enumerate() {
        let l = *l;
        if l.is_empty() || l.starts_with(' ') || l.starts_with('\t') {
            continue;
        }
        let t = l.trim();
        let rest = match t.strip_prefix(kw) {
            Some(r) => r,
            None => continue,
        };
        let name_end = match rest.find(|c: char| c == '(' || c == '[' || c == ' ') {
            Some(i) => i,
            None => continue,
        };
        let name = rest[..name_end].to_string();
        if name.is_empty() || name == "main" {
            continue;
        }
        let open = match rest.find('(') {
            Some(i) => i,
            None => continue,
        };
        let close = match rest.rfind(')') {
            Some(i) if i > open => i,
            _ => continue,
        };
        let params = parse_annotated_params(&rest[open + 1..close], table);
        let tail = &rest[close + 1..];
        let mut unsupported = None;
        if rest[..open].contains('[') {
            unsupported = Some("带类型参数的泛形函数不在闭集内".to_string());
        }
        let ret = if let Some(pos) = tail.rfind("->") {
            let raw = tail[pos + 2..]
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(':')
                .trim_end_matches("raises");
            let raw = raw.trim();
            if raw.is_empty() {
                None
            } else {
                let m = map_ty(table, &strip_type_decor(raw));
                if m.is_none() {
                    unsupported = Some(format!("返回类型 {} 不在闭集 int/float/str/bool 内", raw));
                }
                m
            }
        } else {
            None
        };
        for p in &params {
            if p.lz_type.starts_with('?') {
                unsupported = Some(format!("参数 {} 的类型 {} 不在闭集内", p.name, p.lz_type));
            }
        }
        // 与 python 同一把尺：无返回注解（`->` 之外没有别的返回形态可认）且体里有带值的
        // return ⇒ 值被静默丢弃，点名（T0r49 §一）。
        mark_discarded_return(&ret, &all, idx, tail, &mut unsupported);
        out.push(ForeignFn {
            name,
            params,
            ret,
            unsupported,
        });
    }
    out
}

// ──────────────── Scala ────────────────

const SCALA_TY: [(&str, &str); 6] = [
    ("Int", "int"),
    ("Long", "int"),
    ("Double", "float"),
    ("Float", "float"),
    ("String", "str"),
    ("Boolean", "bool"),
];

pub struct ScalaLang;

impl EmbedLang for ScalaLang {
    fn lang(&self) -> &str {
        "scala"
    }
    fn src_ext(&self) -> &str {
        "scala"
    }
    fn has_main(&self, src: &str) -> bool {
        src.lines().any(|l| {
            let t = l.trim();
            t.starts_with("@main") || t.contains("def main(") || t.contains("extends App")
        })
    }
    fn methods(&self, src: &str) -> Vec<ForeignFn> {
        // Scala 的顶层定义一般在 object 里（缩进）⇒ 只收顶格 def，object 内的由同语言程序自行导入
        fn_from_toplevel(src, &SCALA_TY, "def ")
    }
    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)> {
        vec![(format!("{}.scala", artifact), src.to_string())]
    }
    fn dispatch_file(&self, _artifact: &str, _methods: &[&ForeignFn]) -> Option<(String, String)> {
        None // JVM 侧入口需要 object 包装，本版本不生成 ⇒ 记账为不可调用，不出假垫片
    }
    fn build_cmd(&self, _kind: ArtifactKind, entry: &str) -> Option<CommandSpec> {
        Some(CommandSpec::new("scalac", &[entry]))
    }
    fn run_cmd(&self, kind: ArtifactKind, entry: &str) -> CommandSpec {
        match kind {
            ArtifactKind::Executable => CommandSpec::new("scala", &[entry]),
            ArtifactKind::Library => CommandSpec::new("scala", &[entry]),
        }
    }
}

// ──────────────── C ────────────────

const C_TY: [(&str, &str); 8] = [
    ("int", "int"),
    ("long", "int"),
    ("size_t", "int"),
    ("int64_t", "int"),
    ("double", "float"),
    ("float", "float"),
    ("bool", "bool"),
    ("_Bool", "bool"),
];

pub struct CLane;

impl EmbedLang for CLane {
    fn lang(&self) -> &str {
        "c"
    }
    fn src_ext(&self) -> &str {
        "c"
    }
    fn has_main(&self, src: &str) -> bool {
        toplevel_lines(src).iter().any(|l| {
            let t = l.trim();
            t.contains(" main(") || t.starts_with("int main")
        })
    }
    fn methods(&self, src: &str) -> Vec<ForeignFn> {
        let mut out = Vec::new();
        for l in toplevel_lines(src) {
            let t = l.trim();
            if t.starts_with("#") || t.starts_with("typedef") || t.starts_with("struct") {
                continue;
            }
            let open = match t.find('(') {
                Some(i) => i,
                None => continue,
            };
            let close = match t.rfind(')') {
                Some(i) if i > open => i,
                _ => continue,
            };
            let head = t[..open].trim();
            let name = match head.rfind(' ') {
                Some(i) => head[i + 1..].trim().to_string(),
                None => continue,
            };
            if name.is_empty() || name == "main" || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let ret_raw = head[..head.len() - name.len()].trim();
            let ret = if ret_raw == "void" {
                None
            } else {
                match map_ty(&C_TY, &strip_type_decor(ret_raw)) {
                    Some(v) => Some(v),
                    None => Some(format!("?{}", ret_raw)),
                }
            };
            let mut params = Vec::new();
            let mut unsupported = None;
            let praw = &t[open + 1..close];
            for part in split_top_commas(praw) {
                let one = part.trim();
                if one == "void" || one.is_empty() {
                    continue;
                }
                let pcore = one.split('=').next().unwrap_or("").trim();
                let pname = match pcore.rfind(|c: char| c.is_alphanumeric() || c == '_') {
                    Some(i) => {
                        let mut j = i;
                        while j > 0 && (pcore.as_bytes()[j - 1] as char).is_alphanumeric() == false
                            && (pcore.as_bytes()[j - 1] as char) != '_'
                        {
                            j -= 1;
                        }
                        pcore[j..].to_string()
                    }
                    None => String::new(),
                };
                let pty_raw = if pname.is_empty() {
                    pcore.to_string()
                } else {
                    pcore[..pcore.len() - pname.len()].trim().to_string()
                };
                match map_ty(&C_TY, &strip_type_decor(&pty_raw)) {
                    Some(lz) => params.push(ForeignParam {
                        name: pname,
                        lz_type: lz,
                    }),
                    None => {
                        params.push(ForeignParam {
                            name: pname,
                            lz_type: format!("?{}", pty_raw),
                        });
                        unsupported = Some(format!("参数类型 {} 不在闭集内（指针/结构体不支持）", pty_raw));
                    }
                }
            }
            if let Some(ref r) = ret {
                if r.starts_with('?') {
                    unsupported = Some(format!("返回类型 {} 不在闭集内", r));
                }
            }
            out.push(ForeignFn {
                name,
                params,
                ret,
                unsupported,
            });
        }
        out
    }
    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)> {
        vec![(format!("{}.c", artifact), src.to_string())]
    }
    fn dispatch_file(&self, artifact: &str, methods: &[&ForeignFn]) -> Option<(String, String)> {
        let mut disp = String::new();
        for m in methods.iter().filter(|m| m.callable()) {
            let mut pre = String::new();
            let mut cargs = Vec::new();
            let mut ok_shape = true;
            for p in &m.params {
                let (ctype, conv) = match p.lz_type.as_str() {
                    "int" => ("long", "atol"),
                    "float" => ("double", "atof"),
                    "bool" => ("int", "(int)__lz_is_true"),
                    _ => {
                        ok_shape = false;
                        break;
                    }
                };
                pre.push_str(&format!(
                    "        {} {} = {}(__lz_argv_get(argv, argc, \"{}\"));\n",
                    ctype, p.name, conv, p.name
                ));
                cargs.push(p.name.clone());
            }
            if !ok_shape || m.params.iter().any(|p| p.name.is_empty()) {
                continue;
            }
            let call = format!("{}({})", m.name, cargs.join(", "));
            let body = match m.ret.as_deref() {
                Some("int") => format!("printf(\"OK:%ld\\n\", (long)({}));", call),
                Some("float") => format!("printf(\"OK:%g\\n\", (double)({}));", call),
                Some("bool") => format!("printf(\"OK:%s\\n\", ({} ? \"true\" : \"false\"));", call),
                None => format!("{}; printf(\"OK:\\n\");", call),
                _ => "printf(\"ERR:unsupported return\\n\");".to_string(),
            };
            disp.push_str(&format!(
                "    if (strcmp(argv[1], \"{}\") == 0) {{\n{}        {}\n        return 0;\n    }}\n",
                m.name, pre, body
            ));
        }
        let src = format!(
            "/* {artifact} 的 CLI 分发入口 —— 由 lang-zone 生成，勿手改 */\n/* 请求：argv = <method> [key=value ...]    响应：单行 `OK:<值>` / `ERR:<原因>` */\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n\nstatic const char *__lz_argv_get(char **argv, int argc, const char *key) {{\n    size_t n = strlen(key);\n    for (int i = 2; i < argc; i++) {{\n        if (strncmp(argv[i], key, n) == 0 && argv[i][n] == '=') {{\n            return argv[i] + n + 1;\n        }}\n    }}\n    return \"\";\n}}\n\nstatic int __lz_is_true(const char *s) {{\n    return strcmp(s, \"1\") == 0 || strcmp(s, \"true\") == 0 || strcmp(s, \"True\") == 0;\n}}\n\nint main(int argc, char **argv) {{\n    if (argc < 2) {{\n        printf(\"ERR:missing method\\n\");\n        return 2;\n    }}\n{disp}    printf(\"ERR:no such method: %s\\n\", argv[1]);\n    return 3;\n}}\n",
            artifact = artifact,
            disp = disp,
        );
        Some((format!("__lz_dispatch_{}.c", artifact), src))
    }
    fn build_cmd(&self, kind: ArtifactKind, entry: &str) -> Option<CommandSpec> {
        let stem = entry.trim_end_matches(".c").to_string();
        match kind {
            ArtifactKind::Executable => Some(CommandSpec::new(
                "gcc",
                &["-O2", "-o", &format!("{}.exe", stem), entry],
            )),
            // 库产物：分发入口与源文件一起链成可执行 —— 这就是「建库编译」那一步
            ArtifactKind::Library => Some(CommandSpec::new(
                "gcc",
                &[
                    "-O2",
                    "-o",
                    &format!("{}.exe", stem),
                    &format!("__lz_dispatch_{}.c", stem),
                    entry,
                ],
            )),
        }
    }
    fn run_cmd(&self, _kind: ArtifactKind, entry: &str) -> CommandSpec {
        CommandSpec::new(&format!("./{}.exe", entry.trim_end_matches(".c")), &[])
    }
}

// ──────────────── 通用兜底（未知语言）────────────────

pub struct GenericLang {
    pub canon: String,
}

impl EmbedLang for GenericLang {
    fn lang(&self) -> &str {
        &self.canon
    }
    fn src_ext(&self) -> &str {
        // 未知语言没有可信的扩展名规则，也没有可信的 main 判据 ⇒ 只落盘，不出调用面
        "src"
    }
    fn has_main(&self, _src: &str) -> bool {
        false
    }
    fn methods(&self, _src: &str) -> Vec<ForeignFn> {
        vec![]
    }
    fn extract(&self, artifact: &str, src: &str) -> Vec<(String, String)> {
        vec![(format!("{}.{}", artifact, self.src_ext()), src.to_string())]
    }
    fn dispatch_file(&self, _artifact: &str, _methods: &[&ForeignFn]) -> Option<(String, String)> {
        None
    }
    fn build_cmd(&self, _kind: ArtifactKind, _entry: &str) -> Option<CommandSpec> {
        None
    }
    fn run_cmd(&self, _kind: ArtifactKind, entry: &str) -> CommandSpec {
        CommandSpec::new(&self.canon, &[entry])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_lib_and_main_split() {
        let a = adapter("mojo");
        assert!(!a.has_main("fn greet(name: String) -> String:\n    return name\n"));
        assert!(a.has_main("fn main() raises:\n    print(1)\n"));
        let ms = a.methods("fn add(a: Int64, b: Int64) -> Int64:\n    return a + b\n");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].name, "add");
        assert_eq!(ms[0].ret.as_deref(), Some("int"));
        assert!(ms[0].callable());
    }

    #[test]
    fn non_closed_set_types_are_recorded_not_guessed() {
        let a = adapter("mojo");
        let ms = a.methods("fn bad(x: DenseTensor) -> Int64:\n    return 1\n");
        assert_eq!(ms.len(), 1);
        assert!(!ms[0].callable());
        assert!(ms[0].unsupported.as_ref().unwrap().contains("DenseTensor"));
    }

    #[test]
    fn indented_methods_are_not_toplevel() {
        let a = adapter("python");
        let ms = a.methods("class C:\n    def m(self, x: int) -> int:\n        return x\n");
        assert!(ms.is_empty(), "类内方法不该被当顶层可调用面: {:?}", ms);
    }

    #[test]
    fn python_dispatch_names_artifact_module() {
        let a = adapter("python");
        let ms = a.methods("def greet(name: str) -> str:\n    return \"hi \" + name\n");
        let refs: Vec<&ForeignFn> = ms.iter().collect();
        let (path, body) = a.dispatch_file("python_input", &refs).unwrap();
        assert_eq!(path, "__lz_dispatch_python_input.py");
        assert!(body.contains("ARTIFACT = \"python_input\""));
        assert!(body.contains("\"greet\","));
    }

    #[test]
    fn unknown_lang_gets_no_callable_surface() {
        let t = super::super::resolve("cobol", "m.lz", "IDENTIFICATION DIVISION.\n").unwrap();
        assert!(t.methods.is_empty());
        assert!(!t.has_dispatch_file());
    }

    #[test]
    fn c_int_signatures_parse() {
        let a = adapter("c");
        let ms = a.methods("int add(int a, int b) { return a + b; }\n");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].params.len(), 2);
        assert_eq!(ms[0].ret.as_deref(), Some("int"));
        let p = a.methods("void go(int n) { }\n");
        assert_eq!(p[0].ret, None);
    }
}
