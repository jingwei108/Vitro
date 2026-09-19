///|
/// 语句节点（16 变体，照搬 Rust `vitro_ast/src/stmt.rs`）。
///
/// `Try` 为 **reserved-for-csharp**：C++ 子集不发射（异常走 E4001 拒绝），
/// 节点形态保留给 C# 批次——禁删除禁改语义（总计划 §5 拍板）。
pub(all) enum Stmt {
  Block(Array[Stmt], @source.SourceLoc) // stmts
  VarDecl(Type, String, Expr?, Array[VarDeclExtra], Bool, @source.SourceLoc) // var_type, name, init, extra_vars, is_static
  Expr(Expr, @source.SourceLoc)
  If(Expr, Stmt, Stmt?, @source.SourceLoc) // cond, then, else
  While(Expr, Stmt, @source.SourceLoc)
  DoWhile(Stmt, Expr, @source.SourceLoc)
  For(Stmt?, Expr?, Array[Expr], Stmt, @source.SourceLoc) // init, cond, step, body
  Return(Expr?, @source.SourceLoc)
  Break(@source.SourceLoc)
  Continue(@source.SourceLoc)
  Switch(Expr, Stmt, @source.SourceLoc) // cond, body
  Case(Expr?, Stmt, @source.SourceLoc) // label, stmt
  Goto(String, @source.SourceLoc)
  Label(String, Stmt, @source.SourceLoc)
  // === C++ 新增 ===
  RangeFor(String, Type, Expr, Stmt, @source.SourceLoc) // var, var_type, iter, body
  /// reserved-for-csharp（见包注释）
  Try(Stmt, Array[CatchClause], @source.SourceLoc)
} derive(Debug)

///|
/// `VarDecl.extra_vars` 元素：`int a = 1, b = 2, c;` 的 b/c 尾随声明
/// （Rust 侧为 (Type, String, Option<Expr>) 元组——MoonBit 具名结构化，
/// 元组形态语义不变）。
pub(all) struct VarDeclExtra {
  var_type : Type
  name : String
  init : Expr?
} derive(Debug)

///|
/// catch 子句（Try 的保留配套）。
pub(all) struct CatchClause {
  param_type : Type
  param_name : String?
  body : Stmt
  loc : @source.SourceLoc
} derive(Debug)
