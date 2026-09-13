use super::*;

impl Parser {
    // =========================================================================
    // Type parsing
    // =========================================================================

    pub(crate) fn parse_base_type(&mut self) -> Type {
        // Collect type qualifiers/modifiers (const, signed, unsigned, long, short)
        let mut is_unsigned = false;
        let mut is_const = false;
        loop {
            if self.match_token(TokenType::Const) {
                is_const = true;
                continue;
            }
            if self.match_token(TokenType::Volatile) {
                continue;
            }
            if self.match_token(TokenType::Restrict) {
                continue;
            }
            if self.match_token(TokenType::Register) {
                continue;
            }
            if self.match_token(TokenType::Auto) {
                if self.is_cpp_mode {
                    if is_const {
                        return Type::Auto; // auto is not const-qualifiable in our simplified model
                    }
                    return Type::Auto;
                }
                continue;
            }
            if self.match_token(TokenType::Inline) {
                continue;
            }
            if self.match_token(TokenType::Signed) {
                continue;
            }
            if self.match_token(TokenType::Unsigned) {
                is_unsigned = true;
                continue;
            }
            if self.match_token(TokenType::Long) {
                // Check for 'long long'
                if self.check(TokenType::Long) {
                    self.advance();
                    // F-P0-2：合成 unsigned/const 修饰，此前提前 return 丢弃
                    // （unsigned long long 退化为有符号 long long）
                    return Type::LongLong { is_unsigned, is_const };
                }
                continue;
            }
            if self.match_token(TokenType::Short) {
                continue;
            }
            // GCC 扩展 typeof：在类型限定符循环中直接解析并返回，避免 break 后进入普通类型分支。
            if self.check(TokenType::Identifier)
                && (self.current().text == "typeof"
                    || self.current().text == "__typeof__"
                    || self.current().text == "__typeof"
                    || self.current().text == "typeof_unqual"
                    || self.current().text == "__typeof_unqual__"
                    || self.current().text == "__typeof_unqual")
            {
                // C23 typeof_unqual（E1）：推导结果剥离顶层限定符
                let unqual = self.current().text.contains("unqual");
                let _name_tok = self.advance().clone();
                self.consume(TokenType::LParen, "typeof 后预期 '('");
                let expr = self.parse_expression();
                self.consume(TokenType::RParen, "typeof 预期 ')'");
                return Type::Typeof { expr: Box::new(expr), is_const, unqual };
            }
            break;
        }
        let mut t = if self.match_token(TokenType::Int) {
            if is_unsigned {
                Type::unsigned_int()
            } else {
                Type::int()
            }
        } else if self.match_token(TokenType::Void) {
            Type::void()
        } else if self.match_token(TokenType::Float) {
            Type::float()
        } else if self.match_token(TokenType::Double) {
            Type::double()
        } else if self.match_token(TokenType::Union) {
            if self.check(TokenType::Identifier) {
                let name_tok = self.advance().clone();
                Type::union_type(name_tok.text)
            } else {
                Type::union_type("")
            }
        } else if self.check(TokenType::Long) {
            self.advance();
            if self.check(TokenType::Long) {
                self.advance();
            }
            // F-P0-2：与限定符循环内的 long long 分支一致合成修饰
            Type::LongLong { is_unsigned, is_const }
        } else if self.match_token(TokenType::Char) {
            if is_unsigned {
                Type::Char {
                    is_unsigned: true,
                    is_const: false,
                }
            } else {
                Type::char()
            }
        } else if self.match_token(TokenType::Struct) {
            if self.check(TokenType::Identifier) {
                let name_tok = self.advance().clone();
                // C++ 模式下，如果 struct 名字已注册为类型（class 或 struct），返回该类型
                if self.is_cpp_mode {
                    if let Some(ty) = self.typedef_names.get(&name_tok.text).cloned() {
                        return ty;
                    }
                }
                Type::struct_type(name_tok.text)
            } else if self.check(TokenType::LBrace) {
                let loc = SourceLoc {
                    line: self.previous().line,
                    column: self.previous().column,
                    file_id: 0,
                };
                let name = format!("__anon_struct_{}", self.pos);
                let decl = self.parse_struct_body(name.clone(), loc);
                self.anonymous_structs.push(decl);
                Type::struct_type(name)
            } else {
                self.errors.push(ParseError {
                    message: "struct 后预期标识符或 '{'".to_string(),
                    line: self.current().line,
                    column: self.current().column,
                    code: ErrorCode::E1006_UnsupportedFeature as i32,
                });
                Type::int()
            }
        } else if self.match_token(TokenType::Enum) {
            if self.check(TokenType::Identifier) {
                let name_tok = self.advance().clone();
                // C23 enum : T（E1）：已声明的具名 enum 返回其底层类型
                //（sizeof(enum E) = sizeof(T)）；未声明的按 int 登记并返回。
                if let Some(t) = self.typedef_names.get(&name_tok.text).cloned() {
                    return t;
                }
                self.typedef_names.insert(name_tok.text.clone(), Type::int());
            }
            Type::int()
        } else if self.match_token(TokenType::Bool) {
            Type::int()
        } else if self.check(TokenType::Identifier) {
            let name = self.current().text.clone();
            if self.is_cpp_mode && self.template_names.contains(&name) {
                self.advance();
                // 检查是否是模板类实例化语法，如 vector<int>
                if self.check(TokenType::Lt) {
                    self.advance(); // consume '<'
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
                    Type::TemplateId {
                        base: name,
                        args,
                        is_const: false,
                    }
                } else {
                    Type::Class { name, is_const: false }
                }
            } else if let Some(ty) = self.typedef_names.get(&name).cloned() {
                self.advance();
                // C++ 限定嵌套类型名，如 typedef 的类名后接 ::Inner。
                // 注意：要保留 `Class::Class()` 类外构造函数定义的可能性，
                // 因此当 :: 后是同名标识符时（构造定义），不在这里 consume。
                if self.is_cpp_mode && self.check(TokenType::ColonColon) {
                    if let Type::Class { name: ref class_name, .. } = ty {
                        if self.peek(1).ty == TokenType::Identifier && self.peek(1).text != *class_name {
                            let mut full_name = class_name.clone();
                            while self.check(TokenType::ColonColon) {
                                self.advance();
                                let inner = self.consume(TokenType::Identifier, "预期嵌套类名").text.clone();
                                full_name.push_str("__");
                                full_name.push_str(&inner);
                            }
                            return Type::Class {
                                name: full_name,
                                is_const: false,
                            };
                        }
                    }
                }
                ty
            } else if self.is_cpp_mode {
                let first_name = self.advance().text.clone();
                // 类外构造函数定义 `Class::Class()` 不要在这里 consume ::Class。
                let mut name = first_name.clone();
                while self.check(TokenType::ColonColon)
                    && !(self.peek(1).ty == TokenType::Identifier && self.peek(1).text == first_name)
                {
                    self.advance(); // consume '::'
                    let inner = self.consume(TokenType::Identifier, "预期嵌套类名").text.clone();
                    name.push_str("__");
                    name.push_str(&inner);
                }
                Type::Class { name, is_const: false }
            } else {
                if is_unsigned {
                    Type::unsigned_int()
                } else {
                    Type::int()
                }
            }
        } else {
            if is_unsigned {
                Type::unsigned_int()
            } else {
                Type::int()
            }
        };
        if is_unsigned && !matches!(t.kind(), TypeKind::Int | TypeKind::Char) {
            self.errors.push(ParseError {
                message: format!(
                    "'unsigned' 不能修饰 '{}' 类型",
                    match t.kind() {
                        TypeKind::Float => "float",
                        TypeKind::Double => "double",
                        TypeKind::Struct => "struct",
                        TypeKind::Void => "void",
                        TypeKind::Pointer => "指针",
                        TypeKind::Array => "数组",
                        _ => "此",
                    }
                ),
                line: self.current().line,
                column: self.current().column,
                code: ErrorCode::E1006_UnsupportedFeature as i32,
            });
            return Type::int();
        }
        t.set_const(is_const);
        t
    }
    pub(crate) fn parse_declarator(&mut self, base_type: &Type) -> (Type, String) {
        let mut guard = DeclaratorGuard::default();
        let (node, name) = self.parse_declarator_node(&mut guard, false, true);
        let name = name.unwrap_or_default();
        let ty = Self::interpret_declarator_node(&node, base_type);
        (ty, name)
    }
    /// 解析变量声明符：允许指针/引用/数组后缀，但不把 '(' 当作函数声明符后缀，
    /// 以便支持 C++ 构造函数初始化语法 `Type name(args);`。
    pub(crate) fn parse_var_declarator(&mut self, base_type: &Type) -> (Type, String) {
        let mut guard = DeclaratorGuard::default();
        let (node, name) = self.parse_declarator_node(&mut guard, false, true);
        let name = name.unwrap_or_default();
        let ty = Self::interpret_declarator_node(&node, base_type);
        (ty, name)
    }
    // 声明符节点树：按 C 螺旋规则从内到外解释
    /// 解析声明符节点树（C 螺旋规则）。
    /// `is_abstract = true` 时用于 `sizeof(type)` 等抽象声明符场景，不读取标识符且不检查复杂度。
    pub(crate) fn parse_declarator_node(
        &mut self,
        guard: &mut DeclaratorGuard,
        is_abstract: bool,
        allow_function_suffix: bool,
    ) -> (DeclaratorNode, Option<String>) {
        #[derive(Debug, Clone)]
        enum Prefix {
            Pointer,
            Reference(bool), // is_const
            RValueRef,
        }
        let mut prefixes = Vec::new();
        // F-P0-1：DeclaratorGuard 此前不限 ptr_count，10 万个 `*` 会构造出
        // 同深度的嵌套 DeclaratorNode，后续递归遍历（typeck/codegen）栈溢出。
        const MAX_DECLARATOR_PTR: i32 = 32;
        while self.match_token(TokenType::Star) {
            if !is_abstract {
                guard.ptr_count += 1;
                if guard.ptr_count > MAX_DECLARATOR_PTR {
                    self.errors.push(ParseError {
                        message: format!("声明符指针层级过深（超过 {} 层）", MAX_DECLARATOR_PTR),
                        line: self.current().line,
                        column: self.current().column,
                        code: ErrorCode::E1007_ComplexDeclarator as i32,
                    });
                    // 吞掉剩余连续 `*`，避免它们污染后续解析
                    while self.match_token(TokenType::Star) {}
                    break;
                }
            }
            prefixes.push(Prefix::Pointer);
            // 跳过指针限定符（const/volatile/restrict），教学 VM 中无特殊语义
            while self.match_token(TokenType::Const)
                || self.match_token(TokenType::Volatile)
                || self.match_token(TokenType::Restrict)
            {}
        }
        if self.is_cpp_mode {
            while self.match_token(TokenType::Ampersand) {
                if !is_abstract {
                    guard.ptr_count += 1;
                    if guard.ptr_count > MAX_DECLARATOR_PTR {
                        self.errors.push(ParseError {
                            message: format!("声明符引用层级过深（超过 {} 层）", MAX_DECLARATOR_PTR),
                            line: self.current().line,
                            column: self.current().column,
                            code: ErrorCode::E1007_ComplexDeclarator as i32,
                        });
                        while self.match_token(TokenType::Ampersand) {}
                        break;
                    }
                }
                let is_const = self.match_token(TokenType::Const);
                prefixes.push(Prefix::Reference(is_const));
            }
            while self.match_token(TokenType::AndAnd) {
                if !is_abstract {
                    guard.ptr_count += 1;
                    if guard.ptr_count > MAX_DECLARATOR_PTR {
                        self.errors.push(ParseError {
                            message: format!("声明符引用层级过深（超过 {} 层）", MAX_DECLARATOR_PTR),
                            line: self.current().line,
                            column: self.current().column,
                            code: ErrorCode::E1007_ComplexDeclarator as i32,
                        });
                        while self.match_token(TokenType::AndAnd) {}
                        break;
                    }
                }
                prefixes.push(Prefix::RValueRef);
            }
        }

        let (name, mut node) = if self.match_token(TokenType::LParen) {
            if !is_abstract {
                guard.paren_depth += 1;
                if guard.paren_depth > 2 {
                    self.errors.push(ParseError {
                        message: "声明符括号嵌套过深".to_string(),
                        line: self.current().line,
                        column: self.current().column,
                        code: ErrorCode::E1007_ComplexDeclarator as i32,
                    });
                    while !self.check(TokenType::RParen) && !self.is_at_end() {
                        self.advance();
                    }
                    self.match_token(TokenType::RParen);
                    guard.paren_depth -= 1;
                    return (DeclaratorNode::Base, Some(String::new()));
                }
            }
            let (inner_node, inner_name) = self.parse_declarator_node(guard, is_abstract, allow_function_suffix);
            self.consume(TokenType::RParen, "预期 ')'");
            if !is_abstract {
                guard.paren_depth -= 1;
            }
            (inner_name, inner_node)
        } else if is_abstract {
            (None, DeclaratorNode::Base)
        } else {
            let name_tok = self.consume(TokenType::Identifier, "预期标识符名称").clone();
            (Some(name_tok.text), DeclaratorNode::Base)
        };

        // 收集后缀（它们绑定更紧）
        let mut suffixes = Vec::new();
        loop {
            if self.match_token(TokenType::LBracket) {
                if !is_abstract {
                    guard.suffix_count += 1;
                }
                let size_expr = if self.check(TokenType::RBracket) {
                    None
                } else {
                    let expr = self.parse_assign();
                    Some(Box::new(expr))
                };
                self.consume(TokenType::RBracket, "预期 ']'");
                suffixes.push(DeclaratorSuffix::Array(size_expr));
            } else if allow_function_suffix && self.match_token(TokenType::LParen) {
                if !is_abstract {
                    guard.suffix_count += 1;
                }
                let (params, is_variadic) = self.parse_param_list();
                self.consume(TokenType::RParen, "预期 ')'");
                suffixes.push(DeclaratorSuffix::Function(params, is_variadic));
            } else {
                break;
            }
        }

        // 先应用后缀（它们绑定到标识符更紧）
        for suffix in suffixes {
            match suffix {
                DeclaratorSuffix::Array(size_expr) => {
                    node = DeclaratorNode::Array(Box::new(node), size_expr);
                }
                DeclaratorSuffix::Function(params, is_variadic) => {
                    node = DeclaratorNode::Function(Box::new(node), params, is_variadic);
                }
            }
        }

        // 再应用前缀（从外到内，但解释时从内到外）
        for prefix in prefixes {
            match prefix {
                Prefix::Pointer => node = DeclaratorNode::Pointer(Box::new(node)),
                Prefix::Reference(is_const) => node = DeclaratorNode::Reference(Box::new(node), is_const),
                Prefix::RValueRef => node = DeclaratorNode::RValueRef(Box::new(node)),
            }
        }

        if !is_abstract {
            guard.cross_count = node_cross_count(&node);
            if guard.cross_count > 4 {
                self.errors.push(ParseError {
                    message: "声明符过于复杂".to_string(),
                    line: self.current().line,
                    column: self.current().column,
                    code: 1007,
                });
                return (DeclaratorNode::Base, name);
            }
        }

        (node, name)
    }
    pub(crate) fn array_dim_info(size_expr: &Option<Box<Expr>>) -> (i32, bool, Option<Box<Expr>>) {
        match size_expr {
            None => (-1, false, None),
            Some(expr) => {
                if let Expr::Literal { value, .. } = expr.as_ref() {
                    (*value, false, None)
                } else if let Expr::Unary { op, operand, .. } = expr.as_ref() {
                    // U1#12：`int b[-5]` 的尺寸是 Unary(Neg, Literal(5))——
                    // 此前不识别被当 VLA（0, is_vla），负尺寸静默变成"运行时
                    // 负维度"。折叠一元负字面量，让 typeck 报"负数组大小"。
                    if matches!(op, UnaryOp::Neg) {
                        if let Expr::Literal { value, .. } = operand.as_ref() {
                            (-*value, false, None)
                        } else {
                            (0, true, Some(expr.clone()))
                        }
                    } else {
                        (0, true, Some(expr.clone()))
                    }
                } else {
                    (0, true, Some(expr.clone()))
                }
            }
        }
    }
    pub(crate) fn interpret_declarator_node(node: &DeclaratorNode, base_type: &Type) -> Type {
        match node {
            DeclaratorNode::Base => base_type.clone(),
            DeclaratorNode::Reference(inner, is_const) => {
                let inner_ty = Self::interpret_declarator_node(inner, base_type);
                Type::Reference {
                    base: Box::new(inner_ty),
                    is_const: *is_const,
                }
            }
            DeclaratorNode::RValueRef(inner) => {
                let inner_ty = Self::interpret_declarator_node(inner, base_type);
                Type::RValueRef { base: Box::new(inner_ty) }
            }
            DeclaratorNode::Pointer(inner) => {
                let inner_ty = Self::interpret_declarator_node(inner, base_type);
                match inner.as_ref() {
                    DeclaratorNode::Array(array_inner, size_expr) => {
                        let elem_ty = Self::interpret_declarator_node(array_inner, base_type);
                        let (size, is_vla, vla_dim) = Self::array_dim_info(size_expr);
                        Type::Array {
                            element: Box::new(Type::pointer_to(elem_ty)),
                            array_size: size,
                            dims: vec![size],
                            is_const: false,
                            is_vla,
                            vla_dims: if let Some(e) = vla_dim { vec![e] } else { vec![] },
                        }
                    }
                    _ => Type::pointer_to(inner_ty),
                }
            }
            DeclaratorNode::Array(inner, size_expr) => {
                let inner_ty = Self::interpret_declarator_node(inner, base_type);
                let (size, is_vla, vla_dim) = Self::array_dim_info(size_expr);
                match inner.as_ref() {
                    DeclaratorNode::Pointer(ptr_inner) => {
                        let elem_ty = Self::interpret_declarator_node(ptr_inner, base_type);
                        Type::Pointer {
                            pointee: Box::new(Type::Array {
                                element: Box::new(elem_ty),
                                array_size: size,
                                dims: vec![size],
                                is_const: false,
                                is_vla,
                                vla_dims: if let Some(e) = vla_dim { vec![e] } else { vec![] },
                            }),
                            is_const: false,
                        }
                    }
                    _ => {
                        let (element, mut inner_dims, inner_array_size, mut inner_is_vla, mut inner_vla_dims) =
                            if let Type::Array {
                                element,
                                dims,
                                array_size,
                                is_vla,
                                vla_dims,
                                ..
                            } = &inner_ty
                            {
                                (element.clone(), dims.clone(), *array_size, *is_vla, vla_dims.clone())
                            } else {
                                (Box::new(inner_ty.clone()), Vec::new(), 1, false, vec![])
                            };
                        inner_dims.push(size);
                        if is_vla {
                            inner_is_vla = true;
                            if let Some(e) = vla_dim {
                                inner_vla_dims.push(e);
                            }
                        }
                        let array_size = if size > 0 && inner_array_size > 0 {
                            size * inner_array_size
                        } else {
                            size
                        };
                        Type::Array {
                            element,
                            array_size,
                            dims: inner_dims,
                            is_const: false,
                            is_vla: inner_is_vla,
                            vla_dims: inner_vla_dims,
                        }
                    }
                }
            }
            DeclaratorNode::Function(inner, params, is_variadic) => {
                match inner.as_ref() {
                    DeclaratorNode::Pointer(ptr_inner) => {
                        match ptr_inner.as_ref() {
                            DeclaratorNode::Array(array_inner, size_expr) => {
                                // (*fp[N])(params) → function pointer array
                                let elem_ty = Self::interpret_declarator_node(array_inner, base_type);
                                let (size, is_vla, vla_dim) = Self::array_dim_info(size_expr);
                                Type::Array {
                                    element: Box::new(Type::Pointer {
                                        pointee: Box::new(Type::Function {
                                            return_type: Box::new(elem_ty),
                                            param_types: params.iter().map(|p| p.ty.clone()).collect(),
                                            is_const: false,
                                            is_variadic: *is_variadic,
                                        }),
                                        is_const: false,
                                    }),
                                    array_size: size,
                                    dims: vec![size],
                                    is_const: false,
                                    is_vla,
                                    vla_dims: if let Some(e) = vla_dim { vec![e] } else { vec![] },
                                }
                            }
                            _ => {
                                let func_ptr_type = Type::Pointer {
                                    pointee: Box::new(Type::Function {
                                        return_type: Box::new(base_type.clone()),
                                        param_types: params.iter().map(|p| p.ty.clone()).collect(),
                                        is_const: false,
                                        is_variadic: *is_variadic,
                                    }),
                                    is_const: false,
                                };
                                Self::interpret_declarator_node(ptr_inner, &func_ptr_type)
                            }
                        }
                    }
                    _ => {
                        let inner_ty = Self::interpret_declarator_node(inner, base_type);
                        Type::Pointer {
                            pointee: Box::new(Type::Function {
                                return_type: Box::new(inner_ty),
                                param_types: params.iter().map(|p| p.ty.clone()).collect(),
                                is_const: false,
                                is_variadic: *is_variadic,
                            }),
                            is_const: false,
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn parse_param_list(&mut self) -> (Vec<Param>, bool) {
        let mut params = Vec::new();
        let mut is_variadic = false;
        if self.check(TokenType::RParen) {
            return (params, is_variadic);
        }
        if self.check(TokenType::Void) && self.peek(1).ty == TokenType::RParen {
            self.advance();
            return (params, is_variadic);
        }
        loop {
            // C 变参参数列表末尾的 "..."
            if self.check(TokenType::Ellipsis) {
                self.advance();
                is_variadic = true;
                break;
            }
            let base_type = self.parse_base_type();
            let (pty, pname) = if self.check(TokenType::Comma) || self.check(TokenType::RParen) {
                // 无名参数（函数原型声明）：int foo(int);
                (base_type, String::new())
            } else if self.check(TokenType::Star) {
                // 前瞻：跳过所有 * 后看是否是 Comma/RParen，以支持 int foo(char *);
                let lookahead = self.look_ahead_skip_stars();
                if lookahead < self.tokens.len()
                    && (self.tokens[lookahead].ty == TokenType::Comma || self.tokens[lookahead].ty == TokenType::RParen)
                {
                    let mut ty = base_type;
                    while self.match_token(TokenType::Star) {
                        ty = Type::pointer_to(ty);
                    }
                    (ty, String::new())
                } else {
                    self.parse_declarator(&base_type)
                }
            } else {
                self.parse_declarator(&base_type)
            };
            // In C, array parameters in function declarations decay to pointers.
            // For int a[5] -> int*; for int a[3][3] -> int (*a)[3].
            let pty = if pty.is_array() {
                let is_const = pty.is_const();
                Type::Pointer {
                    pointee: Box::new(pty.subscript_type()),
                    is_const,
                }
            } else {
                pty
            };
            let default_expr = if self.match_token(TokenType::Assign) {
                // 默认参数使用赋值表达式级别，避免逗号运算符把后续参数吞掉。
                Some(self.parse_assign())
            } else {
                None
            };
            params.push(Param {
                ty: pty,
                name: pname,
                loc: SourceLoc {
                    line: self.current().line,
                    column: self.current().column,
                    file_id: 0,
                },
                default: default_expr,
            });
            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        (params, is_variadic)
    }
    /// Parse optional constructor member initializer list: `: field1(expr1), field2(expr2)`
    pub(crate) fn parse_ctor_init_list(&mut self) -> Vec<(String, Expr)> {
        let mut init_list = Vec::new();
        if !self.check(TokenType::Colon) {
            return init_list;
        }
        self.advance(); // consume ':'
        loop {
            let field_tok = self.advance();
            let field_name = field_tok.text.clone();
            self.consume(TokenType::LParen, "预期 '('");
            let init_expr = self.parse_expression();
            self.consume(TokenType::RParen, "预期 ')'");
            init_list.push((field_name, init_expr));
            if !self.check(TokenType::Comma) {
                break;
            }
            self.advance(); // consume ','
        }
        init_list
    }
    /// Merge constructor initializer list into the body by prepending
    /// `this->field = expr;` assignments at the start of the block.
    pub(crate) fn merge_ctor_init_into_body(init_list: Vec<(String, Expr)>, body: Option<Stmt>) -> Option<Stmt> {
        if init_list.is_empty() {
            return body;
        }
        match body {
            Some(Stmt::Block { mut stmts, loc }) => {
                let mut new_stmts = Vec::new();
                for (field, expr) in init_list {
                    let expr_loc = *expr.loc();
                    let assign_expr = Expr::Assign {
                        op: AssignOp::Assign,
                        left: Box::new(Expr::Member {
                            object: Box::new(Expr::This {
                                loc: expr_loc,
                                ty: Type::default(),
                            }),
                            member: field,
                            loc: expr_loc,
                            ty: Type::default(),
                        }),
                        right: Box::new(expr),
                        loc: expr_loc,
                        ty: Type::default(),
                    };
                    new_stmts.push(Stmt::Expr {
                        expr: assign_expr,
                        loc: expr_loc,
                    });
                }
                new_stmts.append(&mut stmts);
                Some(Stmt::Block { stmts: new_stmts, loc })
            }
            other => other,
        }
    }
}
