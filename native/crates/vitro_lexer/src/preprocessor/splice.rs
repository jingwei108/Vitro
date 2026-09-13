//! E2：受限 token 操作——`#` 字符串化与 `##` 拼接。
//!
//! 按 C99 §6.10.3.1/§6.10.3.3 的教学子集实现：
//! - `#` 后跟参数：实参 token 的**原始拼写**（未经宏展开）串化为字符串字面量；
//! - `a ## b`：左操作数的**最后一个** token 与右操作数的**第一个** token 拼接，
//!   其余 token 原序保留；拼接结果必须能词法化为**恰好一个**合法 token
//!   （"结果必须合法"——失败记 `E1016`，fail-soft 保留左操作数）；
//! - 两个操作数都不做宏展开（C 规则特例，不能照抄普通展开流程）。
//!
//! 诚实放弃（进 spec §2.11）：空实参的 placemarker 语义、拼接出预处理数字的
//! 边界形态、`##` 递归构造元编程。

use crate::token::{Token, TokenType};

/// 宏体参数替换 + `#`/`##` 处理（展开前单遍）。
///
/// `params`/`args`：函数式宏的参数名与**未展开**实参；对象宏传 `None`。
/// 返回（替换后的 token 序列, 可选拼接错误消息）。
pub(crate) fn substitute_params(
    body: &[Token],
    params: &[String],
    args: Option<&Vec<Vec<Token>>>,
    call_line: i32,
) -> (Vec<Token>, Option<String>) {
    let empty = Vec::new();
    substitute_params_ext(body, params, args.unwrap_or(&empty), args.unwrap_or(&empty), call_line)
}

/// 双实参形态：`raw_args` 供 `#`/`##` 操作数（C 规则：不展开），
/// `expanded_args` 供普通参数位置（C99 §6.10.3.1：实参先行展开）。
pub(crate) fn substitute_params_ext(
    body: &[Token],
    params: &[String],
    raw_args: &[Vec<Token>],
    expanded_args: &[Vec<Token>],
    call_line: i32,
) -> (Vec<Token>, Option<String>) {
    let mut out: Vec<Token> = Vec::new();
    let mut paste_err: Option<String> = None;
    let mut i = 0;

    fn arg_of<'a>(name: &str, params: &[String], src: &'a [Vec<Token>]) -> Option<&'a Vec<Token>> {
        params.iter().position(|p| p == name).and_then(|idx| src.get(idx))
    }

    while i < body.len() {
        let t = &body[i];

        // `#` 字符串化：必须紧跟参数名
        if t.ty == TokenType::Hash {
            match body.get(i + 1) {
                Some(n) if n.ty == TokenType::Identifier => {
                    if let Some(raw) = arg_of(&n.text, params, raw_args) {
                        let spelling = raw.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" ");
                        out.push(Token {
                            ty: TokenType::String,
                            text: spelling,
                            line: t.line,
                            column: t.column,
                        });
                        i += 2;
                        continue;
                    }
                    paste_err = Some(format!(
                        "# 字符串化的操作数必须是宏参数名，得到 `{}`",
                        n.text
                    ));
                    i += 2;
                    continue;
                }
                _ => {
                    paste_err = Some("# 后必须紧跟宏参数名".to_string());
                    i += 1;
                    continue;
                }
            }
        }

        // `##` 拼接：左取 out 末尾（参数位置替换的是**原始实参**，未展开），
        // 右取下一操作数的首 token
        if t.ty == TokenType::HashHash {
            let left = out.pop();
            let right_tokens: Vec<Token> = match body.get(i + 1) {
                Some(n) if n.ty == TokenType::Identifier => {
                    arg_of(&n.text, params, raw_args).cloned().unwrap_or_else(|| vec![n.clone()])
                }
                Some(n) => vec![n.clone()],
                None => Vec::new(),
            };
            match (left, right_tokens.split_first()) {
                (Some(l), Some((r_first, r_rest))) => {
                    let combined = format!("{}{}", l.text, r_first.text);
                    let (ptoks, perrs) = crate::Lexer::new(&combined).tokenize();
                    let ptoks: Vec<Token> = ptoks.into_iter().filter(|x| x.ty != TokenType::Eof).collect();
                    if perrs.is_empty() && ptoks.len() == 1 && ptoks[0].ty != TokenType::Unknown {
                        let mut pasted = ptoks;
                        pasted[0].line = l.line;
                        pasted[0].column = l.column;
                        out.push(pasted.remove(0));
                        out.extend(r_rest.iter().cloned());
                    } else {
                        // 结果必须合法：失败记 E1016，fail-soft 保留左操作数
                        paste_err = Some(format!(
                            "`{}` 拼接结果 `{}` 不是单个合法 token",
                            l.text, combined
                        ));
                        out.push(l);
                        out.extend(r_rest.iter().cloned());
                    }
                }
                _ => {
                    paste_err = Some("## 缺少拼接操作数".to_string());
                }
            }
            i += body.get(i + 1).map(|_| 2).unwrap_or(1);
            continue;
        }

        // 普通参数：替换为**先行展开的实参**（C99 §6.10.3.1）
        if t.ty == TokenType::Identifier {
            if let Some(exp) = arg_of(&t.text, params, expanded_args) {
                out.extend(exp.iter().cloned());
                i += 1;
                continue;
            }
        }

        out.push(t.clone());
        i += 1;
    }

    let _ = call_line;
    (out, paste_err)
}
