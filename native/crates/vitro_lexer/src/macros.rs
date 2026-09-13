//! 内置预定义宏（stdio.h / stdlib.h / limits.h / stdbool.h / stdarg.h 等常量宏）。

use std::collections::HashMap;

use super::preprocessor::MacroDef;
use super::token::{Token, TokenType};

/// 构造 Vitro 词法分析器默认内置宏表。
pub(crate) fn builtin_macros() -> HashMap<String, MacroDef> {
    let mut macros = HashMap::new();
    // Predefine common stdio macros for fprintf compatibility
    macros.insert(
        "stdout".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "1".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "stderr".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "2".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "EOF".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "-1".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "stdin".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "0".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    // stdlib.h macros
    macros.insert(
        "EXIT_SUCCESS".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "0".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "EXIT_FAILURE".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "1".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "RAND_MAX".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "32767".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    // stdio.h macros
    macros.insert(
        "SEEK_SET".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "0".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "SEEK_CUR".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "1".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "SEEK_END".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "2".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    // limits.h macros
    macros.insert(
        "INT_MAX".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "2147483647".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "INT_MIN".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "-2147483648".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "LONG_MAX".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "2147483647".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "LONG_MIN".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "-2147483648".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "CHAR_BIT".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "8".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    // limits.h 其余宏（E1 B 档）。负值沿用 INT_MIN 的"Number 带负号文本"口径
    //（宏体不经 number() 重扫，parser 直接按 token 文本取值）；
    // ULLONG_MAX 等 (i64::MAX, u64::MAX] 值域按 64 位位模式承载为 unsigned long long。
    for (name, ty, text) in [
        ("SCHAR_MIN", TokenType::Number, "-128"),
        ("SCHAR_MAX", TokenType::Number, "127"),
        ("UCHAR_MAX", TokenType::Number, "255"),
        ("SHRT_MIN", TokenType::Number, "-32768"),
        ("SHRT_MAX", TokenType::Number, "32767"),
        ("USHRT_MAX", TokenType::Number, "65535"),
        ("UINT_MAX", TokenType::UnsignedLiteral, "4294967295"),
        ("ULONG_MAX", TokenType::UnsignedLiteral, "4294967295"),
        ("LLONG_MAX", TokenType::LongLiteral, "9223372036854775807"),
        ("LLONG_MIN", TokenType::LongLiteral, "-9223372036854775808"),
        ("ULLONG_MAX", TokenType::UnsignedLiteral, "18446744073709551615"),
    ] {
        macros.insert(
            name.to_string(),
            MacroDef {
                params: vec![],
                body: vec![Token {
                    ty,
                    text: text.to_string(),
                    line: 0,
                    column: 0,
                }],
            },
        );
    }
    // stdbool.h macros
    macros.insert(
        "true".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "1".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    macros.insert(
        "false".to_string(),
        MacroDef {
            params: vec![],
            body: vec![Token {
                ty: TokenType::Number,
                text: "0".to_string(),
                line: 0,
                column: 0,
            }],
        },
    );
    // stdarg.h 宏：将标准写法映射到 Vitro 内部 host func 调用
    macros.insert(
        "va_start".to_string(),
        MacroDef {
            params: vec!["ap".to_string(), "last".to_string()],
            body: vec![
                Token {
                    ty: TokenType::Identifier,
                    text: "__vitro_va_start".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Ampersand,
                    text: "&".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "ap".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Comma,
                    text: ",".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Ampersand,
                    text: "&".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "last".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Comma,
                    text: ",".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Sizeof,
                    text: "sizeof".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "last".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
            ],
        },
    );
    macros.insert(
        "va_arg".to_string(),
        MacroDef {
            params: vec!["ap".to_string(), "type".to_string()],
            body: vec![
                Token {
                    ty: TokenType::Star,
                    text: "*".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "type".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Star,
                    text: "*".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "__vitro_va_arg".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Ampersand,
                    text: "&".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "ap".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Comma,
                    text: ",".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Sizeof,
                    text: "sizeof".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "type".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
            ],
        },
    );
    macros.insert(
        "va_end".to_string(),
        MacroDef {
            params: vec!["ap".to_string()],
            body: vec![
                Token {
                    ty: TokenType::Identifier,
                    text: "__vitro_va_end".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Ampersand,
                    text: "&".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::LParen,
                    text: "(".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::Identifier,
                    text: "ap".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
                Token {
                    ty: TokenType::RParen,
                    text: ")".to_string(),
                    line: 0,
                    column: 0,
                },
            ],
        },
    );
    macros
}
