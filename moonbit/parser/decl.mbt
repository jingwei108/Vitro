///|
/// 声明族（照搬 Rust `decl.rs`）：struct/union/typedef/enum/函数声明 +
/// C++ class/template 声明 + enum 常量求值器。
///
/// `eval_enum_const` 的 checked 语义照搬（U1#9：MoonBit `Int` 静默回绕
/// 与 Rust release UB 同危险——显式溢出检查返回 None 走"非常量表达式"
/// 诊断）；sizeof 折叠经 `@ast.compute_type_size`（空表场景）。
fn Parser::parse_struct_body(
  self : Parser,
  name : String,
  loc : @source.SourceLoc,
  depth : Int,
) -> @ast.StructDecl {
  ignore(self.consume(LBrace, "预期 '{'"))
  let fields : Array[@ast.StructField] = []
  while !self.check(RBrace) && !self.is_at_end() {
    let field_checkpoint = self.pos
    let base_type = self.parse_base_type(depth)
    let d = self.parse_declarator(base_type, depth)
    if self.pos == field_checkpoint {
      ignore(self.advance())
      break
    }
    fields.push({ ty: d.0, name: d.1, })
    while self.match_token(Comma) {
      let ex = self.parse_declarator(base_type, depth)
      fields.push({ ty: ex.0, name: ex.1, })
    }
    ignore(self.consume(Semicolon, "预期 ';'"))
  }
  ignore(self.consume(RBrace, "预期 '}'"))
  { loc, name, fields, }
}

///|
fn Parser::parse_struct_decl(self : Parser, depth : Int) -> @ast.StructDecl {
  ignore(self.consume(Struct, "预期 'struct'"))
  let name_tok = if self.check(Identifier) {
    self.advance()
  } else {
    let cur = self.current()
    {
      ty: Identifier,
      text: "__anon_struct_\{self.pos}",
      loc: { line: cur.loc.line, column: cur.loc.column, file_id: 0, },
      byte_off: -1,
      real_line: -1,
    }
  }
  let decl = self.parse_struct_body(
    name_tok.text,
    @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column),
    depth,
  )
  ignore(self.consume(Semicolon, "结构体声明后预期 ';'"))
  if self.is_cpp_mode && !decl.name.has_prefix("__anon_struct_") {
    self.typedef_names.set(decl.name, @ast.Type::struct_type(decl.name))
  }
  decl
}

///|
fn Parser::parse_union_decl(self : Parser, depth : Int) -> @ast.StructDecl {
  ignore(self.consume(Union, "预期 'union'"))
  let name_tok = if self.check(Identifier) {
    self.advance()
  } else {
    let cur = self.current()
    {
      ty: Identifier,
      text: "__anon_union_\{self.pos}",
      loc: { line: cur.loc.line, column: cur.loc.column, file_id: 0, },
      byte_off: -1,
      real_line: -1,
    }
  }
  let decl = self.parse_struct_body(
    name_tok.text,
    @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column),
    depth,
  )
  ignore(self.consume(Semicolon, "联合体声明后预期 ';'"))
  decl
}

// =========================================================================
// C++ Class Declaration (Phase 31)
// =========================================================================

///|
fn Parser::parse_class_decl(self : Parser, depth : Int) -> @ast.ClassDecl {
  self.parse_class_decl_inner(false, depth)
}

///|
fn Parser::parse_class_decl_inner(
  self : Parser,
  is_struct : Bool,
  depth : Int,
) -> @ast.ClassDecl {
  let loc = self.cur_loc()
  if is_struct {
    ignore(self.consume(Struct, "预期 'struct'"))
  } else {
    ignore(self.consume(Class, "预期 'class'"))
  }
  let short_name = self.consume(Identifier, "预期类名").text
  let name = if self.current_class.is_empty() {
    short_name
  } else {
    let mut full = ""
    let mut first = true
    for c in self.current_class {
      if first {
        full = c
        first = false
      } else {
        full = full + "__" + c
      }
    }
    full + "__" + short_name
  }
  self.current_class.push(name)
  let mut base : String? = None
  if self.match_token(Colon) {
    // 支持 ': public Base'（public 可选）
    if self.check(Public) || self.check(Private) || self.check(Protected) {
      ignore(self.advance())
    }
    base = Some(self.consume(Identifier, "预期基类名").text)
  }
  ignore(self.consume(LBrace, "预期 '{'"))
  let members : Array[@ast.ClassMember] = []
  // struct 默认 public，class 默认 private
  let mut current_access : @ast.AccessSpec = if is_struct {
    @ast.AccessSpec::Public
  } else {
    @ast.AccessSpec::Private
  }
  while !self.check(RBrace) && !self.is_at_end() {
    // 访问说明符
    if self.check(Public) || self.check(Private) || self.check(Protected) {
      let access = match self.current().ty {
        Public => @ast.AccessSpec::Public
        Private => @ast.AccessSpec::Private
        _ => @ast.AccessSpec::Protected
      }
      ignore(self.advance())
      ignore(self.consume(Colon, "访问说明符后预期 ':'"))
      current_access = access
      continue
    }
    // 跳过 inline / virtual / explicit / static 修饰符
    let mut is_virtual = false
    let mut is_explicit = false
    let mut is_static = false
    while true {
      if self.check(Inline) || self.check(Virtual) || self.check(Explicit) {
        if self.check(Virtual) {
          is_virtual = true
        }
        if self.check(Explicit) {
          is_explicit = true
        }
        ignore(self.advance())
      } else if self.is_cpp_mode && self.is_static_token() {
        is_static = true
        ignore(self.advance())
      } else {
        break
      }
    }
    // 嵌套 struct / class / union / enum
    if self.check(Struct) {
      let decl = self.parse_struct_decl(depth)
      members.push(@ast.ClassMember::NestedStruct(decl, current_access))
      continue
    }
    if self.is_cpp_mode && self.check(Class) {
      let decl = self.parse_class_decl(depth)
      members.push(@ast.ClassMember::NestedClass(decl, current_access))
      continue
    }
    if self.check(Union) {
      let decl = self.parse_union_decl(depth)
      members.push(@ast.ClassMember::NestedStruct(decl, current_access))
      continue
    }
    // 析构函数 ~Name()
    if self.check(BitNot) &&
      self.peek(1).ty == Identifier &&
      self.peek(2).ty == LParen {
      ignore(self.advance()) // ~
      let _dtor_name = self.advance().text
      ignore(_dtor_name)
      ignore(self.consume(LParen, "预期 '('"))
      ignore(self.consume(RParen, "预期 ')'"))
      let body : @ast.Stmt? = if self.check(LBrace) {
        Some(self.parse_statement(depth))
      } else {
        ignore(self.consume(Semicolon, "析构函数声明后预期 ';'"))
        None
      }
      members.push(
        @ast.ClassMember::Destructor(body, current_access, is_virtual),
      )
      continue
    }
    // 构造函数检查（无返回类型）。嵌套类可用短名定义构造。
    let ctor_name_matches = self.check(Identifier) &&
      (self.current().text == name || self.current().text == short_name) &&
      self.peek(1).ty == LParen
    if ctor_name_matches {
      ignore(self.advance()) // class name
      ignore(self.consume(LParen, "预期 '('"))
      let pl = self.parse_param_list(depth)
      ignore(self.consume(RParen, "预期 ')'"))
      let init_list = self.parse_ctor_init_list(depth)
      let body : @ast.Stmt? = if self.check(LBrace) {
        let block = self.parse_statement(depth)
        merge_ctor_init_into_body(init_list, Some(block))
      } else {
        ignore(self.consume(Semicolon, "构造函数声明后预期 ';'"))
        None
      }
      members.push(
        @ast.ClassMember::Constructor(
          pl.0,
          body,
          false,
          current_access,
          is_explicit,
        ),
      )
      continue
    }
    // 构造函数 / 方法 / 字段 需要类型前瞻
    if !self.is_type_token() && !self.check(Identifier) {
      // 未知内容，报错并跳过
      let cur = self.current()
      self.errors.push({
        message: "类成员声明中预期类型或方法名，找到: \{cur.text}",
        line: cur.loc.line,
        column: cur.loc.column,
        code: @diag.ErrorCode::E2005_ExpectedSemicolon.code(),
      })
      ignore(self.advance())
      continue
    }
    let checkpoint = self.save()
    let _member_base_type = self.parse_base_type(depth)
    ignore(_member_base_type)
    let lookahead = self.look_ahead_skip_stars()
    if lookahead < self.tokens.length() &&
      self.tokens[lookahead].ty == Identifier &&
      lookahead + 1 < self.tokens.length() &&
      self.tokens[lookahead + 1].ty == LParen {
      // 方法或构造函数
      self.restore(checkpoint)
      let ret_type = self.parse_base_type(depth)
      let mut ptr_depth = 0
      while self.match_token(Star) {
        ptr_depth += 1
      }
      let mut final_ret = ret_type
      for _ in 0..<ptr_depth {
        final_ret = @ast.Type::pointer_to(final_ret)
      }
      // C++ 引用 / 右值引用返回类型
      if self.is_cpp_mode {
        if self.match_token(Ampersand) {
          let is_const = self.match_token(Const)
          final_ret = @ast.Type::Reference(final_ret, is_const)
        } else if self.match_token(AndAnd) {
          final_ret = @ast.Type::RValueRef(final_ret)
        }
      }
      let method_name = self.advance().text
      ignore(self.consume(LParen, "预期 '('"))
      let pl = self.parse_param_list(depth)
      ignore(self.consume(RParen, "预期 ')'"))
      // 检查 const（仅对方法有效）
      let is_const = self.match_token(Const)
      // 检查 override
      if self.check(Override) {
        ignore(self.advance())
      }
      let is_ctor_like = method_name == name &&
        ptr_depth == 0 &&
        (match final_ret {
          Void(_) | Int(_, _) => true
          _ => false
        })
      if is_ctor_like {
        // 构造函数（简化：与类同名且无返回类型修饰）
        let init_list = self.parse_ctor_init_list(depth)
        let body : @ast.Stmt? = if self.check(LBrace) {
          let block = self.parse_statement(depth)
          merge_ctor_init_into_body(init_list, Some(block))
        } else {
          ignore(self.consume(Semicolon, "构造函数声明后预期 ';'"))
          None
        }
        members.push(
          @ast.ClassMember::Constructor(
            pl.0,
            body,
            false,
            current_access,
            is_explicit,
          ),
        )
      } else {
        let body : @ast.Stmt? = if self.check(LBrace) {
          Some(self.parse_statement(depth))
        } else {
          ignore(self.consume(Semicolon, "方法声明后预期 ';'"))
          None
        }
        members.push(
          @ast.ClassMember::Method(
            method_name,
            final_ret,
            pl.0,
            body,
            is_virtual,
            current_access,
            is_static,
            is_const,
          ),
        )
      }
    } else {
      // 字段声明（支持逗号分隔多字段，如 `int head, tail;`）
      self.restore(checkpoint)
      let field_type = self.parse_base_type(depth)
      let d = self.parse_declarator(field_type, depth)
      members.push(@ast.ClassMember::Field(d.1, d.0, current_access, is_static))
      while self.match_token(Comma) {
        let ex = self.parse_declarator(field_type, depth)
        members.push(
          @ast.ClassMember::Field(ex.1, ex.0, current_access, is_static),
        )
      }
      ignore(self.consume(Semicolon, "字段声明后预期 ';'"))
    }
  }
  ignore(self.consume(RBrace, "预期 '}'"))
  ignore(self.consume(Semicolon, "类声明后预期 ';'"))
  ignore(self.current_class.pop())
  { loc, name, base, members, vtable: None, }
}

// =========================================================================
// C++ Template Declaration (Phase 31)
// =========================================================================

///|
/// 判断当前 `template` 开头是否为显式实例化声明：
/// `template class Name<args>;`（无模板参数列表）。
fn Parser::is_template_explicit_instantiation(self : Parser) -> Bool {
  let mut i = self.pos
  if i >= self.tokens.length() || self.tokens[i].ty != Template {
    return false
  }
  i += 1
  if i >= self.tokens.length() ||
    (self.tokens[i].ty != Class && self.tokens[i].ty != Struct) {
    return false
  }
  i += 1
  if i >= self.tokens.length() || self.tokens[i].ty != Identifier {
    return false
  }
  i += 1
  if i >= self.tokens.length() || self.tokens[i].ty != Lt {
    return false
  }
  i += 1
  let mut depth = 1
  while i < self.tokens.length() && depth > 0 {
    match self.tokens[i].ty {
      Lt => depth += 1
      Gt => {
        depth -= 1
        if depth == 0 {
          break
        }
      }
      _ => ()
    }
    i += 1
  }
  if depth != 0 {
    return false
  }
  i + 1 < self.tokens.length() && self.tokens[i + 1].ty == Semicolon
}

///|
fn Parser::parse_template_instantiation(
  self : Parser,
  depth : Int,
) -> @ast.TemplateInstantiation {
  let loc = self.cur_loc()
  ignore(self.consume(Template, "预期 'template'"))
  ignore(self.consume(Class, "预期 'class'"))
  let base = self.consume(Identifier, "预期模板类名").text
  ignore(self.consume(Lt, "预期 '<'"))
  let args : Array[@ast.TemplateArg] = []
  while !self.check(Gt) && !self.is_at_end() {
    let arg = if self.is_type_token() {
      @ast.TemplateArg::TArg(self.parse_base_type(depth))
    } else {
      @ast.TemplateArg::TExpr(self.parse_template_arg_expr(depth))
    }
    args.push(arg)
    if !self.match_token(Comma) {
      break
    }
  }
  ignore(self.consume(Gt, "预期 '>'"))
  ignore(self.consume(Semicolon, "预期 ';'"))
  { loc, base, args, }
}

///|
fn Parser::parse_template_decl(self : Parser, depth : Int) -> @ast.TemplateDecl {
  let loc = self.cur_loc()
  ignore(self.consume(Template, "预期 'template'"))
  ignore(self.consume(Lt, "预期 '<'"))
  let params : Array[@ast.TemplateParam] = []
  while !self.check(Gt) && !self.is_at_end() {
    if self.check(Typename) || self.check(Class) {
      // 类型模板参数：typename T / class T
      ignore(self.advance())
      let param_name = self.consume(Identifier, "预期模板参数名").text
      params.push(@ast.TemplateParam::TypeParam(param_name, loc))
      // 将模板参数注册为类型名，使其在函数/类体中可被识别
      //（Class 类型占位，TypeChecker 单态化时识别模板参数）
      self.typedef_names.set(param_name, @ast.Type::Class(param_name, false))
    } else if self.is_type_token() {
      // 非类型模板参数：int N, unsigned M, ...
      let ty = self.parse_base_type(depth)
      let param_name = self.consume(Identifier, "预期模板参数名").text
      params.push(@ast.TemplateParam::NonType(param_name, ty, loc))
    } else {
      let cur = self.current()
      self.errors.push({
        message: "预期 'typename'、'class' 或类型名",
        line: cur.loc.line,
        column: cur.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      break
    }
    if !self.match_token(Comma) {
      break
    }
  }
  ignore(self.consume(Gt, "预期 '>'"))
  // 模板后面跟着函数、类或结构体声明
  let decl = if self.check(Class) || self.check(Struct) {
    // 在进入类体前就把类模板名加入 template_names，使类体内
    // `unique_ptr<T>&` 自引用参数正确解析为 TemplateId
    let is_struct = self.check(Struct)
    let checkpoint = self.save()
    ignore(self.advance()) // class / struct
    let class_name = if self.check(Identifier) {
      self.advance().text
    } else {
      ""
    }
    self.restore(checkpoint) // 回退到 class/struct 关键字
    if class_name != "" {
      self.template_names.add(class_name)
    }
    let class_decl = if is_struct {
      let cd = self.parse_class_decl_inner(true, depth)
      self.template_names.add(cd.name)
      cd
    } else {
      let cd = self.parse_class_decl(depth)
      self.template_names.add(cd.name)
      cd
    }
    @ast.Templateable::TClass(class_decl)
  } else {
    // C++ 允许 template<class T> static void foo(...)
    let mut is_template_static = false
    if self.is_cpp_mode && self.is_static_token() {
      is_template_static = true
      ignore(self.advance()) // consume 'static'
    }
    @ast.Templateable::TFunc(
      self.parse_func_decl(is_template_static, false, depth),
    )
  }
  { loc, params, decl, }
}

///|
fn Parser::parse_typedef_struct_decl(
  self : Parser,
  program : @ast.ProgramNode,
  depth : Int,
) -> Unit {
  let cur = self.current()
  let loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
  ignore(self.advance()) // typedef
  ignore(self.advance()) // struct
  let name = if self.check(Identifier) {
    self.advance().text
  } else {
    "__anon_struct_\{self.pos}"
  }
  let decl = self.parse_struct_body(name, loc, depth)
  let alias_tok = self.consume(Identifier, "typedef 后预期标识符名称")
  ignore(self.consume(Semicolon, "typedef 后预期 ';'"))
  program.structs.push(decl)
  self.typedef_names.set(alias_tok.text, @ast.Type::struct_type(name))
}

///|
fn Parser::parse_typedef_enum_decl(
  self : Parser,
  program : @ast.ProgramNode,
  depth : Int,
) -> Unit {
  let cur0 = self.current()
  let loc = @source.SourceLoc::new(cur0.loc.line, cur0.loc.column)
  ignore(self.advance()) // typedef
  ignore(self.advance()) // enum
  let mut enum_name = ""
  if self.check(Identifier) {
    enum_name = self.current().text
    ignore(self.advance())
  }
  // C23 enum : T 底层类型：枚举成员与类型名都按 T 处理
  let mut underlying = @ast.Type::int()
  if self.match_token(Colon) {
    underlying = self.parse_base_type(depth)
    match underlying {
      Int(_, _) | Char(_, _) | LongLong(_, _) => ()
      _ => {
        self.errors.push({
          message: "enum 底层类型必须是整数类型（int / char / long long 及其 unsigned 变体）",
          line: loc.line,
          column: loc.column,
          code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
        })
        underlying = @ast.Type::int()
      }
    }
  }
  ignore(self.consume(LBrace, "enum 后预期 '{'"))
  let mut next_value : Int64 = 0
  while !self.check(RBrace) && !self.is_at_end() {
    let member_tok = self.consume(Identifier, "enum 成员预期标识符")
    if self.match_token(Assign) {
      let val_expr = self.parse_assign(depth)
      // F-P0-3：非裸字面量初始化器经编译期常量求值
      match eval_enum_const(val_expr) {
        Some(v) => next_value = v
        None =>
          self.errors.push({
            message: "enum 初始化器必须是编译期整数常量（支持负数与四则/位运算）",
            line: member_tok.loc.line,
            column: member_tok.loc.column,
            code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
          })
      }
    }
    push_enum_member_global(program, loc, member_tok, next_value, underlying)
    next_value += 1
    if !self.match_token(Comma) {
      break
    }
  }
  ignore(self.consume(RBrace, "enum 成员后预期 '}'"))
  let alias_tok = self.consume(Identifier, "typedef 后预期标识符名称")
  ignore(self.consume(Semicolon, "typedef 后预期 ';'"))
  self.typedef_names.set(alias_tok.text, @ast.Type::int())
  if enum_name != "" {
    self.typedef_names.set(enum_name, underlying)
  }
}

///|
/// enum 成员落全局（E1：成员常量超 i32（enum Big : long long）按
/// LongLiteral 存储——parse_typedef_enum_decl 与 parse_enum_decl 共用）。
fn push_enum_member_global(
  program : @ast.ProgramNode,
  loc : @source.SourceLoc,
  member_tok : @token.Token,
  next_value : Int64,
  underlying : @ast.Type,
) -> Unit {
  let in_i32 = next_value >= -2147483648 && next_value <= 2147483647
  let init : @ast.Expr = if in_i32 {
    @ast.Expr::Literal(
      i64_to_i32_bits(next_value),
      @source.SourceLoc::new(member_tok.loc.line, member_tok.loc.column),
      @ast.Type::int(),
    )
  } else {
    @ast.Expr::LongLiteral(
      next_value,
      @source.SourceLoc::new(member_tok.loc.line, member_tok.loc.column),
      @ast.Type::long_long(),
    )
  }
  program.globals.push({
    loc,
    ty: if in_i32 {
      @ast.Type::int()
    } else {
      underlying
    },
    name: member_tok.text,
    init: Some(init),
    is_static: false,
    is_extern: false,
    source_file: "",
  })
}

///|
fn Parser::parse_func_decl(
  self : Parser,
  is_static : Bool,
  is_extern : Bool,
  depth : Int,
) -> @ast.FuncDecl {
  let base_type = self.parse_base_type(depth)
  let mut ret_type = base_type
  while self.match_token(Star) {
    ret_type = @ast.Type::pointer_to(ret_type)
  }
  if self.is_cpp_mode {
    if self.match_token(Ampersand) {
      let is_const = self.match_token(Const)
      ret_type = @ast.Type::Reference(ret_type, is_const)
    } else if self.match_token(AndAnd) {
      ret_type = @ast.Type::RValueRef(ret_type)
    }
  }
  let name_tok = self.consume(Identifier, "预期函数名称")
  let mut func_name = name_tok.text
  // C++ 类外成员函数定义: Bar::set → Bar__set（模板形参仅消耗 token）
  if self.is_cpp_mode {
    if self.check(Lt) {
      // 消耗可选的模板实参列表，例如 Box<T>::set 中的 <T>
      ignore(self.advance()) // '<'
      while !self.check(Gt) && !self.is_at_end() {
        ignore(self.parse_base_type(depth))
        if !self.match_token(Comma) {
          break
        }
      }
      ignore(self.consume(Gt, "预期 '>'"))
    }
    if self.match_token(ColonColon) {
      let method_tok = self.consume(Identifier, "预期方法名")
      if method_tok.text == func_name {
        // 构造函数类外定义: Counter::Counter() { ... }
        func_name = "__ctor__\{func_name}"
        ret_type = @ast.Type::void_type()
      } else {
        func_name = "\{func_name}__\{method_tok.text}"
      }
    }
  }
  ignore(self.consume(LParen, "预期 '('"))
  let pl = self.parse_param_list(depth)
  ignore(self.consume(RParen, "预期 ')'"))
  if self.is_cpp_mode {
    ignore(self.match_token(Const))
  }
  let body : @ast.Stmt? = if self.check(LBrace) {
    Some(self.parse_block(depth))
  } else {
    ignore(self.consume(Semicolon, "函数声明后预期 ';' 或 '{'"))
    None
  }
  {
    loc: @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column),
    return_type: ret_type,
    name: func_name,
    params: pl.0,
    body,
    is_static,
    is_extern,
    source_file: "",
    is_variadic: pl.1,
  }
}

///|
fn Parser::parse_typedef(self : Parser, depth : Int) -> Unit {
  ignore(self.advance())
  let base_type = self.parse_base_type(depth)
  let d = self.parse_declarator(base_type, depth)
  ignore(self.consume(Semicolon, "typedef 后预期 ';'"))
  self.typedef_names.set(d.1, d.0)
}

///|
fn Parser::parse_enum_decl(
  self : Parser,
  program : @ast.ProgramNode,
  depth : Int,
) -> Unit {
  let cur0 = self.current()
  let loc = @source.SourceLoc::new(cur0.loc.line, cur0.loc.column)
  ignore(self.advance())
  let mut enum_name = ""
  if self.check(Identifier) {
    enum_name = self.current().text
    ignore(self.advance())
  }
  // C23 enum : T 底层类型：枚举成员与类型名都按 T 处理
  let mut underlying = @ast.Type::int()
  if self.match_token(Colon) {
    underlying = self.parse_base_type(depth)
    match underlying {
      Int(_, _) | Char(_, _) | LongLong(_, _) => ()
      _ => {
        self.errors.push({
          message: "enum 底层类型必须是整数类型（int / char / long long 及其 unsigned 变体）",
          line: loc.line,
          column: loc.column,
          code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
        })
        underlying = @ast.Type::int()
      }
    }
  }
  ignore(self.consume(LBrace, "enum 后预期 '{'"))
  let mut next_value : Int64 = 0
  while !self.check(RBrace) && !self.is_at_end() {
    let member_tok = self.consume(Identifier, "enum 成员预期标识符")
    if self.match_token(Assign) {
      let val_expr = self.parse_assign(depth)
      match eval_enum_const(val_expr) {
        Some(v) => next_value = v
        None =>
          self.errors.push({
            message: "enum 初始化器必须是编译期整数常量（支持负数与四则/位运算）",
            line: member_tok.loc.line,
            column: member_tok.loc.column,
            code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
          })
      }
    }
    push_enum_member_global(program, loc, member_tok, next_value, underlying)
    next_value += 1
    if !self.match_token(Comma) {
      break
    }
  }
  ignore(self.consume(RBrace, "enum 成员后预期 '}'"))
  ignore(self.consume(Semicolon, "enum 声明后预期 ';'"))
  if enum_name != "" {
    self.typedef_names.set(enum_name, underlying)
  }
}

///|
/// F-P0-3：enum 初始化器的编译期常量求值。
/// 支持整数字面量、一元负号/按位取反/逻辑非、四则与位运算、比较的常量
/// 折叠、sizeof（内建类型精确求值——空 struct/class 表，struct 布局
/// 参与常量求值属子集边界返回 None）。
///
/// checked 语义（U1#9）：MoonBit Int/Int64 静默回绕（`1 << 1000 = 256`
/// 实测），与 Rust debug panic / release UB 同危险——溢出显式返回 None
/// 走"非常量表达式"诊断。除零为 MoonBit 运行时 trap，常量求值器显式
/// 零检查。
fn eval_enum_const(expr : @ast.Expr) -> Int64? {
  match expr {
    Literal(value, _, _) => Some(value.to_int64())
    LongLiteral(value, _, _) => Some(value)
    // E3：static_assert 的 sizeof（教学最高频用法）
    Sizeof(Some(t), _, _, _) => {
      let empty_structs : Map[String, Array[@ast.StructField]] = Map([])
      let empty_unions : Map[String, Array[@ast.StructField]] = Map([])
      let empty_cls : Map[String, Int] = Map([])
      let sz = @ast.compute_type_size(t, empty_structs, empty_unions, empty_cls)
      if sz > 0 {
        Some(sz.to_int64())
      } else {
        None
      }
    }
    Unary(op, operand, _, _) => {
      // Option 解包（MoonBit 的 `?` 只配 raise/Result——这里显式 match）
      let v = match eval_enum_const(operand) {
        Some(v) => v
        None => return None
      }
      match op {
        // U1#9：取负 checked——Int64::MIN 取负回绕成自身，溢出 None
        @ast.UnaryOp::Neg => if v == I64_MIN { None } else { Some(-v) }
        @ast.UnaryOp::BitNot => Some(v.lnot())
        @ast.UnaryOp::Not => if v == 0 { Some(1) } else { Some(0) }
        _ => None
      }
    }
    Binary(op, left, right, _, _) => {
      let a = match eval_enum_const(left) {
        Some(v) => v
        None => return None
      }
      let b = match eval_enum_const(right) {
        Some(v) => v
        None => return None
      }
      match op {
        @ast.BinaryOp::Add => checked_add64(a, b)
        @ast.BinaryOp::Sub => checked_sub64(a, b)
        @ast.BinaryOp::Mul => checked_mul64(a, b)
        @ast.BinaryOp::Div =>
          if b == 0 || (a == I64_MIN && b == -1) {
            None
          } else {
            Some(a / b)
          }
        @ast.BinaryOp::Mod =>
          if b == 0 || (a == I64_MIN && b == -1) {
            None
          } else {
            Some(a % b)
          }
        // U1#9：移位 checked——仅查移位量（Rust checked_shl 语义：b ≥ 64
        // 或负移位非法 None；左移丢位不算溢出）
        @ast.BinaryOp::Shl =>
          if b < 0 || b >= 64 {
            None
          } else {
            Some(a << b.to_int())
          }
        @ast.BinaryOp::Shr =>
          if b < 0 || b >= 64 {
            None
          } else {
            Some(a >> b.to_int())
          }
        @ast.BinaryOp::BitAnd => Some(a & b)
        @ast.BinaryOp::BitOr => Some(a | b)
        @ast.BinaryOp::BitXor => Some(a ^ b)
        @ast.BinaryOp::Eq => if a == b { Some(1) } else { Some(0) }
        @ast.BinaryOp::Ne => if a != b { Some(1) } else { Some(0) }
        @ast.BinaryOp::Lt => if a < b { Some(1) } else { Some(0) }
        @ast.BinaryOp::Le => if a <= b { Some(1) } else { Some(0) }
        @ast.BinaryOp::Gt => if a > b { Some(1) } else { Some(0) }
        @ast.BinaryOp::Ge => if a >= b { Some(1) } else { Some(0) }
        _ => None
      }
    }
    _ => None
  }
}

///|
const I64_MIN : Int64 = -9223372036854775807 - 1

///|
/// Rust `as i32` 的 i64→i32 位截断（低 32 位符号解释）。
fn i64_to_i32_bits(v : Int64) -> Int {
  let low = v & 0xFFFFFFFF
  if low >= 2147483648 {
    (low - 4294967296).to_int()
  } else {
    low.to_int()
  }
}

///|
/// checked 四则（Int64 静默回绕 → 溢出 None；Rust checked_add 同构）
fn checked_add64(a : Int64, b : Int64) -> Int64? {
  let r = a + b
  // 回绕判定：结果符号与操作数相悖
  if (a > 0 && b > 0 && r < 0) || (a < 0 && b < 0 && r >= 0) {
    None
  } else {
    Some(r)
  }
}

///|
fn checked_sub64(a : Int64, b : Int64) -> Int64? {
  // a - b 溢出仅当 b 为 MIN（减 MIN = 加 MAX+1）
  if b == I64_MIN {
    if a >= 0 {
      None
    } else {
      Some(a - b)
    }
  } else {
    checked_add64(a, -b)
  }
}

///|
fn checked_mul64(a : Int64, b : Int64) -> Int64? {
  if a == 0 || b == 0 {
    return Some(0)
  }
  // MIN 特判先行：MIN×1 = MIN 合法；MIN×(-1) 回绕（数学 2^63 不可表示）；
  // 其余含 MIN 的乘积 |结果| > MAX 全溢出。特判同时防 r/b 的 MIN/-1 trap
  if a == I64_MIN || b == I64_MIN {
    if (a == I64_MIN && b == 1) || (a == 1 && b == I64_MIN) {
      return Some(I64_MIN)
    }
    return None
  }
  let r = a * b
  // 回绕判定：除回去不一致即溢出（r / b != a）
  if r / b != a {
    None
  } else {
    Some(r)
  }
}
