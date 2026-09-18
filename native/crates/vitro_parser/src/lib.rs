//! Vitro 语法分析器。
//!
//! 从 `vitro_native::compiler::parser` 拆分而来，负责将 token 流解析为 AST。

// TODO(#D08): Parser 模块仍包含部分 C++ 语法入口，未来应完全迁移到 parser/cpp.rs。
use vitro_ast::*;
use vitro_lexer::{Token, TokenType};
use vitro_shared::ErrorCode;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: i32,
    pub column: i32,
    pub code: i32,
}

#[derive(Debug, Clone)]
pub(crate) enum DeclaratorNode {
    Base,
    Pointer(Box<DeclaratorNode>),
    Reference(Box<DeclaratorNode>, bool /* is_const */),
    RValueRef(Box<DeclaratorNode>),
    Array(Box<DeclaratorNode>, Option<Box<Expr>>),
    Function(Box<DeclaratorNode>, Vec<Param>, bool),
}

#[derive(Debug, Clone)]
enum DeclaratorSuffix {
    Array(Option<Box<Expr>>),
    Function(Vec<Param>, bool),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DeclaratorGuard {
    paren_depth: i32,
    ptr_count: i32,
    suffix_count: i32,
    cross_count: i32,
}

fn node_cross_count(node: &DeclaratorNode) -> i32 {
    match node {
        DeclaratorNode::Base => 0,
        DeclaratorNode::Pointer(inner) | DeclaratorNode::Reference(inner, _) | DeclaratorNode::RValueRef(inner) => {
            let add = match inner.as_ref() {
                DeclaratorNode::Array(_, _) | DeclaratorNode::Function(_, _, _) => 1,
                _ => 0,
            };
            node_cross_count(inner) + add
        }
        DeclaratorNode::Array(inner, _) | DeclaratorNode::Function(inner, _, _) => node_cross_count(inner),
    }
}


/// U1#10：统一回滚点——pos + errors + **anonymous_structs**。
/// 此前回滚只恢复前两者：试探解析失败后重解析把匿名 struct 二次 push
/// （命名 `__anon_struct_{pos}` 同位置同名 → "重复定义 E3002"），合法
/// 复合字面量 `(struct {int a;}){7}.a` 被误拒（clang 输出 7）。
#[derive(Clone, Copy)]
pub(crate) struct Rollback {
    pos: usize,
    errors_len: usize,
    anon_len: usize,
}

pub struct Parser {
    tokens: Vec<Token>,
    errors: Vec<ParseError>,
    typedef_names: HashMap<String, Type>,
    template_names: std::collections::HashSet<String>,
    pos: usize,
    anonymous_structs: Vec<StructDecl>,
    is_cpp_mode: bool,
    next_lambda_id: u64,
    /// C++ 嵌套类解析上下文：当前外层类名栈，如 ["Outer"] -> ["Outer", "Inner"]。
    current_class: Vec<String>,
    /// 递归下降深度计数（语句/表达式共享），见 MAX_PARSE_DEPTH。
    recursion_depth: i32,
}

/// 递归深度上限：防护粘贴/恶意输入（6 万层 `{{{{`、5 万层 `((((`）导致的
/// 栈溢出（SIGSEGV，无法被 catch_unwind 捕获，IDE 直接崩溃）。
/// 实测每层括号嵌套消耗 ~3KB 栈（完整二元/一元/后缀优先级链 + 大体积 Expr 帧），
/// 300 层即溢出 1MB 线程栈，因此上限必须远低于溢出阈值；合法教学代码的
/// 语句/表达式嵌套远达不到 64 层。
///
/// U1#8 计数口径更新：赋值/三目/一元/初始化列表四个链壳也计入本计数后，
/// 每层括号嵌套消耗 4 计数（primary + assign + ternary + unary 壳），上限
/// 相应 64 → 256——**语义与原"64 层语法嵌套"精确等价**（256/4），同时使
/// 无括号链式递归（赋值链每级 4 计数、一元链每级 2 计数）在 64/128 级
/// 触发（每级链的真实栈帧是完整优先级瀑布 ~13 帧，64 级 ≈ 400KB 安全）。
/// 注意各挂点共享计数是总量语义：混合嵌套形态的总和不得越界。
pub(crate) const MAX_PARSE_DEPTH: i32 = 256;

/// 后置 AST 深度预算（U1#8）：typeck / AST Drop / 快照等所有递归遍历的
/// 安全上界。合法教学代码的表达式/初始化深度极浅（两位数）；宏展开的
/// 算式链可达数百项（如嵌套 SUM 宏）；实测 5000 项左结合链崩在 typeck。
/// 512 对合法场景余量充足、对递归遍历安全（每层栈帧数百字节，
/// 512 层远低于 1MB 线程栈）。
pub(crate) const MAX_AST_DEPTH: usize = 512;

/// 声明符后缀（数组/函数）层级上限（P1，2026-09-18）：单个声明符的
/// 链式后缀数（`a[1][1]...`）。实测 release 1200 层存活、1300 层
/// interpret_declarator_node 递归栈溢出，取崩溃边界之下、合法深层之上。
/// 抽象声明符（`sizeof(int[1]x1300)`）同拦——该路径此前全免检，同样崩溃。
pub(crate) const MAX_DECLARATOR_SUFFIX: i32 = 1250;

/// 类型树深度预算（P1，2026-09-18）：MAX_AST_DEPTH 只覆盖 Expr/Stmt，
/// 深 Type 对它隐身（`int a[1]x1300` 的语句深度仅 2，类型深 1301）——
/// base_element_type / compute_type_size 等递归遍历点会随之栈溢出。
/// 与 MAX_DECLARATOR_SUFFIX 同值冗余：后者拦 parse 期收集，这里后置
/// 兜底 typedef 链等不经后缀收集的深 Type 构造路径。**不并入 512**：
/// Expr 递归安全线（typeck/Drop，实测 5000 项链崩溃）维持不动，而
/// 合法深层声明（1200 层实测存活）需要独立余量。
pub(crate) const MAX_TYPE_DEPTH: usize = 1250;

/// 全程序最大 AST 深度（函数体 / 全局初始化式取最大）；无内容返回 None。
fn program_max_depth(prog: &ProgramNode) -> Option<usize> {
    let mut max = 0usize;
    for g in &prog.globals {
        if let Some(init) = &g.init {
            max = max.max(vitro_ast::depth::expr_depth(init));
        }
    }
    for f in &prog.funcs {
        if let Some(body) = &f.body {
            max = max.max(vitro_ast::depth::stmt_depth(body));
        }
    }
    if max == 0 { None } else { Some(max) }
}

/// 全程序最大类型深度（P1）：globals / structs / unions / classes /
/// funcs（签名 + 函数体内 VarDecl）。深 Type 的源头都在声明点——
/// 表达式内出现的类型只引用已在声明点过检的类型，不需下钻 Expr。
fn program_max_type_depth(prog: &ProgramNode) -> usize {
    fn acc(t: &Type, max: &mut usize) {
        let d = vitro_ast::depth::type_depth(t);
        if d > *max { *max = d; }
    }
    fn acc_body(b: &Option<Stmt>, max: &mut usize) {
        if let Some(b) = b {
            let d = vitro_ast::depth::stmt_type_depth(b);
            if d > *max { *max = d; }
        }
    }
    let mut max = 0usize;
    for g in &prog.globals {
        acc(&g.ty, &mut max);
    }
    for sd in prog.structs.iter().chain(prog.unions.iter()) {
        for f in &sd.fields {
            acc(&f.ty, &mut max);
        }
    }
    // 嵌套类用显式工作栈遍历（测量器自身不递归，同 depth.rs 纪律）
    let mut class_stack: Vec<&ClassDecl> = prog.classes.iter().collect();
    while let Some(c) = class_stack.pop() {
        for m in &c.members {
            match m {
                ClassMember::Field { ty, .. } => acc(ty, &mut max),
                ClassMember::Method { ret, params, body, .. } => {
                    acc(ret, &mut max);
                    for p in params {
                        acc(&p.ty, &mut max);
                    }
                    acc_body(body, &mut max);
                }
                ClassMember::Constructor { params, body, .. } => {
                    for p in params {
                        acc(&p.ty, &mut max);
                    }
                    acc_body(body, &mut max);
                }
                ClassMember::Destructor { body, .. } => acc_body(body, &mut max),
                ClassMember::NestedClass { decl, .. } => class_stack.push(decl),
                ClassMember::NestedStruct { decl, .. } => {
                    for f in &decl.fields {
                        acc(&f.ty, &mut max);
                    }
                }
            }
        }
    }
    for f in &prog.funcs {
        acc(&f.return_type, &mut max);
        for p in &f.params {
            acc(&p.ty, &mut max);
        }
        acc_body(&f.body, &mut max);
    }
    max
}

impl Parser {
    /// 保存回滚点（pos / errors / anonymous_structs 三元快照，U1#10）
    pub(crate) fn save(&self) -> Rollback {
        Rollback {
            pos: self.pos,
            errors_len: self.errors.len(),
            anon_len: self.anonymous_structs.len(),
        }
    }

    /// 回滚到快照点（含 anonymous_structs 截断——消除重解析二次 push）
    pub(crate) fn restore(&mut self, rb: Rollback) {
        self.pos = rb.pos;
        self.errors.truncate(rb.errors_len);
        self.anonymous_structs.truncate(rb.anon_len);
    }

    pub fn new(tokens: Vec<Token>) -> Self {
        Self::with_mode(tokens, false)
    }

    pub fn with_mode(tokens: Vec<Token>, is_cpp_mode: bool) -> Self {
        let mut typedef_names = HashMap::new();
        let mut template_names = std::collections::HashSet::new();
        // 预定义 FILE 类型（stdio.h 中的不透明结构体指针）
        typedef_names.insert("FILE".to_string(), Type::void());
        // 预注册 C++ 内置容器类型名
        if is_cpp_mode {
            cpp::register_cpp_builtin_types(&mut typedef_names, &mut template_names);
        }
        Self {
            tokens,
            errors: Vec::new(),
            typedef_names,
            template_names,
            pos: 0,
            anonymous_structs: Vec::new(),
            is_cpp_mode,
            next_lambda_id: 0,
            current_class: Vec::new(),
            recursion_depth: 0,
        }
    }

    /// 进入一层递归下降（语句/表达式共用一个计数器）。
    /// 超限时记录错误并跳到文件尾——过深嵌套的输入已无恢复价值，
    /// 跳到 EOF 可让所有外层解析循环自然收敛结束。
    /// 返回 false 表示调用方应立即返回中性的占位节点。
    pub(crate) fn enter_depth(&mut self, what: &str) -> bool {
        if self.recursion_depth >= MAX_PARSE_DEPTH {
            self.errors.push(ParseError {
                message: format!("{}嵌套过深（超过 {} 层），已停止解析", what, MAX_PARSE_DEPTH),
                line: self.current().line,
                column: self.current().column,
                code: ErrorCode::E1006_UnsupportedFeature as i32,
            });
            self.pos = self.tokens.len().saturating_sub(1);
            false
        } else {
            self.recursion_depth += 1;
            true
        }
    }

    pub(crate) fn leave_depth(&mut self) {
        self.recursion_depth -= 1;
    }

    pub fn parse(mut self) -> (Option<ProgramNode>, Vec<ParseError>) {
        let prog = match self.parse_program() {
            Some(mut p) => {
                p.structs.append(&mut self.anonymous_structs);
                p
            }
            None => return (None, self.errors),
        };
        // U1#8 后置 AST 深度预算：构造期挂点（五通道 + 链长截断）拦已知
        // 溢出形状，这里迭代测量兜底未预料的新形状。超限时 mem::forget——
        // 递归 Drop 同样会溢出（左结合链 5000 项实测崩在 typeck/Drop），
        // 在"泄漏一次编译的 AST"与"崩溃整个进程"间选前者。
        if let Some(d) = program_max_depth(&prog) {
            if d > MAX_AST_DEPTH {
                self.errors.push(ParseError {
                    message: format!(
                        "AST 嵌套深度 {} 超过预算 {}：过深的表达式/初始化链会使后续处理栈溢出，请拆分表达式",
                        d, MAX_AST_DEPTH
                    ),
                    line: 1,
                    column: 1,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                std::mem::forget(prog);
                return (None, self.errors);
            }
        }
        // P1（2026-09-18）：类型深度预算——深 Type 对上面的 Expr/Stmt 预算
        // 隐身（病态 `int a[1]x1300` 的语句深度仅 2）。同 mem::forget 语义：
        // 递归 Drop 深类型链同样会溢出。
        let tmax = program_max_type_depth(&prog);
        if tmax > MAX_TYPE_DEPTH {
            self.errors.push(ParseError {
                message: format!(
                    "类型嵌套深度 {} 超过预算 {}：过深的类型链（数组后缀/typedef 链）会使后续处理栈溢出",
                    tmax, MAX_TYPE_DEPTH
                ),
                line: 1,
                column: 1,
                code: ErrorCode::E1006_UnsupportedFeature as i32,
            });
            std::mem::forget(prog);
            return (None, self.errors);
        }
        (Some(prog), self.errors)
    }

    // =========================================================================
    // Token helpers
    // =========================================================================

    pub(crate) fn peek(&self, offset: usize) -> &Token {
        if self.pos + offset >= self.tokens.len() {
            static EOF: Token = Token {
                ty: TokenType::Eof,
                text: String::new(),
                line: -1,
                column: -1,
            };
            return &EOF;
        }
        &self.tokens[self.pos + offset]
    }

    pub(crate) fn current(&self) -> &Token {
        self.peek(0)
    }
    pub(crate) fn previous(&self) -> &Token {
        if self.pos == 0 {
            return self.peek(0);
        }
        &self.tokens[self.pos - 1]
    }

    pub(crate) fn check(&self, ty: TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.current().ty == ty
    }

    pub(crate) fn is_at_end(&self) -> bool {
        self.current().ty == TokenType::Eof
    }

    pub(crate) fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        if self.pos == 0 {
            return self.peek(0);
        }
        &self.tokens[self.pos - 1]
    }

    pub(crate) fn match_token(&mut self, ty: TokenType) -> bool {
        if self.check(ty) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn consume(&mut self, ty: TokenType, msg: &str) -> &Token {
        if self.check(ty) {
            return self.advance();
        }
        let code = match ty {
            TokenType::RBrace => ErrorCode::E2006_ExpectedClosingBrace,
            TokenType::RParen => ErrorCode::E2007_ExpectedClosingParen,
            TokenType::RBracket => ErrorCode::E2008_ExpectedClosingBracket,
            _ => ErrorCode::E2005_ExpectedSemicolon,
        };
        // For missing semicolon, report at the previous token's position
        // (where the semicolon should have been) rather than the next token.
        let (err_line, err_column) = if ty == TokenType::Semicolon && self.pos > 0 {
            (self.previous().line, self.previous().column)
        } else {
            (self.current().line, self.current().column)
        };
        self.errors.push(ParseError {
            message: msg.to_string(),
            line: err_line,
            column: err_column,
            code: code as i32,
        });
        // 自动恢复：根据缺失的闭合符号推断恢复集合，减少级联错误
        let recovery = match ty {
            TokenType::RParen => &[TokenType::RParen, TokenType::Semicolon][..],
            TokenType::RBrace => &[TokenType::RBrace, TokenType::Semicolon][..],
            TokenType::RBracket => &[TokenType::RBracket, TokenType::Semicolon][..],
            _ => &[TokenType::Semicolon][..],
        };
        self.synchronize(recovery);
        self.peek(0)
    }

    /// 通用同步：跳过 token 直到遇到恢复集合中的 token 或语句边界。
    pub(crate) fn synchronize(&mut self, recovery_set: &[TokenType]) {
        while !self.is_at_end() {
            let current = self.current().ty;
            if recovery_set.contains(&current) {
                return;
            }
            if self.previous().ty == TokenType::Semicolon {
                return;
            }
            match current {
                TokenType::Int
                | TokenType::Void
                | TokenType::Char
                | TokenType::Float
                | TokenType::Double
                | TokenType::If
                | TokenType::While
                | TokenType::Do
                | TokenType::For
                | TokenType::Return
                | TokenType::Break
                | TokenType::Continue
                | TokenType::Struct
                | TokenType::Switch
                | TokenType::Case
                | TokenType::Default
                | TokenType::Typedef
                | TokenType::Enum
                | TokenType::Unsigned
                | TokenType::Long
                | TokenType::Short
                | TokenType::Signed
                | TokenType::Const
                | TokenType::Bool
                | TokenType::RBrace => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    pub(crate) fn is_type_token(&self) -> bool {
        if self.check(TokenType::Int)
            || self.check(TokenType::Void)
            || self.check(TokenType::Char)
            || self.check(TokenType::Float)
            || self.check(TokenType::Double)
            || self.check(TokenType::Struct)
            || self.check(TokenType::Enum)
            || self.check(TokenType::Unsigned)
            || self.check(TokenType::Long)
            || self.check(TokenType::Short)
            || self.check(TokenType::Signed)
            || self.check(TokenType::Const)
            || self.check(TokenType::Volatile)
            || self.check(TokenType::Bool)
            || self.check(TokenType::Union)
        {
            return true;
        }
        // C++ 模式下 class 和 auto 也是类型 token
        if self.is_cpp_mode && (self.check(TokenType::Class) || self.check(TokenType::Auto)) {
            return true;
        }
        if self.check(TokenType::Identifier) {
            let name = &self.current().text;
            if name == "typeof"
                || name == "__typeof__"
                || name == "__typeof"
                || name == "typeof_unqual"
                || name == "__typeof_unqual__"
                || name == "__typeof_unqual"
            {
                return true;
            }
            return self.typedef_names.contains_key(name);
        }
        false
    }

    fn is_static_token(&self) -> bool {
        self.check(TokenType::Identifier) && self.current().text == "static"
    }

    // =========================================================================
    // Program
    // =========================================================================

    fn parse_program(&mut self) -> Option<ProgramNode> {
        let mut program = ProgramNode::default();

        while !self.is_at_end() {
            // 跳过函数前缀修饰符 inline（可能出现在 static/extern 之前或之后）
            while self.check(TokenType::Inline) {
                self.advance();
            }
            if self.check(TokenType::Identifier)
                && (self.peek(0).text == "_Static_assert" || self.peek(0).text == "static_assert")
            {
                self.parse_static_assert();
                continue;
            }
            // E3：顶层 [[属性]] 前缀——解析并忽略
            if self.check(TokenType::LBracket) && self.peek(1).ty == TokenType::LBracket {
                self.skip_attributes();
                continue;
            }
            if self.check(TokenType::Typedef) {
                let checkpoint_rb = self.save();
                self.advance();
                if self.check(TokenType::Struct) {
                    let s_rb = self.save();
                    self.advance();
                    if self.check(TokenType::Identifier) {
                        self.advance();
                    }
                    if self.check(TokenType::LBrace) {
                        self.restore(checkpoint_rb);
                        self.parse_typedef_struct_decl(&mut program);
                        continue;
                    }
                    self.restore(s_rb);
                }
                if self.check(TokenType::Enum) {
                    let e_rb = self.save();
                    self.advance();
                    if self.check(TokenType::Identifier) {
                        self.advance();
                    }
                    // C23 enum : T：底层类型子句不改变"是否 enum 定义"的判定
                    if self.check(TokenType::Colon) {
                        while !self.is_at_end() && !self.check(TokenType::LBrace) && !self.check(TokenType::Semicolon) {
                            self.advance();
                        }
                    }
                    if self.check(TokenType::LBrace) {
                        self.restore(checkpoint_rb);
                        self.parse_typedef_enum_decl(&mut program);
                        continue;
                    }
                    self.restore(e_rb);
                }
                self.restore(checkpoint_rb);
                self.parse_typedef();
            } else if self.check(TokenType::Enum) {
                let checkpoint_rb = self.save();
                self.advance();
                if self.check(TokenType::Identifier) {
                    self.advance();
                }
                // C23 enum : T：底层类型子句不改变"是否 enum 定义"的判定
                if self.check(TokenType::Colon) {
                    while !self.is_at_end() && !self.check(TokenType::LBrace) && !self.check(TokenType::Semicolon) {
                        self.advance();
                    }
                }
                let is_enum_decl = self.check(TokenType::LBrace);
                self.restore(checkpoint_rb);
                if is_enum_decl {
                    self.parse_enum_decl(&mut program);
                } else {
                    self.parse_global_var_or_func(&mut program, false, false);
                }
            } else if self.check(TokenType::Struct) {
                let checkpoint_rb = self.save();
                self.advance();
                let has_name = self.check(TokenType::Identifier);
                if has_name {
                    self.advance();
                }
                if self.check(TokenType::LBrace) {
                    // 向前跳过结构体体，判断后面是变量声明还是纯类型定义
                    let mut brace_depth = 0;
                    while !self.is_at_end() {
                        if self.check(TokenType::LBrace) {
                            brace_depth += 1;
                        } else if self.check(TokenType::RBrace) {
                            brace_depth -= 1;
                            self.advance();
                            if brace_depth == 0 {
                                break;
                            }
                            continue;
                        }
                        self.advance();
                    }
                    let is_var_decl = self.check(TokenType::Identifier)
                        || self.check(TokenType::Star)
                        || self.check(TokenType::Semicolon);
                    // 纯 struct { ... };（无变量名）仍视为结构体定义
                    let is_pure_decl = self.check(TokenType::Semicolon);
                    self.restore(checkpoint_rb);
                    if is_var_decl && !is_pure_decl {
                        self.parse_global_var_or_func(&mut program, false, false);
                    } else if self.is_cpp_mode && has_name {
                        self.parse_cpp_class_like_struct_decl(&mut program);
                    } else {
                        let struct_decl = self.parse_struct_decl();
                        if self.is_cpp_mode {
                            self.typedef_names
                                .insert(struct_decl.name.clone(), Type::struct_type(struct_decl.name.clone()));
                        }
                        program.structs.push(struct_decl);
                    }
                } else {
                    self.restore(checkpoint_rb);
                    self.parse_global_var_or_func(&mut program, false, false);
                }
            } else if self.check(TokenType::Union) {
                program.unions.push(self.parse_union_decl());
            } else if self.is_cpp_mode && self.check(TokenType::Class) {
                self.parse_cpp_class_decl(&mut program);
            } else if self.is_cpp_mode && self.check(TokenType::Template) {
                self.parse_cpp_template_decl(&mut program);
            } else if self.check(TokenType::Extern) {
                self.advance(); // consume 'extern'
                self.parse_global_var_or_func(&mut program, false, true);
            } else if self.is_type_token() || self.is_static_token() {
                let is_static = self.is_static_token();
                if is_static {
                    self.advance(); // consume 'static'
                }
                self.parse_global_var_or_func(&mut program, is_static, false);
            } else {
                self.errors.push(ParseError {
                    message: format!("预期 struct、函数或全局变量声明，找到: {}", self.current().text),
                    line: self.current().line,
                    column: self.current().column,
                    code: ErrorCode::E2005_ExpectedSemicolon as i32,
                });
                self.advance();
            }
        }

        Some(program)
    }

    /// 从当前位置前瞻，跳过连续的 `*` 指针前缀，返回跳过后的 token 索引。
    fn look_ahead_skip_stars(&self) -> usize {
        let mut lookahead = self.pos;
        while lookahead < self.tokens.len() {
            match self.tokens[lookahead].ty {
                TokenType::Star => lookahead += 1,
                TokenType::Ampersand | TokenType::AndAnd if self.is_cpp_mode => lookahead += 1,
                _ => break,
            }
        }
        lookahead
    }

    fn parse_global_var_or_func(&mut self, program: &mut ProgramNode, is_static: bool, is_extern: bool) {
        let checkpoint = self.save();
        let base_type = self.parse_base_type();
        // C++ 构造函数类外定义：Counter::Counter() { ... }
        if self.try_parse_cpp_ctor_out_of_line(program, &base_type, is_static, is_extern) {
            return;
        }
        // 前瞻：跳过前导 *，检查是否是 identifier (
        let lookahead = self.look_ahead_skip_stars();
        let is_func_decl = if lookahead < self.tokens.len() && self.tokens[lookahead].ty == TokenType::Identifier {
            if lookahead + 1 < self.tokens.len() && self.tokens[lookahead + 1].ty == TokenType::LParen {
                true
            } else if self.is_cpp_mode
                && lookahead + 3 < self.tokens.len()
                && self.tokens[lookahead + 1].ty == TokenType::ColonColon
                && self.tokens[lookahead + 2].ty == TokenType::Identifier
                && self.tokens[lookahead + 3].ty == TokenType::LParen
            {
                // C++ qualified name: Bar::set(...)
                true
            } else {
                false
            }
        } else {
            false
        };
        if is_func_decl {
            self.restore(checkpoint);
            program.funcs.push(self.parse_func_decl(is_static, is_extern));
        } else if self.try_parse_cpp_qualified_static_field(program, &base_type, is_static, is_extern) {
            // 已由 cpp 模块处理
        } else {
            let (ty, name) = self.parse_var_declarator(&base_type);
            let init = if is_extern {
                None
            } else if self.match_token(TokenType::Assign) {
                if self.check(TokenType::LBrace) {
                    Some(self.parse_init_list())
                } else {
                    Some(self.parse_assign())
                }
            } else {
                None
            };
            program.globals.push(GlobalDecl {
                loc: SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                },
                ty: ty.clone(),
                name,
                init,
                is_static,
                is_extern,
                source_file: String::new(),
            });
            while self.match_token(TokenType::Comma) {
                let (extra_ty, extra_name) = self.parse_var_declarator(&base_type);
                let extra_init = if is_extern {
                    None
                } else if self.match_token(TokenType::Assign) {
                    if self.check(TokenType::LBrace) {
                        Some(self.parse_init_list())
                    } else {
                        Some(self.parse_assign())
                    }
                } else {
                    None
                };
                program.globals.push(GlobalDecl {
                    loc: SourceLoc {
                        line: self.previous().line,
                        column: self.previous().column,
                        file_id: 0,
                    },
                    ty: extra_ty,
                    name: extra_name,
                    init: extra_init,
                    is_static,
                    is_extern,
                    source_file: String::new(),
                });
            }
            self.consume(TokenType::Semicolon, "全局变量声明后预期 ';'");
        }
    }

    /// 在模板实参列表中解析非类型表达式实参。
    ///
    /// 模板实参中的 `>` 应作为实参列表结束符，而不是关系运算符。本方法先扫描出
    /// 当前实参的结束位置（顶层逗号或匹配的 `>`），然后在截取的 token 子流中调用
    /// 普通表达式解析器，避免 `>` 被误解析为 `>` 运算符。
    pub(crate) fn parse_template_arg_expr(&mut self) -> Expr {
        let (end_idx, _delim_ty) = self.find_template_arg_delimiter();
        if end_idx <= self.pos {
            self.errors.push(ParseError {
                message: "空模板非类型实参".to_string(),
                line: self.current().line,
                column: self.current().column,
                code: ErrorCode::E2003_ExpectedExpr as i32,
            });
            return Expr::Literal {
                value: 0,
                loc: SourceLoc {
                    line: self.current().line,
                    column: self.current().column,
                    file_id: 0,
                },
                ty: Type::int(),
            };
        }

        let saved_tokens = std::mem::take(&mut self.tokens);
        let saved_pos = self.pos;
        self.tokens = saved_tokens[saved_pos..end_idx].to_vec();
        self.pos = 0;
        let expr = self.parse_assign();
        let consumed = self.pos;
        self.tokens = saved_tokens;
        self.pos = saved_pos + consumed;
        expr
    }

    /// 从当前位置扫描，找到当前模板实参的结束位置。
    ///
    /// 返回的索引指向顶层分隔符（逗号或匹配的 `>`），调用者负责消费该分隔符。
    /// 扫描会正确跳过括号、方括号、花括号内部以及嵌套的 `<...>`。
    fn find_template_arg_delimiter(&self) -> (usize, TokenType) {
        let mut i = self.pos;
        let mut template_depth: i32 = 1;
        let mut paren: i32 = 0;
        let mut bracket: i32 = 0;
        let mut brace: i32 = 0;

        while i < self.tokens.len() {
            let t = &self.tokens[i];
            match t.ty {
                TokenType::LParen => paren += 1,
                TokenType::RParen if paren > 0 => {
                    paren -= 1;
                }
                TokenType::LBracket => bracket += 1,
                TokenType::RBracket if bracket > 0 => {
                    bracket -= 1;
                }
                TokenType::LBrace => brace += 1,
                TokenType::RBrace if brace > 0 => {
                    brace -= 1;
                }
                TokenType::Lt if paren == 0 && bracket == 0 && brace == 0 => template_depth += 1,
                TokenType::Gt if paren == 0 && bracket == 0 && brace == 0 => {
                    template_depth -= 1;
                    if template_depth == 0 {
                        return (i, TokenType::Gt);
                    }
                }
                TokenType::Comma if template_depth == 1 && paren == 0 && bracket == 0 && brace == 0 => {
                    return (i, TokenType::Comma);
                }
                _ => {}
            }
            i += 1;
        }

        (i, TokenType::Eof)
    }
}

mod cpp;
mod decl;
mod expr;
mod stmt;
mod type_;
