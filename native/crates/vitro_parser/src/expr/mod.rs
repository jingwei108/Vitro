use super::*;

mod ops;
mod postfix;
mod primary;
mod unary;

impl Parser {
    pub(crate) fn parse_expression(&mut self) -> Expr {
        self.parse_comma()
    }

    pub(crate) fn parse_comma(&mut self) -> Expr {
        let mut left = self.parse_assign();
        while self.match_token(TokenType::Comma) {
            let right = self.parse_assign();
            let loc = SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            };
            left = Expr::Binary {
                op: BinaryOp::Comma,
                left: Box::new(left),
                right: Box::new(right),
                loc,
                ty: Type::default(),
            };
        }
        left
    }
}
