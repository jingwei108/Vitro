///|
/// Vitro 语法分析器（S3 片）——从 token 流解析 AST。
///
/// 照搬 Rust oracle `native/crates/vitro_parser/src/lib.rs`（4,879 行十文件）
/// 的解析语义；**防护形态不复刻**（勘察 parser 报告 M1，第一阶段计划 P1）：
///
/// 1. **深度统一入口**：Rust 的共享计数器字段 + 8 处壳挂点
///    （statement/assign/ternary/unary/primary/init_list/abstract_declarator/
///    param_list）改为显式 `depth` 参数沿调用链传递——计数语义与 Rust
///    **完全同构**（壳函数检查 `depth >= MAX_PARSE_DEPTH` 对应 Rust 先检
///    后增；body 与下层传 `depth+1`），但新增递归点必须声明 depth（编译
///    器强制），不再依赖人工枚举挂点（J1 的方法论根因）。
/// 2. **回滚快照全量**：Rust 三元快照（pos/errors/anonymous_structs）+
///    三处手工补 typedef_names——本实现单一 Rollback 含全部七类解析期
///    副作用（typedef_names/template_names 不可变结构共享快照 + pos/
///    errors/anon/current_class 长度 + next_lambda_id），A5 回滚面缺口
///    结构性消除。可观测等价：手工补的三处含在 restore 内；sizeof 处
///    的差异按 A5 分析不可观测（污染值与首次写入同值 + 合法 C 不可达）。
/// 3. **活性内部断言**：主循环零推进记账（`stall_count`），不改变恢复
///    控制流（照搬 Rust 三层活性约定），防线侧经 dump 出口观测。
///
/// 病态深度预算四常量照搬（P1 后 Rust 现值）：MAX_PARSE_DEPTH=256 /
/// MAX_AST_DEPTH=512 / MAX_DECLARATOR_SUFFIX=1250 / MAX_TYPE_DEPTH=1250。
pub(all) struct ParseError {
  message : String
  line : Int
  column : Int
  code : Int
} derive(Eq, @debug.Debug)

///|
/// 解析结果（单值返回，S2 LexResult 同款惯例；对应 Rust
/// `parse() -> (Option<ProgramNode>, Vec<ParseError>)`）。
pub(all) struct ParseResult {
  program : @ast.ProgramNode?
  errors : Array[ParseError]
  /// 活性内部断言计数：主循环零推进次数（生产零消费；防线侧 dump 观测，
  /// 勘察 6-3 的 GC 语言观测义务——故障信号从"内存涨"弱化为"无诊断"，
  /// 必须显式记账）
  stall_count : Int
}

///|
/// 声明符节点树（C 螺旋规则中间表示；照搬 Rust `DeclaratorNode`）。
pub(all) enum DeclaratorNode {
  Base
  Pointer(DeclaratorNode)
  Reference(DeclaratorNode, Bool) // is_const
  RValueRef(DeclaratorNode)
  Array(DeclaratorNode, @ast.Expr?) // size_expr
  Function(DeclaratorNode, Array[@ast.Param], Bool) // is_variadic
} derive(@debug.Debug)

///|
enum DeclaratorSuffix {
  ArrSuffix(@ast.Expr?) // size_expr
  FuncSuffix(Array[@ast.Param], Bool) // is_variadic
}

///|
/// 声明符复杂度保险丝组（照搬 Rust：paren_depth/ptr_count/suffix_count/
/// cross_count——P1 后 suffix_count 已接上比较，非死字段）。
struct DeclaratorGuard {
  mut paren_depth : Int
  mut ptr_count : Int
  mut suffix_count : Int
  mut cross_count : Int
}

///|
fn DeclaratorGuard::init() -> DeclaratorGuard {
  { paren_depth: 0, ptr_count: 0, suffix_count: 0, cross_count: 0, }
}

///|
fn node_cross_count(node : DeclaratorNode) -> Int {
  match node {
    Base => 0
    Pointer(inner) | Reference(inner, _) | RValueRef(inner) => {
      let add = match inner {
        Array(_, _) | Function(_, _, _) => 1
        _ => 0
      }
      node_cross_count(inner) + add
    }
    Array(inner, _) | Function(inner, _, _) => node_cross_count(inner)
  }
}

///|
/// 统一回滚点——**七字段全量快照**（形态改造，见包注释 2）。
/// typedef_names / template_names 为可变哈希结构，快照 = 整表拷贝
///（教学程序的符号表小，O(n) 拷贝可接受；回滚即换引用）。
struct Rollback {
  pos : Int
  errors_len : Int
  anon_len : Int
  typedef_names : Map[String, @ast.Type]
  template_names : Set[String]
  current_class_len : Int
  next_lambda_id : Int
}

///|
/// 递归深度上限（照搬 Rust 256；勘察 7-S1 实测 wasm-gc 21 帧瀑布 854 层，
/// 余量充足）。计数口径 = 活跃壳数（每层括号嵌套 4 计数：assign+ternary+
/// unary+primary 壳），与 Rust 共享计数器同构——阈值样本（62/63 层括号）
/// 的触发边界逐位一致是 E3 对拍前提。
const MAX_PARSE_DEPTH : Int = 256

///|
/// 后置 AST 深度预算（照搬 Rust 512；typeck/快照等递归遍历安全上界）。
const MAX_AST_DEPTH : Int = 512

///|
/// 声明符后缀（数组/函数）链式层级上限（照搬 Rust P1 值 1250：release
/// 1200 层存活 / 1300 层崩溃边界之下的防护位）。
const MAX_DECLARATOR_SUFFIX : Int = 1250

///|
/// 类型树深度预算（照搬 Rust P1 值 1250；深 Type 对 MAX_AST_DEPTH 隐身
/// 的后置兜底）。
const MAX_TYPE_DEPTH : Int = 1250

///|
/// 声明符指针前缀上限（照搬 Rust 32）。
const MAX_DECLARATOR_PTR : Int = 32

///|
/// 深度超限中性占位（照搬 Rust `Expr::default()`：Literal 0 + 默认坐标 +
/// Void）。
fn default_expr() -> @ast.Expr {
  @ast.Expr::Literal(0, @source.SourceLoc::default(), @ast.Type::void_type())
}

///|
struct Parser {
  tokens : Array[@token.Token]
  errors : Array[ParseError]
  mut typedef_names : Map[String, @ast.Type]
  mut template_names : Set[String]
  mut pos : Int
  anonymous_structs : Array[@ast.StructDecl]
  is_cpp_mode : Bool
  mut next_lambda_id : Int
  /// C++ 嵌套类解析上下文：当前外层类名栈
  current_class : Array[String]
  /// 活性记账：主循环零推进次数（推进控制流照搬 Rust，此处仅观测）
  mut stall_count : Int
}

///|
fn Parser::save(self : Parser) -> Rollback {
  {
    pos: self.pos,
    errors_len: self.errors.length(),
    anon_len: self.anonymous_structs.length(),
    typedef_names: self.typedef_names.copy(),
    template_names: self.template_names.copy(),
    current_class_len: self.current_class.length(),
    next_lambda_id: self.next_lambda_id,
  }
}

///|
fn Parser::restore(self : Parser, rb : Rollback) -> Unit {
  self.pos = rb.pos
  self.errors.truncate(rb.errors_len)
  self.anonymous_structs.truncate(rb.anon_len)
  self.typedef_names = rb.typedef_names
  self.template_names = rb.template_names
  self.current_class.truncate(rb.current_class_len)
  self.next_lambda_id = rb.next_lambda_id
}

///|
/// 构造解析器（照搬 Rust `with_mode`：预定义 FILE 类型 + C++ 内置容器）。
fn Parser::with_mode(
  tokens : Array[@token.Token],
  is_cpp_mode : Bool,
) -> Parser {
  let typedef_names : Map[String, @ast.Type] = Map([])
  let template_names : Set[String] = Set([])
  // 预定义 FILE 类型（stdio.h 中的不透明结构体指针）
  typedef_names.set("FILE", @ast.Type::void_type())
  // 预注册 C++ 内置容器类型名
  if is_cpp_mode {
    register_cpp_builtin_types(typedef_names, template_names)
  }
  {
    tokens,
    errors: [],
    typedef_names,
    template_names,
    pos: 0,
    anonymous_structs: [],
    is_cpp_mode,
    next_lambda_id: 0,
    current_class: [],
    stall_count: 0,
  }
}

///|
/// 壳挂点深度超限处理（照搬 Rust `enter_depth` 失败分支：记录 E1006 并
/// 跳到文件尾——过深嵌套已无恢复价值，跳到尾部让外层循环自然收敛）。
fn Parser::depth_exceeded(self : Parser, what : String) -> Unit {
  let cur = self.current()
  self.errors.push({
    message: "\{what}嵌套过深（超过 \{MAX_PARSE_DEPTH} 层），已停止解析",
    line: cur.loc.line,
    column: cur.loc.column,
    code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
  })
  let n = self.tokens.length()
  if n > 0 {
    self.pos = n - 1
  }
}

///|
/// 解析入口（对应 Rust `Parser::parse`）：token 流 → (ProgramNode?, errors)。
/// 后置双预算（AST 深度 512 / 类型深度 1250）超限返回 None + E1006（照搬；
/// Rust 的 `mem::forget` 防 Drop 溢出在 GC 下不需要——勘察 7-S7）。
pub fn parse(
  tokens : Array[@token.Token],
  cpp_mode? : Bool = false,
) -> ParseResult {
  let p = Parser::with_mode(tokens, cpp_mode)
  let prog = match p.parse_program() {
    Some(pr) => {
      for d in p.anonymous_structs {
        pr.structs.push(d)
      }
      pr
    }
    None =>
      return { program: None, errors: p.errors, stall_count: p.stall_count, }
  }
  // 后置 AST 深度预算（Expr/Stmt 面）
  match program_max_depth(prog) {
    Some(d) =>
      if d > MAX_AST_DEPTH {
        p.errors.push({
          message: "AST 嵌套深度 \{d} 超过预算 \{MAX_AST_DEPTH}：过深的表达式/初始化链会使后续处理栈溢出，请拆分表达式",
          line: 1,
          column: 1,
          code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
        })
        return { program: None, errors: p.errors, stall_count: p.stall_count, }
      }
    None => ()
  }
  // 类型深度预算（P1：深 Type 对上面的 Expr/Stmt 预算隐身）
  let tmax = program_max_type_depth(prog)
  if tmax > MAX_TYPE_DEPTH {
    p.errors.push({
      message: "类型嵌套深度 \{tmax} 超过预算 \{MAX_TYPE_DEPTH}：过深的类型链（数组后缀/typedef 链）会使后续处理栈溢出",
      line: 1,
      column: 1,
      code: @diag.ErrorCode::E1006_UnsupportedFeature.code(),
    })
    return { program: None, errors: p.errors, stall_count: p.stall_count, }
  }
  { program: Some(prog), errors: p.errors, stall_count: p.stall_count, }
}

// =========================================================================
// 深度测量（照搬 lib.rs 后置预算的测量面；ast 包四函数为迭代显式栈实现）
// =========================================================================

///|
fn program_max_depth(prog : @ast.ProgramNode) -> Int? {
  let mut max = 0
  for g in prog.globals {
    match g.init {
      Some(init) => {
        let d = @ast.expr_depth(init)
        if d > max {
          max = d
        }
      }
      None => ()
    }
  }
  for f in prog.funcs {
    match f.body {
      Some(body) => {
        let d = @ast.stmt_depth(body)
        if d > max {
          max = d
        }
      }
      None => ()
    }
  }
  if max == 0 {
    None
  } else {
    Some(max)
  }
}

///|
fn program_max_type_depth(prog : @ast.ProgramNode) -> Int {
  fn acc(t : @ast.Type, max : Int) -> Int {
    let d = @ast.type_depth(t)
    if d > max {
      d
    } else {
      max
    }
  }
  fn acc_stmt_type(s : @ast.Stmt, max : Int) -> Int {
    let d = @ast.stmt_type_depth(s)
    if d > max {
      d
    } else {
      max
    }
  }
  fn acc_body(b : @ast.Stmt?, max : Int) -> Int {
    match b {
      Some(s) => acc_stmt_type(s, max)
      None => max
    }
  }
  let mut max = 0
  for g in prog.globals {
    max = acc(g.ty, max)
  }
  for sd in prog.structs {
    for f in sd.fields {
      max = acc(f.ty, max)
    }
  }
  for sd in prog.unions {
    for f in sd.fields {
      max = acc(f.ty, max)
    }
  }
  // 嵌套类用显式工作栈遍历（测量器自身不递归，同 Rust depth.rs 纪律）
  let class_stack : Array[@ast.ClassDecl] = []
  for c in prog.classes {
    class_stack.push(c)
  }
  while class_stack.length() > 0 {
    let c = class_stack.remove(class_stack.length() - 1)
    for m in c.members {
      match m {
        Field(_, ty, _, _) => max = acc(ty, max)
        Method(_, ret, params, body, _, _, _, _) => {
          max = acc(ret, max)
          for p in params {
            max = acc(p.ty, max)
          }
          max = acc_body(body, max)
        }
        Constructor(params, body, _, _, _) => {
          for p in params {
            max = acc(p.ty, max)
          }
          max = acc_body(body, max)
        }
        Destructor(body, _, _) => max = acc_body(body, max)
        NestedClass(decl, _) => class_stack.push(decl)
        NestedStruct(decl, _) =>
          for f in decl.fields {
            max = acc(f.ty, max)
          }
      }
    }
  }
  for f in prog.funcs {
    max = acc(f.return_type, max)
    for p in f.params {
      max = acc(p.ty, max)
    }
    max = acc_body(f.body, max)
  }
  max
}

// =========================================================================
// Token helpers（照搬 lib.rs 访问原语；越界哨兵 = Eof/-1/-1 语义）
// =========================================================================

///|
fn eof_token() -> @token.Token {
  {
    ty: Eof,
    text: "",
    loc: { line: -1, column: -1, file_id: 0, },
    byte_off: -1,
    real_line: -1,
  }
}

///|
fn Parser::peek(self : Parser, offset : Int) -> @token.Token {
  if self.pos + offset >= self.tokens.length() {
    return eof_token()
  }
  self.tokens[self.pos + offset]
}

///|
fn Parser::current(self : Parser) -> @token.Token {
  self.peek(0)
}

///|
fn Parser::previous(self : Parser) -> @token.Token {
  if self.pos == 0 {
    return self.peek(0)
  }
  self.tokens[self.pos - 1]
}

///|
fn Parser::check(self : Parser, ty : @token.TokenType) -> Bool {
  if self.is_at_end() {
    return false
  }
  self.current().ty == ty
}

///|
fn Parser::is_at_end(self : Parser) -> Bool {
  self.current().ty == Eof
}

///|
fn Parser::advance(self : Parser) -> @token.Token {
  if !self.is_at_end() {
    self.pos += 1
  }
  if self.pos == 0 {
    return self.peek(0)
  }
  self.tokens[self.pos - 1]
}

///|
fn Parser::match_token(self : Parser, ty : @token.TokenType) -> Bool {
  if self.check(ty) {
    ignore(self.advance())
    true
  } else {
    false
  }
}

///|
fn Parser::consume(
  self : Parser,
  ty : @token.TokenType,
  msg : String,
) -> @token.Token {
  if self.check(ty) {
    return self.advance()
  }
  let code = match ty {
    RBrace => @diag.ErrorCode::E2006_ExpectedClosingBrace
    RParen => @diag.ErrorCode::E2007_ExpectedClosingParen
    RBracket => @diag.ErrorCode::E2008_ExpectedClosingBracket
    _ => @diag.ErrorCode::E2005_ExpectedSemicolon
  }
  // 缺分号报在 previous() 位置（分号本应在处），其余报在 current()——
  // E7 隐性口径，E2 位置列覆盖
  let cur = self.current()
  let prev = self.previous()
  let tmp : (Int, Int) = if ty == Semicolon && self.pos > 0 {
    (prev.loc.line, prev.loc.column)
  } else {
    (cur.loc.line, cur.loc.column)
  }
  self.errors.push({
    message: msg,
    line: tmp.0,
    column: tmp.1,
    code: code.code(),
  })
  // 自动恢复：根据缺失的闭合符号推断恢复集合，减少级联错误
  let recovery : Array[@token.TokenType] = match ty {
    RParen => [RParen, Semicolon]
    RBrace => [RBrace, Semicolon]
    RBracket => [RBracket, Semicolon]
    _ => [Semicolon]
  }
  self.synchronize(recovery)
  self.peek(0)
}

///|
/// 通用同步（照搬：跳到恢复集合中的 token / 语句边界；previous 是分号
/// 则停——可零推进，外层循环兜底是活性三层约定之二三层）。
fn Parser::synchronize(
  self : Parser,
  recovery_set : Array[@token.TokenType],
) -> Unit {
  while !self.is_at_end() {
    let cur = self.current().ty
    if recovery_set.contains(cur) {
      return
    }
    if self.previous().ty == Semicolon {
      return
    }
    match cur {
      Int
      | Void
      | Char
      | Float
      | Double
      | If
      | While
      | Do
      | For
      | Return
      | Break
      | Continue
      | Struct
      | Switch
      | Case
      | Default
      | Typedef
      | Enum
      | Unsigned
      | Long
      | Short
      | Signed
      | Const
      | Bool
      | RBrace => return
      _ => ignore(self.advance())
    }
  }
}

///|
fn Parser::is_type_token(self : Parser) -> Bool {
  if self.check(Int) ||
    self.check(Void) ||
    self.check(Char) ||
    self.check(Float) ||
    self.check(Double) ||
    self.check(Struct) ||
    self.check(Enum) ||
    self.check(Unsigned) ||
    self.check(Long) ||
    self.check(Short) ||
    self.check(Signed) ||
    self.check(Const) ||
    self.check(Volatile) ||
    self.check(Bool) ||
    self.check(Union) {
    return true
  }
  // C++ 模式下 class 和 auto 也是类型 token
  if self.is_cpp_mode && (self.check(Class) || self.check(Auto)) {
    return true
  }
  if self.check(Identifier) {
    let name = self.current().text
    if name == "typeof" ||
      name == "__typeof__" ||
      name == "__typeof" ||
      name == "typeof_unqual" ||
      name == "__typeof_unqual__" ||
      name == "__typeof_unqual" {
      return true
    }
    return self.typedef_names.contains(name)
  }
  false
}

///|
fn Parser::is_static_token(self : Parser) -> Bool {
  self.check(Identifier) && self.current().text == "static"
}

// =========================================================================
// Program（顶层分发照搬 lib.rs parse_program）
// =========================================================================

///|
fn Parser::parse_program(self : Parser) -> @ast.ProgramNode? {
  let program = @ast.ProgramNode::empty()
  while !self.is_at_end() {
    let round_start = self.pos
    // 跳过函数前缀修饰符 inline（可能出现在 static/extern 之前或之后）
    while self.check(Inline) {
      ignore(self.advance())
    }
    if self.check(Identifier) &&
      (
        self.peek(0).text == "_Static_assert" ||
        self.peek(0).text == "static_assert"
      ) {
      self.parse_static_assert(0)
      continue
    }
    // E3：顶层 [[属性]] 前缀——解析并忽略
    if self.check(LBracket) && self.peek(1).ty == LBracket {
      self.skip_attributes()
      continue
    }
    if self.check(Typedef) {
      let checkpoint_rb = self.save()
      ignore(self.advance())
      if self.check(Struct) {
        let s_rb = self.save()
        ignore(self.advance())
        if self.check(Identifier) {
          ignore(self.advance())
        }
        if self.check(LBrace) {
          self.restore(checkpoint_rb)
          self.parse_typedef_struct_decl(program, 0)
          continue
        }
        self.restore(s_rb)
      }
      if self.check(Enum) {
        let e_rb = self.save()
        ignore(self.advance())
        if self.check(Identifier) {
          ignore(self.advance())
        }
        // C23 enum : T：底层类型子句不改变"是否 enum 定义"的判定
        if self.check(Colon) {
          while !self.is_at_end() &&
                !self.check(LBrace) &&
                !self.check(Semicolon) {
            ignore(self.advance())
          }
        }
        if self.check(LBrace) {
          self.restore(checkpoint_rb)
          self.parse_typedef_enum_decl(program, 0)
          continue
        }
        self.restore(e_rb)
      }
      self.restore(checkpoint_rb)
      self.parse_typedef(0)
    } else if self.check(Enum) {
      let checkpoint_rb = self.save()
      ignore(self.advance())
      if self.check(Identifier) {
        ignore(self.advance())
      }
      // C23 enum : T：底层类型子句不改变"是否 enum 定义"的判定
      if self.check(Colon) {
        while !self.is_at_end() && !self.check(LBrace) && !self.check(Semicolon) {
          ignore(self.advance())
        }
      }
      let is_enum_decl = self.check(LBrace)
      self.restore(checkpoint_rb)
      if is_enum_decl {
        self.parse_enum_decl(program, 0)
      } else {
        self.parse_global_var_or_func(program, false, false)
      }
    } else if self.check(Struct) {
      let checkpoint_rb = self.save()
      ignore(self.advance())
      let has_name = self.check(Identifier)
      if has_name {
        ignore(self.advance())
      }
      if self.check(LBrace) {
        // 向前跳过结构体体，判断后面是变量声明还是纯类型定义
        let mut brace_depth = 0
        while !self.is_at_end() {
          if self.check(LBrace) {
            brace_depth += 1
          } else if self.check(RBrace) {
            brace_depth -= 1
            ignore(self.advance())
            if brace_depth == 0 {
              break
            }
            continue
          }
          ignore(self.advance())
        }
        let is_var_decl = self.check(Identifier) ||
          self.check(Star) ||
          self.check(Semicolon)
        // 纯 struct { ... };（无变量名）仍视为结构体定义
        let is_pure_decl = self.check(Semicolon)
        self.restore(checkpoint_rb)
        if is_var_decl && !is_pure_decl {
          self.parse_global_var_or_func(program, false, false)
        } else if self.is_cpp_mode && has_name {
          self.parse_cpp_class_like_struct_decl(program)
        } else {
          let struct_decl = self.parse_struct_decl(0)
          if self.is_cpp_mode {
            self.typedef_names.set(
              struct_decl.name,
              @ast.Type::struct_type(struct_decl.name),
            )
          }
          program.structs.push(struct_decl)
        }
      } else {
        self.restore(checkpoint_rb)
        self.parse_global_var_or_func(program, false, false)
      }
    } else if self.check(Union) {
      program.unions.push(self.parse_union_decl(0))
    } else if self.is_cpp_mode && self.check(Class) {
      self.parse_cpp_class_decl(program)
    } else if self.is_cpp_mode && self.check(Template) {
      self.parse_cpp_template_decl(program)
    } else if self.check(Extern) {
      ignore(self.advance()) // consume 'extern'
      self.parse_global_var_or_func(program, false, true)
    } else if self.is_type_token() || self.is_static_token() {
      let is_static = self.is_static_token()
      if is_static {
        ignore(self.advance()) // consume 'static'
      }
      self.parse_global_var_or_func(program, is_static, false)
    } else {
      let cur = self.current()
      self.errors.push({
        message: "预期 struct、函数或全局变量声明，找到: \{cur.text}",
        line: cur.loc.line,
        column: cur.loc.column,
        code: @diag.ErrorCode::E2005_ExpectedSemicolon.code(),
      })
      ignore(self.advance())
    }
    // 活性内部断言（观测面，不改变控制流——照搬的推进逻辑本身三层兜底）
    if self.pos == round_start {
      self.stall_count += 1
    }
  }
  Some(program)
}

///|
/// 从当前位置前瞻，跳过连续的 `*` 指针前缀（C++ 模式含 &/&&），返回跳过
/// 后的 token 索引。
fn Parser::look_ahead_skip_stars(self : Parser) -> Int {
  let mut lookahead = self.pos
  while lookahead < self.tokens.length() {
    match self.tokens[lookahead].ty {
      Star => lookahead += 1
      Ampersand | AndAnd =>
        if self.is_cpp_mode {
          lookahead += 1
        } else {
          break
        }
      _ => break
    }
  }
  lookahead
}

///|
fn Parser::parse_global_var_or_func(
  self : Parser,
  program : @ast.ProgramNode,
  is_static : Bool,
  is_extern : Bool,
) -> Unit {
  let checkpoint = self.save()
  let base_type = self.parse_base_type(0)
  // C++ 构造函数类外定义：Counter::Counter() { ... }
  if self.try_parse_cpp_ctor_out_of_line(
      program, base_type, is_static, is_extern,
    ) {
    return
  }
  // 前瞻：跳过前导 *，检查是否是 identifier (
  let lookahead = self.look_ahead_skip_stars()
  let is_func_decl = if lookahead < self.tokens.length() &&
    self.tokens[lookahead].ty == Identifier {
    if lookahead + 1 < self.tokens.length() &&
      self.tokens[lookahead + 1].ty == LParen {
      true
    } else if self.is_cpp_mode &&
      lookahead + 3 < self.tokens.length() &&
      self.tokens[lookahead + 1].ty == ColonColon &&
      self.tokens[lookahead + 2].ty == Identifier &&
      self.tokens[lookahead + 3].ty == LParen {
      // C++ qualified name: Bar::set(...)
      true
    } else {
      false
    }
  } else {
    false
  }
  if is_func_decl {
    self.restore(checkpoint)
    program.funcs.push(self.parse_func_decl(is_static, is_extern, 0))
  } else if self.try_parse_cpp_qualified_static_field(
      program, base_type, is_static, is_extern,
    ) {
    // 已由 cpp 模块处理
  } else {
    let v = self.parse_var_declarator(base_type, 0)
    let ty = v.0
    let name = v.1
    let init = if is_extern {
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
    // loc 取初始化器解析后的 previous（Rust 现状口径：global loc 落在
    // 初始化器末 token——无初始化时落在名字 token）
    let prev = self.previous()
    let loc = @source.SourceLoc::new(prev.loc.line, prev.loc.column)
    program.globals.push({
      loc,
      ty,
      name,
      init,
      is_static,
      is_extern,
      source_file: "",
    })
    while self.match_token(Comma) {
      let ex = self.parse_var_declarator(base_type, 0)
      let extra_ty = ex.0
      let extra_name = ex.1
      let extra_init = if is_extern {
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
      let prev2 = self.previous()
      let loc2 = @source.SourceLoc::new(prev2.loc.line, prev2.loc.column)
      program.globals.push({
        loc: loc2,
        ty: extra_ty,
        name: extra_name,
        init: extra_init,
        is_static,
        is_extern,
        source_file: "",
      })
    }
    ignore(self.consume(Semicolon, "全局变量声明后预期 ';'"))
  }
}

///|
/// 在模板实参列表中解析非类型表达式实参（照搬 X1 的显式视图等价形态：
/// Rust `mem::take` + 切片替换 → 本实现保存 (start, end) 边界、用索引
/// 偏移直接在原数组上解析——语义保持 `pos = saved_pos + consumed`）。
fn Parser::parse_template_arg_expr(self : Parser, depth : Int) -> @ast.Expr {
  let found = self.find_template_arg_delimiter()
  let end_idx = found.0
  if end_idx <= self.pos {
    let cur = self.current()
    self.errors.push({
      message: "空模板非类型实参",
      line: cur.loc.line,
      column: cur.loc.column,
      code: @diag.ErrorCode::E2003_ExpectedExpr.code(),
    })
    return @ast.Expr::Literal(
      0,
      @source.SourceLoc::new(cur.loc.line, cur.loc.column),
      @ast.Type::int(),
    )
  }
  // 显式子视图（X8）：临时把 pos 域限制到 [saved_pos, end_idx)，解析后
  // 回填——tokens 数组本身不动，等价 Rust 的子流替换
  let saved_pos = self.pos
  let view : Array[@token.Token] = []
  for i in saved_pos..<end_idx {
    view.push(self.tokens[i])
  }
  let sub = Parser::with_mode(view, self.is_cpp_mode)
  // 符号表与父共享引用（Rust 共享 self 的同款副作用：子流写入直接落父
  // 状态）；错误表为子流独立数组，解析完按序推回。已知等价边界：子流
  // 内深度超限只跳到 view 尾（Rust 跳到父流尾）——256 层模板实参嵌套
  // 属语料外病态，登记执行记录
  sub.typedef_names = self.typedef_names
  sub.template_names = self.template_names
  let expr = sub.parse_assign(depth)
  self.pos = saved_pos + sub.pos
  for e in sub.errors {
    self.errors.push(e)
  }
  self.next_lambda_id = sub.next_lambda_id
  expr
}

///|
/// 从当前位置扫描模板实参结束位置（顶层逗号或匹配的 `>`；跳过 ()[]{}
/// 与嵌套 <>）。返回索引指向分隔符，调用者负责消费。
fn Parser::find_template_arg_delimiter(
  self : Parser,
) -> (Int, @token.TokenType) {
  let mut i = self.pos
  let mut template_depth = 1
  let mut paren = 0
  let mut bracket = 0
  let mut brace = 0
  while i < self.tokens.length() {
    let t = self.tokens[i].ty
    match t {
      LParen => paren += 1
      RParen => if paren > 0 { paren -= 1 }
      LBracket => bracket += 1
      RBracket => if bracket > 0 { bracket -= 1 }
      LBrace => brace += 1
      RBrace => if brace > 0 { brace -= 1 }
      Lt => if paren == 0 && bracket == 0 && brace == 0 { template_depth += 1 }
      Gt =>
        if paren == 0 && bracket == 0 && brace == 0 {
          template_depth -= 1
          if template_depth == 0 {
            return (i, Gt)
          }
        }
      Comma =>
        if template_depth == 1 && paren == 0 && bracket == 0 && brace == 0 {
          return (i, Comma)
        }
      _ => ()
    }
    i += 1
  }
  (i, Eof)
}
