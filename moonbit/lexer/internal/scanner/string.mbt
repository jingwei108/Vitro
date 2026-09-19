///|
/// 字符串字面量（照搬 `string.rs` P4 修复后语义：八进制 \ooo 1~3 位 ≤0xFF、
/// hex \xHH 1~2 位、第 3 位报超范围；value 为**解码值**——8.2-1 口径，
/// 转义产物按 Unicode 码点入串（Rust `value.push(byte as char)` 同构））。
fn Scanner::string_literal(
  self : Scanner,
  start : Int,
  span_start? : Int = -1,
) -> @token.Token {
  // span_start：列号反推基准切片的起点（默认 = start）。u8 前缀分支的
  // Rust 语义：byte_off 记在 u8 起点（next_token 统一记账点），但
  // string_literal() 内部 start = 当前 pos（引号处）——col 基准不含 u8
  let span_from = if span_start < 0 { start } else { span_start }
  ignore(self.advance()) // 开引号
  let value = StringBuilder()
  while !self.at_end() && self.peek(0) != '"' {
    if self.peek(0) == '\n' {
      self.err("字符串不能跨行", E1003_StringCrossLine)
      break
    }
    if self.peek(0) == '\\' && self.pos + 1 < self.length {
      let next = self.peek(1)
      match next {
        'n' => {
          value <+ "\n"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        't' => {
          value <+ "\t"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        'r' => {
          value <+ "\r"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        'a' => {
          value <+ "\u{7}"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        'b' => {
          value <+ "\u{8}"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        'f' => {
          value <+ "\u{C}"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        'v' => {
          value <+ "\u{B}"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        '\\' => {
          value <+ "\\"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        '"' => {
          value <+ "\""
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        _ => ()
      }
      if is_octal_digit(next) {
        // P4：八进制转义 \ooo（1~3 位，值 ≤ 0xFF）；超范围对齐 clang error 口径。
        // 错误恢复值统一为 0（2026-09-19 审阅处置口径）
        let mut digits = 1
        let mut val = next - '0'
        while digits < 3 && is_octal_digit(self.peek(1 + digits)) {
          val = val * 8 + (self.peek(1 + digits) - '0')
          digits += 1
        }
        if val > 0xFF {
          self.err(
            "八进制转义序列超出范围（教学子集最多 3 位且值 ≤ 0xFF）",
            E1006_UnsupportedFeature,
          )
          value <+ "\u{0}"
        } else {
          value <+ "\{char_from_code(val)}"
        }
        for _ in 0..<=digits {
          ignore(self.advance())
        }
        continue
      }
      if next == 'x' {
        // P4：hex 1~2 位（对齐 char 侧 U1#7 与 clang）；第 3 位 hexdigit 报"超出范围"
        let h1 = self.peek(2)
        if !is_ascii_hexdigit(h1) {
          self.err("字符串十六进制转义缺少数字", E1001_UnknownChar)
          value <+ "x"
          ignore(self.advance())
          ignore(self.advance())
          continue
        }
        let two = is_ascii_hexdigit(self.peek(3))
        let after = if two { 4 } else { 3 }
        if is_ascii_hexdigit(self.peek(after)) {
          let mut k = after
          while is_ascii_hexdigit(self.peek(k)) {
            k += 1
          }
          self.err(
            "十六进制转义序列超出范围（教学子集最多 2 位 hex）",
            E1006_UnsupportedFeature,
          )
          for _ in 0..<k {
            ignore(self.advance())
          }
          continue
        }
        let hex = slice_string(self.bytes, self.pos + 2, self.pos + after)
        let byte = parse_hex_byte(hex)
        value <+ "\{char_from_code(byte)}"
        for _ in 0..<after {
          ignore(self.advance())
        }
        continue
      }
      // 未知转义：原样保留两字符中的 next
      value <+ "\{char_from_code(next)}"
      ignore(self.advance())
      ignore(self.advance())
      continue
    }
    let c = self.advance()
    if c >= 0 {
      value <+ "\{char_from_code(c)}"
    }
  }
  if self.at_end() || self.peek(0) != '"' {
    // S2 审阅 P2：行级 fragment 下"行后有换行"的未闭合字符串补报 E1003
    // （不能跨行）——复刻 Rust 整源扫描的 E1003 + E1002 两条；源尾行只报 E1002
    if self.line_mode && self.at_end() && self.line_has_newline {
      self.err("字符串不能跨行", E1003_StringCrossLine)
    }
    self.err("字符串未闭合", E1002_UnterminatedString)
  } else {
    ignore(self.advance()) // 闭引号
  }
  // Rust 口径：col 反推基准 = 源切片（u8 分支不含前缀——Rust 的 start 在引号处），
  // text 字段 = 解码值；byte_off = start（u8 分支为 u8 起点）
  self.make_token_col_basis(
    String,
    value.to_string(),
    start,
    slice_string(self.bytes, span_from, self.pos),
  )
}

///|
/// 字符字面量（照搬 `string.rs` char 侧 U1#7/P4 语义；`text` 为**数值十进制**，
/// invalid 时保留源拼写——Rust `if valid { tok.text = value.to_string() }` 同构）。
fn Scanner::char_literal(self : Scanner, start : Int) -> @token.Token {
  ignore(self.advance()) // 开引号
  let mut value : Int = 0
  let mut valid = true
  if !self.at_end() && self.peek(0) == '\'' {
    self.err("空字符字面量", E1001_UnknownChar)
    valid = false
  } else if !self.at_end() && self.peek(0) == '\\' && self.pos + 1 < self.length {
    let next = self.peek(1)
    let simple : Int? = match next {
      'n' => Some('\n')
      't' => Some('\t')
      'r' => Some('\r')
      'a' => Some(0x07)
      'b' => Some(0x08)
      'f' => Some(0x0C)
      'v' => Some(0x0B)
      '\\' => Some('\\')
      '\'' => Some('\'')
      _ => None
    }
    if simple is Some(v) {
      value = v
      ignore(self.advance())
      ignore(self.advance())
    } else if is_octal_digit(next) {
      // P4：八进制 \ooo（1~3 位 ≤ 0xFF），超范围报错；分支内消费完整序列
      let mut digits = 1
      let mut val = next - '0'
      while digits < 3 && is_octal_digit(self.peek(1 + digits)) {
        val = val * 8 + (self.peek(1 + digits) - '0')
        digits += 1
      }
      if val > 0xFF {
        self.err(
          "八进制转义序列超出范围（教学子集最多 3 位且值 ≤ 0xFF）",
          E1006_UnsupportedFeature,
        )
        valid = false
        value = 0
      } else {
        value = val
      }
      for _ in 0..<=digits {
        ignore(self.advance())
      }
    } else if next == 'x' {
      // U1#7：\x 后 1~2 位 hex；第 3 位 hexdigit 即"超范围"报错（clang 口径）
      let h1 = self.peek(2)
      let h2 = self.peek(3)
      if !is_ascii_hexdigit(h1) {
        // Rust 口径：此处不消费字符（next=='x' 使 match 后的统一推进不触发），
        // 闭引号判定随因 peek 停在 '\\' 而报"未闭合"
        self.err(
          "字符字面量十六进制转义缺少数字",
          E1001_UnknownChar,
        )
        valid = false
        value = 0
      } else {
        let two = is_ascii_hexdigit(h2)
        let after = if two { 4 } else { 3 }
        if is_ascii_hexdigit(self.peek(after)) {
          let mut k = after
          while is_ascii_hexdigit(self.peek(k)) {
            k += 1
          }
          for _ in 0..<k {
            ignore(self.advance())
          }
          self.err(
            "十六进制转义序列超出范围（教学子集最多 2 位 hex）",
            E1006_UnsupportedFeature,
          )
          valid = false
          value = 0
        } else {
          let hex = slice_string(self.bytes, self.pos + 2, self.pos + after)
          value = parse_hex_byte(hex)
          for _ in 0..<after {
            ignore(self.advance())
          }
        }
      }
    } else {
      self.err(
        "未知字符转义: '\\\{char_from_code(next)}'",
        E1001_UnknownChar,
      )
      valid = false
      value = 0
      ignore(self.advance())
      ignore(self.advance())
    }
  } else if !self.at_end() {
    value = self.peek(0)
    ignore(self.advance())
  } else {
    self.err("字符字面量未闭合", E1002_UnterminatedString)
    valid = false
  }
  if !self.at_end() && self.peek(0) == '\'' {
    ignore(self.advance())
  } else {
    self.err("字符字面量未闭合", E1002_UnterminatedString)
    valid = false
  }
  // Rust 口径：col 反推基准 = 源切片（含引号），text 字段 = 数值（valid 时）
  let span = slice_string(self.bytes, start, self.pos)
  if valid {
    self.make_token_col_basis(CharLiteral, "\{value}", start, span)
  } else {
    self.make_token_col_basis(CharLiteral, span, start, span)
  }
}

///|
/// 两位以内 hex 文本 → 字节值（Rust `u8::from_str_radix(.., 16).unwrap_or(0)`）。
fn parse_hex_byte(hex : String) -> Int {
  let mut v : Int = 0
  for i = 0; i < hex.length(); i = i + 1 {
    let c = hex[i].to_int()
    let d : Int = if c >= '0' && c <= '9' {
      c - '0'
    } else if c >= 'a' && c <= 'f' {
      c - 'a' + 10
    } else if c >= 'A' && c <= 'F' {
      c - 'A' + 10
    } else {
      return 0
    }
    v = v * 16 + d
  }
  v
}
