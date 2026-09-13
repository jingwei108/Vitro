//! 字符串与字符字面量解析。

use vitro_shared::ErrorCode;

use super::token::{LexerError, Token, TokenType};
use super::Lexer;

impl Lexer {
    pub(crate) fn string_literal(&mut self) -> Token {
        let start = self.pos;
        self.advance(); // consume opening "
        let mut value = String::new();
        while self.pos < self.chars.len() && self.peek(0) != '"' {
            if self.peek(0) == '\n' {
                self.errors.push(LexerError {
                    message: "字符串不能跨行".to_string(),
                    line: self.line,
                    column: self.column,
                    code: ErrorCode::E1003_StringCrossLine as i32,
                });
                break;
            }
            if self.peek(0) == '\\' && self.pos + 1 < self.chars.len() {
                let next = self.chars[self.pos + 1];
                match next {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    'a' => value.push('\x07'),
                    'b' => value.push('\x08'),
                    'f' => value.push('\x0C'),
                    'v' => value.push('\x0B'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '0' => value.push('\0'),
                    'x' => {
                        // \xHH hex escape
                        let h1 = self.peek(2);
                        let h2 = self.peek(3);
                        if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() {
                            let hex: String = self.chars[self.pos + 2..self.pos + 4].iter().collect();
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                value.push(byte as char);
                            }
                            self.advance();
                            self.advance();
                            self.advance();
                            self.advance();
                            continue;
                        }
                        value.push(next);
                    }
                    _ => value.push(next),
                }
                self.advance();
                self.advance();
            } else {
                let c = self.advance();
                value.push(c);
            }
        }
        if self.pos >= self.chars.len() || self.peek(0) != '"' {
            self.errors.push(LexerError {
                message: "字符串未闭合".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1002_UnterminatedString as i32,
            });
        } else {
            self.advance(); // consume closing "
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let mut tok = self.make_token(TokenType::String, &text);
        tok.text = value;
        tok
    }

    pub(crate) fn char_literal(&mut self) -> Token {
        let start = self.pos;
        self.advance(); // consume opening '
        let mut value = 0i32;
        let mut valid = true;
        if self.pos < self.chars.len() && self.peek(0) == '\'' {
            self.errors.push(LexerError {
                message: "空字符字面量".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1001_UnknownChar as i32,
            });
            valid = false;
        } else if self.pos < self.chars.len() && self.peek(0) == '\\' && self.pos + 1 < self.chars.len() {
            let next = self.chars[self.pos + 1];
            value = match next {
                'n' => '\n' as i32,
                't' => '\t' as i32,
                'r' => '\r' as i32,
                'a' => 0x07,
                'b' => 0x08,
                'f' => 0x0C,
                'v' => 0x0B,
                '\\' => '\\' as i32,
                '\'' => '\'' as i32,
                '0' => 0,
                'x' => {
                    // U1#7：\x 后 1~2 位 hex 均合法（C 标准任意位，教学子集
                    // 上限 2 位）；第 3 位仍是 hexdigit 即"超范围"报错（对齐
                    // clang "hex escape sequence out of range"）。修复前恰好
                    // 吃 2 位导致 '\x1' 单位被误拒、'\x4142' 报"未闭合"错乱。
                    // 本分支内直接消费完整转义序列（含错误路径的残段）。
                    let h1 = self.peek(2);
                    let h2 = self.peek(3);
                    if !h1.is_ascii_hexdigit() {
                        self.errors.push(LexerError {
                            message: "字符字面量十六进制转义缺少数字".to_string(),
                            line: self.line,
                            column: self.column,
                            code: ErrorCode::E1001_UnknownChar as i32,
                        });
                        valid = false;
                        0
                    } else {
                        let two = h2.is_ascii_hexdigit();
                        let after = if two { 4 } else { 3 };
                        if self.peek(after).is_ascii_hexdigit() {
                            let mut k = after;
                            while self.peek(k).is_ascii_hexdigit() {
                                k += 1;
                            }
                            for _ in 0..k {
                                self.advance();
                            }
                            self.errors.push(LexerError {
                                message: "十六进制转义序列超出范围（教学子集最多 2 位 hex）"
                                    .to_string(),
                                line: self.line,
                                column: self.column,
                                code: ErrorCode::E1006_UnsupportedFeature as i32,
                            });
                            valid = false;
                            0
                        } else {
                            let hex: String = self.chars[self.pos + 2..self.pos + after].iter().collect();
                            for _ in 0..after {
                                self.advance();
                            }
                            u8::from_str_radix(&hex, 16).unwrap_or(0) as i32
                        }
                    }
                }
                _ => {
                    self.errors.push(LexerError {
                        message: format!("未知字符转义: '\\{}'", next),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1001_UnknownChar as i32,
                    });
                    valid = false;
                    0
                }
            };
            // 消费 '\' 与转义字符；\x 的完整序列（\ x + hex 位）已在分支内
            // 消费（U1#7），此处只推进普通单字符转义
            if next != 'x' {
                self.advance();
                self.advance();
            }
        } else if self.pos < self.chars.len() {
            value = self.peek(0) as i32;
            self.advance();
        } else {
            self.errors.push(LexerError {
                message: "字符字面量未闭合".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1002_UnterminatedString as i32,
            });
            valid = false;
        }
        if self.pos < self.chars.len() && self.peek(0) == '\'' {
            self.advance();
        } else {
            self.errors.push(LexerError {
                message: "字符字面量未闭合".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1002_UnterminatedString as i32,
            });
            valid = false;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let mut tok = self.make_token(TokenType::CharLiteral, &text);
        if valid {
            tok.text = value.to_string();
        }
        tok
    }
}
