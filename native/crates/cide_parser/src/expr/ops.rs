use super::*;

impl Parser {
pub(crate) fn parse_assign(&mut self) -> Expr {
        // U1#8：本通道递归（赋值表达式）不必然经过 parse_primary 的深度防护，
        // 独立挂 enter_depth——超限返回占位并跳到 EOF 让外层循环收敛。
        if !self.enter_depth("赋值") {
            return Expr::default();
        }
        let r = self.parse_assign_body();
        self.leave_depth();
        r
    }

        fn parse_assign_body(&mut self) -> Expr {
        let left = self.parse_ternary();
        let loc = SourceLoc {
            line: self.previous().line,
            column: self.previous().column,
            file_id: 0,
        };

        if self.match_token(TokenType::Assign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::Assign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::PlusAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::AddAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::MinusAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::SubAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::StarAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::MulAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::SlashAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::DivAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::PercentAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::ModAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::AndAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::AndAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::OrAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::OrAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::XorAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::XorAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::ShlAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::ShlAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        if self.match_token(TokenType::ShrAssign) {
            let right = self.parse_assign();
            return Expr::Assign {
                op: AssignOp::ShrAssign,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }

        left
    }

pub(crate) fn parse_ternary(&mut self) -> Expr {
        // U1#8：本通道递归（三目表达式）不必然经过 parse_primary 的深度防护，
        // 独立挂 enter_depth——超限返回占位并跳到 EOF 让外层循环收敛。
        if !self.enter_depth("三目") {
            return Expr::default();
        }
        let r = self.parse_ternary_body();
        self.leave_depth();
        r
    }

        fn parse_ternary_body(&mut self) -> Expr {
        let cond = self.parse_or();
        if self.match_token(TokenType::Question) {
            let then_branch = self.parse_ternary();
            self.consume(TokenType::Colon, "预期 ':'");
            let else_branch = self.parse_ternary();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            return Expr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                loc,
                ty: Type::default(),
            };
        }
        cond
    }

    pub(crate) fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.match_token(TokenType::OrOr) {
            let right = self.parse_and();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }

    pub(crate) fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_bit_or();
        while self.match_token(TokenType::AndAnd) {
            let right = self.parse_bit_or();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }

    pub(crate) fn parse_bit_or(&mut self) -> Expr {
        let mut left = self.parse_bit_xor();
        while self.match_token(TokenType::BitOr) {
            let right = self.parse_bit_xor();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::BitOr,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }

    pub(crate) fn parse_bit_xor(&mut self) -> Expr {
        let mut left = self.parse_bit_and();
        while self.match_token(TokenType::BitXor) {
            let right = self.parse_bit_and();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::BitXor,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }

    pub(crate) fn parse_bit_and(&mut self) -> Expr {
        let mut left = self.parse_equality();
        while self.match_token(TokenType::Ampersand) {
            let right = self.parse_equality();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::BitAnd,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }

    pub(crate) fn parse_equality(&mut self) -> Expr {
        let mut left = self.parse_relational();
        loop {
            if self.match_token(TokenType::Eq) {
                let right = self.parse_relational();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Eq,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Ne) {
                let right = self.parse_relational();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Ne,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        left
    }

    pub(crate) fn parse_relational(&mut self) -> Expr {
        let mut left = self.parse_shift();
        loop {
            if self.match_token(TokenType::Lt) {
                let right = self.parse_shift();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Lt,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Le) {
                let right = self.parse_shift();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Le,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Gt) {
                let right = self.parse_shift();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Gt,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Ge) {
                let right = self.parse_shift();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Ge,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        left
    }

    pub(crate) fn parse_shift(&mut self) -> Expr {
        let mut left = self.parse_additive();
        loop {
            if self.match_token(TokenType::Shl) {
                let right = self.parse_additive();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Shl,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Shr) {
                let right = self.parse_additive();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Shr,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        left
    }

    pub(crate) fn parse_additive(&mut self) -> Expr {
        let mut left = self.parse_multiplicative();
        loop {
            if self.match_token(TokenType::Plus) {
                let right = self.parse_multiplicative();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Minus) {
                let right = self.parse_multiplicative();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Sub,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        left
    }

    pub(crate) fn parse_multiplicative(&mut self) -> Expr {
        let mut left = self.parse_unary();
        loop {
            if self.match_token(TokenType::Star) {
                let right = self.parse_unary();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Slash) {
                let right = self.parse_unary();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Div,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else if self.match_token(TokenType::Percent) {
                let right = self.parse_unary();
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                left = Expr::Binary {
                    op: BinaryOp::Mod,
                    left: Box::new(left),
                    right: Box::new(right),
                    loc,
                    ty: Type::default(),
                };
            } else {
                break;
            }
        }
        left
    }
}
