///|
/// 字符级词法扫描器（L1/L2 共用底座）。
///
/// 契约（S2 差分对拍口径，与 Rust oracle `vitro_lexer/src/lib.rs` 精确对齐）：
/// - 承载 `Bytes`（UTF-8 字节流）；`byte_off` 为 0 起字节偏移；
/// - `column` 按 **Unicode 标量**计数（多字节序列一次 +1）——复刻 Rust
///   `advance()` 的 char 计数；token 的 `loc.column = column - text 的 UTF-8
///   字节数`——**故意复刻** Rust `make_token` 的量纲混算缺陷（⑥-1/⑥-2），
///   换取 col 逐字节可比；
/// - 主循环 `while` 重派发，禁止递归（⑥-15：递归实现下连续数万行注释栈溢出）；
/// - `#` 的处理由 `macro_body_mode` 决定：true → Hash/HashHash token（L1 raw
///   全程 true；L2 的宏体/`#`/`##` 操作数扫描同）；false → 由上层 pass 消费
///   （本扫描器不处理指令，独立 pass 架构）；
/// - 诊断收集不中断（错误 token 照常产出）。
priv struct Scanner {
  bytes : Bytes
  mut pos : Int
  length : Int
  mut line : Int
  mut column : Int
  mut real_line : Int
  /// 行级 fragment 模式（L2）：源不可见行尾换行由调用方告知——未闭合
  /// 字符串在"行后有换行"时补报 E1003（复刻 Rust 整源扫描的两条诊断，
  /// S2 审阅 P2）
  mut line_mode : Bool
  mut line_has_newline : Bool
  file_id : Int
  errors : Array[@token.LexError]
  cpp_mode : Bool
  /// 起始 byte_off / line / column 基准（fragment 扫描时非 0）。
  base_byte_off : Int
}

///|
/// UTF-8 序列首字节 → 序列长度（1~4；非法首字节按 1 处理不崩，扫描器只推进）。
fn utf8_seq_len(b : Byte) -> Int {
  if b >= b'\xF0' {
    4
  } else if b >= b'\xE0' {
    3
  } else if b >= b'\xC0' {
    2
  } else {
    1
  }
}

///|
/// 解码 `bytes[i]` 起的 UTF-8 码点（Int；越界/非法返回 -1）。
/// 仅用于列计数与"是否某个 ASCII 字符"的判定，非法序列的返回值不参与 token 语义。
fn decode_at(bytes : Bytes, i : Int) -> Int {
  if i >= bytes.length() {
    return -1
  }
  let b0 = bytes[i].to_int()
  if b0 < 0x80 {
    return b0
  }
  let n = utf8_seq_len(bytes[i])
  if i + n > bytes.length() {
    return -1
  }
  if n == 2 {
    ((b0 & 0x1F) << 6) | (bytes[i + 1].to_int() & 0x3F)
  } else if n == 3 {
    ((b0 & 0x0F) << 12) |
    ((bytes[i + 1].to_int() & 0x3F) << 6) |
    (bytes[i + 2].to_int() & 0x3F)
  } else if n == 4 {
    ((b0 & 0x07) << 18) |
    ((bytes[i + 1].to_int() & 0x3F) << 12) |
    ((bytes[i + 2].to_int() & 0x3F) << 6) |
    (bytes[i + 3].to_int() & 0x3F)
  } else {
    -1
  }
}

///|
/// Byte 区间 → String（合法 UTF-8 切片；词法 token 区间恒在字符边界）。
fn slice_string(bytes : Bytes, start : Int, end_ : Int) -> String {
  @utf8.decode(bytes[start:end_]) catch {
    _ => ""
  } // 切片由扫描器保证在码点边界；防御性兜底空串（不可达路径）
}

///|
fn is_ascii_whitespace(c : Int) -> Bool {
  c == ' ' || c == '\t' || c == '\n' || c == '\u{C}' || c == '\r'
}

///|
fn is_ascii_digit(c : Int) -> Bool {
  c >= '0' && c <= '9'
}

///|
fn is_ascii_hexdigit(c : Int) -> Bool {
  is_ascii_digit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}

///|
fn is_ascii_alphabetic(c : Int) -> Bool {
  (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

///|
fn is_ascii_alphanumeric(c : Int) -> Bool {
  is_ascii_alphabetic(c) || is_ascii_digit(c)
}

///|
fn is_octal_digit(c : Int) -> Bool {
  c >= '0' && c <= '7'
}

///|
/// text 的 UTF-8 字节数（Rust `text.len()` 口径——column 反推公式的被减数）。
fn utf8_len(s : String) -> Int {
  @utf8.encode(s).length()
}

///|
/// L1 入口：全源原始词法（macro_body_mode = true——`#` 产出 Hash/HashHash，
/// 与 Rust `tokenize_raw` 出口同构；不做行拼接/指令消费/展开）。
/// BOM **不剥离**（与 Rust raw 同构：BOM → E1001 + Unknown）。
pub fn scan_raw(
  src : Bytes,
  cpp_mode? : Bool = false,
  file_id? : Int = 0,
) -> @token.RawResult {
  let scanner = Scanner::new(
    src,
    base_byte_off=0,
    base_line=1,
    base_column=1,
    file_id~,
    cpp_mode~,
    body_mode=true,
  )
  let tokens : Array[@token.Token] = []
  for ;; {
    let t = scanner.next_token(body_mode=true)
    tokens.push(t)
    if t.ty is Eof {
      break
    }
  }
  { tokens, errors: scanner.errors, }
}

///|
/// 片段扫描：pp pass 对活跃普通行 / 宏体 / `#if` 表达式文本调用。
/// `body_mode = true` 时 `#` 产出 Hash/HashHash（宏体语义）。
/// 返回 token 序列（**不含 Eof**——行/片段边界由调用方管理）。
pub fn scan_fragment(
  text : String,
  base_byte_off~ : Int,
  base_line~ : Int,
  base_real_line? : Int = base_line,
  base_column~ : Int,
  file_id~ : Int,
  cpp_mode? : Bool = false,
  body_mode? : Bool = true,
  errors? : Array[@token.LexError] = [],
  trailing_newline? : Bool = true,
) -> Array[@token.Token] {
  let scanner = Scanner::new(
    @utf8.encode(text),
    base_byte_off~,
    base_line~,
    base_real_line~,
    base_column~,
    file_id~,
    cpp_mode~,
    body_mode~,
  )
  // 行级模式标记须在扫描前就位（E1003+E1002 双诊断判据读取它）
  scanner.line_mode = true
  scanner.line_has_newline = trailing_newline
  let tokens : Array[@token.Token] = []
  for ;; {
    let t = scanner.next_token(body_mode~)
    if t.ty is Eof {
      break
    }
    tokens.push(t)
  }
  for e in scanner.errors {
    errors.push(e)
  }
  tokens
}

///|
fn Scanner::new(
  bytes : Bytes,
  base_byte_off~ : Int,
  base_line~ : Int,
  base_column~ : Int,
  base_real_line? : Int = base_line,
  file_id~ : Int,
  cpp_mode~ : Bool,
  body_mode~ : Bool,
) -> Scanner {
  // body_mode 仅影响 next_token 分支选择，不存字段（见 next_token 注释）
  let _ = body_mode
  {
    bytes,
    pos: 0,
    length: bytes.length(),
    line: base_line,
    column: base_column,
    real_line: base_real_line,
    line_mode: false,
    line_has_newline: true,
    file_id,
    errors: [],
    cpp_mode,
    base_byte_off,
  }
}

///|
/// 前瞻 `off` **字节**处的码点（越界返回 -1 = Rust 的 `'\0'` 哨兵语义）。
/// 所有调用点只对 ASCII 字符做判定；非 ASCII 码点与任何 ASCII 判定均为 false，
/// 与 Rust 按字符偏移的 peek 在这些判定上等价。
fn Scanner::peek(self : Scanner, off : Int) -> Int {
  decode_at(self.bytes, self.pos + off)
}

///|
fn Scanner::at_end(self : Scanner) -> Bool {
  self.pos >= self.length
}

///|
/// 推进一个码点：ASCII 快路径；多字节按序列长度跳（列 +1——Rust char 计数）。
/// 返回被消费的码点（越界返回 -1 且不推进）。
fn Scanner::advance(self : Scanner) -> Int {
  if self.pos >= self.length {
    return -1
  }
  let c = decode_at(self.bytes, self.pos)
  let n = if self.bytes[self.pos] >= b'\x80' {
    utf8_seq_len(self.bytes[self.pos])
  } else {
    1
  }
  self.pos += n
  if c == '\n' {
    self.line += 1
    self.real_line += 1
    self.column = 1
  } else {
    self.column += 1
  }
  c
}

///|
/// 匹配双字符：`peek(1) == expected` 时消费两字符返回 true（Rust `match_char`）。
fn Scanner::match_char(self : Scanner, expected : Int) -> Bool {
  if self.peek(1) != expected {
    return false
  }
  ignore(self.advance())
  ignore(self.advance())
  true
}

///|
/// 列号反推基准可变的组装（string/char 专用：Rust `make_token(ty, &源切片)`
/// 用**源切片**长度减列，随后才覆盖 `tok.text` 为解码值——value 与源拼写
/// 不等长时两口径分叉，此处复刻源切片基准）。
fn Scanner::make_token_col_basis(
  self : Scanner,
  ty : @token.TokenType,
  text : String,
  start : Int,
  col_basis : String,
) -> @token.Token {
  {
    ty,
    text,
    loc: {
      line: self.line,
      column: self.column - utf8_len(col_basis),
      file_id: self.file_id,
    },
    byte_off: self.base_byte_off + start,
    real_line: self.real_line,
  }
}

///|
/// 组装 token。列号反推：`column - text 的 UTF-8 字节数`（复刻口径，见包注释）。
fn Scanner::make_token(
  self : Scanner,
  ty : @token.TokenType,
  text : String,
  start_byte : Int,
) -> @token.Token {
  {
    ty,
    text,
    loc: {
      line: self.line,
      column: self.column - utf8_len(text),
      file_id: self.file_id,
    },
    byte_off: self.base_byte_off + start_byte,
    real_line: self.real_line,
  }
}

///|
fn Scanner::err(
  self : Scanner,
  message : String,
  code : @diag.ErrorCode,
) -> Unit {
  self.errors.push({
    message,
    loc: { line: self.line, column: self.column, file_id: self.file_id, },
    code,
  })
}

///|
/// 跳过 ASCII 空白（Rust `is_ascii_whitespace` 集合——BOM 不在内，⑥-8 由
/// L2 管线的字节预处理阶段剥离）。
fn Scanner::skip_whitespace(self : Scanner) -> Unit {
  while !self.at_end() {
    if is_ascii_whitespace(self.peek(0)) {
      ignore(self.advance())
    } else {
      break
    }
  }
}

///|
fn Scanner::skip_comment(self : Scanner) -> Unit {
  while !self.at_end() && self.peek(0) != '\n' {
    ignore(self.advance())
  }
}

///|
fn Scanner::skip_block_comment(self : Scanner) -> Unit {
  ignore(self.advance()) // '/'
  ignore(self.advance()) // '*'
  while !self.at_end() {
    if self.peek(0) == '*' && self.peek(1) == '/' {
      ignore(self.advance())
      ignore(self.advance())
      return
    }
    ignore(self.advance())
  }
  self.err("块注释未闭合", E1010_UnterminatedComment)
}

///|
/// 主循环：while 重派发（禁递归——⑥-15 栈溢出教训）。
fn Scanner::next_token(self : Scanner, body_mode~ : Bool) -> @token.Token {
  for ;; {
    self.skip_whitespace()
    if self.at_end() {
      return self.make_token(Eof, "", self.pos)
    }
    let c = self.peek(0)
    let start = self.pos
    if is_ascii_alphabetic(c) || c == '_' {
      return self.identifier_or_keyword(start)
    }
    if is_ascii_digit(c) {
      return self.number(start)
    }
    if c == '"' {
      return self.string_literal(start)
    }
    if c == '\'' {
      return self.char_literal(start)
    }
    if c == '/' && self.peek(1) == '/' {
      self.skip_comment()
      continue
    }
    if c == '/' && self.peek(1) == '*' {
      self.skip_block_comment()
      continue
    }
    if c == '#' {
      if body_mode {
        // 宏体内 `#`（字符串化）/ `##`（拼接）是操作符
        if self.match_char('#') {
          return self.make_token(HashHash, "##", start)
        }
        ignore(self.advance())
        return self.make_token(Hash, "#", start)
      }
      // L2 普通行内非首位置的 `#`：独立 pass 架构下指令行已被 pp 截走，
      // 行内 stray `#` 是非法字符（C 标准 stray '#'）——显式诊断。
      //（与 Rust 的差异已登记已知差异清单：Rust 会把任意位置的 `#` 当指令消费。）
      ignore(self.advance())
      self.err("无法识别的字符: '#'", E1001_UnknownChar)
      return self.make_token(Unknown, "#", start)
    }
    return match c {
      '+' =>
        if self.match_char('+') {
          self.make_token(Increment, "++", start)
        } else if self.match_char('=') {
          self.make_token(PlusAssign, "+=", start)
        } else {
          ignore(self.advance())
          self.make_token(Plus, "+", start)
        }
      '-' =>
        if self.match_char('>') {
          if self.peek(0) == '*' {
            ignore(self.advance())
            self.make_token(ArrowStar, "->*", start)
          } else {
            self.make_token(Arrow, "->", start)
          }
        } else if self.match_char('-') {
          self.make_token(Decrement, "--", start)
        } else if self.match_char('=') {
          self.make_token(MinusAssign, "-=", start)
        } else {
          ignore(self.advance())
          self.make_token(Minus, "-", start)
        }
      '*' =>
        if self.match_char('=') {
          self.make_token(StarAssign, "*=", start)
        } else {
          ignore(self.advance())
          self.make_token(Star, "*", start)
        }
      '/' =>
        if self.match_char('=') {
          self.make_token(SlashAssign, "/=", start)
        } else {
          ignore(self.advance())
          self.make_token(Slash, "/", start)
        }
      '%' =>
        if self.match_char('=') {
          self.make_token(PercentAssign, "%=", start)
        } else {
          ignore(self.advance())
          self.make_token(Percent, "%", start)
        }
      '=' =>
        if self.match_char('=') {
          self.make_token(Eq, "==", start)
        } else {
          ignore(self.advance())
          self.make_token(Assign, "=", start)
        }
      '!' =>
        if self.match_char('=') {
          self.make_token(Ne, "!=", start)
        } else {
          ignore(self.advance())
          self.make_token(Not, "!", start)
        }
      '&' =>
        if self.match_char('&') {
          self.make_token(AndAnd, "&&", start)
        } else if self.match_char('=') {
          self.make_token(AndAssign, "&=", start)
        } else {
          ignore(self.advance())
          self.make_token(Ampersand, "&", start)
        }
      '|' =>
        if self.match_char('|') {
          self.make_token(OrOr, "||", start)
        } else if self.match_char('=') {
          self.make_token(OrAssign, "|=", start)
        } else {
          ignore(self.advance())
          self.make_token(BitOr, "|", start)
        }
      '^' =>
        if self.match_char('=') {
          self.make_token(XorAssign, "^=", start)
        } else {
          ignore(self.advance())
          self.make_token(BitXor, "^", start)
        }
      '~' => {
        ignore(self.advance())
        self.make_token(BitNot, "~", start)
      }
      '<' =>
        if self.match_char('<') {
          if self.peek(0) == '=' {
            ignore(self.advance())
            self.make_token(ShlAssign, "<<=", start)
          } else {
            self.make_token(Shl, "<<", start)
          }
        } else if self.match_char('=') {
          self.make_token(Le, "<=", start)
        } else {
          ignore(self.advance())
          self.make_token(Lt, "<", start)
        }
      '>' =>
        if self.match_char('>') {
          if self.peek(0) == '=' {
            ignore(self.advance())
            self.make_token(ShrAssign, ">>=", start)
          } else {
            self.make_token(Shr, ">>", start)
          }
        } else if self.match_char('=') {
          self.make_token(Ge, ">=", start)
        } else {
          ignore(self.advance())
          self.make_token(Gt, ">", start)
        }
      ';' => {
        ignore(self.advance())
        self.make_token(Semicolon, ";", start)
      }
      ',' => {
        ignore(self.advance())
        self.make_token(Comma, ",", start)
      }
      '(' => {
        ignore(self.advance())
        self.make_token(LParen, "(", start)
      }
      ')' => {
        ignore(self.advance())
        self.make_token(RParen, ")", start)
      }
      '{' => {
        ignore(self.advance())
        self.make_token(LBrace, "{", start)
      }
      '}' => {
        ignore(self.advance())
        self.make_token(RBrace, "}", start)
      }
      '[' => {
        ignore(self.advance())
        self.make_token(LBracket, "[", start)
      }
      ']' => {
        ignore(self.advance())
        self.make_token(RBracket, "]", start)
      }
      '.' =>
        if self.match_char('*') {
          self.make_token(DotStar, ".*", start)
        } else if self.peek(1) == '.' && self.peek(2) == '.' {
          ignore(self.advance())
          ignore(self.advance())
          ignore(self.advance())
          self.make_token(Ellipsis, "...", start)
        } else if is_ascii_digit(self.peek(1)) {
          // U1#7：前导点浮点（.5 / .25f）是合法 C，转入 number 的 dot_float 分支
          self.number(start)
        } else {
          ignore(self.advance())
          self.make_token(Dot, ".", start)
        }
      ':' =>
        if self.match_char(':') {
          self.make_token(ColonColon, "::", start)
        } else {
          ignore(self.advance())
          self.make_token(Colon, ":", start)
        }
      '?' => {
        ignore(self.advance())
        self.make_token(Question, "?", start)
      }
      _ => {
        ignore(self.advance())
        self.err(
          "无法识别的字符: '\{char_from_code(c)}'",
          E1001_UnknownChar,
        )
        self.make_token(Unknown, char_to_string(char_from_code(c)), start)
      }
    }
  }
}

///|
fn char_from_code(c : Int) -> Char {
  // 码点 → Char（Unicode 标量域内；-1 哨兵不可达——advance 前有 at_end 检查）
  match c.to_char() {
    Some(ch) => ch
    None => '\u{FFFD}'
  }
}

///|
fn char_to_string(c : Char) -> String {
  "\{c}"
}

///|
fn Scanner::identifier_or_keyword(self : Scanner, start : Int) -> @token.Token {
  while !self.at_end() {
    let c = self.peek(0)
    if is_ascii_alphanumeric(c) || c == '_' {
      ignore(self.advance())
    } else {
      break
    }
  }
  let text = slice_string(self.bytes, start, self.pos)
  // C23 u8 前缀字符串：u8"..." 按普通字符串字面量处理（教学子集差异，spec 已记录）
  if text == "u8" && self.peek(0) == '"' {
    // byte_off 记 u8 起点（Rust next_token 统一记账点）；col 基准从引号起
    return self.string_literal(start, span_start=self.pos)
  }
  let ty = match keyword_type(text) {
    Some(k) => k
    None =>
      if self.cpp_mode {
        match cpp_keyword_type(text) {
          Some(k) => k
          None => Identifier
        }
      } else {
        Identifier
      }
  }
  self.make_token(ty, text, start)
}
