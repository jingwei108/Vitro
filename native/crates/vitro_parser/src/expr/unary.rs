use super::*;

impl Parser {
pub(crate) fn parse_unary(&mut self) -> Expr {
        // U1#8：本通道递归（一元表达式）不必然经过 parse_primary 的深度防护，
        // 独立挂 enter_depth——超限返回占位并跳到 EOF 让外层循环收敛。
        if !self.enter_depth("一元") {
            return Expr::default();
        }
        let r = self.parse_unary_body();
        self.leave_depth();
        r
    }

        fn parse_unary_body(&mut self) -> Expr {
        if self.match_token(TokenType::Sizeof) {
            return self.parse_sizeof();
        }
        if self.match_token(TokenType::Alignof) {
            return self.parse_alignof();
        }
        if self.match_token(TokenType::Offsetof) {
            return self.parse_offsetof();
        }
        if self.is_cpp_mode && self.match_token(TokenType::New) {
            return self.parse_new_expr();
        }
        if self.is_cpp_mode && self.match_token(TokenType::Delete) {
            return self.parse_delete_expr();
        }
        if self.check(TokenType::LParen) {
            let checkpoint = self.save();
            let typedef_snapshot = self.typedef_names.clone();
            self.advance(); // consume '('
            if self.is_type_token() {
                let t = self.parse_type_only();
                // 若 ')' 后接 '{'，则是复合字面量，交给 parse_primary 处理
                if self.match_token(TokenType::RParen) && !self.check(TokenType::LBrace) {
                    let operand = self.parse_unary();
                    let loc = SourceLoc {
                        line: self.previous().line,
                        column: self.previous().column,
                        file_id: 0,
                    };
                    return Expr::Cast {
                        expr: Box::new(operand),
                        target_type: t.clone(),
                        loc,
                        ty: t,
                    };
                }
            }
            self.restore(checkpoint);
            self.typedef_names = typedef_snapshot;
        }
        if self.match_token(TokenType::Minus) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::Not) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::BitNot) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::BitNot,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::Ampersand) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::Addr,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::Star) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::Deref,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::Increment) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::PreInc,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::Decrement) {
            let operand = self.parse_unary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Unary {
                op: UnaryOp::PreDec,
                operand: Box::new(operand),
                loc,
                ty: Type::default(),
            };
        }
        self.parse_postfix()
    }

    pub(crate) fn parse_abstract_declarator(&mut self) -> Option<DeclaratorNode> {
        // U1#8：抽象声明符递归（sizeof(int(*)(*)…) 深链）防御性挂深度防护
        //（外部审查实锤 ~150 层即崩的通道；当前形状多被 primary 覆盖，此处兜底）。
        if !self.enter_depth("抽象声明符") {
            return None;
        }
        let r = self.parse_abstract_declarator_body();
        self.leave_depth();
        r
    }

    fn parse_abstract_declarator_body(&mut self) -> Option<DeclaratorNode> {
        let mut guard = DeclaratorGuard::default();
        let (node, _) = self.parse_declarator_node(&mut guard, true, true);
        if matches!(node, DeclaratorNode::Base) {
            None
        } else {
            Some(node)
        }
    }

    pub(crate) fn parse_sizeof(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };
        if self.match_token(TokenType::LParen) {
            let checkpoint = self.save();
            let mut is_type = false;
            let mut t = Type::default();
            if self.is_type_token() {
                t = self.parse_base_type();
                if let Some(node) = self.parse_abstract_declarator() {
                    t = Self::interpret_declarator_node(&node, &t);
                }
                if self.check(TokenType::RParen) {
                    is_type = true;
                }
            }
            if is_type {
                self.consume(TokenType::RParen, "sizeof(type) 后预期 ')'");
                return Expr::Sizeof {
                    target_type: Some(t),
                    operand: None,
                    loc,
                    ty: Type::int(),
                };
            }
            self.restore(checkpoint);
            let expr = self.parse_expression();
            self.consume(TokenType::RParen, "sizeof(expr) 后预期 ')'");
            return Expr::Sizeof {
                target_type: None,
                operand: Some(Box::new(expr)),
                loc,
                ty: Type::int(),
            };
        }
        let expr = self.parse_unary();
        Expr::Sizeof {
            target_type: None,
            operand: Some(Box::new(expr)),
            loc,
            ty: Type::int(),
        }
    }

    /// C23 `_Alignof` / `alignof`（E1）：结构与 parse_sizeof 同构（类型名/表达式双形态）。
    pub(crate) fn parse_alignof(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };
        if self.match_token(TokenType::LParen) {
            let checkpoint = self.save();
            let typedef_snapshot = self.typedef_names.clone();
            let mut is_type = false;
            let mut t = Type::default();
            if self.is_type_token() {
                t = self.parse_base_type();
                if let Some(node) = self.parse_abstract_declarator() {
                    t = Self::interpret_declarator_node(&node, &t);
                }
                if self.check(TokenType::RParen) {
                    is_type = true;
                }
            }
            if is_type {
                self.consume(TokenType::RParen, "alignof(type) 后预期 ')'");
                return Expr::Alignof {
                    target_type: Some(t),
                    operand: None,
                    loc,
                    ty: Type::int(),
                };
            }
            self.restore(checkpoint);
            self.typedef_names = typedef_snapshot;
            let expr = self.parse_expression();
            self.consume(TokenType::RParen, "alignof(expr) 后预期 ')'");
            return Expr::Alignof {
                target_type: None,
                operand: Some(Box::new(expr)),
                loc,
                ty: Type::int(),
            };
        }
        let expr = self.parse_unary();
        Expr::Alignof {
            target_type: None,
            operand: Some(Box::new(expr)),
            loc,
            ty: Type::int(),
        }
    }

    pub(crate) fn parse_offsetof(&mut self) -> Expr {
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };
        self.consume(TokenType::LParen, "offsetof 后预期 '('");
        let target_type = self.parse_base_type();
        self.consume(TokenType::Comma, "offsetof 参数之间预期 ','");
        let field_tok = self.consume(TokenType::Identifier, "offsetof 预期字段名").clone();
        self.consume(TokenType::RParen, "offsetof 后预期 ')'");
        Expr::Offsetof {
            target_type,
            field: field_tok.text,
            loc,
            ty: Type::int(),
        }
    }

    // =========================================================================
    // C++ Expression Parsers (Phase 31)
    // =========================================================================

    pub(crate) fn parse_type_only(&mut self) -> Type {
        let mut base = self.parse_base_type();
        if let Some(node) = self.parse_abstract_declarator() {
            base = Self::interpret_declarator_node(&node, &base);
        }
        base
    }
}
