use vitro_ast::*;

use super::super::TypeChecker;

impl TypeChecker {
    /// Lambda capture rewriting: replace captured identifiers with this->field
    pub(crate) fn rewrite_lambda_captures(stmt: &mut Stmt, captures: &[(String, Type, bool)], lambda_name: &str) {
        match stmt {
            Stmt::Block { stmts, .. } => {
                for s in stmts {
                    Self::rewrite_lambda_captures(s, captures, lambda_name);
                }
            }
            Stmt::VarDecl { init: Some(e), .. } => {
                Self::rewrite_lambda_captures_in_expr(e, captures, lambda_name);
            }
            Stmt::Expr { expr, .. } => {
                Self::rewrite_lambda_captures_in_expr(expr, captures, lambda_name);
            }
            Stmt::If { cond, then_stmt, else_stmt, .. } => {
                Self::rewrite_lambda_captures_in_expr(cond, captures, lambda_name);
                Self::rewrite_lambda_captures(then_stmt, captures, lambda_name);
                if let Some(s) = else_stmt {
                    Self::rewrite_lambda_captures(s, captures, lambda_name);
                }
            }
            Stmt::While { cond, body, .. } => {
                Self::rewrite_lambda_captures_in_expr(cond, captures, lambda_name);
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            Stmt::DoWhile { body, cond, .. } => {
                Self::rewrite_lambda_captures(body, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(cond, captures, lambda_name);
            }
            Stmt::For { init, cond, step, body, .. } => {
                if let Some(i) = init {
                    Self::rewrite_lambda_captures(i, captures, lambda_name);
                }
                if let Some(c) = cond {
                    Self::rewrite_lambda_captures_in_expr(c, captures, lambda_name);
                }
                for s in step.iter_mut() {
                    Self::rewrite_lambda_captures_in_expr(s, captures, lambda_name);
                }
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            Stmt::Return { value: Some(v), .. } => {
                Self::rewrite_lambda_captures_in_expr(v, captures, lambda_name);
            }
            Stmt::Switch { cond, body, .. } => {
                Self::rewrite_lambda_captures_in_expr(cond, captures, lambda_name);
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            Stmt::RangeFor { iter, body, .. } => {
                Self::rewrite_lambda_captures_in_expr(iter, captures, lambda_name);
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            Stmt::Try { body, .. } => {
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            _ => {}
        }
    }

    pub(crate) fn rewrite_lambda_captures_in_expr(
        expr: &mut Expr,
        captures: &[(String, Type, bool)],
        lambda_name: &str,
    ) {
        match expr {
            Expr::Identifier { name, loc, ty: _ } => {
                for (cap_name, cap_ty, _) in captures.iter() {
                    if cap_name == name {
                        let this_ty = Type::Pointer {
                            pointee: Box::new(Type::Class {
                                name: lambda_name.to_string(),
                                is_const: false,
                            }),
                            is_const: false,
                        };
                        *expr = Expr::Member {
                            object: Box::new(Expr::This { loc: *loc, ty: this_ty.clone() }),
                            member: name.clone(),
                            loc: *loc,
                            ty: cap_ty.clone(),
                        };
                        break;
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                Self::rewrite_lambda_captures_in_expr(left, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(right, captures, lambda_name);
            }
            Expr::Unary { operand, .. } => {
                Self::rewrite_lambda_captures_in_expr(operand, captures, lambda_name);
            }
            Expr::Call { name: _, args, .. } => {
                // name is a String, not an Expr to rewrite
                for a in args.iter_mut() {
                    Self::rewrite_lambda_captures_in_expr(a, captures, lambda_name);
                }
            }
            Expr::MemberCall { object, args, .. } => {
                Self::rewrite_lambda_captures_in_expr(object, captures, lambda_name);
                for a in args.iter_mut() {
                    Self::rewrite_lambda_captures_in_expr(a, captures, lambda_name);
                }
            }
            Expr::Index { array, index, .. } => {
                Self::rewrite_lambda_captures_in_expr(array, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(index, captures, lambda_name);
            }
            Expr::Member { object, .. } => {
                Self::rewrite_lambda_captures_in_expr(object, captures, lambda_name);
            }
            Expr::Assign { left, right, .. } => {
                Self::rewrite_lambda_captures_in_expr(left, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(right, captures, lambda_name);
            }
            Expr::Ternary {
                cond, then_branch, else_branch, ..
            } => {
                Self::rewrite_lambda_captures_in_expr(cond, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(then_branch, captures, lambda_name);
                Self::rewrite_lambda_captures_in_expr(else_branch, captures, lambda_name);
            }
            Expr::Cast { expr, .. } => {
                Self::rewrite_lambda_captures_in_expr(expr, captures, lambda_name);
            }
            Expr::Sizeof { operand: Some(e), .. } => {
                Self::rewrite_lambda_captures_in_expr(e, captures, lambda_name);
            }
            Expr::InitList { elements, .. } => {
                for e in elements.iter_mut() {
                    Self::rewrite_lambda_captures_in_expr(&mut e.value, captures, lambda_name);
                }
            }
            Expr::Offsetof { .. } => {}
            Expr::Lambda { body, .. } => {
                Self::rewrite_lambda_captures(body, captures, lambda_name);
            }
            _ => {}
        }
    }
}
