///|
/// C++ 扩展语法（照搬 Rust `cpp.rs`：状态初始化 + 顶层入口下沉）。
/// TODO(#D08/#D10) 同款债登记：顶层分发仍与 C 状态机共处 parse_program，
/// `is_cpp_mode` 布尔贯穿——独立包切分随 S9 裁定批（A10）。

///|
/// 预注册 C++ 内置容器类型名与模板名。
fn register_cpp_builtin_types(
  typedef_names : Map[String, @ast.Type],
  template_names : Set[String],
) -> Unit {
  for
    name in [
      "vitro_vec_int", "vitro_vec_float", "vitro_vec_char", "vitro_string", "vitro_list_int",
    ] {
    typedef_names.set(name, @ast.Type::Class(name, false))
  }
  // 预注册内置容器模板名，支持显式实例化写法。
  // 注意：vitro_string 本身是最终类名，不作为模板基名注册，
  // 避免与用户代码中的 typedef struct vitro_string { ... } 冲突。
  for name in ["vitro_vec", "vitro_list"] {
    template_names.add(name)
    // 同时加入 typedef_names，使变量声明等类型上下文能识别该模板名
    typedef_names.set(name, @ast.Type::Class(name, false))
  }
}

///|
/// 尝试解析 C++ 构造函数类外定义，如 `Counter::Counter() { ... }`。
/// 成功压入 program.funcs 返回 true；否则不消费 token 返回 false。
fn Parser::try_parse_cpp_ctor_out_of_line(
  self : Parser,
  program : @ast.ProgramNode,
  base_type : @ast.Type,
  is_static : Bool,
  is_extern : Bool,
) -> Bool {
  let base_is_class = match base_type {
    Class(name, _) => name != ""
    _ => false
  }
  if !self.is_cpp_mode ||
    !base_is_class ||
    !self.check(ColonColon) ||
    self.peek(1).ty != Identifier ||
    self.peek(2).ty != LParen ||
    self.peek(1).text != base_type.name() {
    return false
  }
  ignore(self.consume(ColonColon, "预期 '::'"))
  let name_tok = self.consume(Identifier, "预期构造函数名")
  ignore(self.consume(LParen, "预期 '('"))
  let pl = self.parse_param_list(0)
  ignore(self.consume(RParen, "预期 ')'"))
  let body : @ast.Stmt? = if self.check(LBrace) {
    Some(self.parse_block(0))
  } else {
    ignore(self.consume(Semicolon, "构造函数声明后预期 ';' 或 '{'"))
    None
  }
  program.funcs.push({
    loc: @source.SourceLoc::new(name_tok.loc.line, name_tok.loc.column),
    return_type: @ast.Type::void_type(),
    name: "__ctor__\{name_tok.text}",
    params: pl.0,
    body,
    is_static,
    is_extern,
    source_file: "",
    is_variadic: false,
  })
  true
}

///|
/// 尝试解析 C++ 限定静态字段定义，如 `int A::count = 0;`。
/// 成功压入 program.globals 返回 true；否则不消费 token 返回 false。
fn Parser::try_parse_cpp_qualified_static_field(
  self : Parser,
  program : @ast.ProgramNode,
  base_type : @ast.Type,
  is_static : Bool,
  is_extern : Bool,
) -> Bool {
  if !self.is_cpp_mode ||
    self.current().ty != Identifier ||
    self.peek(1).ty != ColonColon ||
    self.peek(2).ty != Identifier {
    return false
  }
  let ty = base_type
  let class_name = self.consume(Identifier, "预期类名").text
  ignore(self.consume(ColonColon, "预期 '::'"))
  let field_tok = self.consume(Identifier, "预期字段名")
  let name = "\{class_name}__\{field_tok.text}"
  let init : @ast.Expr? = if is_extern {
    None
  } else if self.match_token(Assign) {
    if self.check(LBrace) {
      Some(self.parse_init_list(0))
    } else {
      Some(self.parse_assign(0))
    }
  } else {
    None
  }
  let prev = self.previous()
  program.globals.push({
    loc: @source.SourceLoc::new(prev.loc.line, prev.loc.column),
    ty,
    name,
    init,
    is_static,
    is_extern,
    source_file: "",
  })
  ignore(self.consume(Semicolon, "静态成员定义后预期 ';'"))
  true
}

///|
/// 解析 C++ `class` 顶层声明。
fn Parser::parse_cpp_class_decl(
  self : Parser,
  program : @ast.ProgramNode,
) -> Unit {
  let class_decl = self.parse_class_decl(0)
  self.typedef_names.set(
    class_decl.name,
    @ast.Type::Class(class_decl.name, false),
  )
  program.classes.push(class_decl)
}

///|
/// 解析 C++ `template` 顶层声明或显式实例化。
fn Parser::parse_cpp_template_decl(
  self : Parser,
  program : @ast.ProgramNode,
) -> Unit {
  if self.is_template_explicit_instantiation() {
    let inst = self.parse_template_instantiation(0)
    program.template_instantiations.push(inst)
  } else {
    let template_decl = self.parse_template_decl(0)
    // 将类模板名加入 typedef_names
    match template_decl.decl {
      TClass(c) =>
        self.typedef_names.set(c.name, @ast.Type::Class(c.name, false))
      _ => ()
    }
    // 类模板方法类外定义：如 template<class T> void Box<T>::set(T x) {...}
    // 已把 body 合并到对应类模板的 method 声明中，避免生成独立函数模板。
    // MoonBit 结构不可变——Rust 的 *body = f.body.clone() 经拷贝重建实现
    let mut merged = false
    match template_decl.decl {
      TFunc(f) =>
        match f.body {
          Some(body) =>
            // name.split_once("__")：首个 "__" 前为类名
            match find_first_dbl_underscore(f.name) {
              Some(i) => {
                let class_name = f.name.substring(start=0, end=i)
                let method_name = f.name.substring(
                  start=i + 2,
                  end=f.name.length(),
                )
                for k, t in program.templates {
                  match t.decl {
                    TClass(c) =>
                      if c.name == class_name {
                        match fill_class_method_body(c, method_name, body) {
                          Some(nc) => {
                            program.templates[k] = {
                              loc: t.loc,
                              params: t.params,
                              decl: TClass(nc),
                            }
                            merged = true
                            break
                          }
                          None => ()
                        }
                      }
                    _ => ()
                  }
                }
              }
              None => ()
            }
          None => ()
        }
      _ => ()
    }
    if !merged {
      program.templates.push(template_decl)
    }
  }
}

///|
/// 首个 "__" 的字节偏移（Rust split_once("__") 的定位半边）。
fn find_first_dbl_underscore(s : String) -> Int? {
  // '_' = U+005F；按 UTF-16 码元扫（字节位置与码元位置在 ASCII 段一致，
  // 类名/方法名不含非 ASCII——mangle 面同前提）
  let mut i = 0
  while i + 1 < s.length() {
    if s.get(i) == Some(0x5F) && s.get(i + 1) == Some(0x5F) {
      return Some(i)
    }
    i += 1
  }
  None
}

///|
/// 类模板方法类外定义合并：members 中首个同名且无 body 的 Method 填入
/// body（拷贝重建；Rust `*body = f.body.clone()` 等价）。无命中返回 None。
fn fill_class_method_body(
  c : @ast.ClassDecl,
  method_name : String,
  body : @ast.Stmt,
) -> @ast.ClassDecl? {
  let members : Array[@ast.ClassMember] = []
  let mut hit = false
  for m in c.members {
    if !hit {
      match m {
        Method(name, ret, params, None, is_virtual, access, is_static, is_const) =>
          if name == method_name {
            members.push(
              Method(
                name,
                ret,
                params,
                Some(body),
                is_virtual,
                access,
                is_static,
                is_const,
              ),
            )
            hit = true
            continue
          }
        _ => ()
      }
    }
    members.push(m)
  }
  if hit {
    Some({ loc: c.loc, name: c.name, base: c.base, members, vtable: c.vtable, })
  } else {
    None
  }
}

///|
/// 解析 C++ 模式下有名字的 `struct` 声明为 class-like。
/// 调用时当前位置应在 `struct` 关键字处，与 parse_struct_decl 相同。
fn Parser::parse_cpp_class_like_struct_decl(
  self : Parser,
  program : @ast.ProgramNode,
) -> Unit {
  let class_decl = self.parse_class_decl_inner(true, 0)
  self.typedef_names.set(
    class_decl.name,
    @ast.Type::Class(class_decl.name, false),
  )
  program.classes.push(class_decl)
}
