use super::*;

impl Parser {
    pub(crate) fn parse_struct_body(&mut self, name: String, loc: SourceLoc) -> StructDecl {
        self.consume(TokenType::LBrace, "预期 '{'");
        let mut fields = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let field_checkpoint = self.pos;
            let base_type = self.parse_base_type();
            let (fty, fname) = self.parse_declarator(&base_type);
            if self.pos == field_checkpoint {
                self.advance();
                break;
            }
            fields.push(StructField { ty: fty, name: fname });
            while self.match_token(TokenType::Comma) {
                let (extra_ty, extra_name) = self.parse_declarator(&base_type);
                fields.push(StructField { ty: extra_ty, name: extra_name });
            }
            self.consume(TokenType::Semicolon, "预期 ';'");
        }
        self.consume(TokenType::RBrace, "预期 '}'");
        StructDecl { loc, name, fields }
    }
    pub(crate) fn parse_struct_decl(&mut self) -> StructDecl {
        self.consume(TokenType::Struct, "预期 'struct'");
        let name_tok = if self.check(TokenType::Identifier) {
            self.advance().clone()
        } else {
            Token {
                ty: TokenType::Identifier,
                text: format!("__anon_struct_{}", self.pos),
                line: self.current().line,
                column: self.current().column,
            }
        };
        let decl = self.parse_struct_body(
            name_tok.text.clone(),
            SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            },
        );
        self.consume(TokenType::Semicolon, "结构体声明后预期 ';'");
        if self.is_cpp_mode && !decl.name.starts_with("__anon_struct_") {
            self.typedef_names
                .insert(decl.name.clone(), Type::struct_type(decl.name.clone()));
        }
        decl
    }
    pub(crate) fn parse_union_decl(&mut self) -> StructDecl {
        self.consume(TokenType::Union, "预期 'union'");
        let name_tok = if self.check(TokenType::Identifier) {
            self.advance().clone()
        } else {
            Token {
                ty: TokenType::Identifier,
                text: format!("__anon_union_{}", self.pos),
                line: self.current().line,
                column: self.current().column,
            }
        };
        let decl = self.parse_struct_body(
            name_tok.text.clone(),
            SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            },
        );
        self.consume(TokenType::Semicolon, "联合体声明后预期 ';'");
        decl
    }
    // =========================================================================
    // C++ Class Declaration (Phase 31)
    // =========================================================================

    pub(crate) fn parse_class_decl(&mut self) -> ClassDecl {
        self.parse_class_decl_inner(false)
    }
    pub(crate) fn parse_class_decl_inner(&mut self, is_struct: bool) -> ClassDecl {
        let loc = SourceLoc {
            line: self.current().line,
            column: self.current().column,
            file_id: 0,
        };
        if is_struct {
            self.consume(TokenType::Struct, "预期 'struct'");
        } else {
            self.consume(TokenType::Class, "预期 'class'");
        }
        let short_name = self.consume(TokenType::Identifier, "预期类名").text.clone();
        let name = if self.current_class.is_empty() {
            short_name.clone()
        } else {
            let mut full = self.current_class.join("__");
            full.push_str("__");
            full.push_str(&short_name);
            full
        };
        self.current_class.push(name.clone());

        let mut base = None;
        if self.match_token(TokenType::Colon) {
            // 支持 ': public Base'（public 可选）
            if self.check(TokenType::Public) || self.check(TokenType::Private) || self.check(TokenType::Protected) {
                self.advance();
            }
            base = Some(self.consume(TokenType::Identifier, "预期基类名").text.clone());
        }

        self.consume(TokenType::LBrace, "预期 '{'");

        let mut members = Vec::new();
        let mut current_access = if is_struct {
            AccessSpec::Public
        } else {
            AccessSpec::Private
        }; // struct 默认 public，class 默认 private

        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            // 访问说明符
            if self.check(TokenType::Public) || self.check(TokenType::Private) || self.check(TokenType::Protected) {
                let access = match self.current().ty {
                    TokenType::Public => AccessSpec::Public,
                    TokenType::Private => AccessSpec::Private,
                    TokenType::Protected => AccessSpec::Protected,
                    _ => unreachable!(),
                };
                self.advance();
                self.consume(TokenType::Colon, "访问说明符后预期 ':'");
                current_access = access;
                continue;
            }

            // 跳过 inline / virtual / explicit / static 修饰符
            let mut is_virtual = false;
            let mut is_explicit = false;
            let mut is_static = false;
            loop {
                if self.check(TokenType::Inline) || self.check(TokenType::Virtual) || self.check(TokenType::Explicit) {
                    if self.check(TokenType::Virtual) {
                        is_virtual = true;
                    }
                    if self.check(TokenType::Explicit) {
                        is_explicit = true;
                    }
                    self.advance();
                } else if self.is_cpp_mode && self.is_static_token() {
                    is_static = true;
                    self.advance();
                } else {
                    break;
                }
            }

            // 嵌套 struct / class / union / enum
            if self.check(TokenType::Struct) {
                let decl = self.parse_struct_decl();
                members.push(ClassMember::NestedStruct { decl, access: current_access });
                continue;
            }
            if self.is_cpp_mode && self.check(TokenType::Class) {
                let decl = self.parse_class_decl();
                members.push(ClassMember::NestedClass { decl, access: current_access });
                continue;
            }
            if self.check(TokenType::Union) {
                let decl = self.parse_union_decl();
                members.push(ClassMember::NestedStruct { decl, access: current_access });
                continue;
            }
            // 析构函数 ~Name()
            if self.check(TokenType::BitNot)
                && self.peek(1).ty == TokenType::Identifier
                && self.peek(2).ty == TokenType::LParen
            {
                // ~Destructor()
                self.advance(); // ~
                let _dtor_name = self.advance().text.clone();
                self.consume(TokenType::LParen, "预期 '('");
                self.consume(TokenType::RParen, "预期 ')'");
                let body = if self.check(TokenType::LBrace) {
                    Some(self.parse_statement())
                } else {
                    self.consume(TokenType::Semicolon, "析构函数声明后预期 ';'");
                    None
                };
                members.push(ClassMember::Destructor {
                    body,
                    access: current_access,
                    is_virtual,
                });
                continue;
            }

            // 构造函数检查（无返回类型）。嵌套类可用短名定义构造。
            let ctor_name_matches = self.check(TokenType::Identifier)
                && (self.current().text == name || self.current().text == short_name)
                && self.peek(1).ty == TokenType::LParen;
            if ctor_name_matches {
                self.advance(); // class name
                self.consume(TokenType::LParen, "预期 '('");
                let (params, _) = self.parse_param_list();
                self.consume(TokenType::RParen, "预期 ')'");
                let init_list = self.parse_ctor_init_list();
                let body = if self.check(TokenType::LBrace) {
                    let block = self.parse_statement();
                    Self::merge_ctor_init_into_body(init_list, Some(block))
                } else {
                    self.consume(TokenType::Semicolon, "构造函数声明后预期 ';'");
                    None
                };
                members.push(ClassMember::Constructor {
                    params,
                    body,
                    is_default: false,
                    access: current_access,
                    is_explicit,
                });
                continue;
            }

            // 构造函数 / 方法 / 字段 需要类型前瞻
            if !self.is_type_token() && !self.check(TokenType::Identifier) {
                // 未知内容，报错并跳过
                self.errors.push(ParseError {
                    message: format!("类成员声明中预期类型或方法名，找到: {}", self.current().text),
                    line: self.current().line,
                    column: self.current().column,
                    code: ErrorCode::E2005_ExpectedSemicolon as i32,
                });
                self.advance();
                continue;
            }

            let checkpoint = self.pos;
            let _member_base_type = self.parse_base_type();
            let lookahead = self.look_ahead_skip_stars();

            if lookahead < self.tokens.len()
                && self.tokens[lookahead].ty == TokenType::Identifier
                && lookahead + 1 < self.tokens.len()
                && self.tokens[lookahead + 1].ty == TokenType::LParen
            {
                // 方法或构造函数
                self.pos = checkpoint;
                let ret_type = self.parse_base_type();
                let mut ptr_depth = 0;
                while self.match_token(TokenType::Star) {
                    ptr_depth += 1;
                }
                let mut final_ret = ret_type;
                for _ in 0..ptr_depth {
                    final_ret = Type::pointer_to(final_ret);
                }
                // C++ 引用 / 右值引用返回类型
                if self.is_cpp_mode {
                    if self.match_token(TokenType::Ampersand) {
                        let is_const = self.match_token(TokenType::Const);
                        final_ret = Type::Reference {
                            base: Box::new(final_ret),
                            is_const,
                        };
                    } else if self.match_token(TokenType::AndAnd) {
                        final_ret = Type::RValueRef { base: Box::new(final_ret) };
                    }
                }
                let method_name = self.advance().text.clone();
                self.consume(TokenType::LParen, "预期 '('");
                let (params, _) = self.parse_param_list();
                self.consume(TokenType::RParen, "预期 ')'");

                // 检查 const（仅对方法有效）
                let is_const = self.match_token(TokenType::Const);

                // 检查 override
                if self.check(TokenType::Override) {
                    self.advance();
                }

                if method_name == name && ptr_depth == 0 && matches!(final_ret.kind(), TypeKind::Void | TypeKind::Int) {
                    // 构造函数（简化：与类同名且无返回类型修饰）
                    let init_list = self.parse_ctor_init_list();
                    let body = if self.check(TokenType::LBrace) {
                        let block = self.parse_statement();
                        Self::merge_ctor_init_into_body(init_list, Some(block))
                    } else {
                        self.consume(TokenType::Semicolon, "构造函数声明后预期 ';'");
                        None
                    };
                    members.push(ClassMember::Constructor {
                        params,
                        body,
                        is_default: false,
                        access: current_access,
                        is_explicit,
                    });
                } else {
                    let body = if self.check(TokenType::LBrace) {
                        Some(self.parse_statement())
                    } else {
                        self.consume(TokenType::Semicolon, "方法声明后预期 ';'");
                        None
                    };
                    members.push(ClassMember::Method {
                        name: method_name,
                        ret: final_ret,
                        params,
                        body,
                        is_virtual,
                        access: current_access,
                        is_static,
                        is_const,
                    });
                }
            } else {
                // 字段声明（支持逗号分隔多字段，如 `int head, tail;`）
                self.pos = checkpoint;
                let field_type = self.parse_base_type();
                let (ty, field_name) = self.parse_declarator(&field_type);
                members.push(ClassMember::Field {
                    name: field_name,
                    ty,
                    access: current_access,
                    is_static,
                });
                while self.match_token(TokenType::Comma) {
                    let (extra_ty, extra_name) = self.parse_declarator(&field_type);
                    members.push(ClassMember::Field {
                        name: extra_name,
                        ty: extra_ty,
                        access: current_access,
                        is_static,
                    });
                }
                self.consume(TokenType::Semicolon, "字段声明后预期 ';'");
            }
        }

        self.consume(TokenType::RBrace, "预期 '}'");
        self.consume(TokenType::Semicolon, "类声明后预期 ';'");

        self.current_class.pop();
        ClassDecl {
            loc,
            name,
            base,
            members,
            vtable: None,
        }
    }
    // =========================================================================
    // C++ Template Declaration (Phase 31)
    // =========================================================================

    /// 判断当前 `template` 开头是否为显式实例化声明：
    /// `template class Name<args>;`（无模板参数列表）。
    pub(crate) fn is_template_explicit_instantiation(&self) -> bool {
        let mut i = self.pos;
        if i >= self.tokens.len() || self.tokens[i].ty != TokenType::Template {
            return false;
        }
        i += 1;
        if i >= self.tokens.len() || (self.tokens[i].ty != TokenType::Class && self.tokens[i].ty != TokenType::Struct) {
            return false;
        }
        i += 1;
        if i >= self.tokens.len() || self.tokens[i].ty != TokenType::Identifier {
            return false;
        }
        i += 1;
        if i >= self.tokens.len() || self.tokens[i].ty != TokenType::Lt {
            return false;
        }
        i += 1;
        let mut depth = 1;
        while i < self.tokens.len() && depth > 0 {
            match self.tokens[i].ty {
                TokenType::Lt => depth += 1,
                TokenType::Gt => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return false;
        }
        i + 1 < self.tokens.len() && self.tokens[i + 1].ty == TokenType::Semicolon
    }
    pub(crate) fn parse_template_instantiation(&mut self) -> TemplateInstantiation {
        let loc = SourceLoc {
            line: self.current().line,
            column: self.current().column,
            file_id: 0,
        };
        self.consume(TokenType::Template, "预期 'template'");
        self.consume(TokenType::Class, "预期 'class'");
        let base = self.consume(TokenType::Identifier, "预期模板类名").text.clone();
        self.consume(TokenType::Lt, "预期 '<'");
        let mut args = Vec::new();
        while !self.check(TokenType::Gt) && !self.is_at_end() {
            let arg = if self.is_type_token() {
                cide_ast::TemplateArg::Type(self.parse_base_type())
            } else {
                cide_ast::TemplateArg::Expr(self.parse_template_arg_expr())
            };
            args.push(arg);
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::Gt, "预期 '>'");
        self.consume(TokenType::Semicolon, "预期 ';'");
        TemplateInstantiation { loc, base, args }
    }
    pub(crate) fn parse_template_decl(&mut self) -> TemplateDecl {
        let loc = SourceLoc {
            line: self.current().line,
            column: self.current().column,
            file_id: 0,
        };
        self.consume(TokenType::Template, "预期 'template'");
        self.consume(TokenType::Lt, "预期 '<'");

        let mut params = Vec::new();
        while !self.check(TokenType::Gt) && !self.is_at_end() {
            if self.check(TokenType::Typename) || self.check(TokenType::Class) {
                // 类型模板参数：typename T / class T
                self.advance();
                let param_name = self.consume(TokenType::Identifier, "预期模板参数名").text.clone();
                params.push(TemplateParam::Type { name: param_name.clone(), loc });
                // 将模板参数注册为类型名，使其在函数/类体中可被识别
                // 使用 Class 类型作为占位符，以便 TypeChecker 在单态化时识别模板参数
                self.typedef_names.insert(
                    param_name.clone(),
                    Type::Class {
                        name: param_name,
                        is_const: false,
                    },
                );
            } else if self.is_type_token() {
                // 非类型模板参数：int N, unsigned M, ...
                let ty = self.parse_base_type();
                let param_name = self.consume(TokenType::Identifier, "预期模板参数名").text.clone();
                params.push(TemplateParam::NonType {
                    name: param_name.clone(),
                    ty,
                    loc,
                });
            } else {
                self.errors.push(ParseError {
                    message: "预期 'typename'、'class' 或类型名".to_string(),
                    line: self.current().line,
                    column: self.current().column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                break;
            }
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::Gt, "预期 '>'");

        // 模板后面跟着函数、类或结构体声明
        let decl = if self.check(TokenType::Class) || self.check(TokenType::Struct) {
            // 在进入类体前就把类模板名加入 template_names，
            // 使类体内出现 `unique_ptr<T>&` 自身引用参数时能正确解析为 TemplateId。
            let is_struct = self.check(TokenType::Struct);
            let checkpoint = self.pos;
            self.advance(); // class / struct
            let class_name = if self.check(TokenType::Identifier) {
                self.advance().text.clone()
            } else {
                String::new()
            };
            self.pos = checkpoint; // 回退到 class/struct 关键字
            if !class_name.is_empty() {
                self.template_names.insert(class_name);
            }
            if is_struct {
                let class_decl = self.parse_class_decl_inner(true);
                self.template_names.insert(class_decl.name.clone());
                Templateable::Class(Box::new(class_decl))
            } else {
                let class_decl = self.parse_class_decl();
                self.template_names.insert(class_decl.name.clone());
                Templateable::Class(Box::new(class_decl))
            }
        } else {
            // C++ 允许 template<class T> static void foo(...)
            let mut is_template_static = false;
            if self.is_cpp_mode && self.is_static_token() {
                is_template_static = true;
                self.advance(); // consume 'static'
            }
            Templateable::Func(Box::new(self.parse_func_decl(is_template_static, false)))
        };
        TemplateDecl { loc, params, decl }
    }
    pub(crate) fn parse_typedef_struct_decl(&mut self, program: &mut ProgramNode) {
        let loc = self.current().clone();
        self.advance(); // typedef
        self.advance(); // struct
        let name = if self.check(TokenType::Identifier) {
            let t = self.advance().clone();
            t.text
        } else {
            format!("__anon_struct_{}", self.pos)
        };
        let decl = self.parse_struct_body(
            name.clone(),
            SourceLoc {
                line: loc.line,
                column: loc.column,
                file_id: 0,
            },
        );
        let alias_tok = self.consume(TokenType::Identifier, "typedef 后预期标识符名称").clone();
        self.consume(TokenType::Semicolon, "typedef 后预期 ';'");
        program.structs.push(decl);
        self.typedef_names.insert(alias_tok.text, Type::struct_type(name));
    }
    pub(crate) fn parse_typedef_enum_decl(&mut self, program: &mut ProgramNode) {
        let loc = self.current().clone();
        self.advance(); // typedef
        self.advance(); // enum
        let mut enum_name = String::new();
        if self.check(TokenType::Identifier) {
            enum_name = self.current().text.clone();
            self.advance();
        }
        // C23 enum : T 底层类型（E1）：`enum E : unsigned char { ... }`。
        // 枚举成员与类型名都按 T 处理（sizeof(enum E) = sizeof(T)）。
        let mut underlying = Type::int();
        if self.match_token(TokenType::Colon) {
            underlying = self.parse_base_type();
            if !matches!(
                underlying.kind(),
                cide_ast::TypeKind::Int | cide_ast::TypeKind::Char | cide_ast::TypeKind::LongLong
            ) {
                self.errors.push(ParseError {
                    message: "enum 底层类型必须是整数类型（int / char / long long 及其 unsigned 变体）".to_string(),
                    line: loc.line,
                    column: loc.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                underlying = Type::int();
            }
        }
        self.consume(TokenType::LBrace, "enum 后预期 '{'");
        let mut next_value: i64 = 0;
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let member_tok = self.consume(TokenType::Identifier, "enum 成员预期标识符").clone();
            if self.match_token(TokenType::Assign) {
                let val_expr = self.parse_assign();
                // F-P0-3：此前仅匹配裸 Literal，`NEG = -1`（Unary）、`BIG = 1+2`
                // （Binary）静默保持旧值（输出 0 1 2 而非 -1 0 3）
                match eval_enum_const(&val_expr) {
                    Some(v) => next_value = v,
                    None => {
                        self.errors.push(ParseError {
                            message: "enum 初始化器必须是编译期整数常量（支持负数与四则/位运算）".to_string(),
                            line: member_tok.line,
                            column: member_tok.column,
                            code: ErrorCode::E1006_UnsupportedFeature as i32,
                        });
                    }
                }
            }
            program.globals.push(GlobalDecl {
                loc: SourceLoc {
                    line: loc.line,
                    column: loc.column,
                    file_id: 0,
                },
                // E1：成员常量超 i32（enum Big : long long）按 LongLiteral 存储
                ty: if i32::try_from(next_value).is_ok() {
                    Type::int()
                } else {
                    underlying.clone()
                },
                name: member_tok.text,
                init: Some(if i32::try_from(next_value).is_ok() {
                    Expr::Literal {
                        value: next_value as i32,
                        loc: SourceLoc {
                            line: member_tok.line,
                            column: member_tok.column,
                            file_id: 0,
                        },
                        ty: Type::int(),
                    }
                } else {
                    Expr::LongLiteral {
                        value: next_value,
                        loc: SourceLoc {
                            line: member_tok.line,
                            column: member_tok.column,
                            file_id: 0,
                        },
                        ty: Type::long_long(),
                    }
                }),
                is_static: false,
                is_extern: false,
                source_file: String::new(),
            });
            next_value += 1;
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::RBrace, "enum 成员后预期 '}'");
        let alias_tok = self.consume(TokenType::Identifier, "typedef 后预期标识符名称").clone();
        self.consume(TokenType::Semicolon, "typedef 后预期 ';'");
        self.typedef_names.insert(alias_tok.text, Type::int());
        if !enum_name.is_empty() {
            self.typedef_names.insert(enum_name, underlying);
        }
    }
    pub(crate) fn parse_func_decl(&mut self, is_static: bool, is_extern: bool) -> FuncDecl {
        let base_type = self.parse_base_type();

        let mut ret_type = base_type.clone();
        while self.match_token(TokenType::Star) {
            ret_type = Type::pointer_to(ret_type.clone());
        }
        if self.is_cpp_mode {
            if self.match_token(TokenType::Ampersand) {
                let is_const = self.match_token(TokenType::Const);
                ret_type = Type::Reference {
                    base: Box::new(ret_type),
                    is_const,
                };
            } else if self.match_token(TokenType::AndAnd) {
                ret_type = Type::RValueRef { base: Box::new(ret_type) };
            }
        }

        let name_tok = self.consume(TokenType::Identifier, "预期函数名称").clone();
        let mut func_name = name_tok.text.clone();
        // C++ 类外成员函数定义: Bar::set → Bar__set
        // 同时支持模板形式 Box<T>::set（模板参数仅用于消耗 token，名字仍用 Box__set）。
        if self.is_cpp_mode {
            if self.check(TokenType::Lt) {
                // 消耗可选的模板实参列表，例如 Box<T>::set 中的 <T>
                self.advance(); // '<'
                while !self.check(TokenType::Gt) && !self.is_at_end() {
                    self.parse_base_type();
                    if !self.match_token(TokenType::Comma) {
                        break;
                    }
                }
                self.consume(TokenType::Gt, "预期 '>'");
            }
            if self.match_token(TokenType::ColonColon) {
                let method_tok = self.consume(TokenType::Identifier, "预期方法名").clone();
                if method_tok.text == func_name {
                    // 构造函数类外定义: Counter::Counter() { ... }
                    func_name = format!("__ctor__{}", func_name);
                    ret_type = Type::void();
                } else {
                    func_name = format!("{}__{}", func_name, method_tok.text);
                }
            }
        }
        self.consume(TokenType::LParen, "预期 '('");

        let (params, is_variadic) = self.parse_param_list();
        self.consume(TokenType::RParen, "预期 ')'");
        if self.is_cpp_mode {
            self.match_token(TokenType::Const);
        }
        let body = if self.check(TokenType::LBrace) {
            Some(self.parse_block())
        } else {
            self.consume(TokenType::Semicolon, "函数声明后预期 ';' 或 '{'");
            None
        };

        FuncDecl {
            loc: SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            },
            return_type: ret_type,
            name: func_name,
            params,
            body,
            is_static,
            is_extern,
            source_file: String::new(),
            is_variadic,
        }
    }
    pub(crate) fn parse_typedef(&mut self) {
        self.advance();
        let base_type = self.parse_base_type();
        let (ty, name) = self.parse_declarator(&base_type);
        self.consume(TokenType::Semicolon, "typedef 后预期 ';'");
        self.typedef_names.insert(name, ty);
    }
    pub(crate) fn parse_enum_decl(&mut self, program: &mut ProgramNode) {
        let loc = self.current().clone();
        self.advance();
        let mut enum_name = String::new();
        if self.check(TokenType::Identifier) {
            enum_name = self.current().text.clone();
            self.advance();
        }
        // C23 enum : T 底层类型（E1）：`enum E : unsigned char { ... }`。
        // 枚举成员与类型名都按 T 处理（sizeof(enum E) = sizeof(T)）。
        let mut underlying = Type::int();
        if self.match_token(TokenType::Colon) {
            underlying = self.parse_base_type();
            if !matches!(
                underlying.kind(),
                cide_ast::TypeKind::Int | cide_ast::TypeKind::Char | cide_ast::TypeKind::LongLong
            ) {
                self.errors.push(ParseError {
                    message: "enum 底层类型必须是整数类型（int / char / long long 及其 unsigned 变体）".to_string(),
                    line: loc.line,
                    column: loc.column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                underlying = Type::int();
            }
        }
        self.consume(TokenType::LBrace, "enum 后预期 '{'");
        let mut next_value: i64 = 0;
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let member_tok = self.consume(TokenType::Identifier, "enum 成员预期标识符").clone();
            if self.match_token(TokenType::Assign) {
                let val_expr = self.parse_assign();
                // F-P0-3：此前仅匹配裸 Literal，`NEG = -1`（Unary）、`BIG = 1+2`
                // （Binary）静默保持旧值（输出 0 1 2 而非 -1 0 3）
                match eval_enum_const(&val_expr) {
                    Some(v) => next_value = v,
                    None => {
                        self.errors.push(ParseError {
                            message: "enum 初始化器必须是编译期整数常量（支持负数与四则/位运算）".to_string(),
                            line: member_tok.line,
                            column: member_tok.column,
                            code: ErrorCode::E1006_UnsupportedFeature as i32,
                        });
                    }
                }
            }
            program.globals.push(GlobalDecl {
                loc: SourceLoc {
                    line: loc.line,
                    column: loc.column,
                    file_id: 0,
                },
                // E1：成员常量超 i32（enum Big : long long）按 LongLiteral 存储
                ty: if i32::try_from(next_value).is_ok() {
                    Type::int()
                } else {
                    underlying.clone()
                },
                name: member_tok.text,
                init: Some(if i32::try_from(next_value).is_ok() {
                    Expr::Literal {
                        value: next_value as i32,
                        loc: SourceLoc {
                            line: member_tok.line,
                            column: member_tok.column,
                            file_id: 0,
                        },
                        ty: Type::int(),
                    }
                } else {
                    Expr::LongLiteral {
                        value: next_value,
                        loc: SourceLoc {
                            line: member_tok.line,
                            column: member_tok.column,
                            file_id: 0,
                        },
                        ty: Type::long_long(),
                    }
                }),
                is_static: false,
                is_extern: false,
                source_file: String::new(),
            });
            next_value += 1;
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::RBrace, "enum 成员后预期 '}'");
        self.consume(TokenType::Semicolon, "enum 声明后预期 ';'");
        if !enum_name.is_empty() {
            self.typedef_names.insert(enum_name, underlying);
        }
    }
}

/// F-P0-3：enum 初始化器的编译期常量求值。
/// 支持整数字面量、一元负号/按位取反/逻辑非、四则与位运算、比较的常量折叠。
pub(crate) fn eval_enum_const(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal { value, .. } => Some(*value as i64),
        Expr::LongLiteral { value, .. } => Some(*value),
        // E3：static_assert 的 sizeof（教学最高频用法）。内建类型即可精确求值
        //（空 struct/class 表——struct 布局参与常量求值属子集边界，返回 None）
        Expr::Sizeof { target_type: Some(t), .. } => {
            let empty = std::collections::HashMap::new();
            let empty_cls = std::collections::HashMap::new();
            let sz = crate::compute_type_size(t, &empty, &empty, &empty_cls);
            if sz > 0 {
                Some(sz as i64)
            } else {
                None
            }
        }
        Expr::Unary { op, operand, .. } => {
            let v = eval_enum_const(operand)?;
            match op {
                UnaryOp::Neg => Some(-v),
                UnaryOp::BitNot => Some(!v),
                UnaryOp::Not => Some((v == 0) as i64),
                _ => None,
            }
        }
        Expr::Binary { op, left, right, .. } => {
            let a = eval_enum_const(left)?;
            let b = eval_enum_const(right)?;
            match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                BinaryOp::Mod => a.checked_rem(b),
                BinaryOp::Shl => Some(a << b),
                BinaryOp::Shr => Some(a >> b),
                BinaryOp::BitAnd => Some(a & b),
                BinaryOp::BitOr => Some(a | b),
                BinaryOp::BitXor => Some(a ^ b),
                BinaryOp::Eq => Some((a == b) as i64),
                BinaryOp::Ne => Some((a != b) as i64),
                BinaryOp::Lt => Some((a < b) as i64),
                BinaryOp::Le => Some((a <= b) as i64),
                BinaryOp::Gt => Some((a > b) as i64),
                BinaryOp::Ge => Some((a >= b) as i64),
                _ => None,
            }
        }
        _ => None,
    }
}
