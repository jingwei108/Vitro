//! E2：`#if` / `#elif` 条件求值器（非图灵完备的配置求值）。
//!
//! 求值管线（C99 §6.10.1 顺序的教学子集）：
//! 1. 原始行文本中的 `__has_include(<p>)` / `__has_include("p")` 先行替换为
//!    `1` / `0`（它需要原始路径文本，token 化后 `<stdio.h>` 会碎成多个 token）；
//! 2. `defined(X)` / `defined X` 在**宏展开前**提取（C 规则：其操作数不展开）；
//! 3. 其余宏展开（走 [`expander`]，深度保险丝同源）；
//! 4. 残余标识符按 C 规则替换为 `0`；
//! 5. 递归下降求值（`&&`/`||` 短路，短路分支内的除零不触发——与 C 一致）。
//!
//! 分支选择原因记录（白箱教学层）：每次求值把「表达式 → 真假」写进教学追踪。

use super::{expander, MacroTable, TEACHING_TRACE_CAP};
use crate::preprocessor::lex_error;
use crate::token::{LexerError, Token, TokenType};
use vitro_shared::ErrorCode;

/// 一次条件求值的结果（含白箱教学层的原因记录）。
#[derive(Debug, Clone)]
pub struct CondOutcome {
    pub value: bool,
    /// 形如 "`A > 3` → 假（A 未定义按 0）" 的教学说明。
    pub reason: String,
}

/// 求值 `#if` / `#elif` 表达式。
///
/// `raw` 为指令行的原始文本（`#if` 之后的部分）；`has_include` 回调由
/// resolver 提供（参数 = (路径, 是否 `<>` 形式)，U1#11 起与 `#include`
/// 解析口径单源）。出错（语法/除零/展开保险丝）记入 `errors` 并返回假。
pub fn evaluate(
    macros: &MacroTable,
    raw: &str,
    line: i32,
    has_include: &dyn Fn(&str, bool) -> bool,
    errors: &mut Vec<LexerError>,
    trace: &mut Vec<String>,
) -> CondOutcome {
    match evaluate_inner(macros, raw, line, has_include, errors, trace) {
        Ok(v) => v,
        Err(msg) => {
            errors.push(lex_error(
                format!("#if 表达式错误：{}", msg),
                line,
                0,
                ErrorCode::E1014_CondExprError,
            ));
            CondOutcome { value: false, reason: format!("`{}` → 求值失败", raw.trim()) }
        }
    }
}

fn evaluate_inner(
    macros: &MacroTable,
    raw: &str,
    line: i32,
    has_include: &dyn Fn(&str, bool) -> bool,
    errors: &mut Vec<LexerError>,
    trace: &mut Vec<String>,
) -> Result<CondOutcome, String> {
    let _ = line;
    // 1. __has_include 文本级替换（需要原始路径文本）
    let text = replace_has_include(raw, has_include);

    // 2. defined() 提取（宏展开前，C 规则）
    let toks = lex_fragment(&text)?;
    let defd = extract_defined(&toks, macros);

    // 3. 宏展开（深度保险丝同源；表达式模式不记展开链）
    let mut scratch_warnings = Vec::new();
    let mut ctx = super::ExpandCtx {
        macros,
        errors,
        warnings: &mut scratch_warnings,
        trace,
        expr_mode: true,
        emitted: 0,
        budget_exhausted: false,
        depth_exceeded: false,
    };
    let expanded = expander::expand_tokens(&mut ctx, &defd, 0);
    // 表达式展开也可能触发保险丝/拼接错误——warnings 弃用（非致命，且 #if
    // 内不应有可警告的调用形态）

    // 4. 残余标识符 → 0（C 规则）
    let norm: Vec<String> = expanded
        .iter()
        .map(|t| if t.ty == TokenType::Identifier { "0".to_string() } else { t.text.clone() })
        .collect();

    // 5. 解析 + 求值（短路）
    let mut p = Parser { toks: &norm, pos: 0 };
    let v = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(format!("表达式存在无法解释的部分：`{}`", p.toks[p.pos..].join(" ")));
    }

    let reason_word = if v != 0 { "真" } else { "假" };
    if trace.len() < TEACHING_TRACE_CAP {
        trace.push(format!("#if `{}` → {}", raw.trim(), reason_word));
    }
    Ok(CondOutcome { value: v != 0, reason: format!("`{}` → {}", raw.trim(), reason_word) })
}

/// 文本级替换 `__has_include(<p>)` / `__has_include("p")`。
/// 定界形式随路径传给回调（U1#11：`<>` 只查存根、`"` 走候选链——与
/// `#include` 单源，防同一文件内两者判定互相矛盾）。
fn replace_has_include(text: &str, has_include: &dyn Fn(&str, bool) -> bool) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(idx) = rest.find("__has_include") {
        out.push_str(&rest[..idx]);
        rest = &rest[idx + "__has_include".len()..];
        // 可选调用括号：__has_include(<p>) / __has_include(<p>（无括号也接受）
        let after_call = if let Some(t) = rest.trim_start().strip_prefix('(') {
            t
        } else {
            rest.trim_start()
        };
        let (open, close, is_angle) = match after_call.chars().next() {
            Some('<') => ('<', '>', true),
            Some('"') => ('"', '"', false),
            _ => {
                out.push_str("__has_include");
                continue;
            }
        };
        let after_open = &after_call[open.len_utf8()..];
        let Some(close_idx) = after_open.find(close) else {
            out.push_str("__has_include");
            continue;
        };
        let path = after_open[..close_idx].trim();
        let v = if has_include(path, is_angle) { "1" } else { "0" };
        out.push_str(v);
        let mut tail = &after_open[close_idx + close.len_utf8()..];
        // 吞掉调用右括号
        if let Some(t) = tail.trim_start().strip_prefix(')') {
            tail = t;
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// token 级提取 `defined(X)` / `defined X` → `1` / `0`（宏展开前，C 规则）。
fn extract_defined(toks: &[Token], macros: &MacroTable) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.ty == TokenType::Identifier && t.text == "defined" {
            let (arg, next) = if toks.get(i + 1).map(|t| t.ty) == Some(TokenType::LParen) {
                match toks.get(i + 2) {
                    Some(id)
                        if id.ty == TokenType::Identifier
                            && toks.get(i + 3).map(|t| t.ty) == Some(TokenType::RParen) =>
                    {
                        (Some(id.text.clone()), i + 4)
                    }
                    _ => (None, i + 1),
                }
            } else {
                match toks.get(i + 1) {
                    Some(id) if id.ty == TokenType::Identifier => (Some(id.text.clone()), i + 2),
                    _ => (None, i + 1),
                }
            };
            if let Some(name) = arg {
                let v = if macros.contains(&name) { "1" } else { "0" };
                out.push(Token { ty: TokenType::Number, text: v.to_string(), line: t.line, column: t.column });
                i = next;
                continue;
            }
            // 形态不合法：按 0 处理并交给求值器语义（等价于"格式错误的 defined"）
            out.push(Token { ty: TokenType::Number, text: "0".to_string(), line: t.line, column: t.column });
            i = next;
            continue;
        }
        out.push(t.clone());
        i += 1;
    }
    out
}

fn lex_fragment(text: &str) -> Result<Vec<Token>, String> {
    let (toks, errs) = crate::Lexer::new(text).tokenize();
    if errs.is_empty() {
        Ok(toks.into_iter().filter(|t| t.ty != TokenType::Eof).collect())
    } else {
        Err("表达式包含无法词法化的内容".to_string())
    }
}

// ---------- 递归下降求值（i64 值语义；&&/|| 短路；比较产生 0/1） ----------

struct Parser<'a> {
    toks: &'a [String],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(|s| s.as_str())
    }

    fn eat(&mut self, op: &str) -> bool {
        if self.peek() == Some(op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `||`（短路：左真则右不求值，除零不触发；结果 0/1）
    fn parse_or(&mut self) -> Result<i64, String> {
        let mut left = self.parse_and()?;
        while self.eat("||") {
            if left != 0 {
                self.skip_operand()?;
            } else {
                left = if self.parse_and()? != 0 { 1 } else { 0 };
            }
        }
        Ok(left)
    }

    /// `&&`（短路）
    fn parse_and(&mut self) -> Result<i64, String> {
        let mut left = self.parse_eq()?;
        while self.eat("&&") {
            if left != 0 {
                left = if self.parse_eq()? != 0 { 1 } else { 0 };
            } else {
                self.skip_operand()?;
            }
        }
        Ok(left)
    }

    /// 短路时**结构性跳过**一个操作数（不求值——跳过的分支里允许除零，
    /// 这正是 C 短路语义的意义所在）。
    fn skip_operand(&mut self) -> Result<(), String> {
        let before = self.pos;
        let mut depth = 0i32;
        while let Some(t) = self.peek() {
            match t {
                "(" => {
                    depth += 1;
                    self.pos += 1;
                }
                ")" => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    self.pos += 1;
                }
                "&&" | "||" if depth == 0 => break,
                _ => self.pos += 1,
            }
        }
        if self.pos == before {
            return Err("短路分支缺少操作数".to_string());
        }
        Ok(())
    }

    fn parse_eq(&mut self) -> Result<i64, String> {
        let mut left = self.parse_rel()?;
        loop {
            if self.eat("==") {
                let r = self.parse_rel()?;
                left = (left == r) as i64;
            } else if self.eat("!=") {
                let r = self.parse_rel()?;
                left = (left != r) as i64;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_rel(&mut self) -> Result<i64, String> {
        let mut left = self.parse_add()?;
        loop {
            if self.eat("<=") {
                let r = self.parse_add()?;
                left = (left <= r) as i64;
            } else if self.eat(">=") {
                let r = self.parse_add()?;
                left = (left >= r) as i64;
            } else if self.eat("<") {
                let r = self.parse_add()?;
                left = (left < r) as i64;
            } else if self.eat(">") {
                let r = self.parse_add()?;
                left = (left > r) as i64;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_add(&mut self) -> Result<i64, String> {
        let mut left = self.parse_mul()?;
        loop {
            if self.eat("+") {
                left = left.wrapping_add(self.parse_mul()?);
            } else if self.eat("-") {
                left = left.wrapping_sub(self.parse_mul()?);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_mul(&mut self) -> Result<i64, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.eat("*") {
                left = left.wrapping_mul(self.parse_unary()?);
            } else if self.eat("/") {
                let r = self.parse_unary()?;
                if r == 0 {
                    return Err("#if 表达式除零".to_string());
                }
                left /= r;
            } else if self.eat("%") {
                let r = self.parse_unary()?;
                if r == 0 {
                    return Err("#if 表达式取模零".to_string());
                }
                left %= r;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<i64, String> {
        if self.eat("!") {
            return Ok((self.parse_unary()? == 0) as i64);
        }
        if self.eat("-") {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.eat("+") {
            return self.parse_unary();
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, String> {
        if self.eat("(") {
            let v = self.parse_or()?;
            if !self.eat(")") {
                return Err("#if 表达式缺少右括号".to_string());
            }
            return Ok(v);
        }
        match self.peek() {
            Some(s) => {
                let v = parse_c_int(s)
                    .ok_or_else(|| format!("#if 表达式包含无法求值的记号 `{}`", s))?;
                self.pos += 1;
                Ok(v)
            }
            None => Err("#if 表达式意外结束".to_string()),
        }
    }
}

/// 解析 C 预处理整数字面量（十进制/0x/0b/0 八进制 + u/l 后缀）。
fn parse_c_int(s: &str) -> Option<i64> {
    let t = s.trim_end_matches(['u', 'U', 'l', 'L']);
    let (neg, body) = match t.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, t),
    };
    if body.is_empty() {
        return None;
    }
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(b) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        i64::from_str_radix(b, 2).ok()?
    } else if body.len() > 1 && body.starts_with('0') {
        i64::from_str_radix(&body[1..], 8).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}
