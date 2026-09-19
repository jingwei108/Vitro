///|
/// `#if` / `#elif` 条件求值器（C99 §6.10.1 顺序的教学子集 + 完整常量表达式）。
/// 求值管线：
/// 1. `__has_include(<p>)` / `__has_include("p")` 文本级替换（token 化后 `<stdio.h>`
///    会碎成多个 token，路径无法还原）——与 `#include` 解析口径单源（U1#11）；
/// 2. `defined(X)` / `defined X` 在**宏展开前**提取（C 规则：操作数不展开）；
/// 3. 其余宏展开（走 expander，深度保险丝同源；表达式模式不记展开链）；
/// 4. 残余标识符按 C 规则替换为 `0`；
/// 5. 递归下降求值。
///
/// **完整运算符表**（勘察 ⑥-6 修复：Rust 只支持 `|| && == != 关系 + - * / % !`
/// 一元± 括号——`#if FLAGS & MASK` 等真实教学形态报 E1014）：本侧实现
/// `?: 三目（惰性） | ^ & << >> 位运算 ~ 一元` 全集；`||`/`&&` 短路且**结果
/// 归一 0/1**（勘察 ⑥-3 修复：Rust `parse_or` 左真短路返回原值，
/// `#if (2 || 0) == 1` 静默选错分支）；递归深度保险丝 128（勘察 M4 修复：
/// Rust `#if` 迷你解析器无深度保护）。
/// 值语义：Int64 wrapping（Rust `wrapping_*`）；`/` `%` 除零显式报错（MoonBit
/// 整除零是 panic，必须前置检查——F1）。
const COND_DEPTH_FUSE : Int = 128

///|
/// 求值失败（checked error——由 `evaluate_cond` 统一转 E1014 诊断）。
priv suberror CondError {
  Fail(message~ : String)
}

///|
/// 求值 `#if` / `#elif` 表达式（出错记 errors 返回 false；成功写 trace）。
fn[P : @token.SourceProvider] Pp::evaluate_cond(
  self : Pp[P],
  raw : String,
  line : Int,
) -> Bool {
  let outcome : Result[Int64, CondError] = Ok(
    self.evaluate_cond_inner(raw, line),
  ) catch {
    e => Err(e)
  }
  match outcome {
    Ok(v) => {
      self.trace.push(
        CondBranch(expr=leading_trim(raw), value=v != 0L, reason=""),
      )
      v != 0L
    }
    Err(Fail(message~)) => {
      self.errors.push({
        message: "#if 表达式错误：\{message}",
        loc: { line, column: 0, file_id: self.current_file_id(), },
        code: E1014_CondExprError,
      })
      false
    }
  }
}

///|
fn[P : @token.SourceProvider] Pp::evaluate_cond_inner(
  self : Pp[P],
  raw : String,
  line : Int,
) -> Int64 raise CondError {
  // 1. __has_include 文本级替换（与 #include 解析口径单源）
  let text = self.replace_has_include(raw)
  // 2. 词法化（普通模式——表达式内 stray `#` 由扫描器报 E1001 → 求值失败）
  let scratch : Array[@token.LexError] = []
  let toks = @scanner.scan_fragment(
    text,
    base_byte_off=0,
    base_line=line,
    base_column=1,
    file_id=self.current_file_id(),
    cpp_mode=self.cpp_mode,
    body_mode=false,
    errors=scratch,
  )
  if scratch.length() > 0 {
    raise CondError::Fail(message="表达式包含无法词法化的内容")
  }
  // 3. defined() 提取（宏展开前）
  let defd = extract_defined(toks, self.macros)
  // 4. 宏展开（表达式模式；scratch warnings 丢弃——Rust 同构口径）
  let warn_scratch : Array[@token.LexWarning] = []
  let ctx : ExpandCtx = {
    macros: self.macros,
    errors: self.errors,
    warnings: warn_scratch,
    trace: self.trace,
    expr_mode: true,
    emitted: 0,
    budget_exhausted: false,
    depth_exceeded: false,
  }
  let expanded = expand_tokens(ctx, defd, 0)
  // 5. 残余标识符 → 0（C 规则）
  let norm : Array[String] = []
  for t in expanded {
    if t.ty is Identifier {
      norm.push("0")
    } else {
      norm.push(t.text)
    }
  }
  // 6. 解析 + 求值
  let parser : CondParser = { toks: norm, pos: 0, depth: 0, }
  let v = parser.parse_cond()
  if parser.pos != parser.toks.length() {
    let rest : Array[String] = []
    for k in parser.pos..<parser.toks.length() {
      rest.push(parser.toks[k])
    }
    raise CondError::Fail(
      message="表达式存在无法解释的部分：`\{rest.join(" ")}`",
    )
  }
  v
}

///|
/// 文本级替换 `__has_include(<p>)` / `__has_include("p")`（照搬 cond.rs 语义：
/// 定界形式随路径传判定，`<>` 只查存根、`"` 走候选链——U1#11 单源）。
fn[P : @token.SourceProvider] Pp::replace_has_include(
  self : Pp[P],
  text : String,
) -> String {
  let needle = "__has_include"
  let out = StringBuilder()
  let bytes = @utf8.encode(text)
  let needle_bytes = @utf8.encode(needle)
  let n = needle_bytes.length()
  let mut i = 0
  while i < bytes.length() {
    // 匹配 needle
    let mut matched = true
    if i + n > bytes.length() {
      matched = false
    } else {
      for k in 0..<n {
        if bytes[i + k] != needle_bytes[k] {
          matched = false
          break
        }
      }
    }
    if !matched {
      out.write_string(byte_char_string(bytes[i]))
      i += 1
      continue
    }
    // 跳过 needle，扫描调用形态
    let mut j = i + n
    while j < bytes.length() && (bytes[j] == b' ' || bytes[j] == b'\t') {
      j += 1
    }
    let mut k = j
    if k < bytes.length() && bytes[k] == b'(' {
      k += 1
    }
    while k < bytes.length() && (bytes[k] == b' ' || bytes[k] == b'\t') {
      k += 1
    }
    let (close, is_angle) : (Byte, Bool) = if k < bytes.length() &&
      bytes[k] == b'<' {
      (b'>', true)
    } else if k < bytes.length() && bytes[k] == b'"' {
      (b'"', false)
    } else {
      // 无定界符：原样保留 __has_include（由残余标识符→0 兜底）
      out.write_string(needle)
      i += n
      continue
    }
    k += 1
    let path_start = k
    while k < bytes.length() && bytes[k] != close {
      k += 1
    }
    if k >= bytes.length() {
      // 未闭合：原样保留
      out.write_string(needle)
      i += n
      continue
    }
    let path = leading_trim(@utf8.decode(bytes[path_start:k]) catch { _ => "" })
    let v = self.has_include(path, is_angle)
    out.write_string(if v { "1" } else { "0" })
    i = k + 1
    // 吞掉调用右括号（含其间空白）
    while i < bytes.length() && (bytes[i] == b' ' || bytes[i] == b'\t') {
      i += 1
    }
    if i < bytes.length() && bytes[i] == b')' {
      i += 1
    }
  }
  out.to_string()
}

///|
/// 单字节 → 字符串（ASCII 直通；非法序列落 U+FFFD——该路径只在
/// `__has_include` 拼接回显中走，非 token 语义）。
fn byte_char_string(b : Byte) -> String {
  match b.to_int().to_char() {
    Some(c) => "\{c}"
    None => "\u{FFFD}"
  }
}

///|
/// `__has_include` 判定（U1#11 H-2/H-3：与 `#include` 解析口径单源——
/// `<p>` 只查存根；`"p"` 存根优先，未命中走 quote 候选链）。
fn[P : @token.SourceProvider] Pp::has_include(
  self : Pp[P],
  path : String,
  is_angle : Bool,
) -> Bool {
  if @host.stub_lookup(path) is Some(_) {
    return true
  }
  if is_angle {
    return false
  }
  self.resolve_path(path) is Some(_)
}

///|
/// token 级提取 `defined(X)` / `defined X` → `1` / `0`（宏展开前，C 规则）。
fn extract_defined(
  toks : Array[@token.Token],
  macros : Map[String, @token.MacroDef],
) -> Array[@token.Token] {
  let out : Array[@token.Token] = []
  let mut i = 0
  while i < toks.length() {
    let t = toks[i]
    if t.ty is Identifier && t.text == "defined" {
      let paren = i + 1 < toks.length() && toks[i + 1].ty is LParen
      let name : String? = if paren {
        if i + 3 < toks.length() &&
          toks[i + 2].ty is Identifier &&
          toks[i + 3].ty is RParen {
          Some(toks[i + 2].text)
        } else {
          None
        }
      } else if i + 1 < toks.length() && toks[i + 1].ty is Identifier {
        Some(toks[i + 1].text)
      } else {
        None
      }
      match name {
        Some(nm) => {
          let next = if paren { i + 4 } else { i + 2 }
          let v = if macros.contains(nm) { "1" } else { "0" }
          out.push({
            ty: Number,
            text: v,
            loc: t.loc,
            byte_off: t.byte_off,
            real_line: t.real_line,
          })
          i = next
          continue
        }
        None => {
          // 形态不合法：按 0 处理（等价"格式错误的 defined"）
          out.push({
            ty: Number,
            text: "0",
            loc: t.loc,
            byte_off: t.byte_off,
            real_line: t.real_line,
          })
          i += 1
          continue
        }
      }
    }
    out.push(t)
    i += 1
  }
  out
}

///|
// ---------- 递归下降求值（Int64 wrapping；短路 + 归一 0/1；三目惰性） ----------

priv struct CondParser {
  toks : Array[String]
  mut pos : Int
  mut depth : Int
}

///|
fn CondParser::peek(self : CondParser) -> String {
  if self.pos < self.toks.length() {
    self.toks[self.pos]
  } else {
    ""
  }
}

///|
fn CondParser::eat(self : CondParser, op : String) -> Bool {
  if self.pos < self.toks.length() && self.toks[self.pos] == op {
    self.pos += 1
    true
  } else {
    false
  }
}

///|
fn CondParser::enter(self : CondParser) -> Unit raise CondError {
  self.depth += 1
  if self.depth > COND_DEPTH_FUSE {
    raise CondError::Fail(
      message="#if 表达式嵌套过深（上限 \{COND_DEPTH_FUSE}）",
    )
  }
}

///|
fn CondParser::leave(self : CondParser) -> Unit {
  self.depth -= 1
}

///|
/// 条件表达式：`logical_or [? cond : cond]`（右结合；**惰性**——未选中分支
/// 结构性跳过，分支内除零不触发，与 C 一致）。
fn CondParser::parse_cond(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let c = self.parse_or()
  if self.eat("?") {
    // S2 审阅 P1-4 修复：C 标准只求值被选中分支——此前真分支无条件求值，
    // `0 ? 1/0 : 1` 因真分支除零误报。两分支各自惰性（未选中侧结构跳过）
    let taken = c != 0L
    if taken {
      let a = self.parse_cond()
      if !self.eat(":") {
        self.leave()
        raise CondError::Fail(message="#if 表达式三目缺少 ':'")
      }
      self.skip_one_branch() // 假分支不求值（结构跳过）
      self.leave()
      a
    } else {
      self.skip_one_branch() // 真分支不求值（结构跳过）
      if !self.eat(":") {
        self.leave()
        raise CondError::Fail(message="#if 表达式三目缺少 ':'")
      }
      let b = self.parse_cond()
      self.leave()
      b
    }
  } else {
    self.leave()
    c
  }
}

///|
/// 跳过一个三目分支（真分支或假分支）：括号感知，断点为 depth 0 的
/// `:` / `)` / 表达式尾；嵌套 `?` 分支整体跳过。
fn CondParser::skip_one_branch(self : CondParser) -> Unit raise CondError {
  let before = self.pos
  let mut depth_paren = 0
  while self.pos < self.toks.length() {
    match self.peek() {
      "(" => {
        depth_paren += 1
        self.pos += 1
      }
      ")" =>
        if depth_paren == 0 {
          break
        } else {
          depth_paren -= 1
          self.pos += 1
        }
      "?" => {
        // 嵌套三目：真分支 + ':' + 假分支整体跳过
        self.pos += 1
        self.skip_one_branch()
        if self.eat(":") {
          self.skip_one_branch()
        }
      }
      ":" => if depth_paren == 0 { break } else { self.pos += 1 }
      _ => self.pos += 1
    }
  }
  if self.pos == before {
    raise CondError::Fail(message="三目分支缺少操作数")
  }
}

///|
/// `||`（短路：左真则右**结构性跳过**——跳过分支内允许除零；**结果归一 0/1**，
/// 勘察 ⑥-3 修复：Rust 左真短路返回原值，`(2 || 0) == 1` 静默选错分支）。
fn CondParser::parse_or(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_and()
  while self.eat("||") {
    left = if left != 0L {
      // 短路：右操作数不求值
      self.skip_logical_operand()
      1L
    } else if self.parse_and() != 0L {
      1L
    } else {
      0L
    }
  }
  self.leave()
  left
}

///|
/// `&&`（短路；归一 0/1）。
fn CondParser::parse_and(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_bitor()
  while self.eat("&&") {
    left = if left != 0L {
      if self.parse_bitor() != 0L {
        1L
      } else {
        0L
      }
    } else {
      self.skip_logical_operand()
      0L
    }
  }
  self.leave()
  left
}

///|
/// 短路时**结构性跳过**一个操作数（不求值——跳过的分支里允许除零；
/// 断点：depth 0 的 `&&`/`||`/`?`/`:` 与 `)`）。
fn CondParser::skip_logical_operand(self : CondParser) -> Unit raise CondError {
  let before = self.pos
  let mut depth_paren = 0
  while self.pos < self.toks.length() {
    match self.peek() {
      "(" => {
        depth_paren += 1
        self.pos += 1
      }
      ")" =>
        if depth_paren == 0 {
          break
        } else {
          depth_paren -= 1
          self.pos += 1
        }
      "&&" | "||" | "?" | ":" =>
        if depth_paren == 0 {
          break
        } else {
          self.pos += 1
        }
      _ => self.pos += 1
    }
  }
  if self.pos == before {
    raise CondError::Fail(message="短路分支缺少操作数")
  }
}

///|
fn CondParser::parse_bitor(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_bitxor()
  while self.eat("|") {
    left = left | self.parse_bitxor()
  }
  self.leave()
  left
}

///|
fn CondParser::parse_bitxor(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_bitand()
  while self.eat("^") {
    left = left ^ self.parse_bitand()
  }
  self.leave()
  left
}

///|
fn CondParser::parse_bitand(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_eq()
  while self.eat("&") {
    left = left & self.parse_eq()
  }
  self.leave()
  left
}

///|
fn CondParser::parse_eq(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_rel()
  let mut go = true
  while go {
    if self.eat("==") {
      left = if left == self.parse_rel() { 1L } else { 0L }
    } else if self.eat("!=") {
      left = if left != self.parse_rel() { 1L } else { 0L }
    } else {
      go = false
    }
  }
  self.leave()
  left
}

///|
fn CondParser::parse_rel(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_shift()
  let mut go = true
  while go {
    if self.eat("<=") {
      left = if left <= self.parse_shift() { 1L } else { 0L }
    } else if self.eat(">=") {
      left = if left >= self.parse_shift() { 1L } else { 0L }
    } else if self.eat("<") {
      left = if left < self.parse_shift() { 1L } else { 0L }
    } else if self.eat(">") {
      left = if left > self.parse_shift() { 1L } else { 0L }
    } else {
      go = false
    }
  }
  self.leave()
  left
}

///|
fn CondParser::parse_shift(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_add()
  let mut go = true
  while go {
    if self.eat("<<") {
      let r = self.parse_add()
      left = shl_i64(left, r)
    } else if self.eat(">>") {
      let r = self.parse_add()
      left = shr_i64(left, r)
    } else {
      go = false
    }
  }
  self.leave()
  left
}

///|
fn CondParser::parse_add(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_mul()
  let mut go = true
  while go {
    if self.eat("+") {
      left = left + self.parse_mul() // wrapping（MoonBit Int64 溢出回绕 = Rust wrapping_add）
    } else if self.eat("-") {
      left = left - self.parse_mul()
    } else {
      go = false
    }
  }
  self.leave()
  left
}

///|
fn CondParser::parse_mul(self : CondParser) -> Int64 raise CondError {
  self.enter()
  let mut left = self.parse_unary()
  let mut go = true
  while go {
    if self.eat("*") {
      left = left * self.parse_unary()
    } else if self.eat("/") {
      let r = self.parse_unary()
      if r == 0 {
        self.leave()
        raise CondError::Fail(message="#if 表达式除零")
      }
      left = div_i64(left, r)
    } else if self.eat("%") {
      let r = self.parse_unary()
      if r == 0 {
        self.leave()
        raise CondError::Fail(message="#if 表达式取模零")
      }
      left = mod_i64(left, r)
    } else {
      go = false
    }
  }
  self.leave()
  left
}

///|
fn CondParser::parse_unary(self : CondParser) -> Int64 raise CondError {
  self.enter()
  if self.eat("!") {
    let v = if self.parse_unary() == 0L { 1L } else { 0L }
    self.leave()
    return v
  }
  if self.eat("~") {
    let v = 0L - 1L - self.parse_unary() // 按位非 ≡ -1 - x（wrapping）
    self.leave()
    return v
  }
  if self.eat("-") {
    let v = 0L - self.parse_unary() // wrapping_neg 等价（0 - x 回绕）
    self.leave()
    return v
  }
  if self.eat("+") {
    let v = self.parse_unary()
    self.leave()
    return v
  }
  let v = self.parse_primary()
  self.leave()
  v
}

///|
fn CondParser::parse_primary(self : CondParser) -> Int64 raise CondError {
  self.enter()
  if self.eat("(") {
    let v = self.parse_cond()
    if !self.eat(")") {
      self.leave()
      raise CondError::Fail(message="#if 表达式缺少右括号")
    }
    self.leave()
    return v
  }
  if self.pos >= self.toks.length() {
    self.leave()
    raise CondError::Fail(message="#if 表达式意外结束")
  }
  let s = self.toks[self.pos]
  match parse_c_int(s) {
    Some(v) => {
      self.pos += 1
      self.leave()
      v
    }
    None => {
      self.leave()
      raise CondError::Fail(
        message="#if 表达式包含无法求值的记号 `\{s}`",
      )
    }
  }
}

///|
/// Int64 语义辅助（Rust `i64` 除/模/移位语义：除零 Rust panic → 本侧前置
/// 已检查；`i64::MIN / -1` Rust panic → 返回回绕值不崩——教学求值器显式
/// 承接；移位计数 mod 64——Rust `1_i64 << 64` panic，`wrapping_shl` 是
/// mod 64，C 标准此处 UB，教学子集取 mod 64 与 MoonBit 原生语义一致）。
fn div_i64(a : Int64, b : Int64) -> Int64 {
  if b == -1L {
    return 0L - a // i64::MIN / -1 回绕（不崩）
  }
  a / b
}

///|
fn mod_i64(a : Int64, b : Int64) -> Int64 {
  if b == -1L {
    return 0L
  }
  a % b
}

///|
fn shl_i64(a : Int64, n : Int64) -> Int64 {
  let shift = Int64::reinterpret_as_uint64(n).to_int() % 64
  a << shift
}

///|
fn shr_i64(a : Int64, n : Int64) -> Int64 {
  let shift = Int64::reinterpret_as_uint64(n).to_int() % 64
  a >> shift
}

///|
/// 解析 C 预处理整数字面量（十进制/0x/0b/0 八进制 + u/l 后缀；
/// 照搬 `parse_c_int`——char 常量的数值文本同样由此解析，勘察 ⑥-13）。
fn parse_c_int(s : String) -> Int64? {
  // 剥 u/l 后缀
  let mut end_ = s.length()
  while end_ > 0 {
    let c = s[end_ - 1]
    if c == 'u' || c == 'U' || c == 'l' || c == 'L' {
      end_ -= 1
    } else {
      break
    }
  }
  let mut t = s.substring(start=0, end=end_)
  let neg = if t.length() > 0 && t[0] == '-' {
    t = t.substring(start=1)
    true
  } else {
    false
  }
  if t.length() == 0 {
    return None
  }
  let (body, radix) : (String, Int) = if t.length() >= 2 &&
    t[0] == '0' &&
    (t[1] == 'x' || t[1] == 'X') {
    (t.substring(start=2), 16)
  } else if t.length() >= 2 && t[0] == '0' && (t[1] == 'b' || t[1] == 'B') {
    (t.substring(start=2), 2)
  } else if t.length() > 1 && t[0] == '0' {
    (t.substring(start=1), 8)
  } else {
    (t, 10)
  }
  if body.length() == 0 {
    return None
  }
  // 手写累积（按 UInt64 值域解析后位型转 Int64——9223372036854775808 与
  // 负号组合的 Rust None 边界，教学求值器取宽松回绕口径，登记包注释）
  let mut val : UInt64 = 0UL
  let radix_u : UInt64 = if radix == 16 {
    16UL
  } else if radix == 8 {
    8UL
  } else if radix == 2 {
    2UL
  } else {
    10UL
  }
  for i in 0..<body.length() {
    let c = body[i].to_int()
    let d : Int = if c >= '0' && c <= '9' {
      c - '0'
    } else if c >= 'a' && c <= 'f' {
      c - 'a' + 10
    } else if c >= 'A' && c <= 'F' {
      c - 'A' + 10
    } else {
      return None
    }
    if d.to_uint64() >= radix_u {
      return None
    }
    val = val * radix_u + d.to_uint64()
  }
  // S2 审阅 P1-5 修复：超 i64 正值显式拒（None → E1014，Rust i64::from_str_radix
  // 口径）——此前 UInt64 位型重解释使 `#if ULLONG_MAX > 0` 得 -1 > 0 = 假
  //（零诊断静默错值）。Clang 按 uintmax_t 容纳得真——差异登记 D 清单
  if val > 9223372036854775807UL {
    return None
  }
  let v = UInt64::reinterpret_as_int64(val)
  if neg {
    Some(0L - v)
  } else {
    Some(v)
  }
}
