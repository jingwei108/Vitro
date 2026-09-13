use super::*;

impl TypeChecker {
    /// Pass 3.5: Check class method / constructor / destructor bodies.
    pub(crate) fn check_class_methods(&mut self, program: &mut ProgramNode) {
        let mut class_methods: Vec<FuncDecl> = Vec::new();
        for c in &program.classes {
            self.current_class = Some(c.name.clone());
            // Register class fields as pseudo-variables for method body checking
            let class_fields: Vec<(Type, String)> = if let Some(sym) = self.classes.get(&c.name) {
                sym.fields.iter().map(|(ty, name, _)| (ty.clone(), name.clone())).collect()
            } else {
                vec![]
            };
            for member in &c.members {
                match member {
                    ClassMember::Method {
                        name: method_name,
                        ret,
                        params,
                        body: Some(ref b),
                        is_const,
                        is_static,
                        ..
                    } => {
                        self.current_method_is_const = *is_const && !*is_static;
                        let func_params: Vec<Param> = if *is_static {
                            params.to_vec()
                        } else {
                            std::iter::once(Param {
                                name: "this".to_string(),
                                ty: Type::Pointer {
                                    pointee: Box::new(Type::Class {
                                        name: c.name.clone(),
                                        is_const: *is_const,
                                    }),
                                    is_const: *is_const,
                                },
                                loc: c.loc,
                                default: None,
                            })
                            .chain(params.iter().cloned())
                            .collect()
                        };
                        let user_param_types: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                        // D1：mangled 名单源（**不含 this** 的用户参数，与 load_class /
                        // resolve_method_overload 同口径）。
                        let has_overloads = self
                            .find_class_method_sigs(&c.name, method_name)
                            .map(|v| v.len() > 1)
                            .unwrap_or(false);
                        let mangled = Self::method_mangled_name(
                            &c.name,
                            method_name,
                            &user_param_types,
                            has_overloads,
                        );
                        let mut func_decl = FuncDecl {
                            loc: c.loc,
                            return_type: ret.clone(),
                            name: mangled,
                            params: func_params,
                            body: Some(b.clone()),
                            is_static: *is_static,
                            is_extern: false,
                            source_file: String::new(),
                            is_variadic: false,
                        };
                        self.visit_func_decl_with_fields(&mut func_decl, &class_fields);
                        self.current_method_is_const = false;
                        class_methods.push(func_decl);
                    }
                    ClassMember::Constructor { params, body: Some(ref b), .. } => {
                        let ctor_name = Self::constructor_mangled_name(&c.name, params);
                        let mut func_decl = FuncDecl {
                            loc: c.loc,
                            return_type: Type::void(),
                            name: ctor_name,
                            params: std::iter::once(Param {
                                name: "this".to_string(),
                                ty: Type::Pointer {
                                    pointee: Box::new(Type::Class {
                                        name: c.name.clone(),
                                        is_const: false,
                                    }),
                                    is_const: false,
                                },
                                loc: c.loc,
                                default: None,
                            })
                            .chain(params.iter().cloned())
                            .collect(),
                            body: Some(b.clone()),
                            is_static: false,
                            is_extern: false,
                            source_file: String::new(),
                            is_variadic: false,
                        };
                        self.visit_func_decl_with_fields(&mut func_decl, &class_fields);
                        class_methods.push(func_decl);

                        // Synthesize a zero-arg default constructor wrapper if all user
                        // parameters have default values.
                        if !params.is_empty() && params.iter().all(|p| p.default.is_some()) {
                            let default_ctor_name = format!("__ctor__{}", c.name);
                            let full_ctor_name = Self::constructor_mangled_name(&c.name, params);
                            let this_ty = Type::Pointer {
                                pointee: Box::new(Type::Class {
                                    name: c.name.clone(),
                                    is_const: false,
                                }),
                                is_const: false,
                            };
                            let this_expr = Expr::Identifier {
                                name: "this".to_string(),
                                loc: c.loc,
                                ty: this_ty.clone(),
                            };
                            let mut call_args: Vec<Expr> = vec![this_expr];
                            for p in params.iter() {
                                if let Some(ref d) = p.default {
                                    call_args.push(d.clone());
                                }
                            }
                            let wrapper_body = Stmt::Block {
                                stmts: vec![Stmt::Expr {
                                    expr: Expr::Call {
                                        name: full_ctor_name,
                                        args: call_args,
                                        loc: c.loc,
                                        ty: Type::void(),
                                    },
                                    loc: c.loc,
                                }],
                                loc: c.loc,
                            };
                            let wrapper = FuncDecl {
                                loc: c.loc,
                                return_type: Type::void(),
                                name: default_ctor_name,
                                params: vec![Param {
                                    name: "this".to_string(),
                                    ty: this_ty,
                                    loc: c.loc,
                                    default: None,
                                }],
                                body: Some(wrapper_body),
                                is_static: false,
                                is_extern: false,
                                source_file: String::new(),
                                is_variadic: false,
                            };
                            class_methods.push(wrapper);
                        }
                    }
                    ClassMember::Destructor { body: Some(ref b), .. } => {
                        let mut func_decl = FuncDecl {
                            loc: c.loc,
                            return_type: Type::void(),
                            name: format!("__dtor__{}", c.name),
                            params: vec![Param {
                                name: "this".to_string(),
                                ty: Type::Pointer {
                                    pointee: Box::new(Type::Class {
                                        name: c.name.clone(),
                                        is_const: false,
                                    }),
                                    is_const: false,
                                },
                                loc: c.loc,
                                default: None,
                            }],
                            body: Some(b.clone()),
                            is_static: false,
                            is_extern: false,
                            source_file: String::new(),
                            is_variadic: false,
                        };
                        self.visit_func_decl_with_fields(&mut func_decl, &class_fields);
                        class_methods.push(func_decl);
                    }
                    _ => {}
                }
            }
            self.current_class = None;
        }
        program.funcs.extend(class_methods);
    }

    /// 判断参数类型是否是类 `class_name` 的拷贝构造参数（`Class&` 或 `const Class&`）。
    pub(crate) fn is_copy_constructor_param(class_name: &str, ty: &Type) -> bool {
        match ty {
            Type::Reference { base, .. } | Type::RValueRef { base } => {
                if let Type::Class { name, .. } = base.as_ref() {
                    return name == class_name;
                }
            }
            _ => {}
        }
        false
    }

    /// 根据参数列表得到构造函数 mangled 名称。
    /// 拷贝构造函数使用 `__ctor__{Class}__copy`，其他按参数个数编码。
    pub(crate) fn constructor_mangled_name(class_name: &str, params: &[Param]) -> String {
        if params.len() == 1 && Self::is_copy_constructor_param(class_name, &params[0].ty) {
            return format!("__ctor__{}__copy", class_name);
        }
        if params.is_empty() {
            format!("__ctor__{}", class_name)
        } else {
            format!("__ctor__{}__{}", class_name, params.len())
        }
    }

    /// 重载决议：从候选构造函数中选择最佳匹配。
    /// 默认（零参数）构造函数保持 `__ctor__{Class}` 名称；
    /// 拷贝构造函数使用 `__ctor__{Class}__copy`；
    /// 其他带 N 个参数的构造函数编码为 `__ctor__{Class}__N`。
    /// 如果同一个类存在多个**同参数个数但参数类型不同**的构造函数，当前 mangling
    /// 方案无法区分，会报告 E4031 歧义错误而不是静默选择错误路径。
    pub(crate) fn resolve_constructor_overload(
        &mut self,
        class_name: &str,
        arg_types: &[Type],
        loc: SourceLoc,
    ) -> Option<String> {
        let sym = self.classes.get(class_name)?;
        let arg_count = arg_types.len();

        // 拷贝构造函数：单参数且实参为同类（或同类引用）。
        let is_same_class_arg = arg_count == 1
            && match &arg_types[0] {
                Type::Class { name, .. } => name == class_name,
                Type::Reference { base, .. } | Type::RValueRef { base } => {
                    if let Type::Class { name, .. } = base.as_ref() {
                        name == class_name
                    } else {
                        false
                    }
                }
                _ => false,
            };
        if is_same_class_arg {
            let copy_name = format!("__ctor__{}__copy", class_name);
            if sym.methods.contains_key(&copy_name) {
                return Some(copy_name);
            }
        }

        // Collect all constructors that can accept `arg_count` arguments,
        // either exactly or by filling trailing default arguments.
        let mut candidates: Vec<(String, usize)> = Vec::new();
        let ctor_prefix = format!("__ctor__{}", class_name);
        for (name, sigs) in &sym.methods {
            if name.ends_with("__move") || name.ends_with("__copy") {
                continue;
            }
            if !(*name == ctor_prefix || name.starts_with(&format!("{}__", ctor_prefix))) {
                continue;
            }
            for (idx, sig) in sigs.iter().enumerate() {
                let user_count = sig.param_types.len();
                if user_count == arg_count {
                    candidates.push((name.clone(), idx));
                } else if user_count > arg_count {
                    let all_have_defaults = sig.param_defaults.iter().skip(arg_count).all(|d| d.is_some());
                    if all_have_defaults {
                        candidates.push((name.clone(), idx));
                    }
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        if candidates.len() > 1 {
            self.report_error(
                &format!(
                    "类 '{}' 的构造函数存在歧义：存在多个接受 {} 个参数的构造函数",
                    class_name, arg_count
                ),
                &loc,
                ErrorCode::E4031_ConstructorOverloadAmbiguous,
            );
            return None;
        }

        Some(candidates[0].0.clone())
    }

    /// Generate implicit move constructors for classes that contain resources
    /// (pointers, references, or class fields with resources) and do not already
    /// have an explicit move constructor.
    pub(crate) fn generate_implicit_move_ctors(&mut self, program: &mut ProgramNode) {
        let mut move_ctors: Vec<(String, FuncDecl)> = Vec::new();

        // 只为当前编译单元中实际定义的类生成隐式移动构造函数。
        // builtin_layout 预注册的类若未被使用，不应产生额外的移动构造。
        // 按类名排序后遍历：HashSet 的迭代顺序随进程随机种子变化，
        // 会让隐式移动构造函数的生成顺序（进而整段字节码布局）不可重现——
        // 表现为每次重新生成 Bytecode Libc 产物都得到不同的 code 布局
        // （2026-09-11 由产物重生成 diff 定位）。排序保证编译输出确定。
        let mut program_class_names: Vec<String> =
            program.classes.iter().map(|c| c.name.clone()).collect();
        program_class_names.sort();
        program_class_names.dedup();
        for class_name in program_class_names {
            let sym = match self.classes.get(&class_name) {
                Some(s) => s,
                None => continue,
            };
            if !sym.has_resource {
                continue;
            }
            let move_ctor_name = format!("__ctor__{}__move", class_name);
            if self.funcs.contains_key(&move_ctor_name) {
                continue;
            }

            let body = Self::build_implicit_move_ctor_body(&class_name, &sym.fields);
            let func_decl = FuncDecl {
                loc: SourceLoc { line: 0, column: 0, file_id: 0 },
                return_type: Type::void(),
                name: move_ctor_name.clone(),
                params: vec![
                    Param {
                        name: "this".to_string(),
                        ty: Type::Pointer {
                            pointee: Box::new(Type::Class {
                                name: class_name.clone(),
                                is_const: false,
                            }),
                            is_const: false,
                        },
                        loc: SourceLoc { line: 0, column: 0, file_id: 0 },
                        default: None,
                    },
                    Param {
                        name: "other".to_string(),
                        ty: Type::RValueRef {
                            base: Box::new(Type::Class {
                                name: class_name.clone(),
                                is_const: false,
                            }),
                        },
                        loc: SourceLoc { line: 0, column: 0, file_id: 0 },
                        default: None,
                    },
                ],
                body: Some(body),
                is_static: false,
                is_extern: false,
                source_file: String::new(),
                is_variadic: false,
            };

            self.funcs.insert(
                move_ctor_name.clone(),
                FuncSymbol {
                    return_type: Type::void(),
                    param_types: func_decl.params.iter().map(|p| p.ty.clone()).collect(),
                    is_variadic: false,
                    param_defaults: func_decl.params.iter().map(|p| p.default.clone()).collect(),
                },
            );
            move_ctors.push((class_name.clone(), func_decl));
        }

        // Register move ctor signatures in ClassSymbol.methods
        for (class_name, func) in &move_ctors {
            if let Some(sym) = self.classes.get_mut(class_name) {
                sym.methods.entry(func.name.clone()).or_default().push(MethodSig {
                    ret: Type::void(),
                    param_types: func.params.iter().map(|p| p.ty.clone()).collect(),
                    param_defaults: func.params.iter().map(|p| p.default.clone()).collect(),
                    is_virtual: false,
                    is_static: false,
                    is_explicit: false,
                    is_const: false,
                    access: AccessSpec::Public,
                });
            }
        }

        program.funcs.extend(move_ctors.into_iter().map(|(_, f)| f));
    }

    fn build_implicit_move_ctor_body(class_name: &str, fields: &[(Type, String, AccessSpec)]) -> Stmt {
        let loc = SourceLoc { line: 0, column: 0, file_id: 0 };
        let this_ty = Type::Pointer {
            pointee: Box::new(Type::Class {
                name: class_name.to_string(),
                is_const: false,
            }),
            is_const: false,
        };
        let other_ty = Type::RValueRef {
            base: Box::new(Type::Class {
                name: class_name.to_string(),
                is_const: false,
            }),
        };

        let mut stmts = Vec::new();
        for (fty, fname, _) in fields {
            // this->field = other.field;
            stmts.push(Stmt::Expr {
                expr: Expr::Assign {
                    left: Box::new(Expr::Member {
                        object: Box::new(Expr::This { loc, ty: this_ty.clone() }),
                        member: fname.clone(),
                        loc,
                        ty: fty.clone(),
                    }),
                    op: AssignOp::Assign,
                    right: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier {
                            name: "other".to_string(),
                            loc,
                            ty: other_ty.clone(),
                        }),
                        member: fname.clone(),
                        loc,
                        ty: fty.clone(),
                    }),
                    loc,
                    ty: fty.clone(),
                },
                loc,
            });

            // For pointer fields, null out the source to prevent double-free.
            if fty.is_pointer() {
                stmts.push(Stmt::Expr {
                    expr: Expr::Assign {
                        left: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier {
                                name: "other".to_string(),
                                loc,
                                ty: other_ty.clone(),
                            }),
                            member: fname.clone(),
                            loc,
                            ty: fty.clone(),
                        }),
                        op: AssignOp::Assign,
                        right: Box::new(Expr::Literal { value: 0, loc, ty: Type::int() }),
                        loc,
                        ty: fty.clone(),
                    },
                    loc,
                });
            }
        }

        Stmt::Block { stmts, loc }
    }
}
