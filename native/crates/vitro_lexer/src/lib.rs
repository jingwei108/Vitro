//! Vitro 词法分析器。
//!
//! 从 `vitro_native::compiler::lexer` 拆分而来，负责 C/C++ 教学子集的 token 化、宏预处理与条件编译。

// TODO(#D08): Lexer 已承载 C/C++ 混合词法，未来应将 C++ 专属词法拆分到 lexer/cpp.rs。
use vitro_shared::ErrorCode;
use std::path::PathBuf;

pub mod comment;
pub mod keyword;
pub mod macros;
pub mod number;
pub mod preprocessor;
pub mod string;
pub mod token;

pub use preprocessor::{ConditionalState, MacroDef, MacroTable};
pub use token::{LexerError, LexerWarning, Token, TokenType};

pub struct Lexer {
    pub(crate) chars: Vec<char>,
    pub(crate) errors: Vec<LexerError>,
    /// E2 白箱教学层：预处理阶段非致命警告（宏遮蔽 W1018 / 宏参数副作用 W1019）。
    pub(crate) warnings: Vec<LexerWarning>,
    pub(crate) pos: usize,
    pub(crate) line: i32,
    pub(crate) column: i32,
    /// E2：宏定义表单源（内置预定义 + 用户 #define + 遮蔽诊断）。
    pub(crate) macros: MacroTable,
    pub(crate) conditional_stack: Vec<ConditionalState>,
    pub(crate) is_cpp_mode: bool,
    /// E2：宏体内 `#`/`##` 是操作符（非预处理指令）。
    pub(crate) macro_body_mode: bool,
    /// E2：include 解析（include-once + 环检测 + 存根加载）。
    pub(crate) include_resolver: preprocessor::IncludeResolver,
    /// U1#11：include 边界的条件栈记账——`#__vitro_push_dir` 时记录进入头文件
    /// 前的条件栈深度，`#__vitro_pop_dir` / `handle_endif` 时校验头文件把条件
    /// 组闭合干净（跨文件条件栈污染的根治点）。
    pub(crate) include_cond_boundary: Vec<usize>,
    /// E2 白箱教学层：展开链与 #if 分支选择原因（容量封顶）。
    pub(crate) preprocessor_trace: Vec<String>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self::with_mode_and_path(source, false, None)
    }

    pub fn with_mode(source: &str, is_cpp_mode: bool) -> Self {
        Self::with_mode_and_path(source, is_cpp_mode, None)
    }

    pub fn with_base_path(source: &str, base_path: Option<PathBuf>) -> Self {
        Self::with_mode_and_path(source, false, base_path)
    }

    pub fn with_mode_and_path(source: &str, is_cpp_mode: bool, base_path: Option<PathBuf>) -> Self {
        Self {
            chars: source.chars().collect(),
            errors: Vec::new(),
            pos: 0,
            line: 1,
            column: 1,
            macros: MacroTable::builtin(),
            conditional_stack: Vec::new(),
            is_cpp_mode,
            warnings: Vec::new(),
            macro_body_mode: false,
            include_resolver: preprocessor::IncludeResolver::new(base_path.clone()),
            include_cond_boundary: Vec::new(),
            preprocessor_trace: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> (Vec<Token>, Vec<LexerError>) {
        let mut tokens = Vec::new();
        loop {
            let t = self.next_token();
            if t.ty == TokenType::Eof {
                tokens.push(t);
                break;
            }
            tokens.push(t);
        }
        if !self.conditional_stack.is_empty() {
            self.errors.push(LexerError {
                message: "未闭合的 #ifdef / #ifndef 块".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1013_UnclosedConditional as i32,
            });
        }
        let expanded = self.expand_macros(tokens);
        (expanded, std::mem::take(&mut self.errors))
    }

    /// E2：取预处理警告（宏遮蔽/宏参数副作用等，非致命）。
    pub fn into_warnings(&mut self) -> Vec<LexerWarning> {
        std::mem::take(&mut self.warnings)
    }

    /// E2 白箱教学层：展开链与 `#if` 分支选择原因。
    pub fn into_expansion_trace(&mut self) -> Vec<String> {
        std::mem::take(&mut self.preprocessor_trace)
    }

    pub fn into_errors(self) -> Vec<LexerError> {
        self.errors
    }

    fn next_token(&mut self) -> Token {
        // 跳过类输入（条件编译块、空白、注释、预处理指令）统一用 loop 重派发，禁止递归：
        // 递归实现下连续数万行 `//` 注释会逐层递归导致栈溢出（SIGSEGV，无法被
        // catch_unwind 捕获，IDE 直接崩溃）。
        loop {
            // 条件编译跳过模式：跳过所有不活跃的代码块
            while self.is_skipping() {
                match self.skip_inactive_line() {
                    None => return self.make_token(TokenType::Eof, ""),
                    Some(false) => break,
                    Some(true) => continue,
                }
            }

            self.skip_whitespace();

            if self.pos >= self.chars.len() {
                return self.make_token(TokenType::Eof, "");
            }

            let c = self.peek(0);

            if c.is_ascii_alphabetic() || c == '_' {
                return self.identifier_or_keyword();
            }

            if c.is_ascii_digit() {
                return self.number();
            }

            if c == '"' {
                return self.string_literal();
            }

            if c == '\'' {
                return self.char_literal();
            }

            if c == '/' && self.peek(1) == '/' {
                self.skip_comment();
                continue;
            }

            if c == '/' && self.peek(1) == '*' {
                self.skip_block_comment();
                continue;
            }

            if c == '#' {
                if self.macro_body_mode {
                    // E2：宏体内 `#`（字符串化）/ `##`（拼接）是操作符
                    if self.match_char('#') {
                        return self.make_token(TokenType::HashHash, "##");
                    }
                    self.advance();
                    return self.make_token(TokenType::Hash, "#");
                }
                self.skip_preprocessor_directive();
                continue;
            }

            // match 各臂产出 Token；loop 内尾表达式不会自动成为函数返回值，需显式 return。
            return match c {
                '+' => {
                    if self.match_char('+') {
                        return self.make_token(TokenType::Increment, "++");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::PlusAssign, "+=");
                    }
                    self.advance();
                    self.make_token(TokenType::Plus, "+")
                }
                '-' => {
                    if self.match_char('>') {
                        if self.peek(0) == '*' {
                            self.advance();
                            return self.make_token(TokenType::ArrowStar, "->*");
                        }
                        return self.make_token(TokenType::Arrow, "->");
                    }
                    if self.match_char('-') {
                        return self.make_token(TokenType::Decrement, "--");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::MinusAssign, "-=");
                    }
                    self.advance();
                    self.make_token(TokenType::Minus, "-")
                }
                '*' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::StarAssign, "*=");
                    }
                    self.advance();
                    self.make_token(TokenType::Star, "*")
                }
                '/' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::SlashAssign, "/=");
                    }
                    self.advance();
                    self.make_token(TokenType::Slash, "/")
                }
                '%' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::PercentAssign, "%=");
                    }
                    self.advance();
                    self.make_token(TokenType::Percent, "%")
                }
                '=' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::Eq, "==");
                    }
                    self.advance();
                    self.make_token(TokenType::Assign, "=")
                }
                '!' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::Ne, "!=");
                    }
                    self.advance();
                    self.make_token(TokenType::Not, "!")
                }

                '&' => {
                    if self.match_char('&') {
                        return self.make_token(TokenType::AndAnd, "&&");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::AndAssign, "&=");
                    }
                    self.advance();
                    self.make_token(TokenType::Ampersand, "&")
                }
                '|' => {
                    if self.match_char('|') {
                        return self.make_token(TokenType::OrOr, "||");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::OrAssign, "|=");
                    }
                    self.advance();
                    self.make_token(TokenType::BitOr, "|")
                }
                '^' => {
                    if self.match_char('=') {
                        return self.make_token(TokenType::XorAssign, "^=");
                    }
                    self.advance();
                    self.make_token(TokenType::BitXor, "^")
                }
                '~' => {
                    self.advance();
                    self.make_token(TokenType::BitNot, "~")
                }
                '<' => {
                    if self.match_char('<') {
                        if self.peek(0) == '=' {
                            self.advance();
                            return self.make_token(TokenType::ShlAssign, "<<=");
                        }
                        return self.make_token(TokenType::Shl, "<<");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::Le, "<=");
                    }
                    self.advance();
                    self.make_token(TokenType::Lt, "<")
                }
                '>' => {
                    if self.match_char('>') {
                        if self.peek(0) == '=' {
                            self.advance();
                            return self.make_token(TokenType::ShrAssign, ">>=");
                        }
                        return self.make_token(TokenType::Shr, ">>");
                    }
                    if self.match_char('=') {
                        return self.make_token(TokenType::Ge, ">=");
                    }
                    self.advance();
                    self.make_token(TokenType::Gt, ">")
                }
                ';' => {
                    self.advance();
                    self.make_token(TokenType::Semicolon, ";")
                }
                ',' => {
                    self.advance();
                    self.make_token(TokenType::Comma, ",")
                }
                '(' => {
                    self.advance();
                    self.make_token(TokenType::LParen, "(")
                }
                ')' => {
                    self.advance();
                    self.make_token(TokenType::RParen, ")")
                }
                '{' => {
                    self.advance();
                    self.make_token(TokenType::LBrace, "{")
                }
                '}' => {
                    self.advance();
                    self.make_token(TokenType::RBrace, "}")
                }
                '[' => {
                    self.advance();
                    self.make_token(TokenType::LBracket, "[")
                }
                ']' => {
                    self.advance();
                    self.make_token(TokenType::RBracket, "]")
                }
                '.' => {
                    if self.match_char('*') {
                        return self.make_token(TokenType::DotStar, ".*");
                    }
                    if self.peek(1) == '.' && self.peek(2) == '.' {
                        self.advance();
                        self.advance();
                        self.advance();
                        return self.make_token(TokenType::Ellipsis, "...");
                    }
                    // U1#7：前导点浮点（.5 / .25f）是合法 C（clang 正常编译），
                    // 此前落进 Dot 臂被解析为成员访问 → "预期表达式"误拒。
                    // 转入 number()——其 dot_float 分支以 '.'+数字 起步收集。
                    if self.peek(1).is_ascii_digit() {
                        return self.number();
                    }
                    self.advance();
                    self.make_token(TokenType::Dot, ".")
                }
                ':' => {
                    if self.match_char(':') {
                        return self.make_token(TokenType::ColonColon, "::");
                    }
                    self.advance();
                    self.make_token(TokenType::Colon, ":")
                }
                '?' => {
                    self.advance();
                    self.make_token(TokenType::Question, "?")
                }
                _ => {
                    self.advance();
                    self.errors.push(LexerError {
                        message: format!("无法识别的字符: '{}'", c),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1001_UnknownChar as i32,
                    });
                    self.make_token(TokenType::Unknown, &c.to_string())
                }
            };
        }
    }

    fn identifier_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        // C23 u8 前缀字符串：u8"..." 直接按普通字符串字面量处理（教学子集差异：
        // 无独立 char8_t 类型语义，已在 spec 记录）
        if text == "u8" && self.peek(0) == '"' {
            return self.string_literal();
        }
        let ty = keyword::keyword_type(&text)
            .or_else(|| {
                if self.is_cpp_mode {
                    keyword::cpp_keyword_type(&text)
                } else {
                    None
                }
            })
            .unwrap_or(TokenType::Identifier);
        self.make_token(ty, &text)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub(crate) fn peek(&self, offset: usize) -> char {
        if self.pos >= self.chars.len() {
            '\0'
        } else {
            self.chars.get(self.pos + offset).copied().unwrap_or('\0')
        }
    }

    pub(crate) fn advance(&mut self) -> char {
        if self.pos >= self.chars.len() {
            return '\0';
        }
        let c = self.chars.get(self.pos).copied().unwrap_or('\0');
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        c
    }

    pub(crate) fn match_char(&mut self, expected: char) -> bool {
        if self.peek(1) != expected {
            return false;
        }
        self.advance();
        self.advance();
        true
    }

    pub(crate) fn make_token(&self, ty: TokenType, text: &str) -> Token {
        Token {
            ty,
            text: text.to_string(),
            line: self.line,
            column: self.column - text.len() as i32,
        }
    }
}
