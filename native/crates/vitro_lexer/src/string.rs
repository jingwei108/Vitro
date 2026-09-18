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
                    '0'..='7' => {
                        // P4（2026-09-18）：八进制转义 \ooo（1~3 位，值 ≤ 0xFF）。
                        // 修复前仅字面 `\0`——"\012" 变 NUL+"12"（sizeof 6，
                        // clang 4）。超范围对齐 clang error 口径。
                        let mut digits = 1usize;
                        let mut val = next.to_digit(8).unwrap();
                        while digits < 3 && self.peek(1 + digits).is_digit(8) {
                            val = val * 8 + self.peek(1 + digits).to_digit(8).unwrap();
                            digits += 1;
                        }
                        if val > 0xFF {
                            self.errors.push(LexerError {
                                message: "八进制转义序列超出范围（教学子集最多 3 位且值 ≤ 0xFF）".to_string(),
                                line: self.line,
                                column: self.column,
                                code: ErrorCode::E1006_UnsupportedFeature as i32,
                            });
                        }
                        value.push((val & 0xFF) as u8 as char);
                        for _ in 0..=digits {
                            self.advance();
                        }
                        continue;
                    }
                    'x' => {
                        // P4（2026-09-18）：hex 1~2 位（对齐 char 侧 U1#7 与
                        // clang）；修复前恰 2 位——"\x4" 单位被拆成字面 "x4"
                        //（sizeof 3，clang 2）。第 3 位 hexdigit 报"超出范围"
                        //（clang error 口径，文案与 char 侧一致）。
                        let h1 = self.peek(2);
                        if !h1.is_ascii_hexdigit() {
                            self.errors.push(LexerError {
                                message: "字符串十六进制转义缺少数字".to_string(),
                                line: self.line,
                                column: self.column,
                                code: ErrorCode::E1001_UnknownChar as i32,
                            });
                            value.push('x');
                        } else {
                            let two = self.peek(3).is_ascii_hexdigit();
                            let after = if two { 4 } else { 3 };
                            if self.peek(after).is_ascii_hexdigit() {
                                let mut k = after;
                                while self.peek(k).is_ascii_hexdigit() {
                                    k += 1;
                                }
                                self.errors.push(LexerError {
                                    message: "十六进制转义序列超出范围（教学子集最多 2 位 hex）".to_string(),
                                    line: self.line,
                                    column: self.column,
                                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                                });
                                for _ in 0..k {
                                    self.advance();
                                }
                                continue;
                            }
                            let hex: String = self.chars[self.pos + 2..self.pos + after].iter().collect();
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                value.push(byte as char);
                            }
                            for _ in 0..after {
                                self.advance();
                            }
                            continue;
                        }
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
                '0'..='7' => {
                    // P4（2026-09-18）：八进制转义 \ooo（1~3 位，值 ≤ 0xFF），
                    // 与 string 侧同口径（char 侧此前仅字面 `\0`，'\7' 报
                    // "未知字符转义"）。超范围对齐 clang error。本分支内直接
                    // 消费完整转义序列（含错误路径的残段）。
                    let mut digits = 1usize;
                    let mut val = next.to_digit(8).unwrap();
                    while digits < 3 && self.peek(1 + digits).is_digit(8) {
                        val = val * 8 + self.peek(1 + digits).to_digit(8).unwrap();
                        digits += 1;
                    }
                    let v = if val > 0xFF {
                        self.errors.push(LexerError {
                            message: "八进制转义序列超出范围（教学子集最多 3 位且值 ≤ 0xFF）".to_string(),
                            line: self.line,
                            column: self.column,
                            code: ErrorCode::E1006_UnsupportedFeature as i32,
                        });
                        valid = false;
                        0
                    } else {
                        val as i32
                    };
                    for _ in 0..=digits {
                        self.advance();
                    }
                    v
                }
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
            // 消费 '\' 与转义字符；\x 与八进制的完整序列已在分支内消费
            //（U1#7 / P4），此处只推进普通单字符转义
            if next != 'x' && !next.is_digit(8) {
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
