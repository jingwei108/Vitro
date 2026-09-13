//! 注释跳过逻辑。

use vitro_shared::ErrorCode;

use super::token::LexerError;
use super::Lexer;

impl Lexer {
    pub(crate) fn skip_comment(&mut self) {
        while self.pos < self.chars.len() && self.peek(0) != '\n' {
            self.advance();
        }
    }

    pub(crate) fn skip_block_comment(&mut self) {
        self.advance(); // '/'
        self.advance(); // '*'
        while self.pos < self.chars.len() {
            if self.peek(0) == '*' && self.peek(1) == '/' {
                self.advance();
                self.advance();
                return;
            }
            self.advance();
        }
        self.errors.push(LexerError {
            message: "块注释未闭合".to_string(),
            line: self.line,
            column: self.column,
            code: ErrorCode::E1010_UnterminatedComment as i32,
        });
    }
}
