use super::*;

impl TypeChecker {
    // -------------------------------------------------------------------------
    // 合成 AST 小工具
    // -------------------------------------------------------------------------
    pub(crate) fn synth_int_lit(value: i32, loc: SourceLoc) -> Expr {
        Expr::Literal { value, loc, ty: Type::int() }
    }

    pub(crate) fn synth_ident(name: &str, ty: Type, loc: SourceLoc) -> Expr {
        Expr::Identifier {
            name: name.to_string(),
            loc,
            ty,
        }
    }

    pub(crate) fn synth_cast(expr: Expr, target_type: Type, loc: SourceLoc) -> Expr {
        Expr::Cast {
            expr: Box::new(expr),
            target_type: target_type.clone(),
            loc,
            ty: target_type,
        }
    }

    pub(crate) fn synth_this(ptr_ty: Type, loc: SourceLoc) -> Expr {
        Expr::This { loc, ty: ptr_ty }
    }

    pub(crate) fn synth_member_this(class_name: &str, member: &str, ty: Type, loc: SourceLoc) -> Expr {
        Expr::Member {
            object: Box::new(Self::synth_this(
                Type::pointer_to(Type::Class {
                    name: class_name.to_string(),
                    is_const: false,
                }),
                loc,
            )),
            member: member.to_string(),
            loc,
            ty,
        }
    }

    pub(crate) fn synth_assign(left: Expr, right: Expr, loc: SourceLoc) -> Expr {
        Expr::Assign {
            op: AssignOp::Assign,
            left: Box::new(left),
            right: Box::new(right),
            loc,
            ty: Type::void(),
        }
    }

    pub(crate) fn synth_expr_stmt(expr: Expr) -> Stmt {
        let loc = *expr.loc();
        Stmt::Expr { expr, loc }
    }
}
