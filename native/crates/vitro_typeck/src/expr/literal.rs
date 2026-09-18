use super::*;
use vitro_ast::TypeKind;
use vitro_shared::ErrorCode;

impl TypeChecker {
    pub(crate) fn resolve_literal(&mut self, ty: &mut Type) -> Type {
        if ty.is_unsigned() {
            ty.clone()
        } else {
            Type::int()
        }
    }

    /// E1：浮点字面量类型由 parser 按后缀定型（无后缀 double / f·F 后缀 float，
    /// C 标准）；此前无条件返回 float，与 codegen 的 PushConstD 位宽错位。
    #[allow(clippy::unused_self)]
    pub(crate) fn resolve_float_literal(&mut self, ty: &Type) -> Type {
        if ty.kind() == TypeKind::Double {
            Type::double()
        } else {
            Type::float()
        }
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn resolve_long_literal(&mut self) -> Type {
        Type::long_long()
    }

    pub(crate) fn resolve_string_literal(&mut self, value: &str, ty: &mut Type) -> Type {
        // P4：C 语义字节长度（\xHH >= 0x80 转义单字节；sizeof 折叠同源）
        let array_size = vitro_shared::cstring_len(value) as i32 + 1;
        *ty = Type::Array {
            element: Box::new(Type::char()),
            array_size,
            dims: vec![array_size],
            is_const: false,
            is_vla: false,
            vla_dims: vec![],
        };
        ty.clone()
    }

    pub(crate) fn resolve_init_list(&mut self, elements: &mut [InitElement], ty: &mut Type) -> Type {
        for elem in elements.iter_mut() {
            self.resolve_expr_type(&mut elem.value);
        }
        *ty = Type::void();
        ty.clone()
    }

    pub(crate) fn resolve_compound_literal(&mut self, expr: &mut Expr) -> Type {
        if let Expr::CompoundLiteral { target_type, init, loc, ty } = expr {
            match target_type.kind() {
                TypeKind::Struct => {
                    self.check_struct_initializer(target_type, init, loc);
                }
                TypeKind::Array => {
                    self.check_array_initializer(target_type, init, loc);
                }
                TypeKind::Int
                | TypeKind::Char
                | TypeKind::Float
                | TypeKind::Double
                | TypeKind::LongLong
                | TypeKind::Pointer => {
                    if let Expr::InitList { elements, .. } = init.as_mut() {
                        if elements.len() != 1 {
                            self.report_error(
                                "标量复合字面量初始化列表只能有一个元素",
                                loc,
                                ErrorCode::E3009_InvalidArrayInit,
                            );
                        } else {
                            let e_type = self.resolve_expr_type(&mut elements[0].value);
                            if !self.check_assignable(target_type, &e_type, loc) {
                                self.report_error(
                                    &format!("标量复合字面量类型不匹配：期望 '{}'，实际 '{}'", target_type, e_type),
                                    loc,
                                    ErrorCode::E3006_ArrayInitTypeMismatch,
                                );
                            } else {
                                insert_implicit_cast(&mut elements[0].value, target_type);
                            }
                        }
                    } else {
                        self.report_error(
                            "标量复合字面量必须使用 { value } 形式",
                            loc,
                            ErrorCode::E3009_InvalidArrayInit,
                        );
                    }
                }
                _ => {
                    self.report_error(
                        &format!("复合字面量暂不支持类型 '{}'", target_type),
                        loc,
                        ErrorCode::E1006_UnsupportedFeature,
                    );
                }
            }
            *ty = target_type.clone();
            return ty.clone();
        }
        Type::int()
    }
}
