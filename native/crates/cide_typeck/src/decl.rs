use super::*;

impl TypeChecker {
    // =========================================================================
    // Function / Statement visitors
    // =========================================================================

    /// 处理 C++ 类类型变量的构造函数初始化。
    /// 若 `init` 是 `__ctor__*` 调用，则提前声明变量并在参数列表前插入 `this` 指针。
    /// 返回是否按构造函数初始化处理。
    fn try_process_ctor_init(
        &mut self,
        var_name: &str,
        var_type: &Type,
        init: &mut Option<Expr>,
        loc: &SourceLoc,
        is_static: bool,
    ) -> bool {
        let is_ctor_init = init
            .as_ref()
            .map(|e| matches!(e, Expr::Call { name: n, .. } if n.starts_with("__ctor__") && var_type.is_class()))
            .unwrap_or(false);

        if !is_ctor_init {
            // 拷贝初始化语法：`Class b = a;` / `Class b = make();` 等，
            // 若类定义了拷贝/移动构造函数，则重写为对应构造函数调用。
            if var_type.is_class() {
                if let Some(ref mut init_expr) = init {
                    let init_ty = self.resolve_expr_type(init_expr);
                    if let (Type::Class { name: class_name, .. }, Type::Class { name: src_name, .. }) =
                        (var_type, &init_ty)
                    {
                        if class_name.as_str() == src_name.as_str() {
                            let ctor_loc = *init_expr.loc();
                            let is_rvalue = !self.is_lvalue(init_expr) || init_ty.is_rvalue_ref();
                            if is_rvalue {
                                let move_ctor_name = format!("__ctor__{}__move", class_name);
                                let has_move = self
                                    .classes
                                    .get(class_name)
                                    .map(|s| s.methods.contains_key(&move_ctor_name))
                                    .unwrap_or(false);
                                if has_move {
                                    // D14：init 为 Some 由外层 if let 守卫，take 必成功；
                                    // let-else 化以消除 unwrap（静态可证，非行为变更）
                                    let Some(source) = init.take() else {
                                        return false;
                                    };
                                    *init = Some(Expr::Call {
                                        name: move_ctor_name,
                                        args: vec![Expr::Move {
                                            expr: Box::new(source),
                                            loc: ctor_loc,
                                            ty: Type::default(),
                                        }],
                                        loc: ctor_loc,
                                        ty: Type::void(),
                                    });
                                    return self.try_process_ctor_init(var_name, var_type, init, loc, is_static);
                                }
                            }
                            let copy_ctor_name = format!("__ctor__{}__copy", class_name);
                            let has_copy = self
                                .classes
                                .get(class_name)
                                .map(|s| s.methods.contains_key(&copy_ctor_name))
                                .unwrap_or(false);
                            if has_copy {
                                let Some(source) = init.take() else {
                                    return false;
                                };
                                *init = Some(Expr::Call {
                                    name: copy_ctor_name,
                                    args: vec![source],
                                    loc: ctor_loc,
                                    ty: Type::void(),
                                });
                                return self.try_process_ctor_init(var_name, var_type, init, loc, is_static);
                            }
                        }
                    }
                }
            }
            return false;
        }

        self.declare_var(var_name, var_type, false, false, is_static);

        #[allow(clippy::collapsible_match)]
        if let Some(ref mut init_expr) = init {
            if let Expr::Call { name, args, loc: ctor_loc, .. } = init_expr {
                if let Type::Class { name: class_name, .. } = var_type {
                    // 先解析用户实参类型，用于拷贝构造/重载决议。
                    let arg_types: Vec<Type> = args.iter_mut().map(|a| self.resolve_expr_type(a)).collect();
                    // 如果初始化表达式是右值引用（如 std::move），优先选择移动构造函数
                    let is_rvalue_init = args.len() == 1
                        && (matches!(&args[0], Expr::Call { name, .. } if name == "std__move")
                            || matches!(&args[0], Expr::CallPtr { callee, .. } if matches!(callee.as_ref(), Expr::Identifier { name, .. } if name == "std__move"))
                            || args[0].ty().is_rvalue_ref());
                    let ctor_name = if is_rvalue_init {
                        // 优先使用移动构造函数；即使它尚未在 funcs 中注册
                        // （隐式移动构造在 Pass 3.55 生成），也要让后续阶段能找到它。
                        Some(format!("__ctor__{}__move", class_name))
                    } else {
                        self.resolve_constructor_overload(class_name, &arg_types, *ctor_loc)
                    };
                    if let Some(ctor_name) = ctor_name {
                        // Fill trailing default arguments for constructors.
                        if !is_rvalue_init {
                            if let Some(sigs) = self.find_class_method_sigs(class_name, &ctor_name) {
                                if let Some(sig) = sigs.first() {
                                    self.try_fill_default_args(args, &sig.param_defaults, ctor_loc);
                                }
                            }
                        }
                        *name = ctor_name;
                        let this_expr = Expr::Unary {
                            op: cide_ast::UnaryOp::Addr,
                            operand: Box::new(Expr::Identifier {
                                name: var_name.to_string(),
                                loc: *ctor_loc,
                                ty: var_type.clone(),
                            }),
                            loc: *ctor_loc,
                            ty: Type::pointer_to(var_type.clone()),
                        };
                        args.insert(0, this_expr);
                    } else {
                        self.report_error(
                            &format!("类 '{}' 没有接受 {} 个参数的构造函数", class_name, args.len()),
                            loc,
                            ErrorCode::E3003_FuncRedeclared,
                        );
                    }
                }
            }
        }
        true
    }

    pub(super) fn visit_func_decl(&mut self, node: &mut FuncDecl) {
        self.current_file = node.source_file.clone();
        self.current_func_return = node.return_type.clone();
        self.current_func_params.clear();
        self.func_labels.clear();
        self.pending_gotos.clear();
        self.enter_scope();
        for p in &node.params {
            self.current_func_params.insert(p.name.clone());
            self.declare_var(&p.name, &p.ty, false, false, false);
        }
        if let Some(ref mut body) = node.body {
            self.dispatch_stmt(body);
        }
        let unresolved: Vec<(String, SourceLoc)> = self
            .pending_gotos
            .iter()
            .filter(|(label, _)| !self.func_labels.contains_key(label))
            .map(|(label, loc)| (label.clone(), *loc))
            .collect();
        for (label, loc) in unresolved {
            self.report_error(
                &format!("goto 目标标签 '{}' 未定义", label),
                &loc,
                ErrorCode::E3071_UndefinedLabel,
            );
        }
        self.pending_gotos.clear();
        self.func_labels.clear();
        self.exit_scope();
        self.current_func_params.clear();
    }

    fn dispatch_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Block { stmts, .. } => {
                self.enter_scope();
                for s in stmts {
                    self.dispatch_stmt(s);
                }
                self.exit_scope();
            }
            Stmt::VarDecl {
                var_type,
                name,
                init,
                extra_vars,
                loc,
                is_static,
                ..
            } => {
                // Auto type deduction for C++
                if Self::type_has_auto(var_type) {
                    #[allow(clippy::collapsible_match)]
                    if let Some(ref mut init_expr) = init {
                        let deduced = self.deduce_auto_type(init_expr);
                        *var_type = Self::replace_auto_in_type(var_type, deduced);
                    } else {
                        self.report_error(
                            "auto 类型变量必须有初始化表达式",
                            loc,
                            ErrorCode::E4025_AutoRequiresInitializer,
                        );
                        *var_type = Self::replace_auto_in_type(var_type, Type::int());
                    }
                }
                // Class template instantiation
                *var_type = self.resolve_template_id(var_type, loc);
                // typeof type deduction
                if Self::type_has_typeof(var_type) {
                    if let Some(ref mut init_expr) = init {
                        let deduced = self.resolve_expr_type(init_expr);
                        *var_type = Self::resolve_typeof_in_type(var_type, deduced);
                    } else if let Type::Typeof { .. } = var_type {
                        // 需要临时取出表达式来推断类型
                        if let Type::Typeof { expr, .. } = var_type.clone() {
                            let mut expr_mut = *expr;
                            let deduced = self.resolve_expr_type(&mut expr_mut);
                            *var_type = Self::resolve_typeof_in_type(var_type, deduced);
                        }
                    } else {
                        *var_type = Self::resolve_typeof_in_type(var_type, Type::int());
                    }
                }
                // 只有 C++ 构造函数初始化语法需要提前声明变量，以便 this 指针（&var_name）能正确解析。
                // 普通变量（尤其是数组）必须在初始化表达式处理完成后再声明，否则符号表中的类型
                // 无法反映 check_array_initializer 推断出的数组大小。
                let is_ctor_init = self.try_process_ctor_init(name, var_type, init, loc, *is_static);
                // U1#12：数组尺寸合法性（负/零/缺尺寸且无初始化器）——
                // has_init_list = true 时允许 `int t[]={...}` 推断尺寸
                if var_type.is_array() {
                    self.check_array_dims_legality(
                        var_type,
                        // StringLiteral（char s[]="..."）同样可推断尺寸，不得误拒
                        matches!(init, Some(Expr::InitList { .. }) | Some(Expr::StringLiteral { .. })),
                        loc,
                    );
                }
                if let Some(ref mut init_expr) = init {
                    if var_type.is_array() {
                        self.check_array_initializer(var_type, init_expr, loc);
                    } else if var_type.is_struct() && matches!(init_expr, Expr::InitList { .. }) {
                        self.check_struct_initializer(var_type, init_expr, loc);
                    } else if let Expr::Call { name, .. } = init_expr {
                        if name.starts_with("__ctor__") && var_type.is_class() {
                            // 移动构造函数在 Pass 3.55 才生成，此时 funcs 中可能还没有，
                            // 因此跳过完整的函数调用检查；但仍需解析参数表达式以获得正确类型，
                            // 这样 code generation 阶段才能识别 RValueRef 并按地址传递。
                            if name.ends_with("__move") {
                                if let Expr::Call { args, .. } = init_expr {
                                    for arg in args.iter_mut() {
                                        self.resolve_expr_type(arg);
                                    }
                                }
                            } else {
                                self.resolve_expr_type(init_expr);
                            }
                        } else {
                            let init_type = self.resolve_expr_type(init_expr);
                            // W0-4：char 窄化豁免（同主路径）
                            let suppress = var_type.kind() == TypeKind::Char
                                && Self::is_char_safe_initializer(init_expr);
                            if suppress {
                                self.char_narrow_suppress = true;
                            }
                            let assignable_ctor = self.check_assignable(var_type, &init_type, loc);
                            if suppress {
                                self.char_narrow_suppress = false;
                            }
                            if !assignable_ctor {
                                self.report_error(
                                    &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, var_type),
                                    loc,
                                    ErrorCode::E3004_TypeMismatch,
                                );
                            } else {
                                // C++ reference binding lvalue check
                                if let Type::Reference { base, is_const } = var_type {
                                    let is_const_ref = *is_const || base.is_const();
                                    if !is_const_ref && !self.is_lvalue(init_expr) {
                                        self.report_error(
                                            "非 const 引用必须绑定到左值",
                                            loc,
                                            ErrorCode::E4029_ReferenceBindLvalueRequired,
                                        );
                                    }
                                }
                                insert_implicit_cast(init_expr, var_type);
                            }
                        }
                    } else {
                        let init_type = self.resolve_expr_type(init_expr);
                        // W0-4：char 变量的字符常量/值域内整常量初始化是 C 惯用写法，豁免窄化警告
                        let suppress = var_type.kind() == TypeKind::Char
                            && Self::is_char_safe_initializer(init_expr);
                        if suppress {
                            self.char_narrow_suppress = true;
                        }
                        let assignable_var = self.check_assignable(var_type, &init_type, loc);
                        if suppress {
                            self.char_narrow_suppress = false;
                        }
                        if !assignable_var {
                            self.report_error(
                                &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, var_type),
                                loc,
                                ErrorCode::E3004_TypeMismatch,
                            );
                        } else {
                            // C++ reference binding lvalue check
                            if let Type::Reference { base, is_const } = var_type {
                                let is_const_ref = *is_const || base.is_const();
                                if !is_const_ref && !self.is_lvalue(init_expr) {
                                    self.report_error(
                                        "非 const 引用必须绑定到左值",
                                        loc,
                                        ErrorCode::E4029_ReferenceBindLvalueRequired,
                                    );
                                }
                            }
                            insert_implicit_cast(init_expr, var_type);
                        }
                    }
                }
                if !is_ctor_init {
                    self.declare_var(name, var_type, false, false, *is_static);
                }
                for (ety, ename, einit) in extra_vars.iter_mut() {
                    *ety = self.resolve_template_id(ety, loc);
                    // B40: extra_vars 中的类类型变量同样需要构造函数 this 指针插入。
                    let is_extra_ctor = self.try_process_ctor_init(ename, ety, einit, loc, false);
                    if let Some(ref mut init_expr) = einit {
                        if ety.is_array() {
                            self.check_array_initializer(ety, init_expr, loc);
                        } else if ety.is_struct() && matches!(init_expr, Expr::InitList { .. }) {
                            self.check_struct_initializer(ety, init_expr, loc);
                        } else if let Expr::Call { name, .. } = init_expr {
                            if name.starts_with("__ctor__") && ety.is_class() {
                                // 移动构造函数在 Pass 3.55 才生成，此时 funcs 中可能还没有，
                                // 因此跳过完整的函数调用检查；但仍需解析参数表达式以获得正确类型，
                                // 这样 code generation 阶段才能识别 RValueRef 并按地址传递。
                                if name.ends_with("__move") {
                                    if let Expr::Call { args, .. } = init_expr {
                                        for arg in args.iter_mut() {
                                            self.resolve_expr_type(arg);
                                        }
                                    }
                                } else {
                                    self.resolve_expr_type(init_expr);
                                }
                            } else {
                                let init_type = self.resolve_expr_type(init_expr);
                                // W0-4：char 窄化豁免（同主路径）
                                let suppress = ety.kind() == TypeKind::Char
                                    && Self::is_char_safe_initializer(init_expr);
                                if suppress {
                                    self.char_narrow_suppress = true;
                                }
                                let assignable_ctor = self.check_assignable(ety, &init_type, loc);
                                if suppress {
                                    self.char_narrow_suppress = false;
                                }
                                if !assignable_ctor {
                                    self.report_error(
                                        &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, ety),
                                        loc,
                                        ErrorCode::E3004_TypeMismatch,
                                    );
                                } else {
                                    // C++ reference binding lvalue check
                                    if let Type::Reference { is_const, .. } = ety {
                                        if !*is_const && !self.is_lvalue(init_expr) {
                                            self.report_error(
                                                "非 const 引用必须绑定到左值",
                                                loc,
                                                ErrorCode::E4029_ReferenceBindLvalueRequired,
                                            );
                                        }
                                    }
                                    insert_implicit_cast(init_expr, ety);
                                }
                            }
                        } else {
                            let init_type = self.resolve_expr_type(init_expr);
                            // W0-4：同主声明路径的 char 窄化豁免
                            let suppress = ety.kind() == TypeKind::Char
                                && Self::is_char_safe_initializer(init_expr);
                            if suppress {
                                self.char_narrow_suppress = true;
                            }
                            let assignable_ety = self.check_assignable(ety, &init_type, loc);
                            if suppress {
                                self.char_narrow_suppress = false;
                            }
                            if !assignable_ety {
                                self.report_error(
                                    &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, ety),
                                    loc,
                                    ErrorCode::E3004_TypeMismatch,
                                );
                            } else {
                                // C++ reference binding lvalue check
                                if let Type::Reference { is_const, .. } = ety {
                                    if !*is_const && !self.is_lvalue(init_expr) {
                                        self.report_error(
                                            "非 const 引用必须绑定到左值",
                                            loc,
                                            ErrorCode::E4029_ReferenceBindLvalueRequired,
                                        );
                                    }
                                }
                                insert_implicit_cast(init_expr, ety);
                            }
                        }
                    }
                    if !is_extra_ctor {
                        self.declare_var(ename, ety, false, false, false);
                    }
                }
            }
            Stmt::Expr { expr, .. } => {
                self.resolve_expr_type(expr);
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                loc,
            } => {
                self.check_condition(cond, "if 条件", loc);
                self.dispatch_stmt(then_stmt);
                if let Some(ref mut e) = else_stmt {
                    self.dispatch_stmt(e);
                }
            }
            Stmt::While { cond, body, loc } => {
                self.check_condition(cond, "while 条件", loc);
                self.loop_depth += 1;
                self.dispatch_stmt(body);
                self.loop_depth -= 1;
            }
            Stmt::DoWhile { body, cond, loc } => {
                self.loop_depth += 1;
                self.dispatch_stmt(body);
                self.loop_depth -= 1;
                self.check_condition(cond, "do...while 条件", loc);
            }
            Stmt::For { init, cond, step, body, loc } => {
                self.enter_scope();
                if let Some(ref mut i) = init {
                    self.dispatch_stmt(i);
                }
                if let Some(ref mut c) = cond {
                    self.check_condition(c, "for 条件", loc);
                    if let Expr::Binary {
                        op: BinaryOp::Le, left, right, ..
                    } = c
                    {
                        if self.expr_involves_array_or_pointer(left) || self.expr_involves_array_or_pointer(right) {
                            self.report_warning("循环条件中使用了 '<='，如果用于数组索引，可能导致越界（off-by-one 错误）。你是否想使用 '<'？", loc, ErrorCode::W3051_ArrayBoundOffByOne);
                        }
                    }
                }
                for s in step {
                    self.resolve_expr_type(s);
                }
                self.loop_depth += 1;
                self.dispatch_stmt(body);
                self.loop_depth -= 1;
                self.exit_scope();
            }
            Stmt::Return { value, loc } => {
                if self.current_func_return.is_void() {
                    if value.is_some() {
                        self.report_error("void 函数不能有返回值", loc, ErrorCode::E3012_VoidFuncReturnValue);
                    }
                } else {
                    if let Some(ref mut v) = value {
                        let val_type = self.resolve_expr_type(v);
                        let expected = self.current_func_return.clone();
                        if !self.check_assignable(&expected, &val_type, loc) {
                            self.report_error(
                                &format!("返回类型不匹配：期望 '{}'，实际 '{}'", self.current_func_return, val_type),
                                loc,
                                ErrorCode::E3014_ReturnTypeMismatch,
                            );
                        } else {
                            insert_implicit_cast(v, &expected);
                        }
                    } else {
                        self.report_error("非 void 函数必须返回一个值", loc, ErrorCode::E3013_MissingReturnValue);
                    }
                }
            }
            Stmt::Break { loc } if self.loop_depth <= 0 && self.switch_depth <= 0 => {
                self.report_error("break 只能在循环或 switch 体内使用", loc, ErrorCode::E3010_BreakOutsideLoop);
            }
            Stmt::Continue { loc } if self.loop_depth <= 0 => {
                self.report_error("continue 只能在循环体内使用", loc, ErrorCode::E3011_ContinueOutsideLoop);
            }
            Stmt::Switch { cond, body, loc } => {
                self.switch_depth += 1;
                let cond_type = self.resolve_expr_type(cond);
                if !self.is_int(&cond_type) {
                    self.report_error("switch 条件必须是整数类型", loc, ErrorCode::E3046_SwitchCondType);
                }
                self.dispatch_stmt(body);
                self.switch_depth -= 1;
            }
            Stmt::Case { label, stmt, loc } => {
                if let Some(ref mut l) = label {
                    let label_type = self.resolve_expr_type(l);
                    if !self.is_int(&label_type) {
                        self.report_error("case 标签必须是整数常量", loc, ErrorCode::E3047_CaseNotConstant);
                    }
                }
                self.dispatch_stmt(stmt);
            }
            Stmt::Goto { label, loc } => {
                if self.func_labels.contains_key(label) {
                    // Label already defined, ok
                } else {
                    self.pending_gotos.push((label.clone(), *loc));
                }
            }
            Stmt::Label { label, stmt, loc } => {
                if self.func_labels.contains_key(label) {
                    self.report_error(&format!("标签 '{}' 重复定义", label), loc, ErrorCode::E3001_VarRedeclared);
                } else {
                    self.func_labels.insert(label.clone(), *loc);
                }
                self.dispatch_stmt(stmt);
            }
            Stmt::RangeFor { var, var_type, iter, body, loc } => {
                self.enter_scope();
                let iter_type = self.resolve_expr_type(iter);
                // Deduce element type from iterable
                let elem_type = if iter_type.is_array() {
                    iter_type.innermost_element_type()
                } else if let Type::Pointer { pointee, .. } = &iter_type {
                    pointee.as_ref().clone()
                } else if iter_type.is_class() {
                    match iter_type.name() {
                        "cide_vec_int" | "cide_list_int" => Type::int(),
                        "cide_vec_float" => Type::float(),
                        "cide_vec_char" | "cide_string" => Type::char(),
                        _ => {
                            self.report_error("范围 for 不支持该容器类型", loc, ErrorCode::E4020_RangeForNotSupported);
                            Type::int()
                        }
                    }
                } else {
                    self.report_error(
                        "范围 for 的迭代对象必须是数组、指针或内置容器类型",
                        loc,
                        ErrorCode::E4020_RangeForNotSupported,
                    );
                    Type::int()
                };
                let deduced_var_type = match var_type {
                    Type::Auto => elem_type,
                    Type::Reference { is_const, .. } => Type::Reference {
                        base: Box::new(elem_type),
                        is_const: *is_const,
                    },
                    Type::RValueRef { .. } => Type::RValueRef { base: Box::new(elem_type) },
                    _ => var_type.clone(),
                };
                *var_type = deduced_var_type.clone();
                self.declare_var(var, &deduced_var_type, false, false, false);
                self.loop_depth += 1;
                self.dispatch_stmt(body);
                self.loop_depth -= 1;
                self.exit_scope();
            }
            Stmt::Try { loc, .. } => {
                self.report_error("try/catch 异常处理不被支持", loc, ErrorCode::E4001_ExceptionNotSupported);
            }
            _ => {}
        }
    }

    fn check_condition(&mut self, cond: &mut Expr, ctx: &str, loc: &SourceLoc) {
        let ty = self.resolve_expr_type(cond);
        if !self.is_scalar(&ty) && !matches!(ty.kind(), TypeKind::Pointer | TypeKind::Array) {
            self.report_error(
                &format!("{} 必须是整数、浮点数或指针类型", ctx),
                loc,
                ErrorCode::E3015_InvalidCondition,
            );
        }
        let is_assign_expr = |e: &Expr| matches!(e, Expr::Assign { op: AssignOp::Assign, .. });
        if is_assign_expr(cond) {
            self.report_warning(
                "条件中使用了赋值运算符 '='，你是否想使用比较运算符 '=='？",
                loc,
                ErrorCode::W3050_AssignInCondition,
            );
        } else if let Expr::Binary { left, right, .. } = cond {
            if is_assign_expr(left) || is_assign_expr(right) {
                self.report_warning(
                    "条件中包含了赋值表达式，你是否想使用比较运算符 '=='？",
                    loc,
                    ErrorCode::W3050_AssignInCondition,
                );
            }
        }
    }

    /// Visit a function declaration, pre-registering class fields as variables in the outermost scope.
    /// Used for checking class method bodies where fields are implicitly accessible.
    pub(super) fn visit_func_decl_with_fields(&mut self, node: &mut FuncDecl, _fields: &[(Type, String)]) {
        self.current_file = node.source_file.clone();
        self.current_func_return = node.return_type.clone();
        self.current_func_params.clear();
        self.func_labels.clear();
        self.pending_gotos.clear();
        self.enter_scope();
        // Class fields are resolved via implicit this->field in resolve_expr_type
        for p in &node.params {
            self.current_func_params.insert(p.name.clone());
            self.declare_var(&p.name, &p.ty, false, false, false);
        }
        if let Some(ref mut body) = node.body {
            self.dispatch_stmt(body);
        }
        let unresolved: Vec<(String, SourceLoc)> = self
            .pending_gotos
            .iter()
            .filter(|(label, _)| !self.func_labels.contains_key(label))
            .map(|(label, loc)| (label.clone(), *loc))
            .collect();
        for (label, loc) in unresolved {
            self.report_error(
                &format!("goto 目标标签 '{}' 未定义", label),
                &loc,
                ErrorCode::E3071_UndefinedLabel,
            );
        }
        self.pending_gotos.clear();
        self.func_labels.clear();
        self.exit_scope();
        self.current_func_params.clear();
    }

    /// 求值编译期常量表达式（用于非类型模板参数）。
    /// 支持字面量、sizeof、模板参数值、简单四则运算和一元负号。
    pub(crate) fn evaluate_constexpr(
        &self,
        expr: &Expr,
        value_map: &std::collections::HashMap<String, i32>,
    ) -> Option<i32> {
        match expr {
            Expr::Literal { value, .. } => Some(*value),
            Expr::Identifier { name, .. } => value_map.get(name).copied(),
            Expr::Unary { op: UnaryOp::Neg, operand, .. } => self.evaluate_constexpr(operand, value_map).map(|v| -v),
            Expr::Binary { op, left, right, .. } => {
                let l = self.evaluate_constexpr(left, value_map)?;
                let r = self.evaluate_constexpr(right, value_map)?;
                match op {
                    BinaryOp::Add => Some(l + r),
                    BinaryOp::Sub => Some(l - r),
                    BinaryOp::Mul => Some(l * r),
                    BinaryOp::Div => {
                        if r == 0 {
                            None
                        } else {
                            Some(l / r)
                        }
                    }
                    BinaryOp::Mod => {
                        if r == 0 {
                            None
                        } else {
                            Some(l % r)
                        }
                    }
                    _ => None,
                }
            }
            Expr::Sizeof { target_type, .. } => target_type.as_ref().map(|ty| self.compute_type_size(ty)),
            _ => None,
        }
    }

    /// 若 args 长度不足且缺失的尾部参数均有默认值，则把默认参数表达式追加到 args 中。
    /// 返回 true 表示成功填充，调用方不再报参数数量错误。
    pub(crate) fn try_fill_default_args(
        &self,
        args: &mut Vec<Expr>,
        param_defaults: &[Option<Expr>],
        _loc: &SourceLoc,
    ) -> bool {
        if args.len() >= param_defaults.len() {
            return true;
        }
        // 只允许尾部缺失，中间缺失视为不匹配。
        for default in param_defaults.iter().skip(args.len()) {
            if default.is_none() {
                return false;
            }
        }
        for default in param_defaults.iter().skip(args.len()) {
            // D14：尾部默认参数必有值（声明侧强制），None 属声明与调用口径错位——
            // 跳过而非 panic
            let Some(d) = default else { continue };
            args.push(d.clone());
        }
        true
    }

    /// 检查并调整函数调用参数：类型检查、隐式转换、引用参数隐式取地址。
    fn check_and_adjust_call_args(&mut self, name: &str, param_types: &[Type], args: &mut [Expr], loc: &SourceLoc) {
        for (i, (arg, expected)) in args.iter_mut().zip(param_types.iter()).enumerate() {
            let arg_type = self.resolve_expr_type(arg);
            if !self.check_assignable(expected, &arg_type, loc) {
                self.report_error(
                    &format!("函数 '{}' 第 {} 个参数类型不匹配", name, i + 1),
                    loc,
                    ErrorCode::E3038_FuncArgType,
                );
            } else if expected.is_reference() && !arg_type.is_reference() && !arg_type.is_rvalue_ref() {
                let is_const_ref =
                    expected.is_const_reference() || expected.reference_base().map(|b| b.is_const()).unwrap_or(false);
                if !is_const_ref && !self.is_lvalue(arg) {
                    self.report_error(
                        &format!("函数 '{}' 第 {} 个参数：非 const 引用不能绑定到右值", name, i + 1),
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
    }

    pub(crate) fn check_user_func(&mut self, name: &str, args: &mut Vec<Expr>, loc: &SourceLoc) -> Type {
        let sym = self.funcs.get(name).cloned();
        if let Some(sym) = sym {
            let filled = self.try_fill_default_args(args, &sym.param_defaults, loc);
            if !filled
                || args.len() < sym.param_types.len()
                || (args.len() != sym.param_types.len() && !sym.is_variadic)
            {
                self.report_error(
                    &format!(
                        "函数 '{}' 参数数量不匹配：期望 {}，实际 {}",
                        name,
                        sym.param_types.len(),
                        args.len()
                    ),
                    loc,
                    ErrorCode::E3037_FuncArgCount,
                );
            } else {
                self.check_and_adjust_call_args(name, &sym.param_types, args, loc);
                if sym.is_variadic {
                    for arg in args.iter_mut().skip(sym.param_types.len()) {
                        self.resolve_expr_type(arg);
                        apply_default_argument_promotions(arg);
                    }
                }
            }
            return sym.return_type.clone();
        }

        if let Some(sym) = self.static_func_sigs.get(name).cloned() {
            if let Some(files) = self.static_func_files.get(name) {
                if !files.contains(&self.current_file) {
                    self.report_error(
                        &format!("static 函数 '{}' 在其他文件中不可见", name),
                        loc,
                        ErrorCode::E3058_StaticFuncAccess,
                    );
                    return Type::void();
                }
            }
            let filled = self.try_fill_default_args(args, &sym.param_defaults, loc);
            if !filled
                || args.len() < sym.param_types.len()
                || (args.len() != sym.param_types.len() && !sym.is_variadic)
            {
                self.report_error(
                    &format!(
                        "函数 '{}' 参数数量不匹配：期望 {}，实际 {}",
                        name,
                        sym.param_types.len(),
                        args.len()
                    ),
                    loc,
                    ErrorCode::E3037_FuncArgCount,
                );
            } else {
                self.check_and_adjust_call_args(name, &sym.param_types, args, loc);
                if sym.is_variadic {
                    for arg in args.iter_mut().skip(sym.param_types.len()) {
                        self.resolve_expr_type(arg);
                        apply_default_argument_promotions(arg);
                    }
                }
            }
            return sym.return_type.clone();
        }

        // Try template implicit instantiation
        if !args.is_empty() {
            let arg_types: Vec<Type> = args.iter_mut().map(|a| self.resolve_expr_type(a)).collect();
            if let Some((mangled, maybe_new_func)) = self.try_instantiate_template(name, &arg_types) {
                if let Some(new_func) = maybe_new_func {
                    self.pending_instantiations.push((mangled.clone(), new_func));
                }
                return self.check_user_func(&mangled, args, loc);
            }
        }

        // Fallback: Bytecode Libc functions are pre-registered at codegen time
        if let Some((ret_ty, param_types)) = cide_runtime::bytecode_libc_sig::bytecode_libc_sig(name) {
            if args.len() != param_types.len() {
                self.report_error(
                    &format!(
                        "函数 '{}' 参数数量不匹配：期望 {}，实际 {}",
                        name,
                        param_types.len(),
                        args.len()
                    ),
                    loc,
                    ErrorCode::E3037_FuncArgCount,
                );
            } else {
                for (i, (arg, expected)) in args.iter_mut().zip(param_types.iter()).enumerate() {
                    let arg_type = self.resolve_expr_type(arg);
                    if !self.check_assignable(expected, &arg_type, loc) {
                        self.report_error(
                            &format!("函数 '{}' 第 {} 个参数类型不匹配", name, i + 1),
                            loc,
                            ErrorCode::E3038_FuncArgType,
                        );
                    } else {
                        insert_implicit_cast(arg, expected);
                    }
                }
            }
            return ret_ty;
        }

        self.report_error(&format!("未定义的函数 '{}'", name), loc, ErrorCode::E3036_UndefinedFunc);
        Type::void()
    }


}

