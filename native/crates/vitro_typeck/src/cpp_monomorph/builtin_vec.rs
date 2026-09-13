use super::*;

impl TypeChecker {
    pub(crate) fn synthesize_vec_class(&mut self, elem_ty: &Type, loc: &SourceLoc) -> (String, ClassDecl) {
        let mangled = Self::mangle_template_name("vitro_vec", std::slice::from_ref(&TemplateArg::Type(elem_ty.clone())));
        let ptr_ty = Type::pointer_to(elem_ty.clone());
        let member_this = |member: &str, ty: Type| TypeChecker::synth_member_this(&mangled, member, ty, *loc);

        let zero = Self::synth_int_lit(0, *loc);
        let one = Self::synth_int_lit(1, *loc);
        let two = Self::synth_int_lit(2, *loc);
        let null_ptr = Self::synth_cast(zero.clone(), ptr_ty.clone(), *loc);

        // Fields: int n; int m; T* a;
        let mut members = vec![
            ClassMember::Field {
                name: "n".to_string(),
                ty: Type::int(),
                access: AccessSpec::Private,
                is_static: false,
            },
            ClassMember::Field {
                name: "m".to_string(),
                ty: Type::int(),
                access: AccessSpec::Private,
                is_static: false,
            },
            ClassMember::Field {
                name: "a".to_string(),
                ty: ptr_ty.clone(),
                access: AccessSpec::Private,
                is_static: false,
            },
        ];

        // Default constructor: n=0; m=0; a=(T*)0;
        let ctor_body = Stmt::Block {
            stmts: vec![
                Self::synth_expr_stmt(Self::synth_assign(member_this("n", Type::int()), zero.clone(), *loc)),
                Self::synth_expr_stmt(Self::synth_assign(member_this("m", Type::int()), zero.clone(), *loc)),
                Self::synth_expr_stmt(Self::synth_assign(member_this("a", ptr_ty.clone()), null_ptr.clone(), *loc)),
            ],
            loc: *loc,
        };
        let ctor = ClassMember::Constructor {
            params: vec![],
            body: Some(ctor_body),
            is_default: true,
            access: AccessSpec::Public,
            is_explicit: false,
        };

        // Destructor: if (this->a != (T*)0) delete[] this->a;
        let dtor_body = Stmt::Block {
            stmts: vec![Stmt::If {
                cond: Expr::Binary {
                    op: BinaryOp::Ne,
                    left: Box::new(member_this("a", ptr_ty.clone())),
                    right: Box::new(null_ptr.clone()),
                    loc: *loc,
                    ty: Type::int(),
                },
                then_stmt: Box::new(Stmt::Block {
                    stmts: vec![Self::synth_expr_stmt(Expr::Delete {
                        expr: Box::new(member_this("a", ptr_ty.clone())),
                        is_array: true,
                        loc: *loc,
                        ty: Type::void(),
                    })],
                    loc: *loc,
                }),
                else_stmt: None,
                loc: *loc,
            }],
            loc: *loc,
        };
        let dtor = ClassMember::Destructor {
            body: Some(dtor_body),
            access: AccessSpec::Public,
            is_virtual: false,
        };

        // size(): return this->n;
        let size_method = ClassMember::Method {
            name: "size".to_string(),
            ret: Type::int(),
            params: vec![],
            body: Some(Stmt::Block {
                stmts: vec![Stmt::Return {
                    value: Some(member_this("n", Type::int())),
                    loc: *loc,
                }],
                loc: *loc,
            }),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        };

        // get(int i): return this->a[i];
        let get_method = ClassMember::Method {
            name: "get".to_string(),
            ret: elem_ty.clone(),
            params: vec![Param {
                ty: Type::int(),
                name: "i".to_string(),
                loc: *loc,
                default: None,
            }],
            body: Some(Stmt::Block {
                stmts: vec![Stmt::Return {
                    value: Some(Expr::Index {
                        array: Box::new(member_this("a", ptr_ty.clone())),
                        index: Box::new(Self::synth_ident("i", Type::int(), *loc)),
                        loc: *loc,
                        ty: elem_ty.clone(),
                    }),
                    loc: *loc,
                }],
                loc: *loc,
            }),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        };

        // push_back(T x)
        let push_back_body = {
            let member_n = member_this("n", Type::int());
            let member_m = member_this("m", Type::int());
            let member_a = member_this("a", ptr_ty.clone());
            let id_x = Self::synth_ident("x", elem_ty.clone(), *loc);
            let id_na = Self::synth_ident("na", ptr_ty.clone(), *loc);
            let id_i = Self::synth_ident("i", Type::int(), *loc);

            // this->m = this->m ? this->m * 2 : 2
            let realloc_assign = Self::synth_expr_stmt(Self::synth_assign(
                member_m.clone(),
                Expr::Ternary {
                    cond: Box::new(Expr::Binary {
                        op: BinaryOp::Ne,
                        left: Box::new(member_m.clone()),
                        right: Box::new(zero.clone()),
                        loc: *loc,
                        ty: Type::int(),
                    }),
                    then_branch: Box::new(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(member_m.clone()),
                        right: Box::new(two.clone()),
                        loc: *loc,
                        ty: Type::int(),
                    }),
                    else_branch: Box::new(two.clone()),
                    loc: *loc,
                    ty: Type::int(),
                },
                *loc,
            ));

            // T* na = new T[this->m];
            let na_decl = Stmt::VarDecl {
                var_type: ptr_ty.clone(),
                name: "na".to_string(),
                init: Some(Expr::New {
                    elem_type: elem_ty.clone(),
                    size_expr: Some(Box::new(member_m.clone())),
                    init: None,
                    loc: *loc,
                    ty: ptr_ty.clone(),
                }),
                extra_vars: vec![],
                is_static: false,
                loc: *loc,
            };

            // for (int i = 0; i < this->n; i = i + 1) na[i] = this->a[i];
            let copy_loop = Stmt::For {
                init: Some(Box::new(Stmt::VarDecl {
                    var_type: Type::int(),
                    name: "i".to_string(),
                    init: Some(zero.clone()),
                    extra_vars: vec![],
                    is_static: false,
                    loc: *loc,
                })),
                cond: Some(Expr::Binary {
                    op: BinaryOp::Lt,
                    left: Box::new(id_i.clone()),
                    right: Box::new(member_n.clone()),
                    loc: *loc,
                    ty: Type::int(),
                }),
                step: vec![Expr::Assign {
                    op: AssignOp::Assign,
                    left: Box::new(id_i.clone()),
                    right: Box::new(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(id_i.clone()),
                        right: Box::new(one.clone()),
                        loc: *loc,
                        ty: Type::int(),
                    }),
                    loc: *loc,
                    ty: Type::int(),
                }],
                body: Box::new(Stmt::Block {
                    stmts: vec![Self::synth_expr_stmt(Expr::Assign {
                        op: AssignOp::Assign,
                        left: Box::new(Expr::Index {
                            array: Box::new(id_na.clone()),
                            index: Box::new(id_i.clone()),
                            loc: *loc,
                            ty: elem_ty.clone(),
                        }),
                        right: Box::new(Expr::Index {
                            array: Box::new(member_a.clone()),
                            index: Box::new(id_i.clone()),
                            loc: *loc,
                            ty: elem_ty.clone(),
                        }),
                        loc: *loc,
                        ty: elem_ty.clone(),
                    })],
                    loc: *loc,
                }),
                loc: *loc,
            };

            // delete[] this->a;
            let delete_old = Self::synth_expr_stmt(Expr::Delete {
                expr: Box::new(member_a.clone()),
                is_array: true,
                loc: *loc,
                ty: Type::void(),
            });

            // this->a = na;
            let assign_a = Self::synth_expr_stmt(Self::synth_assign(member_a.clone(), id_na.clone(), *loc));

            // this->a[this->n] = x;
            let store_new = Self::synth_expr_stmt(Expr::Assign {
                op: AssignOp::Assign,
                left: Box::new(Expr::Index {
                    array: Box::new(member_a.clone()),
                    index: Box::new(member_n.clone()),
                    loc: *loc,
                    ty: elem_ty.clone(),
                }),
                right: Box::new(id_x.clone()),
                loc: *loc,
                ty: elem_ty.clone(),
            });

            // this->n = this->n + 1;
            let inc_n = Self::synth_expr_stmt(Self::synth_assign(
                member_n.clone(),
                Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(member_n.clone()),
                    right: Box::new(one.clone()),
                    loc: *loc,
                    ty: Type::int(),
                },
                *loc,
            ));

            Stmt::Block {
                stmts: vec![
                    Stmt::If {
                        cond: Expr::Binary {
                            op: BinaryOp::Eq,
                            left: Box::new(member_n.clone()),
                            right: Box::new(member_m.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        then_stmt: Box::new(Stmt::Block {
                            stmts: vec![realloc_assign, na_decl, copy_loop, delete_old, assign_a],
                            loc: *loc,
                        }),
                        else_stmt: None,
                        loc: *loc,
                    },
                    store_new,
                    inc_n,
                ],
                loc: *loc,
            }
        };
        let push_back_method = ClassMember::Method {
            name: "push_back".to_string(),
            ret: Type::void(),
            params: vec![Param {
                ty: elem_ty.clone(),
                name: "x".to_string(),
                loc: *loc,
                default: None,
            }],
            body: Some(push_back_body),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        };

        members.push(ctor);
        members.push(dtor);
        members.push(size_method);
        members.push(get_method);
        members.push(push_back_method);
        let class_decl = ClassDecl {
            loc: *loc,
            name: mangled.clone(),
            base: None,
            members,
            vtable: None,
        };
        (mangled, class_decl)
    }
}
