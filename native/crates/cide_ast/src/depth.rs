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
