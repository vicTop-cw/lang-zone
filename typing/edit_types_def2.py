path = r'e:\IDEProjects\AI\lang-zone\src\types\def.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

old = '''            Type::Named(name) => name.clone(),

            Type::Generic { base, args } => {'''

new = '''            Type::Named(name) => name.clone(),

            Type::Constructor { name, .. } => name.clone(),

            Type::Apply { constructor, args } => {
                // 将构造器名按 Generic 规则映射为 Rust 容器名
                let is_pointer_ctx = matches!(constructor.as_ref(),
                    Type::Named(name) | Type::Constructor { name, .. }
                        if matches!(name.as_str(), "Box" | "Rc" | "Arc" | "Cell" | "RefCell")
                );
                let rust_base = match constructor.as_ref() {
                    Type::Named(name) | Type::Constructor { name, .. } => match name.as_str() {
                        "List" => "Vec",
                        "Dict" => "HashMap",
                        "Set" => "HashSet",
                        "Cell" => "std::cell::Cell",
                        "RefCell" => "std::cell::RefCell",
                        "Rc" => "std::rc::Rc",
                        "Arc" => "std::sync::Arc",
                        _ => name.as_str(),
                    },
                    other => {
                        let mapped = other.to_rust_type_string();
                        let args_s: Vec<String> = args.iter()
                            .map(|a| {
                                let s = a.to_rust_type_string();
                                if s.is_empty() { "_".to_string() } else { s }
                            })
                            .collect();
                        return format!("{}<{}>", mapped, args_s.join(", "));
                    }
                };
                let args_s: Vec<String> = args.iter()
                    .map(|a| {
                        let s = a.to_rust_type_string();
                        if s.is_empty() {
                            "_".to_string()
                        } else if is_pointer_ctx && needs_dyn(a) {
                            format!("dyn {}", s)
                        } else {
                            s
                        }
                    })
                    .collect();
                format!("{}<{}>", rust_base, args_s.join(", "))
            }

            Type::Generic { base, args } => {'''

if old not in content:
    print('OLD NOT FOUND')
    raise SystemExit(1)

content = content.replace(old, new)
with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('OK')
