use super::*;

impl TypeChecker {
    /// 推导 auto 类型为具体类型。
    pub(crate) fn deduce_auto_type(&mut self, init: &Expr) -> Type {
        match init {
            Expr::Literal { .. } => Type::int(),
            // E1：auto 按字面量后缀定型（无后缀 double，f/F 后缀 float）
            Expr::FloatLiteral { ty, .. } => {
                if ty.kind() == vitro_ast::TypeKind::Double {
                    Type::double()
                } else {
                    Type::float()
                }
            }
            Expr::LongLiteral { .. } => Type::long_long(),
            Expr::StringLiteral { .. } => Type::pointer_to(Type::char()),
            Expr::Identifier { name, .. } => {
                if let Some(sym) = self.lookup_var(name) {
                    sym.ty.clone()
                } else {
                    self.report_error(
                        &format!("auto 推导失败：未找到变量 '{}'", name),
                        init.loc(),
                        ErrorCode::E3004_TypeMismatch,
                    );
                    Type::int()
                }
            }
            Expr::Call { ty, .. } | Expr::MemberCall { ty, .. } => ty.clone(),
            Expr::Unary { op, operand, .. } => {
                use vitro_ast::UnaryOp;
                match op {
                    UnaryOp::Addr => Type::pointer_to(self.deduce_auto_type(operand)),
                    UnaryOp::Deref => {
                        let inner = self.deduce_auto_type(operand);
                        if let Type::Pointer { pointee, .. } = inner {
                            *pointee
                        } else {
                            self.report_error(
                                "auto 推导失败：解引用非指针类型",
                                init.loc(),
                                ErrorCode::E3004_TypeMismatch,
                            );
                            Type::int()
                        }
                    }
                    UnaryOp::Neg | UnaryOp::BitNot | UnaryOp::Not => self.deduce_auto_type(operand),
                    UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec => {
                        self.deduce_auto_type(operand)
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                let left_ty = self.deduce_auto_type(left);
                let right_ty = self.deduce_auto_type(right);
                // Prefer the wider of the two scalar types
                if left_ty.kind() == TypeKind::Double || right_ty.kind() == TypeKind::Double {
                    Type::double()
                } else if left_ty.kind() == TypeKind::Float || right_ty.kind() == TypeKind::Float {
                    Type::float()
                } else if left_ty.kind() == TypeKind::LongLong || right_ty.kind() == TypeKind::LongLong {
                    Type::long_long()
                } else {
                    left_ty
                }
            }
            Expr::New { elem_type, .. } => Type::pointer_to(elem_type.clone()),
            Expr::Lambda { unique_id, .. } => Type::Class {
                name: format!("__lambda_{}", unique_id),
                is_const: false,
            },
            Expr::Cast { target_type, .. } => target_type.clone(),
            Expr::Ternary { then_branch, .. } => self.deduce_auto_type(then_branch),
            Expr::Member { ty, .. } => ty.clone(),
            Expr::Index { ty, .. } => ty.clone(),
            Expr::InitList { .. } => {
                self.report_error(
                    "auto 推导失败：初始化列表需要显式类型",
                    init.loc(),
                    ErrorCode::E3004_TypeMismatch,
                );
                Type::int()
            }
            _ => {
                self.report_error("auto 推导失败：不支持的初始化表达式", init.loc(), ErrorCode::E3004_TypeMismatch);
                Type::int()
            }
        }
    }
}
