///|
/// 语句族（照搬 Rust `stmt.rs`）：块/控制流/变量声明语句 + `_Static_assert`
/// 真求值 + `[[属性]]` 跳过。
///
/// 壳挂点 `parse_statement`（块/控制流互递归——6 万层 `{{{{` 曾栈溢出）。

///|
/// `_Static_assert` / `static_assert`（C11/C23，E3：真求值）。
/// 条件经编译期常量求值（与 enum 初始化器同一求值器）：为假即编译错误
///（携带消息），为真不产生任何代码。
fn Parser::parse_static_assert(self : Parser, depth : Int) -> Unit {
  ignore(self.advance()) // _Static_assert / static_assert
  ignore(self.consume(LParen, "static_assert 后预期 '('"))
  // parse_assign：不越过顶层逗号（逗号后是消息字符串，非逗号表达式）
  let cond = self.parse_assign(depth)
  let mut message = ""
  if self.match_token(Comma) {
    if self.match_token(String) {
      message = self.previous().text
    } else {
      // C23 允许逗号后无消息？标准要求字符串字面量——按缺失报错
      let prev = self.previous()
      self.errors.push({
        message: "static_assert 的第二个参数预期字符串字面量",
        line: prev.loc.line,
        column: prev.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
    }
  }
  ignore(self.consume(RParen, "static_assert 预期 ')'"))
  ignore(self.consume(Semicolon, "static_assert 预期 ';'"))
  match eval_enum_const(cond) {
    Some(v) =>
      if v != 0 {
        // 断言成立
      } else {
        let msg_tail = if message.is_empty() { "" } else { "：\{message}" }
        let prev = self.previous()
        self.errors.push({
          message: "static_assert 断言失败\{msg_tail}",
          line: prev.loc.line,
          column: prev.loc.column,
          code: @diag.ErrorCode::E1020_StaticAssertFailed.code(),
        })
      }
    None => {
      let prev = self.previous()
      self.errors.push({
        message: "static_assert 的条件必须是编译期整数常量表达式",
        line: prev.loc.line,
        column: prev.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
    }
  }
}

///|
/// E3：跳过 `[[ ... ]]` 属性子句（解析+忽略，教学子集不实现任何属性语义；
/// 与 Clang 默认模式"未知属性警告后忽略"的可见行为一致）。
fn Parser::skip_attributes(self : Parser) -> Unit {
  while self.check(LBracket) && self.peek(1).ty == LBracket {
    ignore(self.advance()) // [
    ignore(self.advance()) // [
    // 属性体消费到 `]]`（教学子集不支持属性参数内嵌套方括号，spec 记录）
    while !self.is_at_end() {
      if self.check(RBracket) && self.peek(1).ty == RBracket {
        ignore(self.advance()) // ]
        ignore(self.advance()) // ]
        break
      }
      ignore(self.advance())
    }
  }
}

///|
/// 语句壳挂点（块/控制流互递归深度防护）。
fn Parser::parse_statement(self : Parser, depth : Int) -> @ast.Stmt {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("语句")
    return @ast.Stmt::Block([], @source.SourceLoc::default())
  }
  self.parse_statement_inner(depth + 1)
}

///|
fn Parser::parse_statement_inner(self : Parser, depth : Int) -> @ast.Stmt {
  // E3：[[属性]] 前缀——解析并忽略
  if self.check(LBracket) && self.peek(1).ty == LBracket {
    self.skip_attributes()
  }
  let cur = self.current()
  match cur.ty {
    Semicolon => {
      let loc = self.cur_loc()
      ignore(self.advance())
      @ast.Stmt::Block([], loc)
    }
    LBrace => self.parse_block(depth)
    If => self.parse_if_stmt(depth)
    While => self.parse_while_stmt(depth)
    Do => self.parse_do_while_stmt(depth)
    For => self.parse_for_stmt(depth)
    Return => self.parse_return_stmt(depth)
    Break => self.parse_break_stmt()
    Continue => self.parse_continue_stmt()
    Goto => self.parse_goto_stmt()
    Switch => self.parse_switch_stmt(depth)
    Case | Default => self.parse_case_stmt(depth)
    _ if cur.text == "_Static_assert" || cur.text == "static_assert" => {
      // （match guard 限同行——cur 在进入 match 前已是 Identifier 才到这）
      self.parse_static_assert(depth)
      let loc = self.prev_loc()
      @ast.Stmt::Block([], loc)
    }
    _ =>
      if self.is_type_token() ||
        self.is_static_token() ||
        self.check(Register) ||
        self.check(Auto) ||
        self.check(Inline) {
        let is_static = self.is_static_token()
        if is_static {
          ignore(self.advance()) // consume 'static'
        }
        // In C++ mode, 'auto' is a type keyword, not a storage class
        while self.check(Register) ||
              (!self.is_cpp_mode && self.check(Auto)) ||
              self.check(Inline) {
          ignore(self.advance())
        }
        self.parse_var_decl_stmt(is_static, depth)
      } else {
        // Label statement: ident : stmt
        if self.check(Identifier) && self.peek(1).ty == Colon {
          return self.parse_label_stmt(depth)
        }
        let checkpoint_pos = self.pos
        let stmt = self.parse_expr_stmt(depth)
        // NOTE: Parser 零进度保护：若解析未前进则主动同步，避免死循环
        if self.pos == checkpoint_pos {
          self.stall_synchronize()
        }
        stmt
      }
  }
}

///|
/// stmt.rs 零进度保护的同步集（26 项语句起始 token）。
fn Parser::stall_synchronize(self : Parser) -> Unit {
  self.stall_count += 1
  self.synchronize([
    Semicolon,
    RBrace,
    Int,
    Void,
    Char,
    Float,
    Double,
    Long,
    Bool,
    If,
    While,
    Do,
    For,
    Return,
    Break,
    Continue,
    Struct,
    Switch,
    Typedef,
  ])
}

///|
fn Parser::parse_block(self : Parser, depth : Int) -> @ast.Stmt {
  let cur = self.current()
  let loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
  ignore(self.consume(LBrace, "预期 '{'"))
  let stmts : Array[@ast.Stmt] = []
  while !self.check(RBrace) && !self.is_at_end() {
    let stmt_checkpoint = self.pos
    // 局部 typedef 声明不产生运行时语句
    if self.check(Typedef) {
      self.parse_typedef(depth)
      continue
    }
    stmts.push(self.parse_statement(depth))
    if self.pos == stmt_checkpoint {
      self.stall_count += 1
      ignore(self.advance())
    }
  }
  ignore(self.consume(RBrace, "预期 '}'"))
  @ast.Stmt::Block(stmts, loc)
}

///|
fn Parser::parse_var_decl_stmt(
  self : Parser,
  is_static : Bool,
  depth : Int,
) -> @ast.Stmt {
  let cur = self.current()
  let loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
  let base_type = self.parse_base_type(depth)
  // Detect C++ constructor initialization syntax: `Type name(args);`
  // 仅当声明符是裸标识符（无指针/引用/函数指针后缀）时才当构造调用；
  // `int (*fp)(int);` 类函数指针声明走 parse_var_declarator。
  let is_simple_ctor_init = (match base_type {
      Class(_, _) | TemplateId(_, _, _) => true
      _ => false
    }) &&
    self.check(Identifier) &&
    self.peek(1).ty == LParen
  let triple : (@ast.Type, String, @ast.Expr?) = if is_simple_ctor_init {
    let name_tok = self.advance()
    ignore(self.advance()) // consume '('
    let args = self.parse_arg_list(depth)
    ignore(self.consume(RParen, "构造函数初始化预期 ')'"))
    let ctor_name = if args.is_empty() {
      "__ctor__\{base_type.name()}"
    } else {
      "__ctor__\{base_type.name()}__\{args.length()}"
    }
    (
      base_type,
      name_tok.text,
      Some(@ast.Expr::Call(ctor_name, args, loc, @ast.Type::void_type())),
    )
  } else {
    let d = self.parse_var_declarator(base_type, depth)
    let init = if self.match_token(Assign) {
      if self.check(LBrace) {
        Some(self.parse_init_list(depth))
      } else {
        Some(self.parse_assign(depth))
      }
    } else {
      None
    }
    (d.0, d.1, init)
  }
  let var_type = triple.0
  let name = triple.1
  let init = triple.2
  let extra_vars : Array[@ast.VarDeclExtra] = []
  while self.match_token(Comma) {
    let is_extra_ctor_init = (match base_type {
        Class(_, _) | TemplateId(_, _, _) => true
        _ => false
      }) &&
      self.check(Identifier) &&
      self.peek(1).ty == LParen
    if is_extra_ctor_init {
      let name_tok = self.advance()
      ignore(self.advance()) // consume '('
      let args = self.parse_arg_list(depth)
      ignore(self.consume(RParen, "构造函数初始化预期 ')'"))
      let ctor_name = if args.is_empty() {
        "__ctor__\{base_type.name()}"
      } else {
        "__ctor__\{base_type.name()}__\{args.length()}"
      }
      extra_vars.push({
        var_type: base_type,
        name: name_tok.text,
        init: Some(
          @ast.Expr::Call(ctor_name, args, loc, @ast.Type::void_type()),
        ),
      })
    } else {
      let ex = self.parse_var_declarator(base_type, depth)
      let extra_init = if self.match_token(Assign) {
        if self.check(LBrace) {
          Some(self.parse_init_list(depth))
        } else {
          Some(self.parse_assign(depth))
        }
      } else {
        None
      }
      extra_vars.push({ var_type: ex.0, name: ex.1, init: extra_init, })
    }
  }
  ignore(self.consume(Semicolon, "变量声明后预期 ';'"))
  @ast.Stmt::VarDecl(var_type, name, init, extra_vars, is_static, loc)
}

///|
fn Parser::parse_if_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(If, "预期 'if'"))
  ignore(self.consume(LParen, "预期 '('"))
  let cond = self.parse_expression(depth)
  ignore(self.consume(RParen, "预期 ')'"))
  let then_stmt = self.parse_statement(depth)
  let else_stmt : @ast.Stmt? = if self.match_token(Else) {
    Some(self.parse_statement(depth))
  } else {
    None
  }
  @ast.Stmt::If(cond, then_stmt, else_stmt, loc)
}

///|
fn Parser::parse_while_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(While, "预期 'while'"))
  ignore(self.consume(LParen, "预期 '('"))
  let cond = self.parse_expression(depth)
  ignore(self.consume(RParen, "预期 ')'"))
  let body = self.parse_statement(depth)
  @ast.Stmt::While(cond, body, loc)
}

///|
fn Parser::parse_do_while_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(Do, "预期 'do'"))
  let body = self.parse_statement(depth)
  ignore(self.consume(While, "预期 'while'"))
  ignore(self.consume(LParen, "预期 '('"))
  let cond = self.parse_expression(depth)
  ignore(self.consume(RParen, "预期 ')'"))
  ignore(self.consume(Semicolon, "do...while 后预期 ';'"))
  @ast.Stmt::DoWhile(body, cond, loc)
}

///|
fn Parser::parse_break_stmt(self : Parser) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(Break, "预期 'break'"))
  ignore(self.consume(Semicolon, "break 后预期 ';'"))
  @ast.Stmt::Break(loc)
}

///|
fn Parser::parse_continue_stmt(self : Parser) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(Continue, "预期 'continue'"))
  ignore(self.consume(Semicolon, "continue 后预期 ';'"))
  @ast.Stmt::Continue(loc)
}

///|
fn Parser::parse_goto_stmt(self : Parser) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(Goto, "预期 'goto'"))
  let label = self.current().text
  ignore(self.consume(Identifier, "goto 后预期标签名"))
  ignore(self.consume(Semicolon, "goto 后预期 ';'"))
  @ast.Stmt::Goto(label, loc)
}

///|
fn Parser::parse_label_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  let label = self.current().text
  ignore(self.advance()) // consume identifier
  ignore(self.consume(Colon, "标签名后预期 ':'"))
  let stmt = self.parse_statement(depth)
  @ast.Stmt::Label(label, stmt, loc)
}

///|
fn Parser::parse_for_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(For, "预期 'for'"))
  ignore(self.consume(LParen, "预期 '('"))
  // C++ 模式下检测 range for: for (auto x : expr) 或 for (Type x : expr)
  if self.is_cpp_mode {
    let checkpoint = self.save()
    let is_range_for = if self.is_type_token() || self.check(Auto) {
      ignore(self.parse_base_type(depth))
      while self.match_token(Star) {
        ()
      }
      if self.is_cpp_mode && (self.check(Ampersand) || self.check(AndAnd)) {
        ignore(self.advance())
        if self.check(Const) {
          ignore(self.advance())
        }
      }
      if self.check(Identifier) {
        ignore(self.advance())
        self.check(Colon)
      } else {
        false
      }
    } else {
      false
    }
    self.restore(checkpoint)
    if is_range_for {
      let var_type = self.parse_base_type(depth)
      let mut final_type = var_type
      while self.match_token(Star) {
        final_type = @ast.Type::pointer_to(final_type)
      }
      if self.is_cpp_mode {
        if self.match_token(Ampersand) {
          let is_const = self.match_token(Const)
          final_type = @ast.Type::Reference(final_type, is_const)
        } else if self.match_token(AndAnd) {
          final_type = @ast.Type::RValueRef(final_type)
        }
      }
      let var_name = self.advance().text
      ignore(self.consume(Colon, "range for 预期 ':'"))
      let iter = self.parse_expression(depth)
      ignore(self.consume(RParen, "range for 预期 ')'"))
      let body = self.parse_statement(depth)
      return @ast.Stmt::RangeFor(var_name, final_type, iter, body, loc)
    }
  }
  let init : @ast.Stmt? = if self.is_type_token() {
    let cur = self.current()
    let var_loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
    let base_type = self.parse_base_type(depth)
    let d = self.parse_var_declarator(base_type, depth)
    let init_expr = if self.match_token(Assign) {
      if self.check(LBrace) {
        Some(self.parse_init_list(depth))
      } else {
        Some(self.parse_assign(depth))
      }
    } else {
      None
    }
    let extra_vars : Array[@ast.VarDeclExtra] = []
    while self.match_token(Comma) {
      let ex = self.parse_var_declarator(base_type, depth)
      let extra_init = if self.match_token(Assign) {
        if self.check(LBrace) {
          Some(self.parse_init_list(depth))
        } else {
          Some(self.parse_assign(depth))
        }
      } else {
        None
      }
      extra_vars.push({ var_type: ex.0, name: ex.1, init: extra_init, })
    }
    Some(@ast.Stmt::VarDecl(d.0, d.1, init_expr, extra_vars, false, var_loc))
  } else if !self.check(Semicolon) {
    let cur = self.current()
    let es_loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
    let exprs : Array[@ast.Expr] = [self.parse_expression(depth)]
    while self.match_token(Comma) {
      exprs.push(self.parse_expression(depth))
    }
    if exprs.length() == 1 {
      Some(@ast.Stmt::Expr(exprs[0], es_loc))
    } else {
      let stmts : Array[@ast.Stmt] = []
      for e in exprs {
        stmts.push(@ast.Stmt::Expr(e, es_loc))
      }
      Some(@ast.Stmt::Block(stmts, es_loc))
    }
  } else {
    None
  }
  ignore(self.consume(Semicolon, "预期 ';'"))
  let cond : @ast.Expr? = if !self.check(Semicolon) {
    Some(self.parse_expression(depth))
  } else {
    None
  }
  ignore(self.consume(Semicolon, "预期 ';'"))
  let step : Array[@ast.Expr] = []
  if !self.check(RParen) {
    step.push(self.parse_expression(depth))
    while self.match_token(Comma) {
      step.push(self.parse_expression(depth))
    }
  }
  ignore(self.consume(RParen, "预期 ')'"))
  let body = self.parse_statement(depth)
  @ast.Stmt::For(init, cond, step, body, loc)
}

///|
fn Parser::parse_return_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.consume(Return, "预期 'return'"))
  let value : @ast.Expr? = if !self.check(Semicolon) {
    Some(self.parse_expression(depth))
  } else {
    None
  }
  ignore(self.consume(Semicolon, "return 后预期 ';'"))
  @ast.Stmt::Return(value, loc)
}

///|
fn Parser::parse_expr_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  let expr = self.parse_expression(depth)
  ignore(self.consume(Semicolon, "预期 ';'"))
  @ast.Stmt::Expr(expr, loc)
}

///|
fn Parser::parse_switch_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  ignore(self.advance())
  ignore(self.consume(LParen, "switch 后预期 '('"))
  let cond = self.parse_expression(depth)
  ignore(self.consume(RParen, "switch 条件后预期 ')'"))
  let body = self.parse_statement(depth)
  @ast.Stmt::Switch(cond, body, loc)
}

///|
fn Parser::parse_case_stmt(self : Parser, depth : Int) -> @ast.Stmt {
  let loc = self.cur_loc()
  let label : @ast.Expr? = if self.match_token(Case) {
    Some(self.parse_expression(depth))
  } else if self.match_token(Default) {
    None
  } else {
    let cur = self.current()
    self.errors.push({
      message: "预期 'case' 或 'default'",
      line: cur.loc.line,
      column: cur.loc.column,
      code: @diag.ErrorCode::E2004_ExpectedCaseOrDefault.code(),
    })
    return @ast.Stmt::Block([], loc)
  }
  ignore(self.consume(Colon, "case/default 后预期 ':'"))
  let stmts : Array[@ast.Stmt] = []
  while !self.check(Case) &&
        !self.check(Default) &&
        !self.check(RBrace) &&
        !self.is_at_end() {
    let stmt_checkpoint = self.pos
    stmts.push(self.parse_statement(depth))
    if self.pos == stmt_checkpoint {
      self.stall_count += 1
      ignore(self.advance())
    }
  }
  let stmt = if stmts.is_empty() {
    @ast.Stmt::Block([], loc)
  } else if stmts.length() == 1 {
    stmts.remove(0)
  } else {
    @ast.Stmt::Block(stmts, loc)
  }
  @ast.Stmt::Case(label, stmt, loc)
}
