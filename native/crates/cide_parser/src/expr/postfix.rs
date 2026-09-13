use super::*;

impl Parser {
    pub(crate) fn parse_new_expr(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };
        let elem_type = self.parse_base_type();
        let mut final_type = elem_type.clone();
        while self.match_token(TokenType::Star) {
            final_type = Type::pointer_to(final_type);
        }

        let mut size_expr = None;
        let mut init = None;

        if self.match_token(TokenType::LBracket) {
            size_expr = Some(Box::new(self.parse_expression()));
            self.consume(TokenType::RBracket, "new[] 预期 ']'");
        }

        if self.match_token(TokenType::LParen) {
            if self.check(TokenType::RParen) {
                // Empty parens: treat as default constructor call
                init = Some(Box::new(Expr::Call {
                    name: format!("__ctor__{}", elem_type.name()),
                    args: Vec::new(),
                    loc,
                    ty: Type::void(),
                }));
            } else {
                let is_class = matches!(elem_type.kind(), TypeKind::Class | TypeKind::TemplateId);
                if is_class {
                    let ctor_args = self.parse_arg_list();
                    init = Some(Box::new(Expr::Call {
                        name: format!("__ctor__{}__{}", elem_type.name(), ctor_args.len()),
                        args: ctor_args,
                        loc,
                        ty: Type::void(),
                    }));
                } else {
                    init = Some(Box::new(self.parse_expression()));
                }
            }
            self.consume(TokenType::RParen, "new 初始化预期 ')'");
        }

        Expr::New {
            elem_type: final_type,
            size_expr,
            init,
            loc,
            ty: Type::pointer_to(elem_type),
        }
    }

    pub(crate) fn parse_delete_expr(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };
        let mut is_array = false;
        if self.check(TokenType::LBracket) {
            self.advance(); // [
            if self.check(TokenType::RBracket) {
                self.advance(); // ]
                is_array = true;
            } else {
                // 回退，当作普通表达式中的 [ 处理
                // 但这里不太可能，因为 delete [ 后面应该总是 ]
            }
        }
        let expr = Box::new(self.parse_unary());
        Expr::Delete {
            expr,
            is_array,
            loc,
            ty: Type::void(),
        }
    }

    pub(crate) fn parse_lambda_expr(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.current().line,
            column: self.current().column,
            file_id: 0,
        };
        self.consume(TokenType::LBracket, "Lambda 预期 '['");
        let mut capture = Vec::new();
        // 支持 []、[x]、[&x]、[=]、[&]、[a, &b]、[this]、[=, &a]、[&, a]
        if !self.check(TokenType::RBracket) {
            loop {
                if self.check(TokenType::Assign) {
                    self.advance(); // =
                    capture.push(CaptureMode::Implicit);
                } else if self.check(TokenType::Ampersand) {
                    self.advance(); // &
                    if self.check(TokenType::Identifier) {
                        let name = self.current().text.clone();
                        capture.push(CaptureMode::ByReference(name));
                        self.advance();
                    } else {
                        capture.push(CaptureMode::Implicit);
                    }
                } else if self.check(TokenType::Identifier) {
                    let name = self.current().text.clone();
                    capture.push(CaptureMode::ByValue(name));
                    self.advance();
                } else {
                    break;
                }
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RBracket, "Lambda 预期 ']'");
        self.consume(TokenType::LParen, "Lambda 预期 '('");
        let (params, _) = self.parse_param_list();
        self.consume(TokenType::RParen, "Lambda 预期 ')'");

        // 可选的返回类型 -> Type
        if self.check(TokenType::Minus) && self.peek(1).ty == TokenType::Gt {
            self.advance(); // -
            self.advance(); // >
            let _ret_type = self.parse_base_type();
            // 简化：忽略显式返回类型，由 TypeChecker 推断
        }

        let body = Box::new(self.parse_statement());
        let id = self.next_lambda_id;
        self.next_lambda_id += 1;
        Expr::Lambda {
            capture,
            params,
            body,
            unique_id: id,
            loc,
            ty: Type::default(),
        }
    }

    pub(crate) fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            if self.match_token(TokenType::LBracket) {
                let index = self.parse_expression();
                self.consume(TokenType::RBracket, "预期 ']'");
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                expr = Expr::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::LParen) {
                // Function call: direct named call or function pointer call
                let args = self.parse_arg_list();
                self.consume(TokenType::RParen, "预期 ')'");
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                expr = Expr::CallPtr {
                    callee: Box::new(expr),
                    args,
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Dot) || self.match_token(TokenType::Arrow) {
                let member_tok = self.consume(TokenType::Identifier, "预期成员名称").clone();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                // C++ 模式下检查是否是方法调用：obj.method(args)
                if self.is_cpp_mode && self.check(TokenType::LParen) {
                    self.advance(); // consume '('
                    let args = self.parse_arg_list();
                    self.consume(TokenType::RParen, "预期 ')'");
                    expr = Expr::MemberCall {
                        object: Box::new(expr),
                        method: member_tok.text,
                        args,
                        is_virtual: false,
                        resolved_mangled: None,
                        loc,
                        ty: Type::default(),
                    };
                } else {
                    expr = Expr::Member {
                        object: Box::new(expr),
                        member: member_tok.text,
                        loc,
                        ty: Type::default(),
                    };
                }
            } else if self.match_token(TokenType::Increment) {
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                expr = Expr::Unary {
                    op: UnaryOp::PostInc,
                    operand: Box::new(expr),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Decrement) {
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                expr = Expr::Unary {
                    op: UnaryOp::PostDec,
                    operand: Box::new(expr),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        expr
    }

pub(crate) fn parse_init_list(&mut self) -> Expr {
        // U1#8：本通道递归（初始化列表）不必然经过 parse_primary 的深度防护，
        // 独立挂 enter_depth——超限返回占位并跳到 EOF 让外层循环收敛。
        if !self.enter_depth("初始化列表") {
            return Expr::default();
        }
        let r = self.parse_init_list_body();
        self.leave_depth();
        r
    }

        fn parse_init_list_body(&mut self) -> Expr {
        let loc = self.current().clone();
        self.consume(TokenType::LBrace, "初始化列表预期 '{'");
        let mut elements = Vec::new();
        if !self.check(TokenType::RBrace) {
            loop {
                let mut designators = Vec::new();
                // Designated initializer: .field = value or [index] = value
                if self.match_token(TokenType::Dot) {
                    let field_tok = self.consume(TokenType::Identifier, "Designator 预期字段名").clone();
                    designators.push(Designator::Field(field_tok.text));
                    self.consume(TokenType::Assign, "Designated initializer 预期 '='");
                } else if self.match_token(TokenType::LBracket) {
                    let idx_expr = self.parse_assign();
                    self.consume(TokenType::RBracket, "Designator 预期 ']'");
                    designators.push(Designator::Index(Box::new(idx_expr)));
                    self.consume(TokenType::Assign, "Designated initializer 预期 '='");
                }
                let value = if self.check(TokenType::LBrace) {
                    self.parse_init_list()
                } else {
                    self.parse_assign()
                };
                elements.push(InitElement { designators, value });
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RBrace, "初始化列表预期 '}'");
        Expr::InitList {
            elements,
            loc: SourceLoc {
                line: loc.line,
                column: loc.column,
                file_id: 0,
            },
            ty: Type::default(),
        }
    }

    pub(crate) fn parse_arg_list(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        if self.check(TokenType::RParen) {
            return args;
        }
        loop {
            args.push(self.parse_assign());
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        args
    }
}
