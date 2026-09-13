//! 智能补全引擎 v2
//!
//! 利用现有编译管线的 AST 快照 + 轻量级 Token 增量扫描，
//! 提供语义感知的代码补全：成员访问、类型上下文、表达式上下文、
//! 格式字符串、预处理指令等。

use crate::compiler::ast::ProgramNode;
use crate::compiler::lexer::{Lexer, Token, TokenType};
use crate::session::Session;

// ============================================================================
// 数据类型
// ============================================================================

/// 补全候选（FRB 友好，将在 api/vitro.rs 中重新包装）
#[derive(Debug, Clone)]
pub struct CompletionCandidate {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub documentation: String,
    pub insert_text: String,
    pub sort_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Variable,
    Function,
    Struct,
    Union,
    Enum,
    Typedef,
    Field,
    Keyword,
    Macro,
    Snippet,
    FormatSpecifier,
    Operator,
}

impl CompletionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompletionKind::Variable => "variable",
            CompletionKind::Function => "function",
            CompletionKind::Struct => "struct",
            CompletionKind::Union => "union",
            CompletionKind::Enum => "enum",
            CompletionKind::Typedef => "typedef",
            CompletionKind::Field => "field",
            CompletionKind::Keyword => "keyword",
            CompletionKind::Macro => "macro",
            CompletionKind::Snippet => "snippet",
            CompletionKind::FormatSpecifier => "format",
            CompletionKind::Operator => "operator",
        }
    }
}

/// 编译快照：在每次成功编译时从 AST 提取并持久化到 Session
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompletionSnapshot {
    pub functions: Vec<SnapshotFunc>,
    pub globals: Vec<SnapshotVar>,
    pub structs: Vec<SnapshotStruct>,
    pub unions: Vec<SnapshotStruct>,
    pub typedefs: Vec<SnapshotTypedef>,
    pub enums: Vec<SnapshotEnum>,
    pub macros: Vec<String>, // #define 定义的宏名称
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotFunc {
    pub name: String,
    pub return_type: String,
    pub params: Vec<String>,
    pub is_static: bool,
    pub filename: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotVar {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotStruct {
    pub name: String,
    pub fields: Vec<(String, String)>, // (name, type_string)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotTypedef {
    pub name: String,
    pub underlying: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotEnum {
    pub name: String,
    pub variants: Vec<String>,
}

/// 轻量扫描发现的局部符号
#[derive(Debug, Clone)]
struct LocalSymbol {
    name: String,
    ty: String,
    scope_depth: usize,
    #[allow(dead_code)]
    is_param: bool,
}

/// 表达式上下文提示
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprHint {
    General,
    IfCondition,
    AssignRhs,
    MallocArg,
    ForCondition,
}

/// 补全上下文
#[derive(Debug, Clone)]
pub enum CompletionContext {
    /// `expr.` 或 `expr->`（expr 可能为链式如 `a.b.c`）
    MemberAccess { expr: String, is_pointer: bool },
    /// 类型位置（如 `int |`, `struct |`）
    TypePosition,
    /// 预处理
    Preprocessor,
    /// printf / scanf 等格式字符串内
    FormatString { func_name: String },
    /// 一般表达式（含上下文提示）
    Expression { hint: ExprHint },
}

// ============================================================================
// 公开 API
// ============================================================================

/// 从 AST 构建补全快照（在编译管线成功后调用）
pub fn build_snapshot(program: &ProgramNode) -> CompletionSnapshot {
    build_snapshot_impl(program)
}

/// 从当前源码实时解析提取符号快照
///
/// **利用自研 Parser 的错误恢复能力**：即使源码不完整/含语法错误，
/// 自研 Parser 的 `synchronize()` + checkpoint/rollback 机制仍会返回
/// 包含已成功解析部分的 `ProgramNode`。我们从中提取 struct/union/
/// function/global 符号，作为编译快照的实时补充。
///
/// 这比纯 Token 扫描更精确，因为 Parser 能正确识别嵌套类型、
/// 函数签名、数组维度等复杂语法结构。
pub fn build_snapshot_from_source(source: &str) -> CompletionSnapshot {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = crate::compiler::parser::Parser::new(tokens).parse();
    match program {
        Some(p) => build_snapshot_impl(&p),
        None => CompletionSnapshot::default(),
    }
}

fn build_snapshot_impl(program: &ProgramNode) -> CompletionSnapshot {
    let mut snapshot = CompletionSnapshot::default();

    // 结构体 / 联合体
    for s in &program.structs {
        let fields: Vec<(String, String)> = s.fields.iter().map(|f| (f.name.clone(), f.ty.to_string())).collect();
        snapshot.structs.push(SnapshotStruct { name: s.name.clone(), fields });
    }
    for u in &program.unions {
        let fields: Vec<(String, String)> = u.fields.iter().map(|f| (f.name.clone(), f.ty.to_string())).collect();
        snapshot.unions.push(SnapshotStruct { name: u.name.clone(), fields });
    }

    // 全局变量
    for g in &program.globals {
        snapshot.globals.push(SnapshotVar {
            name: g.name.clone(),
            ty: g.ty.to_string(),
        });
    }

    // 函数（含参数签名）
    for f in &program.funcs {
        let params: Vec<String> = f.params.iter().map(|p| format!("{} {}", p.ty, p.name)).collect();
        snapshot.functions.push(SnapshotFunc {
            name: f.name.clone(),
            return_type: f.return_type.to_string(),
            params,
            is_static: f.is_static,
            filename: f.source_file.clone(),
        });
    }

    // 枚举（从 globals 中提取 enum 类型，目前 ast 中没有独立 enum decl，
    // 但 parser 会把 enum 当作 int typedef；我们扫描全局变量中的 enum 声明）
    // 注意：Parser 会把 `enum Color { RED, GREEN }` 解析为 typedef int Color + 全局常量
    // 因此这里不单独处理 enum 定义，而是在 scan_local_macros 中处理 #define

    snapshot
}

/// 合并两个快照：以 `compiled` 为优先，`live` 补充缺失的符号
fn merge_snapshots(compiled: &CompletionSnapshot, live: &CompletionSnapshot) -> CompletionSnapshot {
    let mut merged = compiled.clone();

    // 用 HashSet 去重（基于名称）
    let mut func_names: std::collections::HashSet<String> = merged.functions.iter().map(|f| f.name.clone()).collect();
    for f in &live.functions {
        if func_names.insert(f.name.clone()) {
            merged.functions.push(f.clone());
        }
    }

    let mut global_names: std::collections::HashSet<String> = merged.globals.iter().map(|g| g.name.clone()).collect();
    for g in &live.globals {
        if global_names.insert(g.name.clone()) {
            merged.globals.push(g.clone());
        }
    }

    let mut struct_names: std::collections::HashSet<String> = merged.structs.iter().map(|s| s.name.clone()).collect();
    for s in &live.structs {
        if struct_names.insert(s.name.clone()) {
            merged.structs.push(s.clone());
        }
    }

    let mut union_names: std::collections::HashSet<String> = merged.unions.iter().map(|u| u.name.clone()).collect();
    for u in &live.unions {
        if union_names.insert(u.name.clone()) {
            merged.unions.push(u.clone());
        }
    }

    // typedef / enum / macros：同样 live 补充
    let mut typedef_names: std::collections::HashSet<String> = merged.typedefs.iter().map(|t| t.name.clone()).collect();
    for t in &live.typedefs {
        if typedef_names.insert(t.name.clone()) {
            merged.typedefs.push(t.clone());
        }
    }

    let mut enum_names: std::collections::HashSet<String> = merged.enums.iter().map(|e| e.name.clone()).collect();
    for e in &live.enums {
        if enum_names.insert(e.name.clone()) {
            merged.enums.push(e.clone());
        }
    }

    let mut macro_names: std::collections::HashSet<String> = merged.macros.iter().cloned().collect();
    for m in &live.macros {
        if macro_names.insert(m.clone()) {
            merged.macros.push(m.clone());
        }
    }

    merged
}

/// 获取光标位置的补全候选
///
/// # 参数
/// - `session`: 当前 Session，用于读取编译快照
/// - `source`: 当前文件完整源码
/// - `line`: 0-based 行号
/// - `column`: 0-based 列号
/// - `prefix`: 当前已输入的前缀（由前端提取）
pub fn get_completion_candidates(
    session: &Session,
    source: &str,
    line: usize,
    column: usize,
    prefix: &str,
) -> Vec<CompletionCandidate> {
    // === 核心优化：实时解析源码获取符号快照 ===
    // 利用自研 Parser 的错误恢复能力，即使代码不完整也能提取部分符号。
    // 与编译快照合并，确保编译成功的精确信息优先，同时补充实时编辑中的新符号。
    let compiled_snapshot = &session.compile.completion_snapshot;
    let live_snapshot = build_snapshot_from_source(source);
    let snapshot = merge_snapshots(compiled_snapshot, &live_snapshot);

    let context = detect_context(source, line, column);

    let mut candidates: Vec<CompletionCandidate> = match &context {
        CompletionContext::MemberAccess { expr, is_pointer } => {
            complete_members(session, &snapshot, source, line, column, expr, *is_pointer)
        }
        CompletionContext::TypePosition => complete_types(&snapshot, prefix),
        CompletionContext::Preprocessor => complete_preprocessor(prefix),
        CompletionContext::FormatString { func_name } => complete_format_string(func_name, prefix),
        CompletionContext::Expression { hint } => {
            complete_expression(session, &snapshot, source, line, column, prefix, *hint)
        }
    };

    // 统一按前缀匹配过滤（上下文相关的已经预过滤过，这里做安全兜底）
    if !prefix.is_empty() && !matches!(context, CompletionContext::FormatString { .. }) {
        let lower_prefix = prefix.to_lowercase();
        candidates.retain(|c| c.filter_text().to_lowercase().starts_with(&lower_prefix));
    }

    // 排序
    candidates.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));

    // 限制数量（前端 UI 性能）
    candidates.truncate(50);

    candidates
}

// ============================================================================
// 上下文检测
// ============================================================================

// ============================================================================
// 局部符号增量扫描
// ============================================================================

/// 轻量级 token 扫描，提取光标所在作用域的局部变量和参数
fn scan_local_symbols(source: &str, target_line: usize, _target_column: usize) -> Vec<LocalSymbol> {
    let (tokens, _) = Lexer::new(source).tokenize();
    let mut locals = Vec::new();
    let mut scope_depth: usize = 0;
    let mut i = 0;

    while i < tokens.len() {
        let tok = &tokens[i];

        // 只扫描到目标行（包含目标行）
        if tok.line > target_line as i32 + 1 {
            break;
        }

        match tok.ty {
            TokenType::LBrace => scope_depth += 1,
            TokenType::RBrace => {
                scope_depth = scope_depth.saturating_sub(1);
                // 清除离开作用域的局部变量
                locals.retain(|l: &LocalSymbol| l.scope_depth <= scope_depth);
            }
            TokenType::Int
            | TokenType::Char
            | TokenType::Float
            | TokenType::Double
            | TokenType::Void
            | TokenType::Long
            | TokenType::Short
            | TokenType::Signed
            | TokenType::Unsigned
            | TokenType::Const
            | TokenType::Struct
            | TokenType::Union
            | TokenType::Enum => {
                // 尝试解析类型 + 变量名 模式
                if let Some((ty_str, names, new_i, is_param)) = try_parse_declaration(&tokens, i, scope_depth) {
                    for (name, _init) in names {
                        locals.push(LocalSymbol {
                            name,
                            ty: ty_str.clone(),
                            scope_depth,
                            is_param,
                        });
                    }
                    i = new_i;
                    continue;
                }
            }
            _ => {}
        }

        i += 1;
    }

    locals
}

/// 尝试从 token[i]（类型关键字）开始解析声明
/// 返回 (type_string, vec<(name, has_init)>, next_index, is_param)
#[allow(clippy::type_complexity)]
fn try_parse_declaration(
    tokens: &[Token],
    start: usize,
    _scope_depth: usize,
) -> Option<(String, Vec<(String, bool)>, usize, bool)> {
    let mut i = start;
    let mut type_parts = Vec::new();

    // 阶段 1：收集类型信息
    while i < tokens.len() {
        match tokens[i].ty {
            TokenType::Int
            | TokenType::Char
            | TokenType::Float
            | TokenType::Double
            | TokenType::Void
            | TokenType::Long
            | TokenType::Short
            | TokenType::Signed
            | TokenType::Unsigned
            | TokenType::Const => {
                type_parts.push(tokens[i].text.clone());
                i += 1;
            }
            TokenType::Struct | TokenType::Union | TokenType::Enum => {
                type_parts.push(tokens[i].text.clone());
                i += 1;
                if i < tokens.len() && tokens[i].ty == TokenType::Identifier {
                    type_parts.push(tokens[i].text.clone());
                    i += 1;
                }
            }
            TokenType::Star => {
                type_parts.push("*".to_string());
                i += 1;
            }
            TokenType::Identifier => {
                if type_parts.is_empty() {
                    // typedef 别名，如 `Point p;`
                    type_parts.push(tokens[i].text.clone());
                    i += 1;
                    break;
                }
                // type_parts 非空时，此 identifier 大概率是变量名，停止类型收集
                break;
            }
            _ => break,
        }
    }

    if type_parts.is_empty() {
        return None;
    }

    // 阶段 2：解析变量名列表（i 当前指向第一个变量名）
    if i >= tokens.len() || tokens[i].ty != TokenType::Identifier {
        return None;
    }

    let ty_str = type_parts.join(" ");
    let mut names = Vec::new();
    let mut is_param = false;

    // 解析逗号分隔的多变量：Type name1, name2, name3;
    loop {
        if i >= tokens.len() || tokens[i].ty != TokenType::Identifier {
            break;
        }
        let name = tokens[i].text.clone();
        i += 1;

        // 跳过数组维度 [N]
        while i < tokens.len() && tokens[i].ty == TokenType::LBracket {
            i += 1;
            while i < tokens.len() && tokens[i].ty != TokenType::RBracket {
                i += 1;
            }
            if i < tokens.len() {
                i += 1;
            }
        }

        // 检查是否有初始化 = ...
        let has_init = i < tokens.len() && (tokens[i].ty == TokenType::Assign || tokens[i].ty == TokenType::LParen);

        names.push((name, has_init));

        // 如果是函数定义/声明的参数列表
        if i < tokens.len() && tokens[i].ty == TokenType::Comma {
            i += 1;
            is_param = true;
            continue;
        }

        break;
    }

    // Consume until semicolon or RParen or LBrace
    while i < tokens.len() {
        match tokens[i].ty {
            TokenType::Semicolon | TokenType::RParen | TokenType::LBrace => {
                if tokens[i].ty == TokenType::RParen {
                    is_param = true;
                }
                i += 1;
                break;
            }
            _ => i += 1,
        }
    }

    Some((ty_str, names, i, is_param))
}

// ============================================================================
// 补全生成器
// ============================================================================

pub fn update_completion_snapshot(session: &mut Session, program: &ProgramNode) {
    session.compile.completion_snapshot = build_snapshot(program);
}

pub mod candidates;
mod context;
pub use candidates::*;
pub use context::*;
