use super::*;

impl Parser {
    pub(crate) fn parse_primary(&mut self) -> Expr {
        // F-P0-1 深度防护：括号/复合字面量互递归（5 万层 `((((` 曾直接栈溢出）
        if !self.enter_depth("表达式") {
            return Expr::Literal {
                value: 0,
                loc: SourceLoc::default(),
                ty: Type::int(),
            };
        }
        let expr = self.parse_primary_inner();
        self.leave_depth();
        expr
    }
    fn parse_primary_inner(&mut self) -> Expr {
        if self.match_token(TokenType::Number) {
            let prev = self.previous().clone();
            let value: i32 = prev.text.parse().unwrap_or_else(|_| {
                self.errors.push(ParseError {
                    message: format!("整数常量 '{}' 超出 int 表示范围", prev.text),
                    line: prev.line,
                    column: prev.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                0
            });
            let loc = SourceLoc {
                line: prev.line,
                column: prev.column,
                file_id: 0,
            };
            return Expr::Literal { value, loc, ty: Type::int() };
        }
        if self.match_token(TokenType::UnsignedLiteral) {
            let prev = self.previous().clone();
            // F-P0-2：此前 parse::<u32>() as i32 把 4000000000 截断为负数；
            // 现按值域分派：i32 内 → Literal，否则 → LongLiteral 保真
            let uv: u64 = prev.text.parse::<u64>().unwrap_or_else(|_| {
                self.errors.push(ParseError {
                    message: format!("unsigned 常量 '{}' 超出范围", prev.text),
                    line: prev.line,
                    column: prev.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                0
            });
            let loc = SourceLoc {
                line: prev.line,
                column: prev.column,
                file_id: 0,
            };
            if i32::try_from(uv).is_ok() {
                return Expr::Literal {
                    value: uv as i32,
                    loc,
                    ty: Type::unsigned_int(),
                };
            }
            return Expr::LongLiteral {
                value: uv as i64,
                loc,
                ty: Type::LongLong {
                    is_unsigned: true,
                    is_const: false,
                },
            };
        }
        if self.match_token(TokenType::LongLiteral) {
            let prev = self.previous().clone();
            let value: i64 = prev.text.parse().unwrap_or_else(|_| {
                self.errors.push(ParseError {
                    message: format!("long long 常量 '{}' 超出范围", prev.text),
                    line: prev.line,
                    column: prev.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                0
            });
            let loc = SourceLoc {
                line: prev.line,
                column: prev.column,
                file_id: 0,
            };
            return Expr::LongLiteral {
                value,
                loc,
                ty: Type::long_long(),
            };
        }
        if self.match_token(TokenType::FloatLiteral) {
            let prev = self.previous().clone();
            // C 标准（C89~C23 一致）：无后缀浮点字面量类型是 **double**，
            // 带 f/F 后缀才是 float（E1：此前一律建模为 float，DBL_MIN=2.2e-308
            // 经 f32 位模式存储下溢为 0，且与 Clang 的 epsilon 偏差源于此）
            let has_f_suffix = prev.text.ends_with('f') || prev.text.ends_with('F');
            let text = prev.text.trim_end_matches('f').trim_end_matches('F');
            let value: f64 = text.parse().unwrap_or_else(|_| {
                self.errors.push(ParseError {
                    message: format!("浮点常量 '{}' 格式无效", prev.text),
                    line: prev.line,
                    column: prev.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                0.0
            });
            let loc = SourceLoc {
                line: prev.line,
                column: prev.column,
                file_id: 0,
            };
            let ty = if has_f_suffix { Type::float() } else { Type::double() };
            return Expr::FloatLiteral { value, loc, ty };
        }
        if self.match_token(TokenType::CharLiteral) {
            let prev = self.previous().clone();
            let value: i32 = prev.text.parse().unwrap_or_else(|_| {
                self.errors.push(ParseError {
                    message: format!("字符常量 '{}' 解析失败", prev.text),
                    line: prev.line,
                    column: prev.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                0
            });
            let loc = SourceLoc {
                line: prev.line,
                column: prev.column,
                file_id: 0,
            };
            return Expr::Literal { value, loc, ty: Type::char() };
        }
        if self.match_token(TokenType::String) {
            // C89 相邻字符串字面量拼接（E1 B 档）："ab" "cd" → "abcd"
            let mut value = self.previous().text.clone();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            while self.check(TokenType::String) {
                self.advance();
                value.push_str(&self.previous().text);
            }
            let array_size = value.len() as i32 + 1; // including null terminator
            return Expr::StringLiteral {
                value,
                loc,
                ty: Type::Array {
                    element: Box::new(Type::char()),
                    array_size,
                    dims: vec![array_size],
                    is_const: false,
                    is_vla: false,
                    vla_dims: vec![],
                },
            };
        }
        if self.match_token(TokenType::Null) {
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Literal {
                value: 0,
                loc,
                ty: Type::pointer_to(Type::void()),
            };
        }
        if self.match_token(TokenType::Generic) {
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            self.consume(TokenType::LParen, "_Generic 后预期 '('");
            let control = self.parse_assign();
            self.consume(TokenType::Comma, "_Generic 参数之间预期 ','");
            let mut associations = Vec::new();
            let mut default = None;
            while !self.check(TokenType::RParen) && !self.is_at_end() {
                if self.match_token(TokenType::Default) {
                    self.consume(TokenType::Colon, "default 后预期 ':'");
                    let expr = self.parse_assign();
                    default = Some(Box::new(expr));
                } else {
                    let assoc_type = self.parse_type_only();
                    self.consume(TokenType::Colon, "类型关联后预期 ':'");
                    let expr = self.parse_assign();
                    associations.push((assoc_type, expr));
                }
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
            self.consume(TokenType::RParen, "_Generic 后预期 ')'");
            return Expr::Generic {
                control: Box::new(control),
                associations,
                default,
                loc,
                ty: Type::default(),
            };
        }
        if self.is_cpp_mode && self.match_token(TokenType::This) {
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::This { loc, ty: Type::default() };
        }
        if self.is_cpp_mode && self.check(TokenType::LBracket) {
            return self.parse_lambda_expr();
        }
        if self.check(TokenType::Identifier) && self.peek(0).text == "__asm__" {
            // 支持 GCC 风格内联汇编占位：__asm__("...")
            // 教学子集不执行汇编指令，仅消费语法并返回 void 字面量。
            let name_tok = self.advance().clone();
            self.consume(TokenType::LParen, "__asm__ 后预期 '('");
            self.consume(TokenType::String, "__asm__ 预期汇编字符串");
            self.consume(TokenType::RParen, "__asm__ 预期 ')'");
            let loc = SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            };
            return Expr::Literal {
                value: 0,
                loc,
                ty: Type::void(),
            };
        }
        if self.check(TokenType::Identifier) {
            let name_tok = self.advance().clone();
            let mut name = name_tok.text.clone();
            if self.is_cpp_mode && self.check(TokenType::ColonColon) {
                self.advance(); // ::
                let inner = self.consume(TokenType::Identifier, ":: 后预期标识符").clone();
                name = format!("{}__{}", name, inner.text);
            }
            let loc = SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            };
            return Expr::Identifier { name, loc, ty: Type::default() };
        }
        if self.match_token(TokenType::LParen) {
            let lparen_loc = self.previous().clone();
            let checkpoint = self.save();
            let typedef_snapshot = self.typedef_names.clone();
            // 尝试解析复合字面量 (type-name) { initializer-list }
            if self.is_type_token() {
                let t = self.parse_type_only();
                if self.match_token(TokenType::RParen) && self.check(TokenType::LBrace) {
                    let init = self.parse_init_list();
                    let loc = SourceLoc {
                        line: lparen_loc.line,
                        column: lparen_loc.column,
                        file_id: 0,
                    };
                    return Expr::CompoundLiteral {
                        target_type: t.clone(),
                        init: Box::new(init),
                        loc,
                        ty: t,
                    };
                }
            }
            self.restore(checkpoint);
            self.typedef_names = typedef_snapshot;
            let expr = self.parse_expression();
            self.consume(TokenType::RParen, "预期 ')'");
            return expr;
        }
        self.errors.push(ParseError {
            message: "预期表达式".to_string(),
            line: self.current().line,
            column: self.current().column,
            code: ErrorCode::E2003_ExpectedExpr as i32,
        });
        let loc = SourceLoc {
            line: self.current().line,
            column: self.current().column,
            file_id: 0,
        };
        // 消费当前 token，防止外层 parse_statement 在相同位置无限循环。
        if !self.is_at_end() {
            self.advance();
        }
        Expr::Literal { value: 0, loc, ty: Type::int() }
    }
    // =========================================================================
    // Expressions (precedence climbing)
    // =========================================================================
}
