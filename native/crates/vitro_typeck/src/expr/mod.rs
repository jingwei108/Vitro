use super::*;

mod call;
mod cast;
mod cpp;
mod literal;
mod ops;
mod var;

/// C usual arithmetic conversions: promote two scalar types to a common type.
fn promote_type(a: &Type, b: &Type) -> Type {
    use TypeKind::*;
    let rank = |t: &Type| match t.kind() {
        Double => 4,
        Float => 3,
        LongLong => 2,
        Int => 1,
        Char => 0,
        _ => -1,
    };
    let ra = rank(a);
    let rb = rank(b);
    let (higher, lower) = if ra >= rb { (a, b) } else { (b, a) };
    let is_unsigned = higher.is_unsigned() || lower.is_unsigned();
    match higher.kind() {
        Double => Type::double(),
        Float => Type::float(),
        LongLong => Type::LongLong { is_unsigned, is_const: false },
        Int => Type::Int { is_unsigned, is_const: false },
        Char => Type::Int { is_unsigned, is_const: false },
        _ => Type::int(),
    }
}

impl TypeChecker {
    /// Try to resolve an unqualified function call inside a class member function as a
    /// call to a class method (C++ name hiding). On success, returns the MemberCall
    /// expression on `this` and the result type so the caller can replace the original.
    fn try_resolve_unqualified_method_call(
        &mut self,
        name: &str,
        args: &mut [Expr],
        loc: &SourceLoc,
    ) -> Option<(Expr, Type)> {
        let class_name = self.current_class.clone()?;
        let has_method = self
            .classes
            .get(&class_name)
            .map(|s| s.methods.contains_key(name))
            .unwrap_or(false);
        if !has_method {
            return None;
        }
        let arg_types: Vec<Type> = args.iter_mut().map(|a| self.resolve_expr_type(a)).collect();
        // D1：解析不到匹配重载时**必须报编译诊断**，不能返回 None 静默放行——
        // 否则调用点会退回"按名字找函数"的兜底路径，派发到同名的另一实现
        // （同参数量仅类型不同的重载），最终运行时栈下溢 trap。
        let (sig, mangled) = match self.resolve_method_overload(&class_name, name, &arg_types) {
            Some(v) => v,
            None => {
                self.report_error(
                    &format!(
                        "方法 '{}' 没有匹配的重载：参数类型 ({}) 与已声明的重载签名均不兼容",
                        name,
                        arg_types
                            .iter()
                            .map(vitro_runtime::type_display_name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    loc,
                    ErrorCode::E4026_AmbiguousMethodCall,
                );
                return None;
            }
        };
        let this_ty = Type::Pointer {
            pointee: Box::new(Type::Class {
                name: class_name,
                is_const: self.current_method_is_const,
            }),
            is_const: self.current_method_is_const,
        };
        let mut new_args = Vec::with_capacity(args.len());
        for arg in args.iter_mut() {
            new_args.push(std::mem::take(arg));
        }
        for (arg, expected) in new_args.iter_mut().zip(sig.param_types.iter()) {
            let arg_type = arg.ty().clone();
            if !self.check_assignable(expected, &arg_type, loc) {
                self.report_error(&format!("方法 '{}' 参数类型不匹配", name), loc, ErrorCode::E3038_FuncArgType);
            } else if expected.is_reference() && !arg_type.is_reference() && !arg_type.is_rvalue_ref() {
                let is_const_ref =
                    expected.is_const_reference() || expected.reference_base().map(|b| b.is_const()).unwrap_or(false);
                if !is_const_ref && !self.is_lvalue(arg) {
                    self.report_error(
                        &format!("方法 '{}' 参数：非 const 引用不能绑定到右值", name),
                        loc,
                        ErrorCode::E3038_FuncArgType,
                    );
                } else {
                    let arg_loc = *arg.loc();
                    let old = std::mem::take(arg);
                    *arg = Expr::Unary {
                        op: UnaryOp::Addr,
                        operand: Box::new(old),
                        loc: arg_loc,
                        ty: expected.clone(),
                    };
                }
            } else {
                insert_implicit_cast(arg, expected);
            }
        }
        let ret = sig.ret.clone();
        let new_expr = Expr::MemberCall {
            object: Box::new(Expr::This { loc: *loc, ty: this_ty }),
            method: name.to_string(),
            args: new_args,
            is_virtual: sig.is_virtual,
            resolved_mangled: Some(mangled),
            loc: *loc,
            ty: ret.clone(),
        };
        Some((new_expr, ret))
    }

    pub fn resolve_expr_type(&mut self, expr: &mut Expr) -> Type {
        // Handle offsetof: compute offset at compile time and replace with Literal
        if let Expr::Offsetof { target_type, field, loc, .. } = expr {
            let type_name = target_type.name().to_string();
            let field_name = field.clone();
            let loc_val = *loc;
            let mut offset = 0;
            let mut found = false;
            if let Some(struct_sym) = self.structs.get(&type_name) {
                for (fty, fname) in &struct_sym.fields {
                    if *fname == field_name {
                        found = true;
                        break;
                    }
                    offset += self.compute_type_size(fty);
                }
                if !found {
                    self.report_error(
                        &format!("结构体 '{}' 没有字段 '{}'", type_name, field_name),
                        &loc_val,
                        ErrorCode::E3042_UnknownMember,
                    );
                }
            } else if let Some(union_sym) = self.unions.get(&type_name) {
                // Union: all fields start at offset 0
                if !union_sym.fields.iter().any(|(_, fname)| *fname == field_name) {
                    self.report_error(
                        &format!("联合体 '{}' 没有字段 '{}'", type_name, field_name),
                        &loc_val,
                        ErrorCode::E3042_UnknownMember,
                    );
                }
                offset = 0;
            } else {
                self.report_error(
                    &format!("未知的结构体/联合体类型 '{}'", type_name),
                    &loc_val,
                    ErrorCode::E3004_TypeMismatch,
                );
            }
            *expr = Expr::Literal {
                value: offset,
                loc: loc_val,
                ty: Type::int(),
            };
            return Type::int();
        }

        match expr {
            Expr::Binary { op, left, right, loc, ty } => self.resolve_binary(op, left, right, loc, ty),
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                loc,
                ty,
            } => self.resolve_ternary(cond, then_branch, else_branch, loc, ty),
            Expr::Unary { op, operand, loc, ty } => self.resolve_unary(op, operand, loc, ty),
            Expr::Literal { ty, .. } => self.resolve_literal(ty),
            Expr::FloatLiteral { ty, .. } => self.resolve_float_literal(ty),
            Expr::LongLiteral { .. } => self.resolve_long_literal(),
            Expr::StringLiteral { value, ty, .. } => self.resolve_string_literal(value, ty),
            Expr::Identifier { .. } => self.resolve_identifier(expr),
            Expr::Call { .. } => self.resolve_call(expr),
            Expr::CallPtr { .. } => self.resolve_call_ptr(expr),
            Expr::Index { array, index, loc, ty } => self.resolve_index(array, index, loc, ty),
            Expr::Member { object, member, loc, ty } => self.resolve_member(object, member, loc, ty),
            Expr::Assign { op, left, right, loc, ty } => self.resolve_assign(op, left, right, loc, ty),
            Expr::Sizeof { operand, ty, loc, .. } => self.resolve_sizeof(operand, loc, ty),
            Expr::Alignof { operand, ty, loc, .. } => self.resolve_alignof(operand, loc, ty),
            Expr::Cast {
                expr: inner,
                target_type,
                ty,
                loc,
                ..
            } => self.resolve_cast(inner, target_type, loc, ty),
            Expr::InitList { elements, ty, .. } => self.resolve_init_list(elements, ty),
            Expr::Offsetof { .. } => self.resolve_offsetof_unreachable(),
            Expr::Generic { .. } => self.resolve_generic(expr),
            Expr::CompoundLiteral { .. } => self.resolve_compound_literal(expr),
            Expr::This { loc, ty } => self.resolve_this(loc, ty),
            Expr::MemberCall { .. } => self.resolve_member_call(expr),
            Expr::New { .. } => self.resolve_new(expr),
            Expr::Delete { .. } => self.resolve_delete(expr),
            Expr::Move { .. } => self.resolve_move(expr),
            Expr::Lambda { .. } => self.resolve_lambda(expr),
        }
    }
}
