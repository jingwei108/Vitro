use super::*;

impl TypeChecker {
    pub(crate) fn resolve_sizeof(&mut self, operand: &mut Option<Box<Expr>>, loc: &SourceLoc, ty: &mut Type) -> Type {
        if let Some(ref mut op) = operand {
            self.resolve_expr_type(op);
            if let Expr::Identifier { name, .. } = &**op {
                if self.current_func_params.contains(name) {
                    if let Some(sym) = self.lookup_var(name) {
                        if sym.ty.is_pointer() {
                            self.report_warning(
                                "数组参数已退化为指针，sizeof 结果为指针大小，而非数组总大小。",
                                loc,
                                ErrorCode::W3052_ArrayToPointerDecay,
                            );
                        }
                    }
                }
            }
        }
        *ty = Type::int();
        ty.clone()
    }

    /// C23 alignof（E1）：结果为 int；对齐值由 codegen 的 type_align 计算
    ///（typeck 只需解析操作数类型并定型）。
    pub(crate) fn resolve_alignof(&mut self, operand: &mut Option<Box<Expr>>, _loc: &SourceLoc, ty: &mut Type) -> Type {
        if let Some(ref mut op) = operand {
            self.resolve_expr_type(op);
        }
        *ty = Type::int();
        ty.clone()
    }

    pub(crate) fn resolve_cast(
        &mut self,
        expr_inner: &mut Expr,
        target_type: &Type,
        loc: &SourceLoc,
        ty: &mut Type,
    ) -> Type {
        let expr_ty = self.resolve_expr_type(expr_inner);
        *ty = target_type.clone();
        // Warn on pointer <-> double cast (implementation-defined behavior)
        if (target_type.is_pointer() && matches!(expr_ty.kind(), TypeKind::Float | TypeKind::Double))
            || (expr_ty.is_pointer() && matches!(target_type.kind(), TypeKind::Float | TypeKind::Double))
        {
            self.report_warning(
                &format!("将 '{}' 转换为 '{}' 是实现定义的行为，结果可能不可移植", expr_ty, target_type),
                loc,
                ErrorCode::W3064_DoublePointerCast,
            );
        }
        ty.clone()
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn resolve_offsetof_unreachable(&mut self) -> Type {
        // Should have been replaced by Literal above; unreachable
        Type::int()
    }
}
