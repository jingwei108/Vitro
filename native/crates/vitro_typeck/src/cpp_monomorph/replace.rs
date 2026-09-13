use super::*;

impl TypeChecker {
    /// 若类型（含嵌套）为 TemplateId，触发类模板实例化并返回对应的 Class 类型；否则返回原类型。
    pub(crate) fn resolve_template_id(&mut self, ty: &Type, loc: &SourceLoc) -> Type {
        match ty.clone() {
            Type::TemplateId { base, args, .. } => {
                if let Some((mangled, new_class)) = self.try_monomorphize_class(&base, &args) {
                    self.pending_class_instantiations.push((mangled.clone(), new_class));
                    Type::Class { name: mangled, is_const: false }
                } else if !self.templates.contains_key(&base) {
                    self.report_error(&format!("未知模板类 '{}'", base), loc, ErrorCode::E3023_UndeclaredVar);
                    Type::int()
                } else {
                    let mangled = Self::mangle_template_name(&base, &args);
                    Type::Class { name: mangled, is_const: false }
                }
            }
            Type::Pointer { pointee, is_const } => {
                let new_pointee = self.resolve_template_id(&pointee, loc);
                Type::Pointer {
                    pointee: Box::new(new_pointee),
                    is_const,
                }
            }
            Type::Array {
                element,
                array_size,
                dims,
                is_const,
                is_vla,
                vla_dims,
            } => {
                let new_element = self.resolve_template_id(&element, loc);
                Type::Array {
                    element: Box::new(new_element),
                    array_size,
                    dims,
                    is_const,
                    is_vla,
                    vla_dims,
                }
            }
            Type::Reference { base: inner, is_const } => {
                let new_inner = self.resolve_template_id(&inner, loc);
                Type::Reference {
                    base: Box::new(new_inner),
                    is_const,
                }
            }
            Type::RValueRef { base: inner } => {
                let new_inner = self.resolve_template_id(&inner, loc);
                Type::RValueRef { base: Box::new(new_inner) }
            }
            _ => ty.clone(),
        }
    }

    pub(crate) fn replace_template_type(
        &self,
        ty: &Type,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) -> Type {
        match ty {
            Type::Class { name, .. } if type_map.contains_key(name) => type_map[name].clone(),
            Type::TemplateId { base, args, .. } => {
                // 类模板体内出现同模板类实例化，如 unique_ptr<T>，需要把参数替换后生成 mangled 类名。
                let new_args: Vec<TemplateArg> =
                    args.iter().map(|a| self.replace_template_arg(a, type_map, value_map)).collect();
                let mangled = Self::mangle_template_name(base, &new_args);
                Type::Class { name: mangled, is_const: false }
            }
            Type::Pointer { pointee, is_const } => Type::Pointer {
                pointee: Box::new(self.replace_template_type(pointee, type_map, value_map)),
                is_const: *is_const,
            },
            Type::Array {
                element,
                array_size,
                dims,
                is_const,
                is_vla,
                vla_dims,
            } => {
                let new_element = self.replace_template_type(element, type_map, value_map);
                // Evaluate VLA dimensions that depend on non-type template parameters.
                let mut new_dims = dims.clone();
                let mut new_array_size = *array_size;
                let mut new_is_vla = *is_vla;
                let mut new_vla_dims = vla_dims.clone();
                if *is_vla && !vla_dims.is_empty() {
                    new_dims.clear();
                    new_vla_dims.clear();
                    new_is_vla = false;
                    for (i, dim_expr) in vla_dims.iter().enumerate() {
                        if let Some(v) = self.evaluate_constexpr(dim_expr, value_map) {
                            new_dims.push(v);
                            new_array_size = if i == 0 { v } else { new_array_size * v };
                        } else {
                            new_dims.push(dims.get(i).copied().unwrap_or(0));
                            new_vla_dims.push(dim_expr.clone());
                            new_is_vla = true;
                        }
                    }
                }
                Type::Array {
                    element: Box::new(new_element),
                    array_size: new_array_size,
                    dims: new_dims,
                    is_const: *is_const,
                    is_vla: new_is_vla,
                    vla_dims: new_vla_dims,
                }
            }
            Type::Reference { base, is_const } => Type::Reference {
                base: Box::new(self.replace_template_type(base, type_map, value_map)),
                is_const: *is_const,
            },
            Type::RValueRef { base } => Type::RValueRef {
                base: Box::new(self.replace_template_type(base, type_map, value_map)),
            },
            Type::Function {
                return_type,
                param_types,
                is_const,
                is_variadic,
            } => Type::Function {
                return_type: Box::new(self.replace_template_type(return_type, type_map, value_map)),
                param_types: param_types
                    .iter()
                    .map(|t| self.replace_template_type(t, type_map, value_map))
                    .collect(),
                is_const: *is_const,
                is_variadic: *is_variadic,
            },
            _ => ty.clone(),
        }
    }

    fn replace_template_arg(
        &self,
        arg: &TemplateArg,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) -> TemplateArg {
        match arg {
            TemplateArg::Type(ty) => TemplateArg::Type(self.replace_template_type(ty, type_map, value_map)),
            TemplateArg::Expr(expr) => {
                if let Some(v) = self.evaluate_constexpr(expr, value_map) {
                    TemplateArg::Int(v)
                } else {
                    TemplateArg::Expr(expr.clone())
                }
            }
            TemplateArg::Int(v) => TemplateArg::Int(*v),
        }
    }

    pub(crate) fn replace_template_types_in_stmt(
        &self,
        stmt: &mut Stmt,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) {
        match stmt {
            Stmt::VarDecl { var_type, init, extra_vars, .. } => {
                *var_type = self.replace_template_type(var_type, type_map, value_map);
                for (ety, _, einit) in extra_vars.iter_mut() {
                    *ety = self.replace_template_type(ety, type_map, value_map);
                    if let Some(ref mut e) = einit {
                        self.replace_template_types_in_expr(e, type_map, value_map);
                    }
                }
                if let Some(ref mut e) = init {
                    self.replace_template_types_in_expr(e, type_map, value_map);
                }
            }
            Stmt::Block { stmts, .. } => {
                for s in stmts {
                    self.replace_template_types_in_stmt(s, type_map, value_map);
                }
            }
            Stmt::Switch { body, .. } => {
                self.replace_template_types_in_stmt(body, type_map, value_map);
            }
            Stmt::If { cond, then_stmt, else_stmt, .. } => {
                self.replace_template_types_in_expr(cond, type_map, value_map);
                self.replace_template_types_in_stmt(then_stmt, type_map, value_map);
                if let Some(ref mut e) = else_stmt {
                    self.replace_template_types_in_stmt(e, type_map, value_map);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.replace_template_types_in_expr(cond, type_map, value_map);
                self.replace_template_types_in_stmt(body, type_map, value_map);
            }
            Stmt::DoWhile { body, cond, .. } => {
                self.replace_template_types_in_stmt(body, type_map, value_map);
                self.replace_template_types_in_expr(cond, type_map, value_map);
            }
            Stmt::For { init, cond, step, body, .. } => {
                if let Some(ref mut i) = init {
                    self.replace_template_types_in_stmt(i, type_map, value_map);
                }
                if let Some(ref mut c) = cond {
                    self.replace_template_types_in_expr(c, type_map, value_map);
                }
                for s in step {
                    self.replace_template_types_in_expr(s, type_map, value_map);
                }
                self.replace_template_types_in_stmt(body, type_map, value_map);
            }
            Stmt::Return { value: Some(ref mut v), .. } => {
                self.replace_template_types_in_expr(v, type_map, value_map);
            }
            Stmt::Expr { expr, .. } => {
                self.replace_template_types_in_expr(expr, type_map, value_map);
            }
            Stmt::RangeFor { var_type, iter, body, .. } => {
                *var_type = self.replace_template_type(var_type, type_map, value_map);
                self.replace_template_types_in_expr(iter, type_map, value_map);
                self.replace_template_types_in_stmt(body, type_map, value_map);
            }
            Stmt::Case { stmt, .. } => {
                self.replace_template_types_in_stmt(stmt, type_map, value_map);
            }
            Stmt::Label { stmt, .. } => {
                self.replace_template_types_in_stmt(stmt, type_map, value_map);
            }
            _ => {}
        }
    }

    pub(crate) fn replace_template_types_in_expr(
        &self,
        expr: &mut Expr,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) {
        match expr {
            Expr::Binary { left, right, .. } => {
                self.replace_template_types_in_expr(left, type_map, value_map);
                self.replace_template_types_in_expr(right, type_map, value_map);
            }
            Expr::Unary { operand, .. } => {
                self.replace_template_types_in_expr(operand, type_map, value_map);
            }
            Expr::Ternary {
                cond, then_branch, else_branch, ..
            } => {
                self.replace_template_types_in_expr(cond, type_map, value_map);
                self.replace_template_types_in_expr(then_branch, type_map, value_map);
                self.replace_template_types_in_expr(else_branch, type_map, value_map);
            }
            Expr::Assign { left, right, .. } => {
                self.replace_template_types_in_expr(left, type_map, value_map);
                self.replace_template_types_in_expr(right, type_map, value_map);
            }
            Expr::Call { args, .. } => {
                for a in args {
                    self.replace_template_types_in_expr(a, type_map, value_map);
                }
            }
            Expr::MemberCall { object, args, .. } => {
                self.replace_template_types_in_expr(object, type_map, value_map);
                for a in args {
                    self.replace_template_types_in_expr(a, type_map, value_map);
                }
            }
            Expr::Member { object, ty, .. } => {
                self.replace_template_types_in_expr(object, type_map, value_map);
                *ty = self.replace_template_type(ty, type_map, value_map);
            }
            Expr::Index { array, index, ty, .. } => {
                self.replace_template_types_in_expr(array, type_map, value_map);
                self.replace_template_types_in_expr(index, type_map, value_map);
                *ty = self.replace_template_type(ty, type_map, value_map);
            }
            Expr::Cast { expr, target_type, ty, .. } => {
                self.replace_template_types_in_expr(expr, type_map, value_map);
                *target_type = self.replace_template_type(target_type, type_map, value_map);
                *ty = self.replace_template_type(ty, type_map, value_map);
            }
            Expr::New { elem_type, size_expr, init, .. } => {
                *elem_type = self.replace_template_type(elem_type, type_map, value_map);
                if let Some(ref mut s) = size_expr {
                    self.replace_template_types_in_expr(s, type_map, value_map);
                }
                if let Some(ref mut i) = init {
                    self.replace_template_types_in_expr(i, type_map, value_map);
                }
            }
            Expr::Delete { expr, .. } => {
                self.replace_template_types_in_expr(expr, type_map, value_map);
            }
            Expr::Lambda { params, body, .. } => {
                for p in params.iter_mut() {
                    p.ty = self.replace_template_type(&p.ty, type_map, value_map);
                }
                self.replace_template_types_in_stmt(body, type_map, value_map);
            }
            Expr::InitList { elements, ty, .. } => {
                *ty = self.replace_template_type(ty, type_map, value_map);
                for e in elements.iter_mut() {
                    self.replace_template_types_in_expr(&mut e.value, type_map, value_map);
                }
            }
            Expr::Sizeof { ty, .. } => {
                *ty = self.replace_template_type(ty, type_map, value_map);
            }
            Expr::Offsetof { ty, .. } => {
                *ty = self.replace_template_type(ty, type_map, value_map);
            }
            Expr::Move { expr, .. } => {
                self.replace_template_types_in_expr(expr, type_map, value_map);
            }
            Expr::Identifier { name, loc, .. } if value_map.contains_key(name) => {
                let v = value_map[name];
                *expr = Expr::Literal {
                    value: v,
                    loc: *loc,
                    ty: Type::int(),
                };
            }
            _ => {}
        }
    }
}
