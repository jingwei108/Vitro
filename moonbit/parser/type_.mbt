///|
/// 类型解析（照搬 Rust `type_.rs`）：类型说明符 + 声明符螺旋规则
/// （节点树 + 从内到外解释）。
///
/// depth 透传说明：`parse_declarator_node` 的括号自递归在 Rust 中由
/// `dguard.paren_depth ≤ 2` 限深（非 MAX_PARSE_DEPTH 计数），本实现同款
/// （depth 只透传不递增）；`parse_param_list` 是壳挂点（P1 批挂入，函数
/// 声明符互递归通道），后缀收集处传当前 depth。
///
/// P1 语义照搬：`suffix_count > MAX_DECLARATOR_SUFFIX` 在**收集期**拦截
/// （`a[1]x1300` 类输入报 E1006 并吞剩余后缀），interpret 递归解释树深
/// 因此封顶 1250（MoonBit 各后端余量内，勘察 7-S1：js ≈11k 帧预算）。
fn Parser::parse_base_type(self : Parser, depth : Int) -> @ast.Type {
  // Collect type qualifiers/modifiers (const, signed, unsigned, long, short)
  let mut is_unsigned = false
  let mut is_const = false
  while true {
    if self.match_token(Const) {
      is_const = true
      continue
    }
    if self.match_token(Volatile) {
      continue
    }
    if self.match_token(Restrict) {
      continue
    }
    if self.match_token(Register) {
      continue
    }
    if self.match_token(Auto) {
      if self.is_cpp_mode {
        // auto is not const-qualifiable in our simplified model
        return @ast.Type::Auto
      }
      continue
    }
    if self.match_token(Inline) {
      continue
    }
    if self.match_token(Signed) {
      continue
    }
    if self.match_token(Unsigned) {
      is_unsigned = true
      continue
    }
    if self.match_token(Long) {
      // Check for 'long long'
      if self.check(Long) {
        ignore(self.advance())
        // F-P0-2：合成 unsigned/const 修饰，此前提前 return 丢弃
        //（unsigned long long 退化为有符号 long long）
        return @ast.Type::LongLong(is_unsigned, is_const)
      }
      continue
    }
    if self.match_token(Short) {
      continue
    }
    // GCC 扩展 typeof：在类型限定符循环中直接解析并返回
    if self.check(Identifier) {
      let name = self.current().text
      if name == "typeof" ||
        name == "__typeof__" ||
        name == "__typeof" ||
        name == "typeof_unqual" ||
        name == "__typeof_unqual__" ||
        name == "__typeof_unqual" {
        // C23 typeof_unqual（E1）：推导结果剥离顶层限定符
        let unqual = name.contains("unqual")
        ignore(self.advance())
        ignore(self.consume(LParen, "typeof 后预期 '('"))
        let expr = self.parse_expression(depth)
        ignore(self.consume(RParen, "typeof 预期 ')'"))
        return @ast.Type::Typeof(expr, is_const, unqual)
      }
    }
    break
  }
  let t : @ast.Type = if self.match_token(Int) {
    if is_unsigned {
      @ast.Type::unsigned_int()
    } else {
      @ast.Type::int()
    }
  } else if self.match_token(Void) {
    @ast.Type::void_type()
  } else if self.match_token(Float) {
    @ast.Type::float()
  } else if self.match_token(Double) {
    @ast.Type::double()
  } else if self.match_token(Union) {
    if self.check(Identifier) {
      let name_tok = self.advance()
      @ast.Type::union_type(name_tok.text)
    } else {
      @ast.Type::union_type("")
    }
  } else if self.check(Long) {
    ignore(self.advance())
    if self.check(Long) {
      ignore(self.advance())
    }
    // F-P0-2：与限定符循环内的 long long 分支一致合成修饰
    @ast.Type::LongLong(is_unsigned, is_const)
  } else if self.match_token(Char) {
    if is_unsigned {
      @ast.Type::Char(true, false)
    } else {
      @ast.Type::char()
    }
  } else if self.match_token(Struct) {
    if self.check(Identifier) {
      let name_tok = self.advance()
      // C++ 模式下，如果 struct 名字已注册为类型（class 或 struct），返回该类型
      if self.is_cpp_mode {
        match self.typedef_names.get(name_tok.text) {
          Some(ty) => return ty
          None => ()
        }
      }
      @ast.Type::struct_type(name_tok.text)
    } else if self.check(LBrace) {
      let prev = self.previous()
      let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
      let name = "__anon_struct_\{self.pos}"
      let decl = self.parse_struct_body(name, loc, depth)
      self.anonymous_structs.push(decl)
      @ast.Type::struct_type(name)
    } else {
      let cur = self.current()
      self.errors.push({
        message: "struct 后预期标识符或 '{'",
        line: cur.loc.line,
        column: cur.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      @ast.Type::int()
    }
  } else if self.match_token(Enum) {
    if self.check(Identifier) {
      let name_tok = self.advance()
      // C23 enum : T：已声明的具名 enum 返回其底层类型；未声明的按 int
      // 登记并返回
      match self.typedef_names.get(name_tok.text) {
        Some(t) => return t
        None => self.typedef_names.set(name_tok.text, @ast.Type::int())
      }
    }
    @ast.Type::int()
  } else if self.match_token(Bool) {
    @ast.Type::int()
  } else if self.check(Identifier) {
    let name = self.current().text
    if self.is_cpp_mode && self.template_names.contains(name) {
      ignore(self.advance())
      // 检查是否是模板类实例化语法，如 vector<int>
      if self.check(Lt) {
        ignore(self.advance()) // consume '<'
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
        @ast.Type::TemplateId(name, args, false)
      } else {
        @ast.Type::Class(name, false)
      }
    } else {
      match self.typedef_names.get(name) {
        Some(ty) => {
          ignore(self.advance())
          // C++ 限定嵌套类型名，如 typedef 的类名后接 ::Inner。
          // 保留 `Class::Class()` 类外构造函数定义的可能性——当 :: 后是
          // 同名标识符时不在这里 consume。
          if self.is_cpp_mode && self.check(ColonColon) {
            match ty {
              Class(class_name, _) =>
                if self.peek(1).ty == Identifier &&
                  self.peek(1).text != class_name {
                  let mut full_name = class_name
                  while self.check(ColonColon) {
                    ignore(self.advance())
                    let inner = self.consume(Identifier, "预期嵌套类名")
                    full_name = full_name + "__" + inner.text
                  }
                  return @ast.Type::Class(full_name, false)
                }
              _ => ()
            }
          }
          ty
        }
        None =>
          if self.is_cpp_mode {
            let first_name = self.advance().text
            // 类外构造函数定义 `Class::Class()` 不要在这里 consume ::Class
            let mut cname = first_name
            while self.check(ColonColon) &&
                  !(self.peek(1).ty == Identifier &&
                  self.peek(1).text == first_name) {
              ignore(self.advance()) // consume '::'
              let inner = self.consume(Identifier, "预期嵌套类名")
              cname = cname + "__" + inner.text
            }
            @ast.Type::Class(cname, false)
            // 未知标识符不消费（Rust 同）：token 留给声明符作名字
          } else if is_unsigned {
            @ast.Type::unsigned_int()
          } else {
            @ast.Type::int()
          }
      }
    }
  } else if is_unsigned {
    @ast.Type::unsigned_int()
  } else {
    @ast.Type::int()
  }
  if is_unsigned {
    let bad = match t {
      Int(_, _) | Char(_, _) => false
      _ => true
    }
    if bad {
      let kind_name = match t {
        Float(_) => "float"
        Double(_) => "double"
        Struct(_, _) => "struct"
        Void(_) => "void"
        Pointer(_, _) => "指针"
        Array(_, _, _, _, _, _) => "数组"
        _ => "此"
      }
      let cur = self.current()
      self.errors.push({
        message: "'unsigned' 不能修饰'\{kind_name}' 类型",
        line: cur.loc.line,
        column: cur.loc.column,
        code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
      })
      return @ast.Type::int()
    }
  }
  t.set_const(is_const)
}

///|
fn Parser::parse_declarator(
  self : Parser,
  base_type : @ast.Type,
  depth : Int,
) -> (@ast.Type, String) {
  let dguard = DeclaratorGuard::init()
  let r = self.parse_declarator_node(dguard, false, true, depth)
  let name = match r.1 {
    Some(n) => n
    None => ""
  }
  let ty = self.interpret_declarator_node(r.0, base_type, depth)
  (ty, name)
}

///|
/// 解析变量声明符：允许指针/引用/数组后缀，但不把 '(' 当作函数声明符后缀，
/// 以便支持 C++ 构造函数初始化语法 `Type name(args);`。
fn Parser::parse_var_declarator(
  self : Parser,
  base_type : @ast.Type,
  depth : Int,
) -> (@ast.Type, String) {
  let dguard = DeclaratorGuard::init()
  let r = self.parse_declarator_node(dguard, false, true, depth)
  let name = match r.1 {
    Some(n) => n
    None => ""
  }
  let ty = self.interpret_declarator_node(r.0, base_type, depth)
  (ty, name)
}

///|
enum DeclPrefix {
  PtrPrefix
  RefPrefix(Bool) // is_const
  RValPrefix
}

///|
/// 解析声明符节点树（C 螺旋规则）。
/// `is_abstract = true` 时用于 `sizeof(type)` 等抽象声明符场景，不读取
/// 标识符且不检查复杂度。
fn Parser::parse_declarator_node(
  self : Parser,
  dguard : DeclaratorGuard,
  is_abstract : Bool,
  allow_function_suffix : Bool,
  depth : Int,
) -> (DeclaratorNode, String?) {
  let prefixes : Array[DeclPrefix] = []
  // F-P0-1：ptr_count 限深（10 万个 `*` 的嵌套 DeclaratorNode 会使后续
  // 递归遍历栈溢出）
  while self.match_token(Star) {
    if !is_abstract {
      dguard.ptr_count += 1
      if dguard.ptr_count > MAX_DECLARATOR_PTR {
        let cur = self.current()
        self.errors.push({
          message: "声明符指针层级过深（超过 \{MAX_DECLARATOR_PTR} 层）",
          line: cur.loc.line,
          column: cur.loc.column,
          code: @diag.ErrorCode::E1007_ComplexDeclarator.code(),
        })
        // 吞掉剩余连续 `*`，避免它们污染后续解析
        while self.match_token(Star) {
          ()
        }
        break
      }
    }
    prefixes.push(PtrPrefix)
    // 跳过指针限定符（const/volatile/restrict），教学 VM 中无特殊语义
    while self.match_token(Const) ||
          self.match_token(Volatile) ||
          self.match_token(Restrict) {
      ()
    }
  }
  if self.is_cpp_mode {
    while self.match_token(Ampersand) {
      if !is_abstract {
        dguard.ptr_count += 1
        if dguard.ptr_count > MAX_DECLARATOR_PTR {
          let cur = self.current()
          self.errors.push({
            message: "声明符引用层级过深（超过 \{MAX_DECLARATOR_PTR} 层）",
            line: cur.loc.line,
            column: cur.loc.column,
            code: @diag.ErrorCode::E1007_ComplexDeclarator.code(),
          })
          while self.match_token(Ampersand) {
            ()
          }
          break
        }
      }
      let is_const = self.match_token(Const)
      prefixes.push(RefPrefix(is_const))
    }
    while self.match_token(AndAnd) {
      if !is_abstract {
        dguard.ptr_count += 1
        if dguard.ptr_count > MAX_DECLARATOR_PTR {
          let cur = self.current()
          self.errors.push({
            message: "声明符引用层级过深（超过 \{MAX_DECLARATOR_PTR} 层）",
            line: cur.loc.line,
            column: cur.loc.column,
            code: @diag.ErrorCode::E1007_ComplexDeclarator.code(),
          })
          while self.match_token(AndAnd) {
            ()
          }
          break
        }
      }
      prefixes.push(RValPrefix)
    }
  }
  let head : (String?, DeclaratorNode) = if self.match_token(LParen) {
    if !is_abstract {
      dguard.paren_depth += 1
      if dguard.paren_depth > 2 {
        let cur = self.current()
        self.errors.push({
          message: "声明符括号嵌套过深",
          line: cur.loc.line,
          column: cur.loc.column,
          code: @diag.ErrorCode::E1007_ComplexDeclarator.code(),
        })
        while !self.check(RParen) && !self.is_at_end() {
          ignore(self.advance())
        }
        ignore(self.match_token(RParen))
        dguard.paren_depth -= 1
        return (Base, Some(""))
      }
    }
    let inner = self.parse_declarator_node(
      dguard, is_abstract, allow_function_suffix, depth,
    )
    ignore(self.consume(RParen, "预期 ')'"))
    if !is_abstract {
      dguard.paren_depth -= 1
    }
    (inner.1, inner.0)
  } else if is_abstract {
    (None, Base)
  } else {
    let name_tok = self.consume(Identifier, "预期标识符名称")
    (Some(name_tok.text), Base)
  }
  let name = head.0
  let mut node = head.1
  // 收集后缀（它们绑定更紧）。
  // P1：suffix_count 拦链式后缀（`a[1][1]...` 与 `sizeof(int[1]x1300)`），
  // 抽象路径同样计数——两路都拦
  let suffixes : Array[DeclaratorSuffix] = []
  while true {
    if self.match_token(LBracket) {
      dguard.suffix_count += 1
      if dguard.suffix_count > MAX_DECLARATOR_SUFFIX {
        self.declarator_suffix_overflow(
          MAX_DECLARATOR_SUFFIX,
          allow_function_suffix,
          LBracket,
          depth,
        )
        suffixes.clear()
        break
      }
      let size_expr : @ast.Expr? = if self.check(RBracket) {
        None
      } else {
        Some(self.parse_assign(depth))
      }
      ignore(self.consume(RBracket, "预期 ']'"))
      suffixes.push(ArrSuffix(size_expr))
    } else if allow_function_suffix && self.match_token(LParen) {
      dguard.suffix_count += 1
      if dguard.suffix_count > MAX_DECLARATOR_SUFFIX {
        self.declarator_suffix_overflow(
          MAX_DECLARATOR_SUFFIX,
          allow_function_suffix,
          LParen,
          depth,
        )
        suffixes.clear()
        break
      }
      let pl = self.parse_param_list(depth)
      ignore(self.consume(RParen, "预期 ')'"))
      suffixes.push(FuncSuffix(pl.0, pl.1))
    } else {
      break
    }
  }
  // 先应用后缀（它们绑定到标识符更紧）
  for suffix in suffixes {
    match suffix {
      ArrSuffix(size_expr) => node = Array(node, size_expr)
      FuncSuffix(params, is_variadic) =>
        node = Function(node, params, is_variadic)
    }
  }
  // 再应用前缀（从外到内，但解释时从内到外）
  for prefix in prefixes {
    match prefix {
      PtrPrefix => node = Pointer(node)
      RefPrefix(is_const) => node = Reference(node, is_const)
      RValPrefix => node = RValueRef(node)
    }
  }
  if !is_abstract {
    dguard.cross_count = node_cross_count(node)
    if dguard.cross_count > 4 {
      let cur = self.current()
      self.errors.push({
        message: "声明符过于复杂",
        line: cur.loc.line,
        column: cur.loc.column,
        code: 1007,
      })
      return (Base, name)
    }
  }
  (node, name)
}

///|
/// P1：声明符后缀超限——报 E1006 并吞掉剩余连续后缀（`[expr]` /
/// `(params)`），让外层解析在分号/逗号处干净收敛。已收集的后缀链由
/// 调用方丢弃：interpret 递归解释它同样会溢出。`opened` 是超限时刻已被
/// 外层 match 消费的左括号——先补齐当前后缀的右半，再吞完整后缀。
fn Parser::declarator_suffix_overflow(
  self : Parser,
  limit : Int,
  allow_function_suffix : Bool,
  opened : @token.TokenType,
  depth : Int,
) -> Unit {
  let cur = self.current()
  self.errors.push({
    message: "声明符数组/函数后缀层级过深（超过 \{limit} 层），已停止解析该声明符",
    line: cur.loc.line,
    column: cur.loc.column,
    code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
  })
  let closing : @token.TokenType = if opened == LParen {
    ignore(self.parse_param_list(depth))
    RParen
  } else {
    if !self.check(RBracket) {
      ignore(self.parse_assign(depth))
    }
    RBracket
  }
  if !self.match_token(closing) {
    return // 右半吞失败（如 EOF）：放弃对齐，交给外层报语法错
  }
  while true {
    if self.match_token(LBracket) {
      if !self.check(RBracket) {
        ignore(self.parse_assign(depth))
      }
      if !self.match_token(RBracket) {
        break
      }
    } else if allow_function_suffix && self.match_token(LParen) {
      ignore(self.parse_param_list(depth))
      if !self.match_token(RParen) {
        break
      }
    } else {
      break
    }
  }
}

///|
/// 数组维度信息（U1#12：折叠一元负字面量让 typeck 报"负数组大小"）。
/// 返回 (array_size, is_vla, vla_dim)。
fn array_dim_info(size_expr : @ast.Expr?) -> (Int, Bool, @ast.Expr?) {
  match size_expr {
    None => (-1, false, None)
    Some(expr) =>
      match expr {
        Literal(value, _, _) => (value, false, None)
        Unary(op, operand, _, _) =>
          match op {
            @ast.UnaryOp::Neg =>
              match operand {
                Literal(value, _, _) => (-value, false, None)
                _ => (0, true, Some(expr))
              }
            _ => (0, true, Some(expr))
          }
        _ => (0, true, Some(expr))
      }
  }
}

///|
/// 声明符节点树解释（C 螺旋规则：从内到外；照搬 interpret_declarator_node
/// 的全部特判——Pointer(Array) 函数指针数组 / Array(Pointer) 数组指针 /
/// Function(Pointer(Array)) 函数指针数组 / 多维数组 dims 合并）。
///
/// 递归深度由 MAX_DECLARATOR_SUFFIX（收集期）封顶 1250——不加 depth
/// 参数（Rust 同；该上限在 MoonBit 各后端栈预算内）。
fn Parser::interpret_declarator_node(
  self : Parser,
  node : DeclaratorNode,
  base_type : @ast.Type,
  depth : Int,
) -> @ast.Type {
  ignore(depth)
  match node {
    Base => base_type
    Reference(inner, is_const) => {
      let inner_ty = self.interpret_declarator_node(inner, base_type, depth)
      @ast.Type::Reference(inner_ty, is_const)
    }
    RValueRef(inner) => {
      let inner_ty = self.interpret_declarator_node(inner, base_type, depth)
      @ast.Type::RValueRef(inner_ty)
    }
    Pointer(inner) =>
      match inner {
        Array(array_inner, size_expr) => {
          // int (*a[N])[] 形态：Pointer(Array) → 元素为指针的数组
          let elem_ty = self.interpret_declarator_node(
            array_inner, base_type, depth,
          )
          let info = array_dim_info(size_expr)
          let size = info.0
          let is_vla = info.1
          let vla_dim = info.2
          let vla_dims : Array[@ast.Expr] = match vla_dim {
            Some(e) => [e]
            None => []
          }
          @ast.Type::Array(
            @ast.Type::pointer_to(elem_ty),
            size,
            [size],
            false,
            is_vla,
            vla_dims,
          )
        }
        _ => {
          let inner_ty = self.interpret_declarator_node(inner, base_type, depth)
          @ast.Type::pointer_to(inner_ty)
        }
      }
    Array(inner, size_expr) => {
      let info = array_dim_info(size_expr)
      let size = info.0
      let is_vla = info.1
      let vla_dim = info.2
      match inner {
        Pointer(ptr_inner) => {
          // int (*a)[N] 形态：Array(Pointer) → 指向数组的指针
          let elem_ty = self.interpret_declarator_node(
            ptr_inner, base_type, depth,
          )
          let vla_dims : Array[@ast.Expr] = match vla_dim {
            Some(e) => [e]
            None => []
          }
          @ast.Type::Pointer(
            @ast.Type::Array(elem_ty, size, [size], false, is_vla, vla_dims),
            false,
          )
        }
        _ => {
          // 多维数组：合并内层 dims（int a[2][3] → dims=[2,3]）。
          // 纯 Array 链迭代下钻 + 由内向外折叠（J1 教训的 MoonBit 复现
          // 实测：1200 层后缀在 wasm 测试栈预算内递归解释即溢出——收集
          // 期 1250 预算内的合法深链必须存活）。折叠语义与 Rust 逐层
          // 递归的 `_` 合并分支同构：每层消费下层结果的 dims/size/vla。
          // （Rust 在 Pointer 分支前对 inner_ty 的先算后弃调用不照搬——
          // interpret 纯构造无副作用，省略等价）
          let layers : Array[(Int, Bool, @ast.Expr?)] = [
            (size, is_vla, vla_dim),
          ]
          let mut tail = inner
          while true {
            match tail {
              Array(next, se2) => {
                layers.push(array_dim_info(se2))
                tail = next
              }
              _ => break
            }
          }
          // 尾节点（首个非 Array；深度受 paren/cross 限制，浅解释安全）
          let mut inner_ty = self.interpret_declarator_node(
            tail, base_type, depth,
          )
          // 由内向外折叠（Rust 递归返回路径的同一合并序）
          let mut k = layers.length() - 1
          while k >= 0 {
            let l = layers[k]
            inner_ty = merge_array_wrap(inner_ty, l.0, l.1, l.2)
            k -= 1
          }
          inner_ty
        }
      }
    }
    Function(inner, params, is_variadic) => {
      // 每个构造点独立收集 param_types（Rust 各分支独立 Vec——避免
      // Array 引用共享）
      fn collect(params : Array[@ast.Param]) -> Array[@ast.Type] {
        let ts : Array[@ast.Type] = []
        for p in params {
          ts.push(p.ty)
        }
        ts
      }
      match inner {
        Pointer(ptr_inner) =>
          match ptr_inner {
            Array(array_inner, size_expr) => {
              // (*fp[N])(params) → function pointer array
              let elem_ty = self.interpret_declarator_node(
                array_inner, base_type, depth,
              )
              let info = array_dim_info(size_expr)
              let size = info.0
              let is_vla = info.1
              let vla_dim = info.2
              let vla_dims : Array[@ast.Expr] = match vla_dim {
                Some(e) => [e]
                None => []
              }
              @ast.Type::Array(
                @ast.Type::Pointer(
                  @ast.Type::Function(
                    elem_ty,
                    collect(params),
                    false,
                    is_variadic,
                  ),
                  false,
                ),
                size,
                [size],
                false,
                is_vla,
                vla_dims,
              )
            }
            _ => {
              let func_ptr_type = @ast.Type::Pointer(
                @ast.Type::Function(
                  base_type,
                  collect(params),
                  false,
                  is_variadic,
                ),
                false,
              )
              self.interpret_declarator_node(ptr_inner, func_ptr_type, depth)
            }
          }
        _ => {
          let inner_ty = self.interpret_declarator_node(inner, base_type, depth)
          @ast.Type::Pointer(
            @ast.Type::Function(inner_ty, collect(params), false, is_variadic),
            false,
          )
        }
      }
    }
  }
}

///|
/// 形参表（壳挂点；P1 批挂入——param_list → declarator → 函数后缀 →
/// param_list 互递归通道）。
fn Parser::parse_param_list(
  self : Parser,
  depth : Int,
) -> (Array[@ast.Param], Bool) {
  if depth >= MAX_PARSE_DEPTH {
    self.depth_exceeded("函数声明符")
    return ([], false)
  }
  self.parse_param_list_inner(depth + 1)
}

///|
fn Parser::parse_param_list_inner(
  self : Parser,
  depth : Int,
) -> (Array[@ast.Param], Bool) {
  let params : Array[@ast.Param] = []
  let mut is_variadic = false
  if self.check(RParen) {
    return (params, is_variadic)
  }
  if self.check(Void) && self.peek(1).ty == RParen {
    ignore(self.advance())
    return (params, is_variadic)
  }
  while true {
    // C 变参参数列表末尾的 "..."
    if self.check(Ellipsis) {
      ignore(self.advance())
      is_variadic = true
      break
    }
    let base_type = self.parse_base_type(depth)
    let np : (@ast.Type, String) = if self.check(Comma) || self.check(RParen) {
      // 无名参数（函数原型声明）：int foo(int);
      (base_type, "")
    } else if self.check(Star) {
      // 前瞻：跳过所有 * 后看是否是 Comma/RParen，以支持 int foo(char *);
      let lookahead = self.look_ahead_skip_stars()
      if lookahead < self.tokens.length() &&
        (
          self.tokens[lookahead].ty == Comma ||
          self.tokens[lookahead].ty == RParen
        ) {
        let mut ty = base_type
        while self.match_token(Star) {
          ty = @ast.Type::pointer_to(ty)
        }
        (ty, "")
      } else {
        let d = self.parse_declarator(base_type, depth)
        (d.0, d.1)
      }
    } else {
      let d = self.parse_declarator(base_type, depth)
      (d.0, d.1)
    }
    let pty = np.0
    let pname = np.1
    // In C, array parameters in function declarations decay to pointers.
    // For int a[5] -> int*; for int a[3][3] -> int (*a)[3].
    let pty2 = if pty.is_array() {
      let is_const = pty.is_const()
      @ast.Type::Pointer(pty.subscript_type(), is_const)
    } else {
      pty
    }
    let default_expr : @ast.Expr? = if self.match_token(Assign) {
      // 默认参数使用赋值表达式级别，避免逗号运算符把后续参数吞掉。
      Some(self.parse_assign(depth))
    } else {
      None
    }
    let cur = self.current()
    params.push({
      ty: pty2,
      name: pname,
      loc: @source.SourceLoc::new(cur.loc.line, cur.loc.column),
      default: default_expr,
    })
    if !self.match_token(Comma) {
      break
    }
  }
  (params, is_variadic)
}

///|
/// Parse optional constructor member initializer list: `: field1(expr1), ...`
fn Parser::parse_ctor_init_list(
  self : Parser,
  depth : Int,
) -> Array[(String, @ast.Expr)] {
  let init_list : Array[(String, @ast.Expr)] = []
  if !self.check(Colon) {
    return init_list
  }
  ignore(self.advance()) // consume ':'
  while true {
    let field_tok = self.advance()
    let field_name = field_tok.text
    ignore(self.consume(LParen, "预期 '('"))
    let init_expr = self.parse_expression(depth)
    ignore(self.consume(RParen, "预期 ')'"))
    init_list.push((field_name, init_expr))
    if !self.check(Comma) {
      break
    }
    ignore(self.advance()) // consume ','
  }
  init_list
}

///|
/// Merge constructor initializer list into the body by prepending
/// `this->field = expr;` assignments at the start of the block.
fn merge_ctor_init_into_body(
  init_list : Array[(String, @ast.Expr)],
  body : @ast.Stmt?,
) -> @ast.Stmt? {
  if init_list.is_empty() {
    return body
  }
  match body {
    Some(Block(stmts, loc)) => {
      let new_stmts : Array[@ast.Stmt] = []
      for pair in init_list {
        let field = pair.0
        let expr = pair.1
        let expr_loc = expr.loc()
        let assign_expr = @ast.Expr::Assign(
          @ast.AssignOp::Assign,
          @ast.Expr::Member(
            @ast.Expr::This(expr_loc, @ast.Type::void_type()),
            field,
            expr_loc,
            @ast.Type::void_type(),
          ),
          expr,
          expr_loc,
          @ast.Type::void_type(),
        )
        new_stmts.push(@ast.Stmt::Expr(assign_expr, expr_loc))
      }
      for s in stmts {
        new_stmts.push(s)
      }
      Some(Block(new_stmts, loc))
    }
    other => other
  }
}

///|
/// 多维合并的单层包装（Rust interpret `Array._` 分支的合并体抽函数）：
/// inner_ty 为 Array 时合并 dims（拷贝 + push size）、传播 vla、
/// size×inner_size（正值）；否则以 inner_ty 为元素建单层数组。
fn merge_array_wrap(
  inner_ty : @ast.Type,
  size : Int,
  is_vla : Bool,
  vla_dim : @ast.Expr?,
) -> @ast.Type {
  match inner_ty {
    Array(element, _, dims, _, vla, vd) => {
      // dims/vla_dims 拷贝（Rust clone；引用别名会把 push 写进内层）
      let inner_dims : Array[Int] = []
      for d in dims {
        inner_dims.push(d)
      }
      let inner_array_size = inner_ty.array_size()
      let inner_is_vla = vla
      let inner_vla_dims : Array[@ast.Expr] = []
      for e in vd {
        inner_vla_dims.push(e)
      }
      inner_dims.push(size)
      if is_vla {
        match vla_dim {
          Some(e) => inner_vla_dims.push(e)
          None => ()
        }
      }
      let array_size = if size > 0 && inner_array_size > 0 {
        size * inner_array_size
      } else {
        size
      }
      @ast.Type::Array(
        element,
        array_size,
        inner_dims,
        false,
        inner_is_vla || is_vla,
        inner_vla_dims,
      )
    }
    _ => {
      let vla_dims : Array[@ast.Expr] = match vla_dim {
        Some(e) => [e]
        None => []
      }
      @ast.Type::Array(inner_ty, size, [size], false, is_vla, vla_dims)
    }
  }
}
