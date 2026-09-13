use super::*;

impl TypeChecker {
    pub(crate) fn resolve_call(&mut self, expr: &mut Expr) -> Type {
        let (name, loc) = if let Expr::Call { name, loc, .. } = expr {
            (name.clone(), *loc)
        } else {
            unreachable!()
        };

        // Inside a class member function, an unqualified call may refer to a class
        // method (C++ name hiding). Try method overload resolution first.
        let method_result = if let Expr::Call { args, .. } = expr {
            self.try_resolve_unqualified_method_call(&name, args, &loc)
        } else {
            unreachable!()
        };
        if let Some((new_expr, ret)) = method_result {
            *expr = new_expr;
            return ret;
        }

        if let Expr::Call { args, ty, .. } = expr {
            *ty = self.visit_call(&name, args, &loc);
            ty.clone()
        } else {
            unreachable!()
        }
    }

    pub(crate) fn resolve_call_ptr(&mut self, expr: &mut Expr) -> Type {
        // Extract the callee name (if it is an identifier) and location.
        // Do not keep a borrow of callee so that we can reborrow expr later.
        let (name, loc) = if let Expr::CallPtr { callee, loc, .. } = expr {
            let name = if let Expr::Identifier { name, .. } = callee.as_ref() {
                Some(name.clone())
            } else {
                None
            };
            (name, *loc)
        } else {
            unreachable!()
        };

        if let Some(ref name) = name {
            // Direct named function call: identifier is a known function
            if self.funcs.contains_key(name)
                || self.static_func_sigs.contains_key(name)
                || self.is_builtin_func(name)
                || name.starts_with("std__")
            {
                if let Expr::CallPtr { args, ty, .. } = expr {
                    *ty = self.visit_call(name, args, &loc);
                    return ty.clone();
                }
            }
            // Inside a class member function, an unqualified call may refer to a class method.
            let method_result = if let Expr::CallPtr { args, .. } = expr {
                self.try_resolve_unqualified_method_call(name, args, &loc)
            } else {
                unreachable!()
            };
            if let Some((new_expr, ret)) = method_result {
                *expr = new_expr;
                return ret;
            }
            // Try template implicit instantiation
            let template_result = if let Expr::CallPtr { args, .. } = expr {
                if !args.is_empty() {
                    let arg_types: Vec<Type> = args.iter_mut().map(|a| self.resolve_expr_type(a)).collect();
                    if let Some((mangled, maybe_new_func)) = self.try_instantiate_template(name, &arg_types) {
                        if let Some(new_func) = maybe_new_func {
                            self.pending_instantiations.push((mangled.clone(), new_func));
                        }
                        // Rewrite CallPtr -> Call so BytecodeGen can resolve by mangled name
                        let new_args = std::mem::take(args);
                        Some((mangled, new_args))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                unreachable!()
            };
            if let Some((mangled, new_args)) = template_result {
                *expr = Expr::Call {
                    name: mangled,
                    args: new_args,
                    loc,
                    ty: Type::default(),
                };
                return self.resolve_expr_type(expr);
            }
            // Lambda call: f(args) -> __lambda_N__call(f, args)
            if let Expr::CallPtr { args, ty, .. } = expr {
                let lambda_ty = self.lookup_var(name).map(|s| s.ty.clone());
                let lambda_class = match &lambda_ty {
                    Some(Type::Class { name: class_name, .. }) if class_name.starts_with("__lambda_") => {
                        Some(class_name.clone())
                    }
                    _ => None,
                };
                if let Some(class_name) = lambda_class {
                    let this_operand = Expr::Identifier {
                        name: name.clone(),
                        ty: lambda_ty.unwrap_or_default(),
                        loc,
                    };
                    if let Some((new_expr, ret_ty)) =
                        self.rewrite_lambda_call(&class_name, this_operand, args, &loc)
                    {
                        *expr = new_expr;
                        return ret_ty;
                    }
                    self.report_error(
                        &format!("未找到 Lambda 调用函数 '{}__call'", class_name),
                        &loc,
                        ErrorCode::E3036_UndefinedFunc,
                    );
                    *ty = Type::int();
                    return Type::int();
                }
            } else {
                unreachable!()
            }
        }

        // C++ lambda 立即调用：`[](int a, int b){ return a + b; }(2, 3)`。
        // 此时 callee 是 Lambda 表达式节点而非标识符，上面全部具名函数路径都不适用，
        // 旧实现直接落到下方"非函数指针"兜底并报错（Issue B1）。
        // 语义等价于 `auto __tmp = <lambda>; __tmp(2, 3)`，与变量形式共用同一改写。
        if let Expr::CallPtr { callee, args, loc, ty } = expr {
            if matches!(callee.as_ref(), Expr::Lambda { .. }) {
                // 先解析 lambda 类型：注册 __lambda_N 类与 __lambda_N__call 函数符号
                let lambda_ty = self.resolve_expr_type(callee);
                if let Type::Class { name: class_name, .. } = &lambda_ty {
                    let class_name = class_name.clone();
                    let this_operand = (**callee).clone();
                    let loc = *loc;
                    if let Some((new_expr, ret_ty)) =
                        self.rewrite_lambda_call(&class_name, this_operand, args, &loc)
                    {
                        *expr = new_expr;
                        return ret_ty;
                    }
                    self.report_error(
                        &format!("未找到 Lambda 调用函数 '{}__call'", class_name),
                        &loc,
                        ErrorCode::E3036_UndefinedFunc,
                    );
                    *ty = Type::int();
                    return Type::int();
                }
            }
        }

        let callee = if let Expr::CallPtr { callee, .. } = expr {
            callee.as_mut()
        } else {
            unreachable!()
        };
        let callee_ty = self.resolve_expr_type(callee);
        let func_info = if let Type::Pointer { pointee, .. } = &callee_ty {
            if let Type::Function { param_types, return_type, .. } = pointee.as_ref() {
                Some((param_types.clone(), return_type.as_ref().clone()))
            } else {
                // Allow calling through generic pointer (with warning)
                self.report_warning(
                    "通过通用指针调用函数，建议显式转换为函数指针",
                    &loc,
                    ErrorCode::W3055_VoidPointerCast,
                );
                if let Expr::CallPtr { args, ty, .. } = expr {
                    for arg in args.iter_mut() {
                        self.resolve_expr_type(arg);
                    }
                    *ty = Type::int();
                }
                None
            }
        } else if let Type::Function { param_types, return_type, .. } = &callee_ty {
            // Support (*fp)(args) where callee type is Function directly
            Some((param_types.clone(), return_type.as_ref().clone()))
        } else {
            self.report_error("不能对非函数类型进行调用", &loc, ErrorCode::E3066_CallNonFunction);
            if let Expr::CallPtr { args, ty, .. } = expr {
                for arg in args.iter_mut() {
                    self.resolve_expr_type(arg);
                }
                *ty = Type::int();
            }
            None
        };
        if let Some((param_types, return_type)) = func_info {
            if let Expr::CallPtr { args, ty, .. } = expr {
                if args.len() != param_types.len() {
                    self.report_error(
                        &format!("函数指针调用参数数量不匹配：期望 {}，实际 {}", param_types.len(), args.len()),
                        &loc,
                        ErrorCode::E3037_FuncArgCount,
                    );
                } else {
                    for (i, (arg, expected)) in args.iter_mut().zip(param_types.iter()).enumerate() {
                        let arg_type = self.resolve_expr_type(arg);
                        if !self.check_assignable(expected, &arg_type, &loc) {
                            self.report_error(
                                &format!("函数指针调用第 {} 个参数类型不匹配", i + 1),
                                &loc,
                                ErrorCode::E3038_FuncArgType,
                            );
                        } else {
                            insert_implicit_cast(arg, expected);
                        }
                    }
                }
                *ty = return_type;
            }
        }
        if let Expr::CallPtr { ty, .. } = expr {
            ty.clone()
        } else {
            unreachable!()
        }
    }

    /// C++ lambda 调用的统一改写：`lambda(args)` → `__lambda_N__call(this, args)`。
    ///
    /// 两种书写形式共用本函数，避免"变量形式 / 立即调用形式"双轨语义漂移：
    /// - 变量形式 `auto f = [](int x){ return x; }; f(1)`——`this_operand` 是标识符 `f`
    ///   （codegen 中 lambda 变量槽里存的是闭包对象地址）；
    /// - 立即调用 `[](int a, int b){ return a + b; }(2, 3)`——`this_operand` 是 Lambda
    ///   表达式节点本身（`gen_lambda` 求值后在栈顶留下闭包对象地址，与变量形式同语义）。
    ///
    /// 返回 `(改写后的 Call 表达式, 返回类型)`；`__call` 未注册时返回 `None`，由调用方兜底。
    fn rewrite_lambda_call(
        &mut self,
        class_name: &str,
        this_operand: Expr,
        args: &mut [Expr],
        loc: &SourceLoc,
    ) -> Option<(Expr, Type)> {
        let call_name = format!("{}__call", class_name);
        let func_sym = self.funcs.get(&call_name).cloned()?;
        let expected = func_sym.param_types.clone();
        if args.len() + 1 != expected.len() {
            self.report_error(
                &format!(
                    "Lambda 调用参数数量不匹配：期望 {} 个，实际 {} 个",
                    expected.len().saturating_sub(1),
                    args.len()
                ),
                loc,
                ErrorCode::E3037_FuncArgCount,
            );
        } else {
            for (i, arg) in args.iter_mut().enumerate() {
                let arg_ty = self.resolve_expr_type(arg);
                let expected_ty = &expected[i + 1];
                if !self.check_assignable(expected_ty, &arg_ty, loc) {
                    self.report_error(
                        &format!(
                            "Lambda 调用第 {} 个参数类型不匹配：期望 '{}'，实际 '{}'",
                            i + 1,
                            expected_ty,
                            arg_ty
                        ),
                        loc,
                        ErrorCode::E3004_TypeMismatch,
                    );
                } else {
                    insert_implicit_cast(arg, expected_ty);
                }
            }
        }
        let mut new_args = vec![this_operand];
        new_args.extend(args.iter().cloned());
        let ret_ty = func_sym.return_type.clone();
        Some((
            Expr::Call {
                name: call_name,
                args: new_args,
                loc: *loc,
                ty: ret_ty.clone(),
            },
            ret_ty,
        ))
    }

    pub(crate) fn resolve_member_call(&mut self, expr: &mut Expr) -> Type {
        // Resolve object type and class name up front, then drop the borrow of `expr`
        // before reborrowing it for the container / overload handling.
        let (loc, class_name, method, obj_type) = if let Expr::MemberCall { object, method, loc, .. } = expr {
            let obj_type = self.resolve_expr_type(object);
            // Resolve class type from object (value, pointer, or reference)
            let class_name = if let Type::Class { name, .. } = &obj_type {
                name.clone()
            } else if let Type::Pointer { pointee, .. } = &obj_type {
                if let Type::Class { name, .. } = pointee.as_ref() {
                    name.clone()
                } else {
                    self.report_error("成员调用只能用于类类型", loc, ErrorCode::E3041_MemberNonStruct);
                    if let Expr::MemberCall { ty, .. } = expr {
                        *ty = Type::int();
                        return ty.clone();
                    }
                    unreachable!()
                }
            } else if let Type::Reference { base, .. } | Type::RValueRef { base } = &obj_type {
                if let Type::Class { name, .. } = base.as_ref() {
                    name.clone()
                } else {
                    self.report_error("成员调用只能用于类类型", loc, ErrorCode::E3041_MemberNonStruct);
                    if let Expr::MemberCall { ty, .. } = expr {
                        *ty = Type::int();
                        return ty.clone();
                    }
                    unreachable!()
                }
            } else {
                self.report_error("成员调用只能用于类类型", loc, ErrorCode::E3041_MemberNonStruct);
                if let Expr::MemberCall { ty, .. } = expr {
                    *ty = Type::int();
                    return ty.clone();
                }
                unreachable!()
            };
            (*loc, class_name, method.clone(), obj_type)
        } else {
            unreachable!()
        };

        // Builtin container member calls are lowered to host function calls.
        let container_result = if let Expr::MemberCall { object, args, .. } = expr {
            self.try_resolve_container_member_call(&class_name, &method, object, args, &loc)
        } else {
            unreachable!()
        };
        if let Some((host_func, addr_expr, call_args, result_ty)) = container_result {
            if let Expr::MemberCall { is_virtual, .. } = expr {
                *is_virtual = false;
            }
            let mut full_args = vec![addr_expr];
            full_args.extend(call_args);
            let ret = result_ty.clone();
            *expr = Expr::Call {
                name: host_func,
                args: full_args,
                loc,
                ty: result_ty,
            };
            return ret;
        }

        // Resolve user argument types first for overload resolution.
        let overload_result = if let Expr::MemberCall { args, .. } = expr {
            let arg_types: Vec<Type> = args.iter_mut().map(|a| self.resolve_expr_type(a)).collect();
            self.resolve_method_overload(&class_name, &method, &arg_types)
        } else {
            unreachable!()
        };
        match overload_result {
            None => {
                self.report_error(
                    &format!("类 '{}' 没有与参数匹配的方法 '{}'", class_name, method),
                    &loc,
                    ErrorCode::E3042_UnknownMember,
                );
                if let Expr::MemberCall { ty, .. } = expr {
                    *ty = Type::int();
                    ty.clone()
                } else {
                    unreachable!()
                }
            }
            Some((sig, mangled)) => {
                // Fill default arguments for trailing missing parameters.
                if let Expr::MemberCall { args, .. } = expr {
                    let filled = self.try_fill_default_args(args, &sig.param_defaults, &loc);
                    if !filled || args.len() != sig.param_types.len() {
                        self.report_error(
                            &format!(
                                "方法 '{}' 参数数量不匹配：期望 {}，实际 {}",
                                method,
                                sig.param_types.len(),
                                args.len()
                            ),
                            &loc,
                            ErrorCode::E3037_FuncArgCount,
                        );
                    }
                }

                // Check access control (simplified: allow public, block private from outside)
                if matches!(sig.access, AccessSpec::Private) && self.current_class.as_ref() != Some(&class_name) {
                    self.report_error(
                        &format!("无法访问类 '{}' 的私有成员 '{}'", class_name, method),
                        &loc,
                        ErrorCode::E4024_PrivateMemberAccess,
                    );
                }

                // Check const-correctness: const object cannot call non-const method
                if !sig.is_const && !sig.is_static {
                    let obj_is_const = match &obj_type {
                        Type::Class { is_const, .. } => *is_const,
                        Type::Pointer { pointee, is_const } => {
                            if let Type::Class { is_const: ic, .. } = pointee.as_ref() {
                                *ic || *is_const
                            } else {
                                *is_const
                            }
                        }
                        Type::Reference { base, is_const } => {
                            if let Type::Class { is_const: ic, .. } = base.as_ref() {
                                *ic || *is_const
                            } else {
                                *is_const
                            }
                        }
                        _ => false,
                    };
                    if obj_is_const {
                        self.report_error(
                            &format!("不能在 const 对象上调用非 const 方法 '{}'", method),
                            &loc,
                            ErrorCode::E3065_ConstViolation,
                        );
                    }
                }

                // Apply implicit conversions / reference address-of
                if let Expr::MemberCall {
                    args,
                    ty,
                    is_virtual,
                    resolved_mangled,
                    ..
                } = expr
                {
                    for (arg, expected) in args.iter_mut().zip(sig.param_types.iter()) {
                        let arg_type = arg.ty().clone();
                        if !self.check_assignable(expected, &arg_type, &loc) {
                            self.report_error(
                                &format!("方法 '{}' 参数类型不匹配", method),
                                &loc,
                                ErrorCode::E3038_FuncArgType,
                            );
                        } else if expected.is_reference() && !arg_type.is_reference() && !arg_type.is_rvalue_ref() {
                            let is_const_ref = expected.is_const_reference()
                                || expected.reference_base().map(|b| b.is_const()).unwrap_or(false);
                            if !is_const_ref && !self.is_lvalue(arg) {
                                self.report_error(
                                    &format!("方法 '{}' 参数：非 const 引用不能绑定到右值", method),
                                    &loc,
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
                    *ty = sig.ret.clone();
                    *is_virtual = sig.is_virtual;
                    *resolved_mangled = Some(mangled);
                    ty.clone()
                } else {
                    unreachable!()
                }
            }
        }
    }
}
