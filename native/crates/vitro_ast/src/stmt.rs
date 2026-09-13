//! AST 语句节点。

use super::expr::Expr;
use super::types::Type;
use vitro_shared::source_loc::SourceLoc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Stmt {
    Block {
        stmts: Vec<Stmt>,
        loc: SourceLoc,
    },
    VarDecl {
        var_type: Type,
        name: String,
        init: Option<Expr>,
        extra_vars: Vec<(Type, String, Option<Expr>)>,
        is_static: bool,
        loc: SourceLoc,
    },
    Expr {
        expr: Expr,
        loc: SourceLoc,
    },
    If {
        cond: Expr,
        then_stmt: Box<Stmt>,
        else_stmt: Option<Box<Stmt>>,
        loc: SourceLoc,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
        loc: SourceLoc,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
        loc: SourceLoc,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Vec<Expr>,
        body: Box<Stmt>,
        loc: SourceLoc,
    },
    Return {
        value: Option<Expr>,
        loc: SourceLoc,
    },
    Break {
        loc: SourceLoc,
    },
    Continue {
        loc: SourceLoc,
    },
    Switch {
        cond: Expr,
        body: Box<Stmt>,
        loc: SourceLoc,
    },
    Case {
        label: Option<Expr>,
        stmt: Box<Stmt>,
        loc: SourceLoc,
    },
    Goto {
        label: String,
        loc: SourceLoc,
    },
    Label {
        label: String,
        stmt: Box<Stmt>,
        loc: SourceLoc,
    },
    // === C++ 新增 ===
    RangeFor {
        var: String,
        var_type: Type,
        iter: Box<Expr>,
        body: Box<Stmt>,
        loc: SourceLoc,
    },
    Try {
        body: Box<Stmt>,
        catches: Vec<CatchClause>,
        loc: SourceLoc,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatchClause {
    pub param_type: Type,
    pub param_name: Option<String>,
    pub body: Stmt,
    pub loc: SourceLoc,
}
