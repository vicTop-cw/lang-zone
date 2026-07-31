path = r'e:\IDEProjects\AI\lang-zone\src\parser\parser.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

old_func = '''    /// 解析泛型参数，同时捕获定义点约束和默认值
    fn parse_generic_params_rich(&mut self) -> Result<(Vec<String>, Vec<(String, Vec<Type>)>, Vec<(String, Type)>), String> {
        self.expect(Token::Lt)?;
        let mut names = Vec::new();
        let mut bounds: Vec<(String, Vec<Type>)> = Vec::new();
        let mut defaults: Vec<(String, Type)> = Vec::new();

        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };

            // T: Clone + Ord → 定义点约束
            if self.check(&Token::Colon) {
                self.advance();
                let mut b = Vec::new();
                loop {
                    b.push(self.parse_type()?);
                    if self.check(&Token::Plus) { self.advance(); } else { break; }
                }
                bounds.push((name.clone(), b));
            }

            // T = i64 → 默认类型参数
            if self.check(&Token::Eq) {
                self.advance();
                let default = self.parse_type()?;
                defaults.push((name.clone(), default));
            }

            names.push(name);'''

new_func = '''    /// 解析泛型参数，同时捕获定义点约束、默认值和 kind（HKT）。
    /// F[_] 表示 kind * -> *。
    fn parse_generic_params_rich(&mut self) -> Result<(Vec<String>, Vec<(String, Kind)>, Vec<(String, Vec<Type>)>, Vec<(String, Type)>), String> {
        self.expect(Token::Lt)?;
        let mut names = Vec::new();
        let mut kinds: Vec<(String, Kind)> = Vec::new();
        let mut bounds: Vec<(String, Vec<Type>)> = Vec::new();
        let mut defaults: Vec<(String, Type)> = Vec::new();

        loop {
            let name = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(format!("Expected generic param, got {:?}", t)),
            };

            // HKT kind 标注: F[_] → * -> *
            if self.check(&Token::LBrack) {
                self.advance();
                let mut arity = 0;
                loop {
                    if self.check(&Token::Underscore) {
                        self.advance();
                        arity += 1;
                    } else if self.check(&Token::Ident(_)) {
                        // 也允许 F[T] 这样写（T 被忽略，仅计 arity）
                        self.advance();
                        arity += 1;
                    } else {
                        break;
                    }
                    if self.check(&Token::Comma) {
                        self.advance();
                        continue;
                    }
                    break;
                }
                self.expect(Token::RBrack)?;
                if arity == 0 {
                    return Err(format!("kind 标注 [{}] 至少需要一个占位符", name));
                }
                let mut params = Vec::new();
                for _ in 0..arity { params.push(Kind::Star); }
                kinds.push((name.clone(), Kind::Arrow { params, ret: Box::new(Kind::Star) }));
            }

            // T: Clone + Ord → 定义点约束
            if self.check(&Token::Colon) {
                self.advance();
                let mut b = Vec::new();
                loop {
                    b.push(self.parse_type()?);
                    if self.check(&Token::Plus) { self.advance(); } else { break; }
                }
                bounds.push((name.clone(), b));
            }

            // T = i64 → 默认类型参数
            if self.check(&Token::Eq) {
                self.advance();
                let default = self.parse_type()?;
                defaults.push((name.clone(), default));
            }

            names.push(name);'''

if old_func not in content:
    print('FUNC OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_func, new_func)

# Update return statement at end of parse_generic_params_rich
old_ret = '''        Ok((names, bounds, defaults))
    }

    fn parse_params(&mut self) -> Result<(Vec<Param>, Option<VariadicSpec>), String> {'''
new_ret = '''        Ok((names, kinds, bounds, defaults))
    }

    fn parse_params(&mut self) -> Result<(Vec<Param>, Option<VariadicSpec>), String> {'''
if old_ret not in content:
    print('RET OLD NOT FOUND')
    raise SystemExit(1)
content = content.replace(old_ret, new_ret)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('OK')
