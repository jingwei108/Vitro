///|
/// 表达式优先级瀑布（照搬 Rust `expr/` 五文件：mod/ops/primary/postfix/
/// unary）。21 帧瀑布中 4 个壳挂点（assign/ternary/unary/primary）按
/// 包注释 1 的 depth 参数规则：壳检查 `depth`、body 与下层壳传 `depth+1`；
/// 瀑布兄弟函数（or/and/…）透传。
///
/// 中性占位：`Type::default()` = Void(false)（Rust Default 同）；深度
/// 超限占位 = Literal 0 + 默认坐标（Rust Expr::default 同）。
fn Parser::prev_loc(self : Parser) -> @source.SourceLoc {
  let p = self.previous()
  @source.SourceLoc::new(p.loc.line, p.loc.column)
}

///|
fn Parser::cur_loc(self : Parser) -> @source.SourceLoc {
  let c = self.current()
  @source.SourceLoc::new(c.loc.line, c.loc.column)
}

///|
fn Parser::parse_expression(self : Parser, depth : Int) -> @ast.Expr {
  self.parse_comma(depth)
}

///|
fn Parser::parse_comma(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_assign(depth)
  while self.match_token(Comma) {
    let right = self.parse_assign(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::Comma,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

// =========================================================================
// 赋值（壳挂点）
// =========================================================================

///|
fn Parser::parse_assign(self : Parser, depth : Int) -> @ast.Expr {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("赋值")
    return default_expr()
  }
  self.parse_assign_body(depth + 1)
}

///|
fn Parser::parse_assign_body(self : Parser, depth : Int) -> @ast.Expr {
  let left = self.parse_ternary(depth)
  let loc = self.prev_loc()
  let ops : Array[(@token.TokenType, @ast.AssignOp)] = [
    (Assign, @ast.AssignOp::Assign),
    (PlusAssign, @ast.AssignOp::AddAssign),
    (MinusAssign, @ast.AssignOp::SubAssign),
    (StarAssign, @ast.AssignOp::MulAssign),
    (SlashAssign, @ast.AssignOp::DivAssign),
    (PercentAssign, @ast.AssignOp::ModAssign),
    (AndAssign, @ast.AssignOp::AndAssign),
    (OrAssign, @ast.AssignOp::OrAssign),
    (XorAssign, @ast.AssignOp::XorAssign),
    (ShlAssign, @ast.AssignOp::ShlAssign),
    (ShrAssign, @ast.AssignOp::ShrAssign),
  ]
  for pair in ops {
    if self.match_token(pair.0) {
      let right = self.parse_assign(depth)
      return @ast.Expr::Assign(pair.1, left, right, loc, @ast.Type::void_type())
    }
  }
  left
}

// =========================================================================
// 三目（壳挂点）
// =========================================================================

///|
fn Parser::parse_ternary(self : Parser, depth : Int) -> @ast.Expr {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("三目")
    return default_expr()
  }
  self.parse_ternary_body(depth + 1)
}

///|
fn Parser::parse_ternary_body(self : Parser, depth : Int) -> @ast.Expr {
  let cond = self.parse_or(depth)
  if self.match_token(Question) {
    let then_branch = self.parse_ternary(depth)
    ignore(self.consume(Colon, "预期 ':'"))
    let else_branch = self.parse_ternary(depth)
    let loc = self.prev_loc()
    return @ast.Expr::Ternary(
      cond,
      then_branch,
      else_branch,
      loc,
      @ast.Type::void_type(),
    )
  }
  cond
}

// =========================================================================
// 二元优先级链（11 级，兄弟函数透传 depth）
// =========================================================================

///|
fn Parser::parse_or(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_and(depth)
  while self.match_token(OrOr) {
    let right = self.parse_and(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::Or,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

///|
fn Parser::parse_and(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_bit_or(depth)
  while self.match_token(AndAnd) {
    let right = self.parse_bit_or(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::And,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

///|
fn Parser::parse_bit_or(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_bit_xor(depth)
  while self.match_token(BitOr) {
    let right = self.parse_bit_xor(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::BitOr,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

///|
fn Parser::parse_bit_xor(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_bit_and(depth)
  while self.match_token(BitXor) {
    let right = self.parse_bit_and(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::BitXor,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

///|
fn Parser::parse_bit_and(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_equality(depth)
  while self.match_token(Ampersand) {
    let right = self.parse_equality(depth)
    let loc = self.prev_loc()
    left = @ast.Expr::Binary(
      @ast.BinaryOp::BitAnd,
      left,
      right,
      loc,
      @ast.Type::void_type(),
    )
  }
  left
}

///|
fn Parser::parse_equality(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_relational(depth)
  while true {
    if self.match_token(Eq) {
      let right = self.parse_relational(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Eq,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Ne) {
      let right = self.parse_relational(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Ne,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  left
}

///|
fn Parser::parse_relational(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_shift(depth)
  while true {
    if self.match_token(Lt) {
      let right = self.parse_shift(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Lt,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Le) {
      let right = self.parse_shift(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Le,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Gt) {
      let right = self.parse_shift(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Gt,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Ge) {
      let right = self.parse_shift(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Ge,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  left
}

///|
fn Parser::parse_shift(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_additive(depth)
  while true {
    if self.match_token(Shl) {
      let right = self.parse_additive(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Shl,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Shr) {
      let right = self.parse_additive(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Shr,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  left
}

///|
fn Parser::parse_additive(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_multiplicative(depth)
  while true {
    if self.match_token(Plus) {
      let right = self.parse_multiplicative(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Add,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Minus) {
      let right = self.parse_multiplicative(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Sub,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  left
}

///|
fn Parser::parse_multiplicative(self : Parser, depth : Int) -> @ast.Expr {
  let mut left = self.parse_unary(depth)
  while true {
    if self.match_token(Star) {
      let right = self.parse_unary(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Mul,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Slash) {
      let right = self.parse_unary(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Div,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Percent) {
      let right = self.parse_unary(depth)
      let loc = self.prev_loc()
      left = @ast.Expr::Binary(
        @ast.BinaryOp::Mod,
        left,
        right,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  left
}

// =========================================================================
// 一元（壳挂点）+ sizeof/alignof/offsetof/cast
// =========================================================================

///|
fn Parser::parse_unary(self : Parser, depth : Int) -> @ast.Expr {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("一元")
    return default_expr()
  }
  self.parse_unary_body(depth + 1)
}

///|
fn Parser::parse_unary_body(self : Parser, depth : Int) -> @ast.Expr {
  if self.match_token(Sizeof) {
    return self.parse_sizeof(depth)
  }
  if self.match_token(Alignof) {
    return self.parse_alignof(depth)
  }
  if self.match_token(Offsetof) {
    return self.parse_offsetof()
  }
  if self.is_cpp_mode && self.match_token(New) {
    return self.parse_new_expr(depth)
  }
  if self.is_cpp_mode && self.match_token(Delete) {
    return self.parse_delete_expr(depth)
  }
  if self.check(LParen) {
    let checkpoint = self.save()
    ignore(self.advance()) // consume '('
    if self.is_type_token() {
      let t = self.parse_type_only(depth)
      // 若 ')' 后接 '{'，则是复合字面量，交给 parse_primary 处理
      if self.match_token(RParen) && !self.check(LBrace) {
        let operand = self.parse_unary(depth)
        let loc = self.prev_loc()
        return @ast.Expr::Cast(operand, t, loc, t)
      }
    }
    self.restore(checkpoint) // typedef_names 在全量快照内，随 restore 回滚
  }
  let unary_ops : Array[(@token.TokenType, @ast.UnaryOp)] = [
    (Minus, @ast.UnaryOp::Neg),
    (Not, @ast.UnaryOp::Not),
    (BitNot, @ast.UnaryOp::BitNot),
    (Ampersand, @ast.UnaryOp::Addr),
    (Star, @ast.UnaryOp::Deref),
    (Increment, @ast.UnaryOp::PreInc),
    (Decrement, @ast.UnaryOp::PreDec),
  ]
  for pair in unary_ops {
    if self.match_token(pair.0) {
      let operand = self.parse_unary(depth)
      let loc = self.prev_loc()
      return @ast.Expr::Unary(pair.1, operand, loc, @ast.Type::void_type())
    }
  }
  self.parse_postfix(depth)
}

///|
/// 抽象声明符（壳挂点；sizeof(int(*)(*)…) 深链防御）。
fn Parser::parse_abstract_declarator(
  self : Parser,
  depth : Int,
) -> DeclaratorNode? {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("抽象声明符")
    return None
  }
  let node = self.parse_abstract_declarator_body(depth + 1)
  match node {
    Base => None
    other => Some(other)
  }
}

///|
fn Parser::parse_abstract_declarator_body(
  self : Parser,
  depth : Int,
) -> DeclaratorNode {
  let dguard = DeclaratorGuard::init()
  self.parse_declarator_node(dguard, true, true, depth).0
}

///|
fn Parser::parse_sizeof(self : Parser, depth : Int) -> @ast.Expr {
  let loc = self.prev_loc()
  if self.match_token(LParen) {
    let checkpoint = self.save()
    let mut is_type = false
    let mut t = @ast.Type::void_type()
    if self.is_type_token() {
      t = self.parse_base_type(depth)
      match self.parse_abstract_declarator(depth) {
        Some(node) => t = self.interpret_declarator_node(node, t, depth)
        None => ()
      }
      if self.check(RParen) {
        is_type = true
      }
    }
    if is_type {
      ignore(self.consume(RParen, "sizeof(type) 后预期 ')'"))
      return @ast.Expr::Sizeof(Some(t), None, loc, @ast.Type::int())
    }
    self.restore(checkpoint) // sizeof 无 typedef 快照（A5：全量快照下随 restore 回滚）
    let expr = self.parse_expression(depth)
    ignore(self.consume(RParen, "sizeof(expr) 后预期 ')'"))
    return @ast.Expr::Sizeof(None, Some(expr), loc, @ast.Type::int())
  }
  let expr = self.parse_unary(depth)
  @ast.Expr::Sizeof(None, Some(expr), loc, @ast.Type::int())
}

///|
/// C23 `_Alignof` / `alignof`：结构与 parse_sizeof 同构（类型名/表达式
/// 双形态）；typedef 快照两侧形态不同（Rust 此处手工补快照，本实现
/// restore 全量快照覆盖——等价性见包注释 2）。
fn Parser::parse_alignof(self : Parser, depth : Int) -> @ast.Expr {
  let loc = self.prev_loc()
  if self.match_token(LParen) {
    let checkpoint = self.save()
    let mut is_type = false
    let mut t = @ast.Type::void_type()
    if self.is_type_token() {
      t = self.parse_base_type(depth)
      match self.parse_abstract_declarator(depth) {
        Some(node) => t = self.interpret_declarator_node(node, t, depth)
        None => ()
      }
      if self.check(RParen) {
        is_type = true
      }
    }
    if is_type {
      ignore(self.consume(RParen, "alignof(type) 后预期 ')'"))
      return @ast.Expr::Alignof(Some(t), None, loc, @ast.Type::int())
    }
    self.restore(checkpoint)
    let expr = self.parse_expression(depth)
    ignore(self.consume(RParen, "alignof(expr) 后预期 ')'"))
    return @ast.Expr::Alignof(None, Some(expr), loc, @ast.Type::int())
  }
  let expr = self.parse_unary(depth)
  @ast.Expr::Alignof(None, Some(expr), loc, @ast.Type::int())
}

///|
fn Parser::parse_offsetof(self : Parser) -> @ast.Expr {
  let loc = self.prev_loc()
  ignore(self.consume(LParen, "offsetof 后预期 '('"))
  let target_type = self.parse_base_type(0)
  ignore(self.consume(Comma, "offsetof 参数之间预期 ','"))
  let field_tok = self.consume(Identifier, "offsetof 预期字段名")
  ignore(self.consume(RParen, "offsetof 后预期 ')'"))
  @ast.Expr::Offsetof(target_type, field_tok.text, loc, @ast.Type::int())
}

///|
/// 类型名解析（cast / sizeof(type) / _Generic 关联；照搬 parse_type_only）。
fn Parser::parse_type_only(self : Parser, depth : Int) -> @ast.Type {
  let base = self.parse_base_type(depth)
  match self.parse_abstract_declarator(depth) {
    Some(node) => self.interpret_declarator_node(node, base, depth)
    None => base
  }
}

// =========================================================================
// 后缀 + 初始化列表 + 实参表
// =========================================================================

///|
fn Parser::parse_postfix(self : Parser, depth : Int) -> @ast.Expr {
  let mut expr = self.parse_primary(depth)
  while true {
    if self.match_token(LBracket) {
      let index = self.parse_expression(depth)
      ignore(self.consume(RBracket, "预期 ']'"))
      let loc = self.prev_loc()
      expr = @ast.Expr::Index(expr, index, loc, @ast.Type::void_type())
    } else if self.match_token(LParen) {
      // Function call: direct named call or function pointer call
      let args = self.parse_arg_list(depth)
      ignore(self.consume(RParen, "预期 ')'"))
      let loc = self.prev_loc()
      expr = @ast.Expr::CallPtr(expr, args, loc, @ast.Type::void_type())
    } else if self.match_token(Dot) || self.match_token(Arrow) {
      let member_tok = self.consume(Identifier, "预期成员名称")
      let loc = self.prev_loc()
      // C++ 模式下检查是否是方法调用：obj.method(args)
      if self.is_cpp_mode && self.check(LParen) {
        ignore(self.advance()) // consume '('
        let args = self.parse_arg_list(depth)
        ignore(self.consume(RParen, "预期 ')'"))
        expr = @ast.Expr::MemberCall(
          expr,
          member_tok.text,
          args,
          false,
          None,
          loc,
          @ast.Type::void_type(),
        )
      } else {
        expr = @ast.Expr::Member(
          expr,
          member_tok.text,
          loc,
          @ast.Type::void_type(),
        )
      }
    } else if self.match_token(Increment) {
      let loc = self.prev_loc()
      expr = @ast.Expr::Unary(
        @ast.UnaryOp::PostInc,
        expr,
        loc,
        @ast.Type::void_type(),
      )
    } else if self.match_token(Decrement) {
      let loc = self.prev_loc()
      expr = @ast.Expr::Unary(
        @ast.UnaryOp::PostDec,
        expr,
        loc,
        @ast.Type::void_type(),
      )
    } else {
      break
    }
  }
  expr
}

///|
/// 初始化列表（壳挂点；嵌套 `{...}` 递归）。
fn Parser::parse_init_list(self : Parser, depth : Int) -> @ast.Expr {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("初始化列表")
    return default_expr()
  }
  self.parse_init_list_body(depth + 1)
}

///|
fn Parser::parse_init_list_body(self : Parser, depth : Int) -> @ast.Expr {
  let cur = self.current()
  let loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
  ignore(self.consume(LBrace, "初始化列表预期 '{'"))
  let elements : Array[@ast.InitElement] = []
  if !self.check(RBrace) {
    while true {
      let designators : Array[@ast.Designator] = []
      // Designated initializer: .field = value or [index] = value
      if self.match_token(Dot) {
        let field_tok = self.consume(Identifier, "Designator 预期字段名")
        designators.push(@ast.Designator::Field(field_tok.text))
        ignore(self.consume(Assign, "Designated initializer 预期 '='"))
      } else if self.match_token(LBracket) {
        let idx_expr = self.parse_assign(depth)
        ignore(self.consume(RBracket, "Designator 预期 ']'"))
        designators.push(@ast.Designator::Index(idx_expr))
        ignore(self.consume(Assign, "Designated initializer 预期 '='"))
      }
      let value = if self.check(LBrace) {
        self.parse_init_list(depth)
      } else {
        self.parse_assign(depth)
      }
      elements.push({ designators, value, })
      if !self.match_token(Comma) {
        break
      }
    }
  }
  ignore(self.consume(RBrace, "初始化列表预期 '}'"))
  @ast.Expr::InitList(elements, loc, @ast.Type::void_type())
}

///|
fn Parser::parse_arg_list(self : Parser, depth : Int) -> Array[@ast.Expr] {
  let args : Array[@ast.Expr] = []
  if self.check(RParen) {
    return args
  }
  while true {
    args.push(self.parse_assign(depth))
    if !self.match_token(Comma) {
      break
    }
  }
  args
}

// =========================================================================
// primary（壳挂点）
// =========================================================================

///|
fn Parser::parse_primary(self : Parser, depth : Int) -> @ast.Expr {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("表达式")
    return @ast.Expr::Literal(0, @source.SourceLoc::default(), @ast.Type::int())
  }
  self.parse_primary_inner(depth + 1)
}

///|
fn Parser::parse_primary_inner(self : Parser, depth : Int) -> @ast.Expr {
  if self.match_token(Number) {
    let prev = self.previous()
    let value = self.parse_int_or_report(prev, "整数常量")
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    return @ast.Expr::Literal(value, loc, @ast.Type::int())
  }
  if self.match_token(UnsignedLiteral) {
    let prev = self.previous()
    // F-P0-2：按值域分派——i32 内 → Literal，否则 → LongLiteral 保真
    //（> i64::MAX 的值如 ULLONG_MAX 按 64 位位模式承载为负——Rust `uv as i64`
    // 的 bitcast 语义，配一元负号即得 LLONG_MIN 位模式）
    let uv = self.parse_uint64_or_report(prev, "unsigned 常量")
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    if uv <= 2147483647 {
      return @ast.Expr::Literal(uv.to_int(), loc, @ast.Type::unsigned_int())
    }
    return @ast.Expr::LongLiteral(
      uv.reinterpret_as_int64(),
      loc,
      @ast.Type::LongLong(true, false),
    )
  }
  if self.match_token(LongLiteral) {
    let prev = self.previous()
    let value = self.parse_i64_or_report(prev, "long long 常量")
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    return @ast.Expr::LongLiteral(value, loc, @ast.Type::long_long())
  }
  if self.match_token(FloatLiteral) {
    let prev = self.previous()
    // C 标准：无后缀浮点字面量类型是 double，带 f/F 后缀才是 float
    let has_f_suffix = prev.text.has_suffix("f") || prev.text.has_suffix("F")
    let mut text = prev.text
    while text.has_suffix("f") || text.has_suffix("F") {
      text = text.substring(start=0, end=text.length() - 1)
    }
    let value = self.parse_double_or_report(prev, text)
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    let ty = if has_f_suffix { @ast.Type::float() } else { @ast.Type::double() }
    return @ast.Expr::FloatLiteral(value, loc, ty)
  }
  if self.match_token(CharLiteral) {
    let prev = self.previous()
    let value = self.parse_int_or_report(prev, "字符常量")
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    return @ast.Expr::Literal(value, loc, @ast.Type::char())
  }
  if self.match_token(String) {
    // C89 相邻字符串字面量拼接："ab" "cd" → "abcd"
    let mut value = self.previous().text
    let prev0 = self.previous()
    let loc = @source.SourceLoc::new(prev0.loc.line, prev0.loc.column)
    while self.check(String) {
      ignore(self.advance())
      value = value + self.previous().text
    }
    // M12：字节长度必须 UTF-8 编码计（Rust value.len()；String::length()
    // 是 UTF-16 码元数——"中文" 会少 4）
    let array_size = @utf8.encode(value).length() + 1
    // including null terminator
    return @ast.Expr::StringLiteral(
      value,
      loc,
      @ast.Type::Array(
        @ast.Type::char(),
        array_size,
        [array_size],
        false,
        false,
        [],
      ),
    )
  }
  if self.match_token(Null) {
    let loc = self.prev_loc()
    return @ast.Expr::Literal(
      0,
      loc,
      @ast.Type::pointer_to(@ast.Type::void_type()),
    )
  }
  if self.match_token(Generic) {
    let loc = self.prev_loc()
    ignore(self.consume(LParen, "_Generic 后预期 '('"))
    let control = self.parse_assign(depth)
    ignore(self.consume(Comma, "_Generic 参数之间预期 ','"))
    let associations : Array[(@ast.Type, @ast.Expr)] = []
    let mut default : @ast.Expr? = None
    while !self.check(RParen) && !self.is_at_end() {
      if self.match_token(Default) {
        ignore(self.consume(Colon, "default 后预期 ':'"))
        let expr = self.parse_assign(depth)
        default = Some(expr)
      } else {
        let assoc_type = self.parse_type_only(depth)
        ignore(self.consume(Colon, "类型关联后预期 ':'"))
        let expr = self.parse_assign(depth)
        associations.push((assoc_type, expr))
      }
      if !self.match_token(Comma) {
        break
      }
    }
    ignore(self.consume(RParen, "_Generic 后预期 ')'"))
    return @ast.Expr::Generic(
      control,
      associations,
      default,
      loc,
      @ast.Type::void_type(),
    )
  }
  if self.is_cpp_mode && self.match_token(This) {
    let loc = self.prev_loc()
    return @ast.Expr::This(loc, @ast.Type::void_type())
  }
  if self.is_cpp_mode && self.check(LBracket) {
    return self.parse_lambda_expr(depth)
  }
  if self.check(Identifier) && self.peek(0).text == "__asm__" {
    // GCC 风格内联汇编占位：__asm__("...")——消费语法返回 void 字面量
    let name_tok = self.advance()
    ignore(self.consume(LParen, "__asm__ 后预期 '('"))
    ignore(self.consume(String, "__asm__ 预期汇编字符串"))
    ignore(self.consume(RParen, "__asm__ 预期 ')'"))
    let loc = @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column)
    return @ast.Expr::Literal(0, loc, @ast.Type::void_type())
  }
  if self.check(Identifier) {
    let name_tok = self.advance()
    let mut name = name_tok.text
    if self.is_cpp_mode && self.check(ColonColon) {
      ignore(self.advance()) // ::
      let inner = self.consume(Identifier, ":: 后预期标识符")
      name = "\{name}__\{inner.text}"
    }
    let loc = @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column)
    return @ast.Expr::Identifier(name, loc, @ast.Type::void_type())
  }
  if self.match_token(LParen) {
    let lparen = self.previous()
    let checkpoint = self.save()
    // 尝试解析复合字面量 (type-name) { initializer-list }
    if self.is_type_token() {
      let t = self.parse_type_only(depth)
      if self.match_token(RParen) && self.check(LBrace) {
        let init = self.parse_init_list(depth)
        let loc = @source.SourceLoc::new(lparen.loc.line, lparen.loc.column)
        return @ast.Expr::CompoundLiteral(t, init, loc, t)
      }
    }
    self.restore(checkpoint) // typedef_names 全量快照随 restore 回滚
    let expr = self.parse_expression(depth)
    ignore(self.consume(RParen, "预期 ')'"))
    return expr
  }
  let cur = self.current()
  self.errors.push({
    message: "预期表达式",
    line: cur.loc.line,
    column: cur.loc.column,
    code: @diag.ErrorCode::E2003_ExpectedExpr.code(),
  })
  let loc = @source.SourceLoc::new(cur.loc.line, cur.loc.column)
  // 消费当前 token，防止外层 parse_statement 在相同位置无限循环。
  if !self.is_at_end() {
    ignore(self.advance())
  }
  @ast.Expr::Literal(0, loc, @ast.Type::int())
}

// =========================================================================
// 数值文本 parse（Rust parse::<T>() 失败即 E1006 + 0 的同构）
// =========================================================================

///|
fn Parser::parse_int_or_report(
  self : Parser,
  tok : @token.Token,
  what : String,
) -> Int {
  let v = @string.parse_int(tok.text) catch {
    _ => {
      self.errors.push({
        message: "\{what} '\{tok.text}' 超出 int 表示范围",
        line: tok.loc.line,
        column: tok.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      return 0
    }
  }
  v
}

///|
fn Parser::parse_uint64_or_report(
  self : Parser,
  tok : @token.Token,
  what : String,
) -> UInt64 {
  let v = @string.parse_uint64(tok.text) catch {
    _ => {
      self.errors.push({
        message: "\{what} '\{tok.text}' 超出范围",
        line: tok.loc.line,
        column: tok.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      return 0
    }
  }
  v
}

///|
fn Parser::parse_i64_or_report(
  self : Parser,
  tok : @token.Token,
  what : String,
) -> Int64 {
  let v = @string.parse_int64(tok.text) catch {
    _ => {
      self.errors.push({
        message: "\{what} '\{tok.text}' 超出范围",
        line: tok.loc.line,
        column: tok.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      return 0
    }
  }
  v
}

///|
fn Parser::parse_double_or_report(
  self : Parser,
  tok : @token.Token,
  text : String,
) -> Double {
  let v = @string.parse_double(text) catch {
    _ => {
      self.errors.push({
        message: "浮点常量 '\{tok.text}' 格式无效",
        line: tok.loc.line,
        column: tok.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      return 0.0
    }
  }
  v
}

// =========================================================================
// C++ 表达式（new/delete/lambda；语义照搬 postfix.rs/unary.rs 的 C++ 段）
// =========================================================================

///|
fn Parser::parse_new_expr(self : Parser, depth : Int) -> @ast.Expr {
  let loc = self.prev_loc()
  let elem_type = self.parse_base_type(depth)
  let mut final_type = elem_type
  while self.match_token(Star) {
    final_type = @ast.Type::pointer_to(final_type)
  }
  let mut size_expr : @ast.Expr? = None
  let mut init : @ast.Expr? = None
  if self.match_token(LBracket) {
    size_expr = Some(self.parse_expression(depth))
    ignore(self.consume(RBracket, "new[] 预期 ']'"))
  }
  if self.match_token(LParen) {
    if self.check(RParen) {
      // Empty parens: treat as default constructor call
      init = Some(
        @ast.Expr::Call(
          "__ctor__\{elem_type.name()}",
          [],
          loc,
          @ast.Type::void_type(),
        ),
      )
    } else {
      let is_class = match elem_type {
        Class(_, _) | TemplateId(_, _, _) => true
        _ => false
      }
      if is_class {
        let ctor_args = self.parse_arg_list(depth)
        init = Some(
          @ast.Expr::Call(
            "__ctor__\{elem_type.name()}__\{ctor_args.length()}",
            ctor_args,
            loc,
            @ast.Type::void_type(),
          ),
        )
      } else {
        init = Some(self.parse_expression(depth))
      }
    }
    ignore(self.consume(RParen, "new 初始化预期 ')'"))
  }
  @ast.Expr::New(
    final_type,
    size_expr,
    init,
    loc,
    @ast.Type::pointer_to(elem_type),
  )
}

///|
fn Parser::parse_delete_expr(self : Parser, depth : Int) -> @ast.Expr {
  let loc = self.prev_loc()
  let mut is_array = false
  if self.check(LBracket) {
    ignore(self.advance()) // [
    if self.check(RBracket) {
      ignore(self.advance()) // ]
      is_array = true
    } else {
      // 回退，当作普通表达式中的 [ 处理（实际不太可能）
    }
  }
  let expr = self.parse_unary(depth)
  @ast.Expr::Delete(expr, is_array, loc, @ast.Type::void_type())
}

///|
fn Parser::parse_lambda_expr(self : Parser, depth : Int) -> @ast.Expr {
  let loc = self.cur_loc()
  ignore(self.consume(LBracket, "Lambda 预期 '['"))
  let capture : Array[@ast.CaptureMode] = []
  // 支持 []、[x]、[&x]、[=]、[&]、[a, &b]、[this]、[=, &a]、[&, a]
  if !self.check(RBracket) {
    while true {
      if self.check(Assign) {
        ignore(self.advance()) // =
        capture.push(@ast.CaptureMode::Implicit)
      } else if self.check(Ampersand) {
        ignore(self.advance()) // &
        if self.check(Identifier) {
          capture.push(@ast.CaptureMode::ByReference(self.current().text))
          ignore(self.advance())
        } else {
          capture.push(@ast.CaptureMode::Implicit)
        }
      } else if self.check(Identifier) {
        capture.push(@ast.CaptureMode::ByValue(self.current().text))
        ignore(self.advance())
      } else {
        break
      }
      if !self.match_token(Comma) {
        break
      }
    }
  }
  ignore(self.consume(RBracket, "Lambda 预期 ']'"))
  ignore(self.consume(LParen, "Lambda 预期 '('"))
  let pl = self.parse_param_list(depth)
  ignore(self.consume(RParen, "Lambda 预期 ')'"))
  // 可选的返回类型 -> Type（简化：忽略显式返回类型，由 TypeChecker 推断）
  if self.check(Minus) && self.peek(1).ty == Gt {
    ignore(self.advance()) // -
    ignore(self.advance()) // >
    let _ret_type = self.parse_base_type(depth)
    ignore(_ret_type)
  }
  let body = self.parse_statement(depth)
  let id = self.next_lambda_id
  self.next_lambda_id += 1
  @ast.Expr::Lambda(
    capture,
    pl.0,
    body,
    // Int→UInt 无直接转换（core/uint 仅 to_int64 单向）——lambda 编号
    // 为非负小整数，经 Double 中转数值无损
    id.to_double().to_uint(),
    loc,
    @ast.Type::void_type(),
  )
}
