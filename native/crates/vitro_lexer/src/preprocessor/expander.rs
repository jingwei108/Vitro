//! E2：token 树转录展开器（Rust `macro_rules!` 架构同族）。
//!
//! - **展开深度保险丝**（64）：自引用/互引用宏超过深度即报 `E1017`，不再递归；
//! - **展开链教学追踪**：顶层宏展开记录 `NAME(args) ⇒ 结果拼写`（白箱教学层，
//!   容量封顶；`#if` 表达式内不记录）；
//! - **宏参数副作用检测**：参数在宏体出现 ≥2 次且实参含 `++`/`--`/赋值时记
//!   `W1019` 警告（`SQ(i++)` 双重求值是 C 宏经典陷阱）；
//! - **自引用停止**：展开栈查重（等价红蓝标记，C99 6.10.3.1 的教学子集实现）；
//! - **`#` / `##`**：委托 [`super::splice`]（操作数不预先展开，C 规则）；
//! - **`do { ... } while(0)` 自动包装**（H01 行为契约）：参数化宏体为大括号块
//!   且调用后紧跟分号时启用。

use super::splice;
use super::{lex_error, ExpandCtx, EXPAND_DEPTH_FUSE};
use crate::token::{Token, TokenType};
use crate::Lexer;
use vitro_shared::ErrorCode;

pub(crate) fn expand_tokens(ctx: &mut ExpandCtx, tokens: &[Token], depth: usize) -> Vec<Token> {
    expand_inner(ctx, tokens, depth, &mut std::collections::HashSet::new())
}

fn expand_inner(
    ctx: &mut ExpandCtx,
    tokens: &[Token],
    depth: usize,
    expanding: &mut std::collections::HashSet<String>,
) -> Vec<Token> {
    if budget_hit(ctx, tokens) {
        return Vec::new();
    }
    // U1#6：深度保险丝移入每层递归入口。此前只在 depth=0 的公开入口检查
    // = 死代码：自引用/互引用靠 expanding 集合停止，但**不同名**对象宏链
    // `A0→A1→…→A5000` 不触发查重，逐层递归直至栈溢出（实测 5000 层崩）。
    if depth >= EXPAND_DEPTH_FUSE {
        if !ctx.depth_exceeded {
            ctx.depth_exceeded = true;
            ctx.errors.push(lex_error(
                format!(
                    "宏展开深度超过保险丝（{} 层）。请检查是否存在过长的宏展开链（如 #define A0 A1、#define A1 A2 依次接力）",
                    EXPAND_DEPTH_FUSE
                ),
                tokens.first().map(|t| t.line).unwrap_or(0),
                0,
                ErrorCode::E1017_ExpandDepthExceeded,
            ));
        }
        return tokens.to_vec();
    }
    let mut result: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i];
        if tok.ty != TokenType::Identifier {
            result.push(tok.clone());
            i += 1;
            continue;
        }
        let Some(mdef) = ctx.macros.get(&tok.text).cloned() else {
            result.push(tok.clone());
            i += 1;
            continue;
        };
        if expanding.contains(&tok.text) {
            // 自引用停止（展开栈查重）：C99 6.10.3.1 的"蓝漆"教学子集等价物
            result.push(tok.clone());
            i += 1;
            continue;
        }

        if mdef.params.is_empty() {
            expanding.insert(tok.text.clone());
            // `#`/`##` 在对象宏体内同样受限处理（无参数表）
            let (substituted, paste_err) = splice::substitute_params(&mdef.body, &[], None, tok.line);
            if let Some(msg) = paste_err {
                ctx.errors.push(lex_error(msg, tok.line, tok.column, ErrorCode::E1016_TokenPasteInvalid));
            }
            let expanded = expand_inner(ctx, &substituted, depth + 1, expanding);
            record_trace(ctx, &tok.text, None, &expanded, depth);
            expanding.remove(&tok.text);
            for mut mt in expanded {
                mt.line = tok.line;
                mt.column = tok.column;
                result.push(mt);
            }
            i += 1;
            continue;
        }

        // 参数化宏：下一个 token 必须是 '('，否则按普通标识符保留
        if tokens.get(i + 1).map(|t| t.ty) != Some(TokenType::LParen) {
            result.push(tok.clone());
            i += 1;
            continue;
        }

        // 捕获实参（平衡括号内按逗号切分）
        let mut args: Vec<Vec<Token>> = Vec::new();
        let mut current_arg: Vec<Token> = Vec::new();
        let mut depth_paren = 1;
        let mut j = i + 2;
        while j < tokens.len() && depth_paren > 0 {
            match tokens[j].ty {
                TokenType::LParen => {
                    depth_paren += 1;
                    current_arg.push(tokens[j].clone());
                }
                TokenType::RParen => {
                    depth_paren -= 1;
                    if depth_paren == 0 {
                        break;
                    }
                    current_arg.push(tokens[j].clone());
                }
                TokenType::Comma if depth_paren == 1 => {
                    args.push(std::mem::take(&mut current_arg));
                }
                _ => current_arg.push(tokens[j].clone()),
            }
            j += 1;
        }
        args.push(current_arg);

        if args.len() != mdef.params.len() {
            // 参数数量不匹配：不展开，保留原 token（与既有行为一致）
            result.push(tok.clone());
            i += 1;
            continue;
        }

        // 白箱教学层：宏参数副作用检测
        warn_side_effects(ctx, &tok.text, &mdef, &args, tok);

        // C99 §6.10.3.1：实参在替换前**先行完整展开**（本宏自身名此时尚未涂蓝，
        // 因此 MAX(MAX(1,5),3) 这类同宏嵌套正确工作）；体含 `#`/`##` 时例外——
        // 操作数必须用未展开实参（C99 §6.10.3.3）。
        let body_has_operators = mdef
            .body
            .iter()
            .any(|t| t.ty == TokenType::Hash || t.ty == TokenType::HashHash);
        let subst_args: Vec<Vec<Token>> = if body_has_operators {
            args.clone()
        } else {
            args.iter().map(|a| expand_inner(ctx, a, depth + 1, expanding)).collect()
        };

        expanding.insert(tok.text.clone());
        // `#` 字符串化 / `##` 拼接：操作数取原始实参；普通参数取（已展开的）实参
        let (substituted, paste_err) =
            splice::substitute_params_ext(&mdef.body, &mdef.params, &args, &subst_args, tok.line);
        if let Some(msg) = paste_err {
            ctx.errors.push(lex_error(msg, tok.line, tok.column, ErrorCode::E1016_TokenPasteInvalid));
        }

        // H01 行为契约：宏体为大括号块且调用后紧跟分号 → 包装 do{...}while(0)
        let body_is_brace_block = mdef.body.first().map(|t| t.ty) == Some(TokenType::LBrace)
            && mdef.body.last().map(|t| t.ty) == Some(TokenType::RBrace);
        let followed_by_semicolon = tokens.get(j + 1).map(|t| t.ty) == Some(TokenType::Semicolon);
        let mut substituted = substituted;
        if body_is_brace_block && followed_by_semicolon {
            substituted.insert(0, Token { ty: TokenType::Do, text: "do".to_string(), line: tok.line, column: tok.column });
            substituted.push(Token { ty: TokenType::While, text: "while".to_string(), line: tok.line, column: tok.column });
            substituted.push(Token { ty: TokenType::LParen, text: "(".to_string(), line: tok.line, column: tok.column });
            substituted.push(Token { ty: TokenType::Number, text: "0".to_string(), line: tok.line, column: tok.column });
            substituted.push(Token { ty: TokenType::RParen, text: ")".to_string(), line: tok.line, column: tok.column });
        }

        let expanded = expand_inner(ctx, &substituted, depth + 1, expanding);
        record_trace(ctx, &tok.text, Some(&args), &expanded, depth);
        expanding.remove(&tok.text);
        for mut mt in expanded {
            mt.line = tok.line;
            mt.column = tok.column;
            result.push(mt);
        }
        i = j + 1;
        continue;
    }
    // U1#6：按累计字节计费（token 文本长度 +1 分隔），token 个数口径下
    // 4KB 字面量 × 2^14 = 16384 个 token 不触预算但实际产出 67MB。
    ctx.emitted = ctx
        .emitted
        .saturating_add(result.iter().map(|t| t.text.len() + 1).sum::<usize>());
    result
}

/// 规模保险丝：深度限深不限宽（`REP(x) x x` 每层产出翻倍），累计产出
/// 字节数超预算即熔断——错误只报一次。
fn budget_hit(ctx: &mut ExpandCtx, tokens: &[Token]) -> bool {
    if ctx.emitted <= super::EXPAND_BYTE_BUDGET {
        return false;
    }
    if !ctx.budget_exhausted {
        ctx.budget_exhausted = true;
        ctx.errors.push(lex_error(
            format!(
                "宏展开规模超过预算（累计产出超过 {} 字节）。请检查是否存在指数级展开的宏（如 #define REP(x) x x 的深层嵌套，或大字面量被反复复制）",
                super::EXPAND_BYTE_BUDGET
            ),
            tokens.first().map(|t| t.line).unwrap_or(0),
            0,
            ErrorCode::E1017_ExpandDepthExceeded,
        ));
    }
    true
}

/// 展开链教学追踪（仅最外层调用、非表达式模式记录，容量封顶在 push 处）。
fn record_trace(ctx: &mut ExpandCtx, name: &str, args: Option<&[Vec<Token>]>, result: &[Token], depth: usize) {
    if depth != 0 || ctx.expr_mode {
        return;
    }
    let call = match args {
        Some(a) => format!(
            "{}({})",
            name,
            a.iter().map(|arg| spelling(arg)).collect::<Vec<_>>().join(", ")
        ),
        None => name.to_string(),
    };
    super::push_trace(ctx.trace, format!("{} ⇒ {}", call, spelling(result)));
}

fn spelling(toks: &[Token]) -> String {
    let mut s = toks.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    // U1#6：单条 trace 长度封顶——条数封顶（TEACHING_TRACE_CAP）不防单条，
    // 病态展开的结果拼写拼进一条 trace 同样是病态内存。
    if s.len() > super::TRACE_ENTRY_MAX_CHARS {
        s.truncate(super::TRACE_ENTRY_MAX_CHARS);
        s.push_str(" …(截断)");
    }
    s
}

/// W1019：参数在宏体出现 ≥2 次且实参含副作用运算符。
fn warn_side_effects(
    ctx: &mut ExpandCtx,
    macro_name: &str,
    mdef: &super::MacroDef,
    args: &[Vec<Token>],
    call: &Token,
) {
    for (idx, param) in mdef.params.iter().enumerate() {
        let occurrences = mdef
            .body
            .iter()
            .filter(|bt| bt.ty == TokenType::Identifier && bt.text == *param)
            .count();
        if occurrences < 2 {
            continue;
        }
        let Some(arg) = args.get(idx) else { continue };
        let has_side_effect = arg.iter().any(|t| {
            matches!(
                t.ty,
                TokenType::Increment
                    | TokenType::Decrement
                    | TokenType::Assign
                    | TokenType::PlusAssign
                    | TokenType::MinusAssign
                    | TokenType::StarAssign
                    | TokenType::SlashAssign
                    | TokenType::PercentAssign
            )
        });
        if has_side_effect {
            ctx.warnings.push(crate::token::LexerWarning {
                message: format!(
                    "宏 '{}' 的参数 '{}' 在宏体中出现 {} 次，而实参含副作用运算符（++/--/赋值）——展开后该副作用会执行多次，结果可能与预期不符（C 宏经典陷阱）",
                    macro_name, param, occurrences
                ),
                line: call.line,
                column: call.column,
                code: vitro_shared::ErrorCode::W1019_MacroArgSideEffect as i32,
            });
        }
    }
}

impl Lexer {
    /// 全量宏展开（tokenize 收尾阶段调用；公开路径的唯一入口）。
    pub(crate) fn expand_macros(&mut self, tokens: Vec<Token>) -> Vec<Token> {
        let mut ctx = ExpandCtx {
            macros: &self.macros,
            errors: &mut self.errors,
            warnings: &mut self.warnings,
            trace: &mut self.preprocessor_trace,
            expr_mode: false,
            emitted: 0,
            budget_exhausted: false,
            depth_exceeded: false,
        };
        expand_tokens(&mut ctx, &tokens, 0)
    }
}
