//! 数字字面量解析（整数、浮点数、后缀处理）。

use cide_shared::ErrorCode;

use super::token::{LexerError, Token, TokenType};
use super::Lexer;

impl Lexer {
    pub(crate) fn number(&mut self) -> Token {
        let start = self.pos;
        // 0x/0b/0八进制 前缀已按整数求值（不再走浮点/十进制解析路径）
        let mut has_integer_prefix = false;
        let mut val: u64 = 0;

        // Hexadecimal literal: 0x + hexdigit，允许 ' 数字分隔符（C23）
        if self.peek(0) == '0' && (self.peek(1) == 'x' || self.peek(1) == 'X') {
            has_integer_prefix = true;
            self.advance(); // '0'
            self.advance(); // 'x' or 'X'
            let hex_start = self.pos;
            while self.pos < self.chars.len() {
                let c = self.peek(0);
                if c.is_ascii_hexdigit() {
                    self.advance();
                } else if c == '\'' && self.peek(1).is_ascii_hexdigit() {
                    self.advance(); // 分隔符不进值
                } else {
                    break;
                }
            }
            if self.pos == hex_start {
                self.errors.push(LexerError {
                    message: "十六进制数字格式错误".to_string(),
                    line: self.line,
                    column: self.column,
                    code: ErrorCode::E1001_UnknownChar as i32,
                });
                return self.make_token(TokenType::Unknown, "0x");
            }
            let hex_str: String = self.chars[hex_start..self.pos].iter().filter(|&&c| c != '\'').collect();
            match u64::from_str_radix(&hex_str, 16) {
                Ok(v) => val = v,
                Err(_) => {
                    self.errors.push(LexerError {
                        message: format!("十六进制数值 0x{} 超出可表示范围", hex_str),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1006_UnsupportedFeature as i32,
                    });
                    return self.make_token(TokenType::Number, "0");
                }
            }
        }
        // Binary literal（C23）: 0b + [01]，允许 ' 数字分隔符
        else if self.peek(0) == '0' && (self.peek(1) == 'b' || self.peek(1) == 'B') {
            has_integer_prefix = true;
            self.advance(); // '0'
            self.advance(); // 'b' or 'B'
            let bin_start = self.pos;
            while self.pos < self.chars.len() {
                let c = self.peek(0);
                if c == '0' || c == '1' {
                    self.advance();
                } else if c == '\'' && (self.peek(1) == '0' || self.peek(1) == '1') {
                    self.advance(); // 分隔符不进值
                } else {
                    break;
                }
            }
            if self.pos == bin_start {
                self.errors.push(LexerError {
                    message: "二进制数字格式错误".to_string(),
                    line: self.line,
                    column: self.column,
                    code: ErrorCode::E1001_UnknownChar as i32,
                });
                return self.make_token(TokenType::Unknown, "0b");
            }
            let bin_str: String = self.chars[bin_start..self.pos].iter().filter(|&&c| c != '\'').collect();
            match u64::from_str_radix(&bin_str, 2) {
                Ok(v) => val = v,
                Err(_) => {
                    self.errors.push(LexerError {
                        message: format!("二进制数值 0b{} 超出可表示范围", bin_str),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1006_UnsupportedFeature as i32,
                    });
                    return self.make_token(TokenType::Number, "0");
                }
            }
        }
        // Octal literal: 0[0-7]+
        else if self.peek(0) == '0' && self.peek(1).is_ascii_digit() {
            has_integer_prefix = true;
            self.advance(); // '0'
            let oct_start = self.pos;
            while self.pos < self.chars.len() && self.peek(0) >= '0' && self.peek(0) <= '7' {
                self.advance();
            }
            if self.pos > oct_start {
                let oct_str: String = self.chars[oct_start..self.pos].iter().collect();
                val = u64::from_str_radix(&oct_str, 8).unwrap_or(0);
            }
        }
        // Decimal literal，允许 ' 数字分隔符（C23，如 1'000'000）
        else {
            while self.pos < self.chars.len() {
                let c = self.peek(0);
                if c.is_ascii_digit() {
                    self.advance();
                } else if c == '\'' && self.peek(1).is_ascii_digit() {
                    self.advance(); // 分隔符不进值
                } else {
                    break;
                }
            }
        }

        // check for float literal (3.14 / 3.14f / 3.f / 1e5 / 1.5e-3)
        // 注意：检查必须在十进制数字扫描**之后**——此时 peek 停在 '.' 或 'e' 上。
        // 指数部分（C89）此前缺失，float.h 的 DBL_EPSILON=2.22e-16 等宏全部解析失败
        let dot_float = !has_integer_prefix
            && self.peek(0) == '.'
            && (self.peek(1).is_ascii_digit() || self.peek(1) == 'f' || self.peek(1) == 'F');
        let exp_float = !has_integer_prefix
            && (self.peek(0) == 'e' || self.peek(0) == 'E')
            && (self.peek(1).is_ascii_digit()
                || ((self.peek(1) == '+' || self.peek(1) == '-') && self.peek(2).is_ascii_digit()));
        if dot_float || exp_float {
            if dot_float {
                self.advance(); // '.'
                while self.pos < self.chars.len() && self.peek(0).is_ascii_digit() {
                    self.advance();
                }
            }
            // 指数部分可在小数点之后链式出现（2.2e-3），此处以当前 peek 重新判定
            if (self.peek(0) == 'e' || self.peek(0) == 'E')
                && (self.peek(1).is_ascii_digit()
                    || ((self.peek(1) == '+' || self.peek(1) == '-') && self.peek(2).is_ascii_digit()))
            {
                self.advance(); // 'e' / 'E'
                if self.peek(0) == '+' || self.peek(0) == '-' {
                    self.advance();
                }
                while self.pos < self.chars.len() && self.peek(0).is_ascii_digit() {
                    self.advance();
                }
            }
            // Optional float suffix: f/F
            if self.peek(0) == 'f' || self.peek(0) == 'F' {
                self.advance();
            }
            let text: String = self.chars[start..self.pos].iter().collect();
            return self.make_token(TokenType::FloatLiteral, &text);
        }

        // For decimal, compute value now (before suffix parsing)
        if !has_integer_prefix {
            let text: String = self.chars[start..self.pos].iter().filter(|&&c| c != '\'').collect();
            val = text.parse::<u64>().unwrap_or(0);
        }

        // Parse suffix: [Uu][Ll]? or [Ll][Uu]?
        let mut has_u = false;
        let mut has_l = false;

        if self.peek(0) == 'U' || self.peek(0) == 'u' {
            has_u = true;
            self.advance();
            if self.peek(0) == 'L' || self.peek(0) == 'l' {
                has_l = true;
                self.advance();
                if self.peek(0) == 'L' || self.peek(0) == 'l' {
                    self.advance();
                }
            }
        } else if self.peek(0) == 'L' || self.peek(0) == 'l' {
            has_l = true;
            self.advance();
            if self.peek(0) == 'L' || self.peek(0) == 'l' {
                self.advance();
            }
            if self.peek(0) == 'U' || self.peek(0) == 'u' {
                has_u = true;
                self.advance();
            }
        }

        // Determine token type according to C standard rules
        if has_u && has_l {
            // unsigned long / unsigned long long — 按值域升级：u32 内 → unsigned，
            // i64 内 → long long（此前 >u32::MAX 直接报"不支持"，8000000000ULL 被误拒）；
            // (i64::MAX, u64::MAX]（如 ULLONG_MAX）以 64 位位模式承载为 unsigned long long
            //（E1 B 档：u64 坑修复；有符号解释在此值域无意义，已在 spec 记录）
            if u32::try_from(val).is_ok() {
                return self.make_token(TokenType::UnsignedLiteral, &val.to_string());
            }
            if i64::try_from(val).is_ok() {
                return self.make_token(TokenType::LongLiteral, &val.to_string());
            }
            return self.make_token(TokenType::UnsignedLiteral, &val.to_string());
        }

        if has_u {
            if u32::try_from(val).is_ok() {
                return self.make_token(TokenType::UnsignedLiteral, &val.to_string());
            }
            if i64::try_from(val).is_ok() {
                return self.make_token(TokenType::LongLiteral, &val.to_string());
            }
            return self.make_token(TokenType::UnsignedLiteral, &val.to_string());
        }

        if has_l {
            if i64::try_from(val).is_err() {
                // C 标准十进制常量溢出有符号域时转 unsigned long long（如 LLONG_MIN 的
                // 量值 9223372036854775808LL）；配合一元负号即得 LLONG_MIN 位模式
                return self.make_token(TokenType::UnsignedLiteral, &val.to_string());
            }
            return self.make_token(TokenType::LongLiteral, &val.to_string());
        }

        // No suffix: apply C standard type promotion rules
        if has_integer_prefix {
            if i32::try_from(val).is_ok() {
                self.make_token(TokenType::Number, &val.to_string())
            } else if u32::try_from(val).is_ok() {
                self.make_token(TokenType::UnsignedLiteral, &val.to_string())
            } else if i64::try_from(val).is_ok() {
                self.make_token(TokenType::LongLiteral, &val.to_string())
            } else {
                self.errors.push(LexerError {
                    message: format!("整数常量 {} 超出可表示范围", val),
                    line: self.line,
                    column: self.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                self.make_token(TokenType::Number, "0")
            }
        } else {
            // Decimal: no automatic unsigned promotion
            if i32::try_from(val).is_ok() {
                self.make_token(TokenType::Number, &val.to_string())
            } else if i64::try_from(val).is_ok() {
                self.make_token(TokenType::LongLiteral, &val.to_string())
            } else {
                self.errors.push(LexerError {
                    message: format!("整数常量 {} 超出可表示范围", val),
                    line: self.line,
                    column: self.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                self.make_token(TokenType::Number, "0")
            }
        }
    }
}
