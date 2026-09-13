//! AST 表达式节点。

use super::decl::Param;
use super::stmt::Stmt;
use super::types::Type;
use vitro_shared::source_loc::SourceLoc;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Comma,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Addr,
    Deref,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Designator {
    Field(String),
    Index(Box<Expr>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitElement {
    pub designators: Vec<Designator>,
    pub value: Expr,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Literal {
        value: i32,
        loc: SourceLoc,
        ty: Type,
    },
    FloatLiteral {
        value: f64,
        loc: SourceLoc,
        ty: Type,
    },
    LongLiteral {
        value: i64,
        loc: SourceLoc,
        ty: Type,
    },
    StringLiteral {
        value: String,
        loc: SourceLoc,
        ty: Type,
    },
    Identifier {
        name: String,
        loc: SourceLoc,
        ty: Type,
    },
    Call {
        name: String,
        args: Vec<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    CallPtr {
        callee: Box<Expr>,
        args: Vec<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Member {
        object: Box<Expr>,
        member: String,
        loc: SourceLoc,
        ty: Type,
    },
    Assign {
        op: AssignOp,
        left: Box<Expr>,
        right: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    Sizeof {
        target_type: Option<Type>,
        operand: Option<Box<Expr>>,
        loc: SourceLoc,
        ty: Type,
    },
    /// C23 `_Alignof` / `alignof`（E1）：取类型的自然对齐。
    /// 注意：Vitro 的 struct 布局是 packed（无对齐填充），本节点按"成员最大
    /// 自然对齐"语义取值（与 Clang 口径一致），与自身布局的差异已在 spec 记录。
    Alignof {
        target_type: Option<Type>,
        operand: Option<Box<Expr>>,
        loc: SourceLoc,
        ty: Type,
    },
    Cast {
        expr: Box<Expr>,
        target_type: Type,
        loc: SourceLoc,
        ty: Type,
    },
    InitList {
        elements: Vec<InitElement>,
        loc: SourceLoc,
        ty: Type,
    },
    Offsetof {
        target_type: Type,
        field: String,
        loc: SourceLoc,
        ty: Type,
    },
    Generic {
        control: Box<Expr>,
        associations: Vec<(Type, Expr)>,
        default: Option<Box<Expr>>,
        loc: SourceLoc,
        ty: Type,
    },
    CompoundLiteral {
        target_type: Type,
        init: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
    // === C++ 新增 ===
    This {
        loc: SourceLoc,
        ty: Type,
    },
    MemberCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        is_virtual: bool,
        /// The mangled function name chosen by overload resolution.
        /// If `None`, codegen will recompute the fallback name.
        resolved_mangled: Option<String>,
        loc: SourceLoc,
        ty: Type,
    },
    New {
        elem_type: Type,
        size_expr: Option<Box<Expr>>,
        init: Option<Box<Expr>>,
        loc: SourceLoc,
        ty: Type,
    },
    Delete {
        expr: Box<Expr>,
        is_array: bool,
        loc: SourceLoc,
        ty: Type,
    },
    Lambda {
        capture: Vec<CaptureMode>,
        params: Vec<Param>,
        body: Box<Stmt>,
        unique_id: u64,
        loc: SourceLoc,
        ty: Type,
    },
    Move {
        expr: Box<Expr>,
        loc: SourceLoc,
        ty: Type,
    },
}

impl Default for Expr {
    fn default() -> Self {
        Expr::Literal {
            value: 0,
            loc: SourceLoc::default(),
            ty: Type::default(),
        }
    }
}

macro_rules! expr_field {
    ($self:expr, $field:ident) => {
        match $self {
            Expr::Binary { $field, .. } => $field,
            Expr::Unary { $field, .. } => $field,
            Expr::Literal { $field, .. } => $field,
            Expr::FloatLiteral { $field, .. } => $field,
            Expr::LongLiteral { $field, .. } => $field,
            Expr::StringLiteral { $field, .. } => $field,
            Expr::Identifier { $field, .. } => $field,
            Expr::Call { $field, .. } => $field,
            Expr::CallPtr { $field, .. } => $field,
            Expr::Index { $field, .. } => $field,
            Expr::Member { $field, .. } => $field,
            Expr::Assign { $field, .. } => $field,
            Expr::Ternary { $field, .. } => $field,
            Expr::Sizeof { $field, .. } => $field,
            Expr::Alignof { $field, .. } => $field,
            Expr::Cast { $field, .. } => $field,
            Expr::InitList { $field, .. } => $field,
            Expr::Offsetof { $field, .. } => $field,
            Expr::Generic { $field, .. } => $field,
            Expr::CompoundLiteral { $field, .. } => $field,
            // === C++ 新增 ===
            Expr::This { $field, .. } => $field,
            Expr::MemberCall { $field, .. } => $field,
            Expr::New { $field, .. } => $field,
            Expr::Delete { $field, .. } => $field,
            Expr::Lambda { $field, .. } => $field,
            Expr::Move { $field, .. } => $field,
        }
    };
}

impl Expr {
    pub fn loc(&self) -> &SourceLoc {
        expr_field!(self, loc)
    }
    pub fn ty(&self) -> &Type {
        expr_field!(self, ty)
    }
    pub fn set_ty(&mut self, new_ty: Type) {
        *expr_field!(self, ty) = new_ty;
    }
}

// 需要从 decl.rs 重新导出以打破循环依赖，同时保持外部 API 不变。
pub use super::decl::CaptureMode;
