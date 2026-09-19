///|
/// 二元运算符（19 个，照搬 Rust BinaryOp）。
pub(all) enum BinaryOp {
  Add
  Sub
  Mul
  Div
  Mod
  Eq
  Ne
  Lt
  Le
  Gt
  Ge
  And
  Or
  BitAnd
  BitOr
  BitXor
  Shl
  Shr
  Comma
} derive(Debug, Eq, Compare)

///|
/// 一元运算符（9 个，照搬 Rust UnaryOp）。
pub(all) enum UnaryOp {
  Neg
  Not
  BitNot
  Addr
  Deref
  PreInc
  PreDec
  PostInc
  PostDec
} derive(Debug, Eq, Compare)

///|
/// 赋值运算符（11 个，照搬 Rust AssignOp）。
pub(all) enum AssignOp {
  Assign
  AddAssign
  SubAssign
  MulAssign
  DivAssign
  ModAssign
  AndAssign
  OrAssign
  XorAssign
  ShlAssign
  ShrAssign
} derive(Debug, Eq, Compare)

///|
/// 初始化器指派符（designated initializer）。
pub(all) enum Designator {
  Field(String)
  Index(Expr)
} derive(Debug)

///|
/// 初始化列表元素（指派符序列 + 值）。
pub(all) struct InitElement {
  designators : Array[Designator]
  value : Expr
} derive(Debug)

///|
/// 表达式节点（26 变体，照搬 Rust `vitro_ast/src/expr.rs`；每节点携带
/// `@source.SourceLoc` 与推导后 `Type`——typeck 前的 ty 为 Void 默认值）。
///
/// 字段序与 Rust 对齐（Binary: op, left, right, loc, ty），E1 dump 的
/// 键序即此序（canonicalize 归一后与 serde 侧对齐）。
pub(all) enum Expr {
  Binary(BinaryOp, Expr, Expr, @source.SourceLoc, Type)
  Unary(UnaryOp, Expr, @source.SourceLoc, Type)
  Literal(Int, @source.SourceLoc, Type) // value
  FloatLiteral(Double, @source.SourceLoc, Type) // value
  LongLiteral(Int64, @source.SourceLoc, Type) // value
  StringLiteral(String, @source.SourceLoc, Type) // value
  Identifier(String, @source.SourceLoc, Type) // name
  Call(String, Array[Expr], @source.SourceLoc, Type) // name, args
  CallPtr(Expr, Array[Expr], @source.SourceLoc, Type) // callee, args
  Index(Expr, Expr, @source.SourceLoc, Type) // array, index
  Member(Expr, String, @source.SourceLoc, Type) // object, member
  Assign(AssignOp, Expr, Expr, @source.SourceLoc, Type)
  Ternary(Expr, Expr, Expr, @source.SourceLoc, Type) // cond, then, else
  Sizeof(Type?, Expr?, @source.SourceLoc, Type) // target_type, operand
  /// C23 `_Alignof` / `alignof`：取成员最大自然对齐（packed 布局差异
  /// 已在 spec 记录——照搬 Rust 侧注释语义）。
  Alignof(Type?, Expr?, @source.SourceLoc, Type)
  Cast(Expr, Type, @source.SourceLoc, Type) // expr, target_type
  InitList(Array[InitElement], @source.SourceLoc, Type)
  Offsetof(Type, String, @source.SourceLoc, Type) // target_type, field
  Generic(Expr, Array[(Type, Expr)], Expr?, @source.SourceLoc, Type) // control, associations, default
  CompoundLiteral(Type, Expr, @source.SourceLoc, Type) // target_type, init
  // === C++ 新增 ===
  This(@source.SourceLoc, Type)
  MemberCall(Expr, String, Array[Expr], Bool, String?, @source.SourceLoc, Type) // object, method, args, is_virtual, resolved_mangled
  New(Type, Expr?, Expr?, @source.SourceLoc, Type) // elem_type, size_expr, init
  Delete(Expr, Bool, @source.SourceLoc, Type) // expr, is_array
  Lambda(Array[CaptureMode], Array[Param], Stmt, UInt, @source.SourceLoc, Type) // capture, params, body, unique_id
  Move(Expr, @source.SourceLoc, Type)
} derive(Debug)

///|
/// 表达式的源位置（Rust expr_field! 宏的单点等价）。
pub fn Expr::loc(self : Expr) -> @source.SourceLoc {
  match self {
    Binary(_, _, _, loc, _)
    | Unary(_, _, loc, _)
    | Literal(_, loc, _)
    | FloatLiteral(_, loc, _)
    | LongLiteral(_, loc, _)
    | StringLiteral(_, loc, _)
    | Identifier(_, loc, _)
    | Call(_, _, loc, _)
    | CallPtr(_, _, loc, _)
    | Index(_, _, loc, _)
    | Member(_, _, loc, _)
    | Assign(_, _, _, loc, _)
    | Ternary(_, _, _, loc, _)
    | Sizeof(_, _, loc, _)
    | Alignof(_, _, loc, _)
    | Cast(_, _, loc, _)
    | InitList(_, loc, _)
    | Offsetof(_, _, loc, _)
    | Generic(_, _, _, loc, _)
    | CompoundLiteral(_, _, loc, _)
    | This(loc, _)
    | MemberCall(_, _, _, _, _, loc, _)
    | New(_, _, _, loc, _)
    | Delete(_, _, loc, _)
    | Lambda(_, _, _, _, loc, _)
    | Move(_, loc, _) => loc
  }
}

///|
/// 表达式的推导类型。
pub fn Expr::ty(self : Expr) -> Type {
  match self {
    Binary(_, _, _, _, ty)
    | Unary(_, _, _, ty)
    | Literal(_, _, ty)
    | FloatLiteral(_, _, ty)
    | LongLiteral(_, _, ty)
    | StringLiteral(_, _, ty)
    | Identifier(_, _, ty)
    | Call(_, _, _, ty)
    | CallPtr(_, _, _, ty)
    | Index(_, _, _, ty)
    | Member(_, _, _, ty)
    | Assign(_, _, _, _, ty)
    | Ternary(_, _, _, _, ty)
    | Sizeof(_, _, _, ty)
    | Alignof(_, _, _, ty)
    | Cast(_, _, _, ty)
    | InitList(_, _, ty)
    | Offsetof(_, _, _, ty)
    | Generic(_, _, _, _, ty)
    | CompoundLiteral(_, _, _, ty)
    | This(_, ty)
    | MemberCall(_, _, _, _, _, _, ty)
    | New(_, _, _, _, ty)
    | Delete(_, _, _, ty)
    | Lambda(_, _, _, _, _, ty)
    | Move(_, _, ty) => ty
  }
}
