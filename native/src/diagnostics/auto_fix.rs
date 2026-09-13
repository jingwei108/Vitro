//! 自动修复应用器：根据诊断中的结构化修复信息，对源码执行替换/插入/删除。
//!
//! 来源：前端切割（2026-09-11）时从 FRB 出口层（`src/api/vitro.rs`）迁入——
//! `apply_fix` 的本体此前只存在于 FRB api 模块，前端迁出会连带丢失该能力。
//! 现作为语言中立层的一部分，serve / capi / wasm 三个出口均可复用。
//!
//! fix_kind: 0=None, 1=ReplaceText, 2=InsertText, 3=DeleteText, 4=ManualHint。

use crate::session::Diagnostic;

pub fn apply_fix(source: String, diag: Diagnostic) -> Option<String> {
    match diag.fix_kind {
        1 => apply_replace(&source, &diag),
        2 => apply_insert(&source, &diag),
        3 => apply_delete(&source, &diag),
        _ => None,
    }
}

/// 将诊断列号安全转换为目标行的字节偏移。
///
/// 坐标来源混杂：lexer/错误目录按字节计列，部分诊断与前端按字符计列。
/// 含中文（UTF-8 多字节）的行上，按错误语义切片会落在字符中间导致 panic。
/// 策略：优先按字节边界解释（与生成侧一致）；落点不是字符边界时退回按
/// 字符索引解释；最终 clamp 到行内，保证任何输入都不会在切片时 panic。
fn safe_byte_col(line: &str, col: usize) -> usize {
    if col <= line.len() && line.is_char_boundary(col) {
        return col;
    }
    match line.char_indices().nth(col) {
        Some((idx, _)) => idx,
        None => line.len(),
    }
}

fn apply_replace(source: &str, diag: &Diagnostic) -> Option<String> {
    let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
    let start_line = diag.replace_start_line as usize;
    let end_line = diag.replace_end_line as usize;

    if start_line == 0 || end_line == 0 || start_line > lines.len() || end_line > lines.len() {
        return None;
    }
    let start_idx = start_line - 1;
    let end_idx = end_line - 1;
    let start_col = safe_byte_col(&lines[start_idx], diag.replace_start_column as usize);
    let end_col = safe_byte_col(&lines[end_idx], diag.replace_end_column as usize);
    if start_col > end_col && start_idx == end_idx {
        return None;
    }

    let before = lines[start_idx][..start_col].to_string();
    let after = lines[end_idx][end_col..].to_string();
    let mut new_line = before;
    new_line.push_str(&diag.replacement_text);
    new_line.push_str(&after);

    lines.drain(start_idx..=end_idx);
    lines.insert(start_idx, new_line);
    Some(lines.join("\n"))
}

fn apply_insert(source: &str, diag: &Diagnostic) -> Option<String> {
    let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
    let start_line = diag.replace_start_line as usize;

    if start_line == 0 || start_line > lines.len() {
        return None;
    }

    let start_col = safe_byte_col(&lines[start_line - 1], diag.replace_start_column as usize);
    lines[start_line - 1].insert_str(start_col, &diag.replacement_text);
    Some(lines.join("\n"))
}

fn apply_delete(source: &str, diag: &Diagnostic) -> Option<String> {
    let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
    let start_line = diag.replace_start_line as usize;
    let end_line = diag.replace_end_line as usize;

    if start_line == 0 || end_line == 0 || start_line > lines.len() || end_line > lines.len() {
        return None;
    }
    let start_idx = start_line - 1;
    let end_idx = end_line - 1;
    let start_col = safe_byte_col(&lines[start_idx], diag.replace_start_column as usize);
    let end_col = safe_byte_col(&lines[end_idx], diag.replace_end_column as usize);
    if start_col > end_col && start_idx == end_idx {
        return None;
    }

    if start_idx == end_idx {
        lines[start_idx].replace_range(start_col..end_col, "");
    } else {
        let before = lines[start_idx][..start_col].to_string();
        let after = lines[end_idx][end_col..].to_string();
        let mut new_line = before;
        new_line.push_str(&after);
        lines.drain(start_idx..=end_idx);
        lines.insert(start_idx, new_line);
    }
    Some(lines.join("\n"))
}
