use super::*;

impl TypeChecker {
    pub(crate) fn synthesize_list_class(&mut self, elem_ty: &Type, loc: &SourceLoc) -> (String, ClassDecl) {
        let node_mangled =
            Self::mangle_template_name("vitro_list_node", std::slice::from_ref(&TemplateArg::Type(elem_ty.clone())));
        let list_mangled =
            Self::mangle_template_name("vitro_list", std::slice::from_ref(&TemplateArg::Type(elem_ty.clone())));

        let node_ptr_ty = Type::pointer_to(Type::Class {
            name: node_mangled.clone(),
            is_const: false,
        });
        let list_ptr_ty = Type::pointer_to(Type::Class {
            name: list_mangled.clone(),
            is_const: false,
        });

        let node_this = Self::synth_this(node_ptr_ty.clone(), *loc);
        let list_this = Self::synth_this(list_ptr_ty.clone(), *loc);

        let zero = Self::synth_int_lit(0, *loc);
        let one = Self::synth_int_lit(1, *loc);
        let null_node = Self::synth_cast(zero.clone(), node_ptr_ty.clone(), *loc);

        // =====================================================================
        // 1. 合成 vitro_list_node<T>
        // =====================================================================
        let node_members = vec![
            ClassMember::Field {
                name: "data".to_string(),
                ty: elem_ty.clone(),
                access: AccessSpec::Public,
                is_static: false,
            },
            ClassMember::Field {
                name: "next".to_string(),
                ty: node_ptr_ty.clone(),
                access: AccessSpec::Public,
                is_static: false,
            },
        ];

        let node_ctor_body = Stmt::Block {
            stmts: vec![Self::synth_expr_stmt(Expr::Assign {
                op: AssignOp::Assign,
                left: Box::new(Expr::Member {
                    object: Box::new(node_this.clone()),
                    member: "next".to_string(),
                    loc: *loc,
                    ty: node_ptr_ty.clone(),
                }),
                right: Box::new(null_node.clone()),
                loc: *loc,
                ty: Type::void(),
            })],
            loc: *loc,
        };
        let node_ctor = ClassMember::Constructor {
            params: vec![],
            body: Some(node_ctor_body),
            is_default: true,
            access: AccessSpec::Public,
            is_explicit: false,
        };

        let node_class = ClassDecl {
            loc: *loc,
            name: node_mangled.clone(),
            base: None,
            members: {
                let mut m = node_members;
                m.push(node_ctor);
                m
            },
            vtable: None,
        };
        self.register_single_class_layout(&node_mangled, &node_class);
        self.pending_class_instantiations.push((node_mangled.clone(), node_class));

        // =====================================================================
        // 2. 合成 vitro_list<T>
        // =====================================================================
        let member_head = Expr::Member {
            object: Box::new(list_this.clone()),
            member: "head".to_string(),
            loc: *loc,
            ty: node_ptr_ty.clone(),
        };
        let member_tail = Expr::Member {
            object: Box::new(list_this.clone()),
            member: "tail".to_string(),
            loc: *loc,
            ty: node_ptr_ty.clone(),
        };
        let member_n = Expr::Member {
            object: Box::new(list_this.clone()),
            member: "n".to_string(),
            loc: *loc,
            ty: Type::int(),
        };

        let mut list_members = vec![
            ClassMember::Field {
                name: "head".to_string(),
                ty: node_ptr_ty.clone(),
                access: AccessSpec::Private,
                is_static: false,
            },
            ClassMember::Field {
                name: "tail".to_string(),
                ty: node_ptr_ty.clone(),
                access: AccessSpec::Private,
                is_static: false,
            },
            ClassMember::Field {
                name: "n".to_string(),
                ty: Type::int(),
                access: AccessSpec::Private,
                is_static: false,
            },
        ];

        // Default constructor
        let ctor_body = Stmt::Block {
            stmts: vec![
                Self::synth_expr_stmt(Self::synth_assign(member_head.clone(), null_node.clone(), *loc)),
                Self::synth_expr_stmt(Self::synth_assign(member_tail.clone(), null_node.clone(), *loc)),
                Self::synth_expr_stmt(Self::synth_assign(member_n.clone(), zero.clone(), *loc)),
            ],
            loc: *loc,
        };
        list_members.push(ClassMember::Constructor {
            params: vec![],
            body: Some(ctor_body),
            is_default: true,
            access: AccessSpec::Public,
            is_explicit: false,
        });

        // size()
        list_members.push(ClassMember::Method {
            name: "size".to_string(),
            ret: Type::int(),
            params: vec![],
            body: Some(Stmt::Block {
                stmts: vec![Stmt::Return {
                    value: Some(member_n.clone()),
                    loc: *loc,
                }],
                loc: *loc,
            }),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        });

        // Helper: synthesize `this->head` / `this->tail` / `this->n` as expressions
        let synth_list_member = |member: &str, ty: Type| Expr::Member {
            object: Box::new(Self::synth_this(list_ptr_ty.clone(), *loc)),
            member: member.to_string(),
            loc: *loc,
            ty,
        };

        // push_back(T x)
        let push_back_body = {
            let node_new = Self::synth_ident("node", node_ptr_ty.clone(), *loc);
            let id_x = Self::synth_ident("x", elem_ty.clone(), *loc);
            Stmt::Block {
                stmts: vec![
                    // vitro_list_node<T>* node = new vitro_list_node<T>;
                    Stmt::VarDecl {
                        var_type: node_ptr_ty.clone(),
                        name: "node".to_string(),
                        init: Some(Expr::New {
                            elem_type: Type::Class {
                                name: node_mangled.clone(),
                                is_const: false,
                            },
                            size_expr: None,
                            init: None,
                            loc: *loc,
                            ty: node_ptr_ty.clone(),
                        }),
                        extra_vars: vec![],
                        is_static: false,
                        loc: *loc,
                    },
                    // node->data = x;
                    Self::synth_expr_stmt(Expr::Assign {
                        op: AssignOp::Assign,
                        left: Box::new(Expr::Member {
                            object: Box::new(node_new.clone()),
                            member: "data".to_string(),
                            loc: *loc,
                            ty: elem_ty.clone(),
                        }),
                        right: Box::new(id_x.clone()),
                        loc: *loc,
                        ty: Type::void(),
                    }),
                    // node->next = (vitro_list_node<T>*)0;
                    Self::synth_expr_stmt(Expr::Assign {
                        op: AssignOp::Assign,
                        left: Box::new(Expr::Member {
                            object: Box::new(node_new.clone()),
                            member: "next".to_string(),
                            loc: *loc,
                            ty: node_ptr_ty.clone(),
                        }),
                        right: Box::new(null_node.clone()),
                        loc: *loc,
                        ty: Type::void(),
                    }),
                    // if (this->tail) this->tail->next = node; else this->head = node;
                    Stmt::If {
                        cond: Expr::Binary {
                            op: BinaryOp::Ne,
                            left: Box::new(synth_list_member("tail", node_ptr_ty.clone())),
                            right: Box::new(null_node.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        then_stmt: Box::new(Stmt::Block {
                            stmts: vec![Self::synth_expr_stmt(Expr::Assign {
                                op: AssignOp::Assign,
                                left: Box::new(Expr::Member {
                                    object: Box::new(synth_list_member("tail", node_ptr_ty.clone())),
                                    member: "next".to_string(),
                                    loc: *loc,
                                    ty: node_ptr_ty.clone(),
                                }),
                                right: Box::new(node_new.clone()),
                                loc: *loc,
                                ty: Type::void(),
                            })],
                            loc: *loc,
                        }),
                        else_stmt: Some(Box::new(Stmt::Block {
                            stmts: vec![Self::synth_expr_stmt(Self::synth_assign(
                                synth_list_member("head", node_ptr_ty.clone()),
                                node_new.clone(),
                                *loc,
                            ))],
                            loc: *loc,
                        })),
                        loc: *loc,
                    },
                    // this->tail = node;
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("tail", node_ptr_ty.clone()),
                        node_new.clone(),
                        *loc,
                    )),
                    // this->n = this->n + 1;
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("n", Type::int()),
                        Expr::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(synth_list_member("n", Type::int())),
                            right: Box::new(one.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        *loc,
                    )),
                ],
                loc: *loc,
            }
        };
        list_members.push(ClassMember::Method {
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
        });

        // get(int i)
        let get_body = {
            let id_i = Self::synth_ident("i", Type::int(), *loc);
            let id_p = Self::synth_ident("p", node_ptr_ty.clone(), *loc);
            Stmt::Block {
                stmts: vec![
                    // vitro_list_node<T>* p = this->head;
                    Stmt::VarDecl {
                        var_type: node_ptr_ty.clone(),
                        name: "p".to_string(),
                        init: Some(synth_list_member("head", node_ptr_ty.clone())),
                        extra_vars: vec![],
                        is_static: false,
                        loc: *loc,
                    },
                    // while (i > 0 && p != (vitro_list_node<T>*)0) { i = i - 1; p = p->next; }
                    Stmt::While {
                        cond: Expr::Binary {
                            op: BinaryOp::And,
                            left: Box::new(Expr::Binary {
                                op: BinaryOp::Gt,
                                left: Box::new(id_i.clone()),
                                right: Box::new(zero.clone()),
                                loc: *loc,
                                ty: Type::int(),
                            }),
                            right: Box::new(Expr::Binary {
                                op: BinaryOp::Ne,
                                left: Box::new(id_p.clone()),
                                right: Box::new(null_node.clone()),
                                loc: *loc,
                                ty: Type::int(),
                            }),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        body: Box::new(Stmt::Block {
                            stmts: vec![
                                Self::synth_expr_stmt(Self::synth_assign(
                                    id_i.clone(),
                                    Expr::Binary {
                                        op: BinaryOp::Sub,
                                        left: Box::new(id_i.clone()),
                                        right: Box::new(one.clone()),
                                        loc: *loc,
                                        ty: Type::int(),
                                    },
                                    *loc,
                                )),
                                Self::synth_expr_stmt(Self::synth_assign(
                                    id_p.clone(),
                                    Expr::Member {
                                        object: Box::new(id_p.clone()),
                                        member: "next".to_string(),
                                        loc: *loc,
                                        ty: node_ptr_ty.clone(),
                                    },
                                    *loc,
                                )),
                            ],
                            loc: *loc,
                        }),
                        loc: *loc,
                    },
                    // if (p == (vitro_list_node<T>*)0) { T __default; return __default; }
                    // else { return p->data; }
                    Stmt::If {
                        cond: Expr::Binary {
                            op: BinaryOp::Eq,
                            left: Box::new(id_p.clone()),
                            right: Box::new(null_node.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        then_stmt: Box::new(Stmt::Block {
                            stmts: vec![
                                Stmt::VarDecl {
                                    var_type: elem_ty.clone(),
                                    name: "__default".to_string(),
                                    init: None,
                                    extra_vars: vec![],
                                    is_static: false,
                                    loc: *loc,
                                },
                                Stmt::Return {
                                    value: Some(Self::synth_ident("__default", elem_ty.clone(), *loc)),
                                    loc: *loc,
                                },
                            ],
                            loc: *loc,
                        }),
                        else_stmt: Some(Box::new(Stmt::Block {
                            stmts: vec![Stmt::Return {
                                value: Some(Expr::Member {
                                    object: Box::new(id_p.clone()),
                                    member: "data".to_string(),
                                    loc: *loc,
                                    ty: elem_ty.clone(),
                                }),
                                loc: *loc,
                            }],
                            loc: *loc,
                        })),
                        loc: *loc,
                    },
                ],
                loc: *loc,
            }
        };
        list_members.push(ClassMember::Method {
            name: "get".to_string(),
            ret: elem_ty.clone(),
            params: vec![Param {
                ty: Type::int(),
                name: "i".to_string(),
                loc: *loc,
                default: None,
            }],
            body: Some(get_body),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        });

        // clear()
        let clear_body = {
            let id_p = Self::synth_ident("p", node_ptr_ty.clone(), *loc);
            let id_next = Self::synth_ident("next", node_ptr_ty.clone(), *loc);
            Stmt::Block {
                stmts: vec![
                    // vitro_list_node<T>* p = this->head;
                    Stmt::VarDecl {
                        var_type: node_ptr_ty.clone(),
                        name: "p".to_string(),
                        init: Some(synth_list_member("head", node_ptr_ty.clone())),
                        extra_vars: vec![],
                        is_static: false,
                        loc: *loc,
                    },
                    // while (p != (vitro_list_node<T>*)0) { vitro_list_node<T>* next = p->next; delete p; p = next; }
                    Stmt::While {
                        cond: Expr::Binary {
                            op: BinaryOp::Ne,
                            left: Box::new(id_p.clone()),
                            right: Box::new(null_node.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        body: Box::new(Stmt::Block {
                            stmts: vec![
                                Stmt::VarDecl {
                                    var_type: node_ptr_ty.clone(),
                                    name: "next".to_string(),
                                    init: Some(Expr::Member {
                                        object: Box::new(id_p.clone()),
                                        member: "next".to_string(),
                                        loc: *loc,
                                        ty: node_ptr_ty.clone(),
                                    }),
                                    extra_vars: vec![],
                                    is_static: false,
                                    loc: *loc,
                                },
                                Self::synth_expr_stmt(Expr::Delete {
                                    expr: Box::new(id_p.clone()),
                                    is_array: false,
                                    loc: *loc,
                                    ty: Type::void(),
                                }),
                                Self::synth_expr_stmt(Self::synth_assign(id_p.clone(), id_next.clone(), *loc)),
                            ],
                            loc: *loc,
                        }),
                        loc: *loc,
                    },
                    // this->head = (vitro_list_node<T>*)0;
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("head", node_ptr_ty.clone()),
                        null_node.clone(),
                        *loc,
                    )),
                    // this->tail = (vitro_list_node<T>*)0;
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("tail", node_ptr_ty.clone()),
                        null_node.clone(),
                        *loc,
                    )),
                    // this->n = 0;
                    Self::synth_expr_stmt(Self::synth_assign(synth_list_member("n", Type::int()), zero.clone(), *loc)),
                ],
                loc: *loc,
            }
        };
        list_members.push(ClassMember::Method {
            name: "clear".to_string(),
            ret: Type::void(),
            params: vec![],
            body: Some(clear_body.clone()),
            is_virtual: false,
            access: AccessSpec::Public,
            is_static: false,
            is_const: false,
        });

        // Destructor: inline clear logic
        let dtor_body = {
            let id_p = Self::synth_ident("p", node_ptr_ty.clone(), *loc);
            let id_next = Self::synth_ident("next", node_ptr_ty.clone(), *loc);
            Stmt::Block {
                stmts: vec![
                    Stmt::VarDecl {
                        var_type: node_ptr_ty.clone(),
                        name: "p".to_string(),
                        init: Some(synth_list_member("head", node_ptr_ty.clone())),
                        extra_vars: vec![],
                        is_static: false,
                        loc: *loc,
                    },
                    Stmt::While {
                        cond: Expr::Binary {
                            op: BinaryOp::Ne,
                            left: Box::new(id_p.clone()),
                            right: Box::new(null_node.clone()),
                            loc: *loc,
                            ty: Type::int(),
                        },
                        body: Box::new(Stmt::Block {
                            stmts: vec![
                                Stmt::VarDecl {
                                    var_type: node_ptr_ty.clone(),
                                    name: "next".to_string(),
                                    init: Some(Expr::Member {
                                        object: Box::new(id_p.clone()),
                                        member: "next".to_string(),
                                        loc: *loc,
                                        ty: node_ptr_ty.clone(),
                                    }),
                                    extra_vars: vec![],
                                    is_static: false,
                                    loc: *loc,
                                },
                                Self::synth_expr_stmt(Expr::Delete {
                                    expr: Box::new(id_p.clone()),
                                    is_array: false,
                                    loc: *loc,
                                    ty: Type::void(),
                                }),
                                Self::synth_expr_stmt(Self::synth_assign(id_p.clone(), id_next.clone(), *loc)),
                            ],
                            loc: *loc,
                        }),
                        loc: *loc,
                    },
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("head", node_ptr_ty.clone()),
                        null_node.clone(),
                        *loc,
                    )),
                    Self::synth_expr_stmt(Self::synth_assign(
                        synth_list_member("tail", node_ptr_ty.clone()),
                        null_node.clone(),
                        *loc,
                    )),
                    Self::synth_expr_stmt(Self::synth_assign(synth_list_member("n", Type::int()), zero.clone(), *loc)),
                ],
                loc: *loc,
            }
        };
        list_members.push(ClassMember::Destructor {
            body: Some(dtor_body),
            access: AccessSpec::Public,
            is_virtual: false,
        });
        let list_class = ClassDecl {
            loc: *loc,
            name: list_mangled.clone(),
            base: None,
            members: list_members,
            vtable: None,
        };
        (list_mangled, list_class)
    }
}
