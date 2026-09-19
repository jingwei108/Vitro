///|
/// 指令消费（行级——勘察 ⑥-7 修复：include 路径在**行内**解析，未闭合不再
/// 把余下全文当路径）。`rest` 为 `#` 之后的整行文本；`line` 为该行仿古主坐标
/// 行号；活跃行的 token 追加进 `out`。
fn[P : @token.SourceProvider] Pp::handle_directive(
  self : Pp[P],
  rest : String,
  line : Int,
  out : Array[@token.Token],
) -> Unit {
  let rest = leading_trim(rest)
  // 指令名
  let mut n = 0
  while n < rest.length() {
    let c = rest[n].to_int()
    if (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_' {
      n += 1
    } else {
      break
    }
  }
  let directive = rest.substring(start=0, end=n)
  let after = rest.substring(start=n)
  // `#if` / `#elif` 需要整行原始文本（__has_include 的路径无法从 token 还原）
  if directive == "if" || directive == "elif" {
    if directive == "if" {
      self.handle_if(after, line)
    } else {
      self.handle_elif(after, line)
    }
    return
  }
  match directive {
    "define" => if !self.is_skipping() { self.parse_define(after, line) }
    "undef" =>
      if !self.is_skipping() {
        let name = read_ident(after)
        ignore(self.macros.remove(name))
      }
    "ifdef" => {
      let ident = read_ident(after)
      let parent_skipping = self.is_skipping()
      let active = if parent_skipping {
        false
      } else {
        self.macros.contains(ident)
      }
      self.cond_stack.push({
        active,
        has_else: false,
        taken: active,
        open_line: line,
      })
    }
    "ifndef" => {
      let ident = read_ident(after)
      let parent_skipping = self.is_skipping()
      let active = if parent_skipping {
        false
      } else {
        !self.macros.contains(ident)
      }
      self.cond_stack.push({
        active,
        has_else: false,
        taken: active,
        open_line: line,
      })
    }
    "else" => self.handle_else(line)
    "endif" => self.handle_endif(line)
    // C23（修复 ⑥-11：Rust 静默当未知指令跳过 → 选错分支零诊断）
    "elifdef" => self.handle_elifdef(after, line, negated=false)
    "elifndef" => self.handle_elifdef(after, line, negated=true)
    "include" =>
      if !self.is_skipping() {
        self.handle_include(after, line, out)
      }
    // 标准指令：显式诊断（修复 ⑥-12：Rust 落未知分支静默跳过、零诊断——
    // 决议记载"已支持"与代码不符；W 级码表暂缺，severity 精确化随码表扩容裁定）
    "error" =>
      if !self.is_skipping() {
        self.err(
          "#error 指令: \{leading_trim(after)}",
          line,
          code=E1006_UnsupportedFeature,
        )
      }
    "warning" =>
      if !self.is_skipping() {
        self.err(
          "#warning 指令: \{leading_trim(after)}",
          line,
          code=E1006_UnsupportedFeature,
        )
      }
    // 空指令 `#`：C 标准 no-op（Rust 落未知分支静默跳过——行为等价）
    "" => ()
    // 未知指令：显式诊断（修复 ⑥-11——Rust `_ =>` 静默跳行使 `#elifdef` 类
    // 拼写错误静默选错分支；教学引擎对"静默选错分支"零容忍）
    _ =>
      if !self.is_skipping() {
        self.err(
          "无法识别的预处理指令 '#\{directive}'（已跳过该行）",
          line,
          code=E1006_UnsupportedFeature,
        )
      }
  }
}

///|
/// 行内读标识符（跳过前导空白后取 [A-Za-z0-9_]+）。
fn read_ident(s : String) -> String {
  let s = leading_trim(s)
  let mut n = 0
  while n < s.length() {
    let c = s[n].to_int()
    if (c >= 'a' && c <= 'z') ||
      (c >= 'A' && c <= 'Z') ||
      (c >= '0' && c <= '9') ||
      c == '_' {
      n += 1
    } else {
      break
    }
  }
  s.substring(start=0, end=n)
}

///|
// ---------- 条件组 ----------

fn[P : @token.SourceProvider] Pp::handle_if(
  self : Pp[P],
  raw : String,
  line : Int,
) -> Unit {
  let parent_skipping = self.is_skipping()
  let state : CondState = if parent_skipping {
    // 外层已跳过：不求值（短路语义），组内永不激活
    { active: false, has_else: false, taken: true, open_line: line, }
  } else {
    let outcome = self.evaluate_cond(raw, line)
    { active: outcome, has_else: false, taken: outcome, open_line: line, }
  }
  self.cond_stack.push(state)
}

///|
fn[P : @token.SourceProvider] Pp::handle_elif(
  self : Pp[P],
  raw : String,
  line : Int,
) -> Unit {
  let parent_skipping = self.parent_skipping()
  let n = self.cond_stack.length()
  if n == 0 {
    self.err(
      "没有匹配的 #if/#ifdef/#ifndef 就出现 #elif",
      line,
      code=E1011_UnmatchedConditional,
    )
    return
  }
  if self.cond_stack[n - 1].has_else {
    self.err("#elif 出现在 #else 之后", line, code=E1012_DuplicateElse)
    return
  }
  if parent_skipping || self.cond_stack[n - 1].taken {
    self.cond_stack[n - 1].active = false
    return
  }
  let outcome = self.evaluate_cond(raw, line)
  self.cond_stack[n - 1].active = outcome
  self.cond_stack[n - 1].taken = outcome
}

///|
/// `#elifdef` / `#elifndef`（C23；clang 22 支持——语义 = defined 判定版 #elif）。
fn[P : @token.SourceProvider] Pp::handle_elifdef(
  self : Pp[P],
  after : String,
  line : Int,
  negated~ : Bool,
) -> Unit {
  let parent_skipping = self.parent_skipping()
  let n = self.cond_stack.length()
  if n == 0 {
    self.err(
      "没有匹配的 #if/#ifdef/#ifndef 就出现 #elifndef/#elifdef",
      line,
      code=E1011_UnmatchedConditional,
    )
    return
  }
  if self.cond_stack[n - 1].has_else {
    self.err("#elifdef 出现在 #else 之后", line, code=E1012_DuplicateElse)
    return
  }
  if parent_skipping || self.cond_stack[n - 1].taken {
    self.cond_stack[n - 1].active = false
    return
  }
  let ident = read_ident(after)
  let mut active = self.macros.contains(ident)
  if negated {
    active = !active
  }
  // trace 不记（#if/#elif 之外的新指令形状——Rust wire format 未覆盖，
  // 保持 trace 输出与 Rust 逐字节一致）
  self.cond_stack[n - 1].active = active
  self.cond_stack[n - 1].taken = active
}

///|
fn[P : @token.SourceProvider] Pp::handle_else(self : Pp[P], line : Int) -> Unit {
  let parent_skipping = self.parent_skipping()
  let n = self.cond_stack.length()
  if n == 0 {
    self.err(
      "没有匹配的 #if/#ifdef/#ifndef 就出现 #else",
      line,
      code=E1011_UnmatchedConditional,
    )
    return
  }
  if self.cond_stack[n - 1].has_else {
    self.err("重复的 #else", line, code=E1012_DuplicateElse)
    return
  }
  self.cond_stack[n - 1].has_else = true
  // 父级跳过态下保持不激活（显式化以便后续 #elif 正确）
  self.cond_stack[n - 1].active = !self.cond_stack[n - 1].taken &&
    !parent_skipping
}

///|
fn[P : @token.SourceProvider] Pp::handle_endif(
  self : Pp[P],
  line : Int,
) -> Unit {
  // U1#11：include 边界记账——头文件内多余的 #endif 不得弹掉包含者的条件组
  if self.include_stack.length() > 0 {
    let boundary = self.include_stack[self.include_stack.length() - 1].cond_boundary
    if self.cond_stack.length() <= boundary {
      self.err(
        "头文件中的 #endif 没有匹配的 #if/#ifdef/#ifndef（将弹掉包含文件的条件编译组，已忽略该 #endif）",
        line,
        code=E1011_UnmatchedConditional,
      )
      return
    }
  }
  if self.cond_stack.length() == 0 {
    self.err(
      "没有匹配的 #if/#ifdef/#ifndef 就出现 #endif",
      line,
      code=E1011_UnmatchedConditional,
    )
    return
  }
  ignore(self.cond_stack.pop())
}

///|
// ---------- include ----------

///|
/// 解析 include 路径与定界形式：`(路径, 是否 <> 形式)`。
/// 行内解析（修复 ⑥-7：扫描不到定界符即止，不吞后续行）。
fn parse_include_path(after : String) -> (String, Bool)? {
  let s = leading_trim(after)
  if s.length() == 0 {
    return None
  }
  let delimiter = s[0].to_int()
  if delimiter != '<' && delimiter != '"' {
    return None
  }
  let end_char : Int = if delimiter == '<' { '>' } else { '"' }
  let mut i = 1
  while i < s.length() {
    if s[i].to_int() == end_char {
      return Some((s.substring(start=1, end=i), delimiter == '<'))
    }
    i += 1
  }
  // 未闭合：路径截到行尾（不再消费余下全文——Rust E1021 消息内嵌多行源码）
  Some((s.substring(start=1), delimiter == '<'))
}

///|
fn[P : @token.SourceProvider] Pp::handle_include(
  self : Pp[P],
  after : String,
  line : Int,
  out : Array[@token.Token],
) -> Unit {
  let path_info = parse_include_path(after)
  if path_info is None {
    return
  }
  let (path, is_angle) = path_info.unwrap()
  // 动态嵌套深度保险丝（U1#11；文案照搬）
  if self.include_stack.length() >= MAX_INCLUDE_DEPTH {
    self.err(
      "#include 嵌套过深（上限 \{MAX_INCLUDE_DEPTH} 层，已跳过 '\{path}'）",
      line,
      code=E1015_IncludeCycle,
    )
    return
  }
  let is_stub = @host.stub_lookup(path) is Some(_)
  // H-3（U1#11 收紧）：`<>` 只匹配标准库存根，不搜索文件系统（与 Clang 对齐；
  // 存量 655 处 `<>` include 全为标准头名，零存量依赖）
  if is_angle && !is_stub {
    self.err(
      "<> 形式只匹配标准库头文件（未收录 '\{path}'）；自定义头文件请使用 #include \"\{path}\"",
      line,
      code=E1021_IncludeNotFound,
    )
    return
  }
  // include-once / 环检测
  let resolved : String? = if is_stub { None } else { self.resolve_path(path) }
  let key = if is_stub {
    "stub:\{path}"
  } else {
    match resolved {
      Some(r) => r
      None => {
        // H-1（U1#11）：找不到头文件必须定位报错（E1021，include 行）
        self.err(
          "找不到头文件 '\{path}'（已搜索：当前文件目录、源文件目录；标准库存根未收录该名字）",
          line,
          code=E1021_IncludeNotFound,
        )
        return
      }
    }
  }
  if self.processed.contains(key) {
    return // include-once：静默跳过（守卫语义内置）
  }
  if !is_stub {
    match self.detect_cycle(key) {
      Some(cycle) => {
        self.err(
          "检测到 #include 依赖环：\{cycle}（已跳过该 include）",
          line,
          code=E1015_IncludeCycle,
        )
        return
      }
      None => ()
    }
  }
  self.processed.add(key)
  // 内容读取（存根 or provider；缓存单源）
  let content : String = if is_stub {
    @host.stub_lookup(path).unwrap()
  } else {
    match self.read_cached(key) {
      Some(c) => c
      None => {
        self.err(
          "找不到头文件 '\{path}'（已搜索：当前文件目录、源文件目录；标准库存根未收录该名字）",
          line,
          code=E1021_IncludeNotFound,
        )
        return
      }
    }
  }
  // file_id 分配（同路径同 id——诊断归属单源）
  let file_id = match self.file_ids.get(key) {
    Some(id) => id
    None => {
      let id = self.next_file_id
      self.next_file_id += 1
      self.file_ids[key] = id
      id
    }
  }
  // 仿古行号基准（逐字节复刻 Rust splice 补偿后的主坐标系行号；负行号是
  // ③-3 行号补偿缺陷的忠实复刻，L2 对拍口径故意保留——真实归属由
  // file_id + LineMap 携带）：
  // - 存根：Rust 不做哨兵包装，直接 splice——line = L + 1 - N，
  //   内容第 k 行 token line = L + 1 - N + (k-1)；
  // - 自定义头：Rust 以哨兵指令包裹（inserted = N + 3）——line = L - N - 2，
  //   内容第 k 行 token line = L - N - 2 + k（哨兵行不产 token）。
  //（N = 内容换行数）
  let content_newlines = count_char(content, '\n')
  let line_base = if is_stub {
    line - content_newlines + 1
  } else {
    line - content_newlines - 2 + 1
  }
  let span_first = line_base
  // include 栈（quote 相对解析的"包含者目录" + 条件栈边界）
  let frame_dir = if is_stub { "" } else { dirname_of(key) }
  self.include_stack.push({
    dir: frame_dir,
    cond_boundary: self.cond_stack.length(),
    file_id,
  })
  let content_tokens = self.run_pp(
    content,
    main_line_base=line_base,
    main_byte_base=0,
    main_file_id=file_id,
    span_main_first=Some(span_first),
    span_file_line=1,
  )
  // U1#11：头文件末尾条件组闭合校验（pop_include_frame 内，文案照搬）
  ignore(self.pop_include_frame())
  for t in content_tokens {
    out.push(t)
  }
}

///|
/// 弹出 include 帧并校验条件栈边界（U1#11 文案照搬）。
/// 返回弹出的帧（供调用方读 dir）。
fn[P : @token.SourceProvider] Pp::pop_include_frame(
  self : Pp[P],
) -> IncludeFrame? {
  if self.include_stack.length() == 0 {
    return None
  }
  let frame = self.include_stack.pop().unwrap()
  if self.cond_stack.length() > frame.cond_boundary {
    let hint = if self.cond_stack.length() > 0 {
      self.cond_stack[self.cond_stack.length() - 1].open_line
    } else {
      0
    }
    self.err(
      "头文件内未闭合的 #if/#ifdef/#ifndef（已在头文件末尾自动闭合，请检查头文件的条件编译配对）",
      hint,
      code=E1013_UnclosedConditional,
    )
    while self.cond_stack.length() > frame.cond_boundary {
      ignore(self.cond_stack.pop())
    }
  }
  Some(frame)
}

///|
// ---------- define ----------

fn[P : @token.SourceProvider] Pp::parse_define(
  self : Pp[P],
  after : String,
  line : Int,
) -> Unit {
  let after = leading_trim(after)
  if after.length() == 0 {
    self.err("#define 后预期宏名称", line, code=E1005_InvalidDefine)
    return
  }
  let c0 = after[0].to_int()
  if !((c0 >= 'a' && c0 <= 'z') || (c0 >= 'A' && c0 <= 'Z') || c0 == '_') {
    self.err("#define 后预期宏名称", line, code=E1005_InvalidDefine)
    return
  }
  let name = read_ident(after)
  let rest = after.substring(start=name.length())
  // 参数列表：宏名后**紧跟** '('（中间无空白——C 标准）
  let params : Array[String] = []
  let mut body_text = rest
  if rest.length() > 0 && rest[0].to_int() == '(' {
    let mut i = 1
    let mut ok = false
    for ;; {
      // skip 空白
      while i < rest.length() && (rest[i] == ' ' || rest[i] == '\t') {
        i += 1
      }
      if i < rest.length() && rest[i].to_int() == ')' {
        i += 1
        ok = true
        break
      }
      if i >= rest.length() || !is_ident_start(rest[i].to_int()) {
        self.err("宏参数必须是标识符", line, code=E1005_InvalidDefine)
        return
      }
      let pstart = i
      while i < rest.length() && is_ident_char(rest[i].to_int()) {
        i += 1
      }
      params.push(rest.substring(start=pstart, end=i))
      // skip 空白
      while i < rest.length() && (rest[i] == ' ' || rest[i] == '\t') {
        i += 1
      }
      if i < rest.length() && rest[i].to_int() == ',' {
        i += 1
      } else if i < rest.length() && rest[i].to_int() == ')' {
        i += 1
        ok = true
        break
      } else {
        self.err("宏参数列表格式错误", line, code=E1005_InvalidDefine)
        return
      }
    }
    if !ok {
      return
    }
    body_text = rest.substring(start=i)
  }
  // 宏名与 body 之间的空白（不吞下一行）
  let body_text = leading_trim(body_text)
  // 宏体以 macro_body 模式词法化（`#`/`##` 是操作符）；子扫描错误**丢弃**
  //（复刻 Rust `let (body_tokens, _) = body_lexer.tokenize()` 的静默口径）
  let scratch : Array[@token.LexError] = []
  let body = @scanner.scan_fragment(
    body_text,
    base_byte_off=0,
    base_line=line,
    base_column=1,
    file_id=self.current_file_id(),
    cpp_mode=self.cpp_mode,
    body_mode=true,
    errors=scratch,
  )
  // 宏名后紧跟 `(` = 函数式（含零参数 `FOO()`——修复 ⑥-9：Rust `params.is_empty()`
  // 无法区分两者，`#define FOO() 42` 被误当对象宏，调用报 E3066 错位到 typeck）
  let def : @token.MacroDef = if rest.length() > 0 && rest[0].to_int() == '(' {
    Function(params~, body~)
  } else {
    Object(body~)
  }
  self.define_macro(name, def, line)
}

///|
fn is_ident_start(c : Int) -> Bool {
  (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

///|
fn is_ident_char(c : Int) -> Bool {
  is_ident_start(c) || (c >= '0' && c <= '9')
}

///|
fn count_char(s : String, target : Char) -> Int {
  let mut n = 0
  for c in s {
    if c == target {
      n += 1
    }
  }
  n
}

///|
/// 路径的目录部分（最后一个 '/' 之前；无 '/' 返回 ""——同目录相对语义）。
fn dirname_of(path : String) -> String {
  let mut last : Int? = None
  for i = 0; i < path.length(); i = i + 1 {
    if path[i] == '/' {
      last = Some(i)
    }
  }
  match last {
    Some(i) => path.substring(start=0, end=i)
    None => ""
  }
}
