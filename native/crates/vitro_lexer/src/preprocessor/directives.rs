//! C 预处理指令分发：宏定义、条件编译（`#if`/`#elif`/`#else`/`#endif`）、
//! `#undef`、`#include`（ModuleGraph 解析）。
//!
//! E2 重构说明：本文件是指令的**消费骨架**（与 lexer 主扫描行内集成，保持
//! 行号补偿等既有机制），判定/展开/解析的语义本体分别在 `cond` / `expander`
//! / `macro_table` / `resolver` / `splice` 子模块。

use vitro_shared::ErrorCode;
use std::path::PathBuf;

use super::resolver::IncludeResolver;
use super::{lex_error, ConditionalState, MacroDef};
use crate::preprocessor::cond;
use crate::token::{LexerError, Token, TokenType};
use crate::Lexer;

/// 条件编译状态（重导出兼容）。
pub use super::ConditionalState as CondState;

impl Lexer {
    pub(crate) fn is_skipping(&self) -> bool {
        self.conditional_stack.iter().any(|s| !s.active)
    }

    /// **父级**是否处于跳过态（排除栈顶）——`#elif` / `#else` 的判定依据。
    fn parent_skipping(&self) -> bool {
        self.conditional_stack
            .iter()
            .rev()
            .skip(1)
            .any(|s| !s.active)
    }

    /// 在条件编译跳过模式下，跳过一个不活跃的逻辑行（或注释块、预处理指令）。
    /// 返回 None 表示已到 EOF；Some(false) 表示预处理指令导致 skipping 结束；
    /// Some(true) 表示仍在 skipping 中，应继续循环。
    pub(crate) fn skip_inactive_line(&mut self) -> Option<bool> {
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
        if self.pos >= self.chars.len() {
            return None;
        }
        if self.peek(0) == '/' && self.peek(1) == '*' {
            self.advance();
            self.advance();
            while self.pos < self.chars.len() {
                if self.peek(0) == '*' && self.peek(1) == '/' {
                    self.advance();
                    self.advance();
                    break;
                }
                self.advance();
            }
            return Some(true);
        }
        if self.peek(0) == '/' && self.peek(1) == '/' {
            while self.pos < self.chars.len() && self.peek(0) != '\n' {
                self.advance();
            }
            if self.pos < self.chars.len() {
                self.advance();
            }
            return Some(true);
        }
        if self.peek(0) == '#' {
            self.skip_preprocessor_directive();
            return Some(self.is_skipping());
        }
        while self.pos < self.chars.len() && self.peek(0) != '\n' {
            self.advance();
        }
        if self.pos < self.chars.len() {
            self.advance();
        }
        Some(true)
    }

    pub(crate) fn skip_preprocessor_directive(&mut self) {
        self.advance(); // consume '#'
        self.skip_whitespace();

        // 读取指令名
        let dir_start = self.pos;
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            // 允许下划线：E2 目录哨兵  / 
            if c.is_ascii_alphabetic() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let directive: String = self.chars[dir_start..self.pos].iter().collect();

        // `#if` / `#elif` 需要整行原始文本（__has_include 的路径无法从
        // token 还原），其余指令按各自消费方式处理
        if directive == "if" || directive == "elif" {
            let rest = self.rest_of_line();
            match directive.as_str() {
                "if" => self.handle_if(&rest),
                _ => self.handle_elif(&rest),
            }
            return;
        }

        match directive.as_str() {
            "define" => {
                if !self.is_skipping() {
                    self.parse_define_directive();
                } else {
                    self.skip_to_line_end();
                }
            }
            // E2：拼接自定义头文件时的目录哨兵（线性扫描下精确维护目录栈）。
            // U1#11：哨兵同时承担**条件栈边界记账**——push 时记录进入头文件前
            // 的条件栈深度，pop 时校验头文件是否把条件组闭合干净（跨文件
            // 条件栈污染的根治点，见 handle_endif / __vitro_pop_dir）。
            "__vitro_push_dir" => {
                let dir = self.rest_of_line().trim().to_string();
                self.include_resolver.dir_stack.push(PathBuf::from(dir));
                self.include_cond_boundary.push(self.conditional_stack.len());
            }
            "__vitro_pop_dir" => {
                self.include_resolver.dir_stack.pop();
                if let Some(boundary) = self.include_cond_boundary.pop() {
                    if self.conditional_stack.len() > boundary {
                        // 头文件内有未闭合的 #if/#ifdef/#ifndef：若放任泄漏，
                        // 包含者后续代码会被整段吞掉、EOF 的 E1013 错位到主文件
                        // 末尾——在头文件边界闭合并定位报错。
                        self.errors.push(lex_error(
                            "头文件内未闭合的 #if/#ifdef/#ifndef（已在头文件末尾自动闭合，请检查头文件的条件编译配对）",
                            self.line,
                            0,
                            ErrorCode::E1013_UnclosedConditional,
                        ));
                        self.conditional_stack.truncate(boundary);
                    }
                }
            }
            "undef" => {
                if !self.is_skipping() {
                    self.skip_whitespace();
                    let name = self.read_ident();
                    self.macros.undefine(&name);
                }
                self.skip_to_line_end();
            }
            "include" => {
                if !self.is_skipping() {
                    // include 指令行（供 E1021/E1015 定位报错；此时行尾尚未消费）
                    let include_line = self.line;
                    self.skip_whitespace();
                    if let Some((path, is_angle)) = self.parse_include_path() {
                        // 先整行消费 include 行（含换行），拼接内容从下一行起
                        self.skip_to_line_end();
                        if self.pos < self.chars.len() {
                            self.advance(); // consume '\n'
                        }
                        self.handle_include(&path, is_angle, include_line);
                    } else {
                        self.skip_to_line_end();
                    }
                } else {
                    self.skip_to_line_end();
                }
            }
            "ifdef" => {
                let ident = self.read_ident();
                self.skip_to_line_end();
                let parent_skipping = self.is_skipping();
                let active = if parent_skipping {
                    false
                } else {
                    self.macros.contains(&ident)
                };
                self.conditional_stack
                    .push(ConditionalState { active, has_else: false, taken: active });
            }
            "ifndef" => {
                let ident = self.read_ident();
                self.skip_to_line_end();
                let parent_skipping = self.is_skipping();
                let active = if parent_skipping {
                    false
                } else {
                    !self.macros.contains(&ident)
                };
                self.conditional_stack
                    .push(ConditionalState { active, has_else: false, taken: active });
            }
            "else" => {
                self.handle_else();
            }
            "endif" => {
                self.handle_endif();
            }
            _ => {
                // 未知指令（含 #warning 等），跳过整行
                self.skip_to_line_end();
            }
        }
    }

    fn rest_of_line(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.chars.len() && self.peek(0) != '\n' {
            self.advance();
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn skip_to_line_end(&mut self) {
        while self.pos < self.chars.len() && self.peek(0) != '\n' {
            self.advance();
        }
    }

    fn read_ident(&mut self) -> String {
        self.skip_whitespace();
        let start = self.pos;
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        self.chars[start..self.pos].iter().collect()
    }

    // ---------- 条件组 ----------

    fn handle_if(&mut self, raw: &str) {
        let parent_skipping = self.is_skipping();
        let state = if parent_skipping {
            // 外层已跳过：不求值（短路语义），组内永不激活
            ConditionalState { active: false, has_else: false, taken: true }
        } else {
            let outcome = cond::evaluate(
                &self.macros,
                raw,
                self.line,
                &|p: &str, is_angle: bool| self.include_resolver.has_include(p, is_angle),
                &mut self.errors,
                &mut self.preprocessor_trace,
            );
            ConditionalState { active: outcome.value, has_else: false, taken: outcome.value }
        };
        self.conditional_stack.push(state);
    }

    fn handle_elif(&mut self, raw: &str) {
        // 先消费行尾（保持与旧 else/endif 一致的扫描位置语义）
        let parent_skipping = self.parent_skipping();
        if let Some(state) = self.conditional_stack.last_mut() {
            if state.has_else {
                self.errors.push(lex_error(
                    "#elif 出现在 #else 之后",
                    self.line,
                    0,
                    ErrorCode::E1012_DuplicateElse,
                ));
                return;
            }
        }
        self.skip_to_line_end();
        let Some(state) = self.conditional_stack.last_mut() else {
            self.errors.push(lex_error(
                "没有匹配的 #if/#ifdef/#ifndef 就出现 #elif",
                self.line,
                0,
                ErrorCode::E1011_UnmatchedConditional,
            ));
            return;
        };
        if parent_skipping || state.taken {
            state.active = false;
            return;
        }
        let outcome = cond::evaluate(
            &self.macros,
            raw,
            self.line,
            &|p: &str, is_angle: bool| self.include_resolver.has_include(p, is_angle),
            &mut self.errors,
            &mut self.preprocessor_trace,
        );
        state.active = outcome.value;
        state.taken = outcome.value;
    }

    fn handle_else(&mut self) {
        self.skip_to_line_end();
        let parent_skipping = self.parent_skipping();
        if let Some(state) = self.conditional_stack.last_mut() {
            if state.has_else {
                self.errors.push(lex_error(
                    "重复的 #else",
                    self.line,
                    0,
                    ErrorCode::E1012_DuplicateElse,
                ));
            } else {
                state.has_else = true;
                // 父级跳过态下保持不激活（显式化以便后续 #elif 正确）
                state.active = !state.taken && !parent_skipping;
            }
        } else {
            self.errors.push(lex_error(
                "没有匹配的 #if/#ifdef/#ifndef 就出现 #else",
                self.line,
                0,
                ErrorCode::E1011_UnmatchedConditional,
            ));
        }
    }

    fn handle_endif(&mut self) {
        self.skip_to_line_end();
        // U1#11：include 边界记账——头文件内多余的 #endif 不得弹掉包含者的
        // 条件组（否则包含者的结构被破坏、E1011 错位到主文件自己的 #endif 行）。
        if let Some(&boundary) = self.include_cond_boundary.last() {
            if self.conditional_stack.len() <= boundary {
                self.errors.push(lex_error(
                    "头文件中的 #endif 没有匹配的 #if/#ifdef/#ifndef（将弹掉包含文件的条件编译组，已忽略该 #endif）",
                    self.line,
                    0,
                    ErrorCode::E1011_UnmatchedConditional,
                ));
                return;
            }
        }
        if self.conditional_stack.pop().is_none() {
            self.errors.push(lex_error(
                "没有匹配的 #if/#ifdef/#ifndef 就出现 #endif",
                self.line,
                0,
                ErrorCode::E1011_UnmatchedConditional,
            ));
        }
    }

    // ---------- include ----------

    fn handle_include(&mut self, path: &str, is_angle: bool, include_line: i32) {
        // 动态嵌套深度保险丝（U1#11）：dir_stack 深度即当前 include 嵌套层数，
        // 超限报 E1015 并跳过拼接——保险丝可触发性由
        // test_u11_include_depth_fuse 锚定。
        if self.include_resolver.dir_stack.len() >= super::resolver::MAX_INCLUDE_DEPTH {
            self.errors.push(lex_error(
                format!(
                    "#include 嵌套过深（上限 {} 层，已跳过 '{}'）",
                    super::resolver::MAX_INCLUDE_DEPTH,
                    path
                ),
                include_line,
                0,
                ErrorCode::E1015_IncludeCycle,
            ));
            return;
        }

        let is_stub = IncludeResolver::load_stub(path).is_some();
        // H-3（U1#11 收紧）：`<>` 只匹配标准库存根，不再搜索文件系统目录——
        // 与 Clang 的 <> 语义对齐。存量语料扫描（2026-09-14）：655 处 `<>`
        // include 全部为标准头名，零存量依赖。
        if is_angle && !is_stub {
            self.errors.push(lex_error(
                format!(
                    "<> 形式只匹配标准库头文件（未收录 '{}'）；自定义头文件请使用 #include \"{}\"",
                    path, path
                ),
                include_line,
                0,
                ErrorCode::E1021_IncludeNotFound,
            ));
            return;
        }
        match self.include_resolver.should_include(path, is_stub) {
            Ok(()) => {}
            Err(Some(cycle)) => {
                self.errors.push(lex_error(
                    format!("检测到 #include 依赖环：{}（已跳过该 include）", cycle),
                    include_line,
                    0,
                    ErrorCode::E1015_IncludeCycle,
                ));
                return;
            }
            Err(None) => return, // include-once：静默跳过
        }

        let content = if is_stub {
            IncludeResolver::load_stub(path).map(|s| s.to_string())
        } else {
            self.include_resolver.resolve_path(path).and_then(|full| std::fs::read_to_string(full).ok())
        };
        // H-1（U1#11）：找不到头文件必须定位报错（E1021，include 行）——
        // 修复前此处静默 `return`，错误错位到使用点（E3023 报在几十行之外），
        // 学生按提示找声明永远找不到根因。
        let Some(mut content) = content else {
            self.errors.push(lex_error(
                format!(
                    "找不到头文件 '{}'（已搜索：当前文件目录、源文件目录；标准库存根未收录该名字）",
                    path
                ),
                include_line,
                0,
                ErrorCode::E1021_IncludeNotFound,
            ));
            return;
        };

        // 自定义头文件：以哨兵指令包裹内容，线性扫描到内容首/尾时精确压/弹
        // 目录栈（嵌套 quote-include 按"包含者目录优先"解析，C 语义）
        let header_dir: Option<String> = if !is_stub {
            self.include_resolver
                .resolve_path(path)
                .and_then(|full| full.parent().map(|p| p.to_string_lossy().to_string()))
        } else {
            None
        };
        if let Some(dir) = header_dir {
            content = format!("#__vitro_push_dir {}
{}
#__vitro_pop_dir
", dir, content);
        }

        // 保留 include 内容的原始换行：`#define` / `#ifdef` 等指令必须独占一行。
        // 拼接点 = 当前 pos（include 行已被整行消费，含换行）——旧实现在行尾之前
        // 拼接、随后"跳到行尾"会吃掉内容首行（存量头文件首行均为注释而未暴露）。
        let content_chars: Vec<char> = content.chars().collect();
        let inserted_newlines = content_chars.iter().filter(|c| **c == '\n').count() as i32;
        self.chars.splice(self.pos..self.pos, content_chars);
        // 行号补偿：插入内容自带 N 个换行，扫描它会让 self.line 前进 N 行；
        // 先扣掉 N，扫描完后 self.line 恰好回到 include 内容之后的行号。
        self.line -= inserted_newlines;
    }

    /// 解析 include 路径与定界形式：返回 `(路径, 是否 <> 形式)`。
    /// U1#11 起携带定界符——`<>` 只查标准库存根（H-3），`"` 走文件系统候选链，
    /// 两形式的判定在 handle_include / has_include 单源分发。
    pub(crate) fn parse_include_path(&mut self) -> Option<(String, bool)> {
        let delimiter = self.peek(0);
        if delimiter != '<' && delimiter != '"' {
            return None;
        }
        self.advance(); // consume opening delimiter
        let start = self.pos;
        let end_delim = if delimiter == '<' { '>' } else { '"' };
        while self.pos < self.chars.len() && self.peek(0) != end_delim {
            self.advance();
        }
        let path: String = self.chars[start..self.pos].iter().collect();
        if self.pos < self.chars.len() && self.peek(0) == end_delim {
            self.advance(); // consume closing delimiter
        }
        Some((path, delimiter == '<'))
    }

    // ---------- define / undef ----------

    pub(crate) fn parse_define_directive(&mut self) {
        self.skip_whitespace();
        if self.pos >= self.chars.len() || !(self.peek(0).is_ascii_alphabetic() || self.peek(0) == '_') {
            self.errors.push(LexerError {
                message: "#define 后预期宏名称".to_string(),
                line: self.line,
                column: self.column,
                code: ErrorCode::E1005_InvalidDefine as i32,
            });
            self.skip_to_line_end();
            return;
        }

        let name_start = self.pos;
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let name: String = self.chars[name_start..self.pos].iter().collect();

        let mut params: Vec<String> = Vec::new();
        // 参数列表：宏名后紧跟 '('（中间无空白）
        if self.peek(0) == '(' {
            self.advance();
            loop {
                self.skip_whitespace();
                if self.peek(0) == ')' {
                    self.advance();
                    break;
                }
                if !(self.peek(0).is_ascii_alphabetic() || self.peek(0) == '_') {
                    self.errors.push(LexerError {
                        message: "宏参数必须是标识符".to_string(),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1005_InvalidDefine as i32,
                    });
                    self.skip_to_line_end();
                    return;
                }
                let p_start = self.pos;
                while self.pos < self.chars.len() {
                    let c = self.peek(0);
                    if c.is_ascii_alphanumeric() || c == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                params.push(self.chars[p_start..self.pos].iter().collect());
                self.skip_whitespace();
                if self.peek(0) == ',' {
                    self.advance();
                } else if self.peek(0) == ')' {
                    self.advance();
                    break;
                } else {
                    self.errors.push(LexerError {
                        message: "宏参数列表格式错误".to_string(),
                        line: self.line,
                        column: self.column,
                        code: ErrorCode::E1005_InvalidDefine as i32,
                    });
                    self.skip_to_line_end();
                    return;
                }
            }
        }

        // 跳过宏名与 body 之间的空白，但不要把下一行也吞进来
        while self.pos < self.chars.len() {
            let c = self.peek(0);
            if c == ' ' || c == '\t' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }

        let body_start = self.pos;
        while self.pos < self.chars.len() && self.peek(0) != '\n' {
            self.advance();
        }
        let body: String = self.chars[body_start..self.pos].iter().collect();

        // E2：宏体内 `#`/`##` 是操作符而非预处理指令——子词法器以宏体模式工作
        let mut body_lexer = Lexer::with_mode_and_path(&body, false, None);
        body_lexer.macro_body_mode = true;
        let (body_tokens, _) = body_lexer.tokenize();
        let body_tokens: Vec<Token> =
            body_tokens.into_iter().filter(|t| t.ty != TokenType::Eof).collect();

        let mdef = MacroDef { params, body: body_tokens };
        self.macros.define(&name, mdef, self.line, &mut self.warnings);
    }
}
