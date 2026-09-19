///|
/// AST 深度测量（照搬 Rust `vitro_ast/src/depth.rs`，U1#8）：迭代式 DFS，
/// 显式栈而非递归——被测的就是病态深 AST，递归测量自身会先溢出
/// （左结合链 5000 项实测崩在 typeck / AST Drop，同等深度的测量递归同理）。
///
/// 深度语义：叶子节点为 1；`a+a+...+a`（n 项左结合链）深度为 n；
/// 函数体 `Block{stmts}` 为 1 + max(各语句深度)——语句序列不增加嵌套，
/// 只有真正的结构嵌套才计入。
priv enum DepthNode {
  E(Expr)
  S(Stmt)
}

///|
/// 表达式最大嵌套深度（叶子为 1）。
pub fn expr_depth(e : Expr) -> Int {
  let stack : Array[(DepthNode, Int)] = [(E(e), 1)]
  depth_of(stack)
}

///|
/// 语句最大嵌套深度（叶子为 1；函数体 Block 作为根传入）。
pub fn stmt_depth(s : Stmt) -> Int {
  let stack : Array[(DepthNode, Int)] = [(S(s), 1)]
  depth_of(stack)
}

///|
fn depth_of(stack : Array[(DepthNode, Int)]) -> Int {
  let mut max = 0
  while stack.length() > 0 {
    let (node, d) = stack.pop().unwrap()
    if d > max {
      max = d
    }
    match node {
      E(e) => push_expr_children(e, d, stack)
      S(s) => push_stmt_children(s, d, stack)
    }
  }
  max
}

///|
fn push_expr_children(
  e : Expr,
  d : Int,
  stack : Array[(DepthNode, Int)],
) -> Unit {
  let dd = d + 1
  match e {
    Binary(_, left, right, _, _) | Assign(_, left, right, _, _) => {
      stack.push((E(left), dd))
      stack.push((E(right), dd))
    }
    Unary(_, operand, _, _)
    | Cast(operand, _, _, _)
    | Delete(operand, _, _, _)
    | Move(operand, _, _) => stack.push((E(operand), dd))
    Literal(_, _, _)
    | FloatLiteral(_, _, _)
    | LongLiteral(_, _, _)
    | StringLiteral(_, _, _)
    | Identifier(_, _, _)
    | This(_, _)
    | Offsetof(_, _, _, _) => ()
    Call(_, args, _, _) =>
      for a in args {
        stack.push((E(a), dd))
      }
    CallPtr(callee, args, _, _) => {
      stack.push((E(callee), dd))
      for a in args {
        stack.push((E(a), dd))
      }
    }
    Index(array, index, _, _) => {
      stack.push((E(array), dd))
      stack.push((E(index), dd))
    }
    Member(object, _, _, _) => stack.push((E(object), dd))
    Ternary(cond, then_branch, else_branch, _, _) => {
      stack.push((E(cond), dd))
      stack.push((E(then_branch), dd))
      stack.push((E(else_branch), dd))
    }
    Sizeof(_, operand, _, _) | Alignof(_, operand, _, _) =>
      match operand {
        Some(op) => stack.push((E(op), dd))
        None => ()
      }
    InitList(elements, _, _) =>
      for el in elements {
        stack.push((E(el.value), dd))
      }
    Generic(control, associations, default, _, _) => {
      stack.push((E(control), dd))
      for assoc in associations {
        stack.push((E(assoc.1), dd))
      }
      match default {
        Some(dflt) => stack.push((E(dflt), dd))
        None => ()
      }
    }
    CompoundLiteral(_, init, _, _) => stack.push((E(init), dd))
    MemberCall(object, _, args, _, _, _, _) => {
      stack.push((E(object), dd))
      for a in args {
        stack.push((E(a), dd))
      }
    }
    New(_, size_expr, init, _, _) => {
      match size_expr {
        Some(se) => stack.push((E(se), dd))
        None => ()
      }
      match init {
        Some(i) => stack.push((E(i), dd))
        None => ()
      }
    }
    Lambda(_, _, body, _, _, _) => stack.push((S(body), dd))
  }
}

///|
fn push_stmt_children(
  s : Stmt,
  d : Int,
  stack : Array[(DepthNode, Int)],
) -> Unit {
  let dd = d + 1
  match s {
    Block(stmts, _) =>
      for st in stmts {
        stack.push((S(st), dd))
      }
    VarDecl(_, _, init, extra_vars, _, _) => {
      match init {
        Some(i) => stack.push((E(i), dd))
        None => ()
      }
      for extra in extra_vars {
        match extra.init {
          Some(i) => stack.push((E(i), dd))
          None => ()
        }
      }
    }
    Expr(expr, _) => stack.push((E(expr), dd))
    If(cond, then_stmt, else_stmt, _) => {
      stack.push((E(cond), dd))
      stack.push((S(then_stmt), dd))
      match else_stmt {
        Some(e) => stack.push((S(e), dd))
        None => ()
      }
    }
    While(cond, body, _) => {
      stack.push((E(cond), dd))
      stack.push((S(body), dd))
    }
    DoWhile(body, cond, _) => {
      stack.push((S(body), dd))
      stack.push((E(cond), dd))
    }
    For(init, cond, step, body, _) => {
      match init {
        Some(i) => stack.push((S(i), dd))
        None => ()
      }
      match cond {
        Some(c) => stack.push((E(c), dd))
        None => ()
      }
      for s in step {
        stack.push((E(s), dd))
      }
      stack.push((S(body), dd))
    }
    Return(value, _) =>
      match value {
        Some(v) => stack.push((E(v), dd))
        None => ()
      }
    Break(_) | Continue(_) | Goto(_) => ()
    Switch(cond, body, _) => {
      stack.push((E(cond), dd))
      stack.push((S(body), dd))
    }
    Case(label, stmt, _) => {
      match label {
        Some(l) => stack.push((E(l), dd))
        None => ()
      }
      stack.push((S(stmt), dd))
    }
    Label(_, stmt, _) => stack.push((S(stmt), dd))
    RangeFor(_, _, iter, body, _) => {
      stack.push((E(iter), dd))
      stack.push((S(body), dd))
    }
    Try(body, catches, _) => {
      stack.push((S(body), dd))
      for c in catches {
        stack.push((S(c.body), dd))
      }
    }
  }
}

// =========================================================================
// 类型深度（P1，2026-09-18）
// =========================================================================

///|
/// 类型树最大嵌套深度（叶子标量为 1；`int[1][1]...[1]` 的 n 层后缀深度
/// n+1）。
///
/// 病态深 Type（链式数组后缀 / typedef 链）对 Expr/Stmt 深度预算隐身：
/// `int a[1]x1300` 的 VarDecl 深度只有 2，而它的类型深 1301——
/// 递归遍历点会随之栈溢出。显式栈测量，独立预算（parser
/// `MAX_TYPE_DEPTH`），不并入 `MAX_AST_DEPTH=512`：合法深层声明
/// （实测 release 1200 层存活）的余量与 Expr 安全线解耦。
pub fn type_depth(t : Type) -> Int {
  let stack : Array[(Type, Int)] = [(t, 1)]
  let mut max = 0
  let work = stack
  while work.length() > 0 {
    let (ty, d) = work.pop().unwrap()
    if d > max {
      max = d
    }
    let dd = d + 1
    match ty {
      Pointer(pointee, _)
      | Array(pointee, _, _, _, _, _)
      | Reference(pointee, _)
      | RValueRef(pointee) => work.push((pointee, dd))
      Function(return_type, param_types, _, _) => {
        work.push((return_type, dd))
        for p in param_types {
          work.push((p, dd))
        }
      }
      _ => ()
    }
  }
  max
}

///|
/// 语句树内所有声明类型的最大深度（遍历嵌套语句找 VarDecl，取其
/// var_type / extra_vars 类型的 type_depth 最大值；不钻 Expr——表达式内
/// 出现的类型只引用已在声明点过检的类型）。
pub fn stmt_type_depth(s : Stmt) -> Int {
  let stack : Array[Stmt] = [s]
  let mut max = 0
  let work = stack
  while work.length() > 0 {
    let stmt = work.pop().unwrap()
    match stmt {
      VarDecl(var_type, _, _, extra_vars, _, _) => {
        let d = type_depth(var_type)
        if d > max {
          max = d
        }
        for extra in extra_vars {
          let ed = type_depth(extra.var_type)
          if ed > max {
            max = ed
          }
        }
      }
      Block(stmts, _) =>
        for st in stmts {
          work.push(st)
        }
      If(_, then_stmt, else_stmt, _) => {
        work.push(then_stmt)
        match else_stmt {
          Some(e) => work.push(e)
          None => ()
        }
      }
      While(_, body, _) | DoWhile(body, _, _) | Label(_, body, _) =>
        work.push(body)
      For(init, _, _, body, _) => {
        match init {
          Some(i) => work.push(i)
          None => ()
        }
        work.push(body)
      }
      Switch(_, body, _) => work.push(body)
      Case(_, stmt, _) => work.push(stmt)
      RangeFor(_, _, _, body, _) => work.push(body)
      Try(body, catches, _) => {
        work.push(body)
        for c in catches {
          work.push(c.body)
        }
      }
      _ => ()
    }
  }
  max
}
