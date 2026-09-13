use super::*;

impl TypeChecker {
    pub(crate) fn resolve_binary(
        &mut self,
        op: &BinaryOp,
        left: &mut Expr,
        right: &mut Expr,
        loc: &SourceLoc,
        ty: &mut Type,
    ) -> Type {
        let left_type = self.resolve_expr_type(left);
        let right_type = self.resolve_expr_type(right);
        *ty = match op {
            BinaryOp::Add | BinaryOp::Sub => {
                let left_is_ptrlike = left_type.is_pointer() || left_type.is_array();
                let right_is_ptrlike = right_type.is_pointer() || right_type.is_array();
                if self.is_scalar(&left_type) && self.is_scalar(&right_type) {
                    super::promote_type(&left_type, &right_type)
                } else if left_is_ptrlike && self.is_int(&right_type) {
                    if left_type.is_array() {
                        Type::pointer_to(left_type.subscript_type())
                    } else {
                        left_type.clone()
                    }
                } else if self.is_int(&left_type) && right_is_ptrlike && matches!(op, BinaryOp::Add) {
                    if right_type.is_array() {
                        Type::pointer_to(right_type.subscript_type())
                    } else {
                        right_type.clone()
                    }
                } else if left_is_ptrlike && right_is_ptrlike && matches!(op, BinaryOp::Sub) {
                    Type::int()
                } else {
                    self.report_error(
                        "算术运算要求两边都是 int 类型，或指针与整数",
                        loc,
                        ErrorCode::E3016_ArithmeticTypeError,
                    );
                    Type::int()
                }
            }
            BinaryOp::Mul | BinaryOp::Div => {
                if !self.is_scalar(&left_type) || !self.is_scalar(&right_type) {
                    self.report_error(
                        "乘除运算要求两边都是 int 或 float 类型",
                        loc,
                        ErrorCode::E3016_ArithmeticTypeError,
                    );
                }
                super::promote_type(&left_type, &right_type)
            }
            BinaryOp::Mod => {
                if !self.is_int(&left_type) && !matches!(left_type.kind(), TypeKind::LongLong)
                    || !self.is_int(&right_type) && !matches!(right_type.kind(), TypeKind::LongLong)
                {
                    self.report_error("取模运算要求两边都是 int 类型", loc, ErrorCode::E3016_ArithmeticTypeError);
                }
                super::promote_type(&left_type, &right_type)
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                if !self.is_comparable(&left_type, &right_type) {
                    self.report_error("类型不兼容，无法比较", loc, ErrorCode::E3017_ComparisonTypeError);
                }
                Type::int()
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let left_is_ptrlike = matches!(left_type.kind(), TypeKind::Pointer | TypeKind::Array);
                let right_is_ptrlike = matches!(right_type.kind(), TypeKind::Pointer | TypeKind::Array);
                if !(self.is_scalar(&left_type) && self.is_scalar(&right_type) || left_is_ptrlike && right_is_ptrlike) {
                    self.report_error(
                        "关系运算要求两边都是 int/float 类型或同类型指针",
                        loc,
                        ErrorCode::E3018_RelationTypeError,
                    );
                }
                Type::int()
            }
            BinaryOp::And | BinaryOp::Or => {
                let left_ok = self.is_scalar(&left_type) || left_type.is_pointer() || left_type.is_array();
                let right_ok = self.is_scalar(&right_type) || right_type.is_pointer() || right_type.is_array();
                if !left_ok || !right_ok {
                    self.report_error("逻辑运算要求两边都是标量、指针或数组类型", loc, ErrorCode::E3019_LogicTypeError);
                }
                Type::int()
            }
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                // E1 B 档：放行 long long（E3048 半成品缺陷——typeck 拒绝但算术族
                // 已有 Q 系列，属系统性误拒的一半）
                if !self.is_int(&left_type) && !matches!(left_type.kind(), TypeKind::LongLong)
                    || !self.is_int(&right_type) && !matches!(right_type.kind(), TypeKind::LongLong)
                {
                    self.report_error("位运算要求两边都是整数类型", loc, ErrorCode::E3048_BitOpTypeError);
                }
                super::promote_type(&left_type, &right_type)
            }
            BinaryOp::Shl | BinaryOp::Shr => {
                if !self.is_int(&left_type) && !matches!(left_type.kind(), TypeKind::LongLong)
                    || !self.is_int(&right_type) && !matches!(right_type.kind(), TypeKind::LongLong)
                {
                    self.report_error("位运算要求两边都是整数类型", loc, ErrorCode::E3048_BitOpTypeError);
                }
                // Result type is the promoted left operand type (C semantics)
                if left_type.kind() == TypeKind::Char {
                    Type::Int {
                        is_unsigned: left_type.is_unsigned(),
                        is_const: false,
                    }
                } else {
                    left_type.clone()
                }
            }
            BinaryOp::Comma => {
                // Comma operator: evaluate left, discard result, return right's type
                right_type.clone()
            }
        };
        ty.clone()
    }

    pub(crate) fn resolve_ternary(
        &mut self,
        cond: &mut Expr,
        then_branch: &mut Expr,
        else_branch: &mut Expr,
        loc: &SourceLoc,
        ty: &mut Type,
    ) -> Type {
        let cond_type = self.resolve_expr_type(cond);
        if !self.is_scalar(&cond_type) && !matches!(cond_type.kind(), TypeKind::Pointer | TypeKind::Array) {
            self.report_error(
                "三目运算符条件必须是 int、float 或指针类型",
                loc,
                ErrorCode::E3020_UnaryTypeError,
            );
        }
        let then_type = self.resolve_expr_type(then_branch);
        let else_type = self.resolve_expr_type(else_branch);
        // 对数组类型执行通常转换：数组在大多数表达式中退化为指向首元素的指针。
        // 例如 `" "`（char[2]）和 `""`（char[1]）在三目运算符中应统一为 char*。
        let decay = |t: &Type| -> Type {
            if let Type::Array { element, .. } = t {
                Type::pointer_to(*element.clone())
            } else {
                t.clone()
            }
        };
        let then_decayed = decay(&then_type);
        let else_decayed = decay(&else_type);
        if then_decayed.kind() != else_decayed.kind()
            || then_decayed.name() != else_decayed.name()
            || then_decayed != else_decayed
        {
            self.report_error("三目运算符分支类型不匹配", loc, ErrorCode::E3004_TypeMismatch);
        }
        *ty = then_decayed.clone();
        ty.clone()
    }

    pub(crate) fn resolve_unary(&mut self, op: &UnaryOp, operand: &mut Expr, loc: &SourceLoc, ty: &mut Type) -> Type {
        let operand_type = self.resolve_expr_type(operand);
        *ty = match op {
            UnaryOp::Neg => {
                if !self.is_scalar(&operand_type) {
                    self.report_error("取负运算要求操作数是 int 或 float 类型", loc, ErrorCode::E3020_UnaryTypeError);
                }
                if operand_type.kind() == TypeKind::Double {
                    Type::double()
                } else if operand_type.kind() == TypeKind::Float {
                    Type::float()
                } else if operand_type.kind() == TypeKind::LongLong {
                    Type::long_long()
                } else if operand_type.is_unsigned() {
                    Type::unsigned_int()
                } else {
                    Type::int()
                }
            }
            UnaryOp::Not => {
                if !self.is_scalar(&operand_type) && !operand_type.is_pointer() && !operand_type.is_array() {
                    self.report_error("逻辑非要求操作数是标量、指针或数组类型", loc, ErrorCode::E3020_UnaryTypeError);
                }
                Type::int()
            }
            UnaryOp::BitNot => {
                // E1 B 档：放行 long long（同 E3048 半成品缺陷）；结果随操作数宽度
                if !self.is_int(&operand_type) && !matches!(operand_type.kind(), TypeKind::LongLong) {
                    self.report_error("按位取反要求操作数是整数类型", loc, ErrorCode::E3020_UnaryTypeError);
                }
                if matches!(operand_type.kind(), TypeKind::LongLong) {
                    operand_type.clone()
                } else {
                    Type::int()
                }
            }
            UnaryOp::Addr => Type::pointer_to(operand_type.clone()),
            UnaryOp::Deref => {
                if !operand_type.is_pointer() && !operand_type.is_array() {
                    self.report_error("解引用要求指针类型", loc, ErrorCode::E3021_DerefNonPointer);
                    Type::int()
                } else {
                    match operand_type {
                        Type::Pointer { pointee, .. } | Type::Array { element: pointee, .. } => *pointee.clone(),
                        _ => Type::int(),
                    }
                }
            }
            UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                // T-P0-3/T-P1-3：允许 long long（is_int 不含 LongLong 曾系统性误拒）；
                // char 数组元素/成员自增的元素类型是 Char，同样按整数放行
                if !self.is_int(&operand_type)
                    && operand_type.kind() != TypeKind::Char
                    && operand_type.kind() != TypeKind::Float
                    && operand_type.kind() != TypeKind::Double
                    && operand_type.kind() != TypeKind::LongLong
                    && !operand_type.is_pointer()
                {
                    self.report_error("自增/自减要求数值类型或指针类型", loc, ErrorCode::E3022_IncDecTypeError);
                }
                if let Expr::Identifier { name, .. } = &*operand {
                    if let Some(sym) = self.lookup_var(name) {
                        if sym.ty.is_const() {
                            self.report_error(
                                &format!("不能修改常量变量 '{}'", name),
                                loc,
                                ErrorCode::E3049_AssignToConst,
                            );
                        }
                    }
                }
                // C 语义：++/-- 的结果是左值，类型与操作数相同
                // （此前非指针一律标 int，导致 ++doubleVar 作为 printf %f 实参被误报）
                operand_type.clone()
            }
        };
        ty.clone()
    }

    pub(crate) fn resolve_assign(
        &mut self,
        op: &AssignOp,
        left: &mut Expr,
        right: &mut Expr,
        loc: &SourceLoc,
        ty: &mut Type,
    ) -> Type {
        let right_type = self.resolve_expr_type(right);
        let left_type = self.resolve_expr_type(left);
        let is_lvalue = matches!(
            &*left,
            Expr::Identifier { .. } | Expr::Index { .. } | Expr::Member { .. } | Expr::Unary { op: UnaryOp::Deref, .. }
        ) || (matches!(&*left, Expr::Call { .. } | Expr::CallPtr { .. } | Expr::MemberCall { .. })
            && (left_type.is_reference() || left_type.is_rvalue_ref()));
        if !is_lvalue {
            self.report_error("赋值左边必须是可修改的左值", loc, ErrorCode::E3043_AssignToRValue);
        }
        if let Expr::Identifier { name, .. } = &*left {
            if let Some(sym) = self.lookup_var(name) {
                if sym.ty.is_const() {
                    self.report_error(&format!("不能给常量变量 '{}' 赋值", name), loc, ErrorCode::E3049_AssignToConst);
                }
            }
        }
        if let Expr::Unary {
            op: UnaryOp::Deref, operand, ..
        } = &*left
        {
            if let Type::Pointer { pointee, .. } = operand.ty() {
                if pointee.is_const() {
                    self.report_error("不能通过非 const 指针修改 const 数据", loc, ErrorCode::E3065_ConstViolation);
                }
            }
        }
        if let Expr::Member { object, .. } = &*left {
            let obj_ty = object.ty();
            if let Type::Pointer { pointee, is_const } = &obj_ty {
                if let Type::Class { is_const: ic, .. } = pointee.as_ref() {
                    if *ic || *is_const {
                        self.report_error("不能修改 const 对象的成员", loc, ErrorCode::E3065_ConstViolation);
                    }
                }
            } else if let Type::Class { is_const, .. } = &obj_ty {
                if *is_const {
                    self.report_error("不能修改 const 对象的成员", loc, ErrorCode::E3065_ConstViolation);
                }
            }
        }
        let is_ptr_compound = matches!(*op, AssignOp::AddAssign | AssignOp::SubAssign);
        let left_is_ptr = left_type.is_pointer();
        let left_is_func_ptr = left_type.is_function_pointer();
        let right_is_int = self.is_int(&right_type) || right_type.kind() == TypeKind::LongLong;
        let ptr_compound_ok = is_ptr_compound && left_is_ptr && !left_is_func_ptr && right_is_int;

        if ptr_compound_ok {
            // 指针 +=/-= 整数：跳过普通赋值兼容性检查，右侧保持整数类型。
        } else if !self.check_assignable(&left_type, &right_type, loc) {
            self.report_error(
                &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", right_type, left_type),
                loc,
                ErrorCode::E3044_AssignTypeMismatch,
            );
        } else {
            insert_implicit_cast(right, &left_type);
        }

        if *op != AssignOp::Assign {
            if ptr_compound_ok {
                // 指针与整数的 +=/-= 已允许。
            } else if left_is_ptr {
                let msg = if is_ptr_compound && left_is_func_ptr {
                    "函数指针不支持算术运算"
                } else if is_ptr_compound && !right_is_int {
                    "指针的复合赋值要求右侧为整数类型"
                } else {
                    "指针不支持此复合赋值运算符"
                };
                self.report_error(msg, loc, ErrorCode::E3045_CompoundAssignType);
            } else if !self.is_scalar(&left_type) || !self.is_scalar(&right_type) {
                self.report_error(
                    "复合赋值要求两边都是标量类型（int、float、double 或 long long）",
                    loc,
                    ErrorCode::E3045_CompoundAssignType,
                );
            }
        }
        *ty = left_type.clone();
        ty.clone()
    }

    pub(crate) fn resolve_generic(&mut self, expr: &mut Expr) -> Type {
        if let Expr::Generic {
            control,
            associations,
            default,
            loc,
            ty,
        } = expr
        {
            let control_type = self.resolve_expr_type(control);
            // 数组在大多数表达式中退化为指向首元素的指针，
            // 使字符串字面量等能匹配 char* 关联。
            let control_decayed = if let Type::Array { element, .. } = &control_type {
                Type::pointer_to(*element.clone())
            } else {
                control_type.clone()
            };

            let mut selected: Option<&mut Expr> = None;
            for (assoc_ty, assoc_expr) in associations.iter_mut() {
                if *assoc_ty == control_decayed {
                    selected = Some(assoc_expr);
                    break;
                }
            }

            let result_ty = if let Some(sel) = selected {
                self.resolve_expr_type(sel)
            } else if let Some(default_expr) = default.as_deref_mut() {
                self.resolve_expr_type(default_expr)
            } else {
                self.report_error(
                    "_Generic 控制表达式类型无匹配关联且无 default 分支",
                    loc,
                    ErrorCode::E3004_TypeMismatch,
                );
                Type::int()
            };
            *ty = result_ty.clone();
            return result_ty;
        }
        Type::int()
    }
}
