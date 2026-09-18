//! AST 深度测量（U1#8）：迭代式 DFS，为 parser 的后置深度预算提供单源实现。
//!
//! 显式栈而非递归——被测的就是病态深 AST，递归测量自身会先溢出
//! （左结合链 5000 项实测崩在 typeck / AST Drop，同等深度的测量递归同理）。
//!
//! 深度语义：叶子节点为 1；`a+a+...+a`（n 项左结合链）深度为 n；
//! 函数体 `Block{stmts}` 为 1 + max(各语句深度)——语句序列不增加嵌套，
//! 只有真正的结构嵌套才计入。

use super::expr::Expr;
use super::stmt::Stmt;
use super::types::Type;

/// 栈元素：表达式或语句节点引用。
enum Node<'a> {
    E(&'a Expr),
    S(&'a Stmt),
}

/// 表达式最大嵌套深度（叶子为 1）。
pub fn expr_depth(e: &Expr) -> usize {
    let mut stack: Vec<(Node<'_>, usize)> = vec![(Node::E(e), 1)];
    depth_of(&mut stack)
}

/// 语句最大嵌套深度（叶子为 1；函数体 Block 作为根传入）。
pub fn stmt_depth(s: &Stmt) -> usize {
    let mut stack: Vec<(Node<'_>, usize)> = vec![(Node::S(s), 1)];
    depth_of(&mut stack)
}

fn depth_of(stack: &mut Vec<(Node<'_>, usize)>) -> usize {
    let mut max = 0usize;
    while let Some((node, d)) = stack.pop() {
        if d > max {
            max = d;
        }
        match node {
            Node::E(e) => push_expr_children(e, d, stack),
            Node::S(s) => push_stmt_children(s, d, stack),
        }
    }
    max
}

fn push_expr_children<'a>(e: &'a Expr, d: usize, stack: &mut Vec<(Node<'a>, usize)>) {
    let dd = d + 1;
    match e {
        Expr::Binary { left, right, .. } | Expr::Assign { left, right, .. } => {
            stack.push((Node::E(left), dd));
            stack.push((Node::E(right), dd));
        }
        Expr::Unary { operand, .. }
        | Expr::Cast { expr: operand, .. }
        | Expr::Delete { expr: operand, .. }
        | Expr::Move { expr: operand, .. } => stack.push((Node::E(operand), dd)),
        Expr::Literal { .. }
        | Expr::FloatLiteral { .. }
        | Expr::LongLiteral { .. }
        | Expr::StringLiteral { .. }
        | Expr::Identifier { .. }
        | Expr::This { .. }
        | Expr::Offsetof { .. } => {}
        Expr::Call { args, .. } => {
            for a in args {
                stack.push((Node::E(a), dd));
            }
        }
        Expr::CallPtr { callee, args, .. } => {
            stack.push((Node::E(callee), dd));
            for a in args {
                stack.push((Node::E(a), dd));
            }
        }
        Expr::Index { array, index, .. } => {
            stack.push((Node::E(array), dd));
            stack.push((Node::E(index), dd));
        }
        Expr::Member { object, .. } => stack.push((Node::E(object), dd)),
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            stack.push((Node::E(cond), dd));
            stack.push((Node::E(then_branch), dd));
            stack.push((Node::E(else_branch), dd));
        }
        Expr::Sizeof { operand, .. } | Expr::Alignof { operand, .. } => {
            if let Some(op) = operand {
                stack.push((Node::E(op), dd));
            }
        }
        Expr::InitList { elements, .. } => {
            for el in elements {
                stack.push((Node::E(&el.value), dd));
            }
        }
        Expr::Generic { control, associations, default, .. } => {
            stack.push((Node::E(control), dd));
            for (_, a) in associations {
                stack.push((Node::E(a), dd));
            }
            if let Some(dflt) = default {
                stack.push((Node::E(dflt), dd));
            }
        }
        Expr::CompoundLiteral { init, .. } => stack.push((Node::E(init), dd)),
        Expr::MemberCall { object, args, .. } => {
            stack.push((Node::E(object), dd));
            for a in args {
                stack.push((Node::E(a), dd));
            }
        }
        Expr::New { size_expr, init, .. } => {
            if let Some(se) = size_expr {
                stack.push((Node::E(se), dd));
            }
            if let Some(i) = init {
                stack.push((Node::E(i), dd));
            }
        }
        Expr::Lambda { body, .. } => stack.push((Node::S(body), dd)),
    }
}

fn push_stmt_children<'a>(s: &'a Stmt, d: usize, stack: &mut Vec<(Node<'a>, usize)>) {
    let dd = d + 1;
    match s {
        Stmt::Block { stmts, .. } => {
            for st in stmts {
                stack.push((Node::S(st), dd));
            }
        }
        Stmt::VarDecl { init, extra_vars, .. } => {
            if let Some(i) = init {
                stack.push((Node::E(i), dd));
            }
            for (_, _, i) in extra_vars {
                if let Some(i) = i {
                    stack.push((Node::E(i), dd));
                }
            }
        }
        Stmt::Expr { expr, .. } => stack.push((Node::E(expr), dd)),
        Stmt::If { cond, then_stmt, else_stmt, .. } => {
            stack.push((Node::E(cond), dd));
            stack.push((Node::S(then_stmt), dd));
            if let Some(e) = else_stmt {
                stack.push((Node::S(e), dd));
            }
        }
        Stmt::While { cond, body, .. } => {
            stack.push((Node::E(cond), dd));
            stack.push((Node::S(body), dd));
        }
        Stmt::DoWhile { body, cond, .. } => {
            stack.push((Node::S(body), dd));
            stack.push((Node::E(cond), dd));
        }
        Stmt::For { init, cond, step, body, .. } => {
            if let Some(i) = init {
                stack.push((Node::S(i), dd));
            }
            if let Some(c) = cond {
                stack.push((Node::E(c), dd));
            }
            for s in step {
                stack.push((Node::E(s), dd));
            }
            stack.push((Node::S(body), dd));
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                stack.push((Node::E(v), dd));
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Goto { .. } => {}
        Stmt::Switch { cond, body, .. } => {
            stack.push((Node::E(cond), dd));
            stack.push((Node::S(body), dd));
        }
        Stmt::Case { label, stmt, .. } => {
            if let Some(l) = label {
                stack.push((Node::E(l), dd));
            }
            stack.push((Node::S(stmt), dd));
        }
        Stmt::Label { stmt, .. } => stack.push((Node::S(stmt), dd)),
        Stmt::RangeFor { iter, body, .. } => {
            stack.push((Node::E(iter), dd));
            stack.push((Node::S(body), dd));
        }
        Stmt::Try { body, catches, .. } => {
            stack.push((Node::S(body), dd));
            for c in catches {
                stack.push((Node::S(&c.body), dd));
            }
        }
    }
}

// =========================================================================
// 类型深度（P1，2026-09-18）
// =========================================================================

/// 类型树最大嵌套深度（叶子标量为 1；`int[1][1]...[1]` 的 n 层后缀深度 n+1）。
///
/// 病态深 Type（链式数组后缀 / typedef 链）对 Expr/Stmt 深度预算隐身：
/// `int a[1]x1300` 的 VarDecl 深度只有 2，而它的类型深 1301——
/// `base_element_type` / `compute_type_size`（vitro_ast）等递归遍历点
/// 会随之栈溢出。显式栈测量，独立预算（见 parser `MAX_TYPE_DEPTH`），
/// 不并入 `MAX_AST_DEPTH=512`：合法深层声明（实测 release 1200 层存活）
/// 的余量与 Expr 安全线解耦。
pub fn type_depth(t: &Type) -> usize {
    let mut stack: Vec<(&Type, usize)> = vec![(t, 1)];
    let mut max = 0usize;
    while let Some((ty, d)) = stack.pop() {
        if d > max {
            max = d;
        }
        let dd = d + 1;
        match ty {
            Type::Pointer { pointee, .. }
            | Type::Array { element: pointee, .. }
            | Type::Reference { base: pointee, .. }
            | Type::RValueRef { base: pointee, .. } => {
                stack.push((pointee, dd));
            }
            Type::Function { return_type, param_types, .. } => {
                stack.push((return_type, dd));
                for p in param_types {
                    stack.push((p, dd));
                }
            }
            _ => {}
        }
    }
    max
}

/// 语句树内所有声明类型的最大深度（遍历嵌套语句找 VarDecl，取其
/// var_type / extra_vars 类型的 `type_depth` 最大值；不钻 Expr——
/// 表达式内出现的类型只引用已在声明点过检的类型）。
pub fn stmt_type_depth(s: &Stmt) -> usize {
    let mut stack: Vec<&Stmt> = vec![s];
    let mut max = 0usize;
    while let Some(stmt) = stack.pop() {
        match stmt {
            Stmt::VarDecl { var_type, extra_vars, .. } => {
                max = max.max(type_depth(var_type));
                for (ty, _, _) in extra_vars {
                    max = max.max(type_depth(ty));
                }
            }
            Stmt::Block { stmts, .. } => stack.extend(stmts.iter()),
            Stmt::If { then_stmt, else_stmt, .. } => {
                stack.push(then_stmt);
                if let Some(e) = else_stmt {
                    stack.push(e);
                }
            }
            Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Label { stmt: body, .. } => {
                stack.push(body);
            }
            Stmt::For { init, body, .. } => {
                if let Some(i) = init {
                    stack.push(i);
                }
                stack.push(body);
            }
            Stmt::Switch { body, .. } => stack.push(body),
            Stmt::Case { stmt, .. } => stack.push(stmt),
            Stmt::RangeFor { body, .. } => stack.push(body),
            Stmt::Try { body, catches, .. } => {
                stack.push(body);
                for c in catches {
                    stack.push(&c.body);
                }
            }
            _ => {}
        }
    }
    max
}
