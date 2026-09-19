///|
/// 无符号 64 位累积解析（radix ∈ {2,8,10,16}；分隔符 `'` 已由调用方滤除）。
/// 溢出返回 None——对应 Rust `u64::from_str_radix` 的 `Err`。
/// MoonBit `UInt64` 四则静默回绕（F1），溢出判定显式前置：
/// `val > UMAX/radix` 即溢出（UMAX ≡ -1 mod radix，d ≤ radix-1 时充要）。
fn parse_u64(text : String, radix : UInt64) -> UInt64? {
  let limit : UInt64 = 18446744073709551615UL / radix
  let mut val : UInt64 = 0UL
  for i = 0; i < text.length(); i = i + 1 {
    let c = text[i].to_int()
    let d : Int = if c >= '0' && c <= '9' {
      c - '0'
    } else if c >= 'a' && c <= 'f' {
      c - 'a' + 10
    } else if c >= 'A' && c <= 'F' {
      c - 'A' + 10
    } else {
      return None
    }
    if d.to_uint64() >= radix {
      return None
    }
    if val > limit {
      return None
    }
    val = val * radix + d.to_uint64()
  }
  Some(val)
}

///|
/// u32 值域判定（Rust `u32::try_from(val).is_ok()`）。
fn fits_u32(val : UInt64) -> Bool {
  val <= 4294967295UL
}

///|
/// i64 值域判定（Rust `i64::try_from(val).is_ok()`）。
fn fits_i64(val : UInt64) -> Bool {
  val <= 9223372036854775807UL
}

///|
fn u64_to_string(val : UInt64) -> String {
  "\{val}"
}

///|
/// 数字字面量（照搬 `number.rs` 全语义：hex/bin/octal/decimal + `'` 分隔符 +
/// 前导点/指数浮点 + U/L 后缀值域分派；U1#7/P4 修复口径随源搬）。
fn Scanner::number(self : Scanner, start : Int) -> @token.Token {
  let mut has_integer_prefix = false
  let mut val : UInt64 = 0UL
  // Hexadecimal: 0x + hexdigit，允许 ' 分隔符（C23）
  if self.peek(0) == '0' && (self.peek(1) == 'x' || self.peek(1) == 'X') {
    has_integer_prefix = true
    ignore(self.advance()) // '0'
    ignore(self.advance()) // 'x' / 'X'
    let hex_start = self.pos
    while !self.at_end() {
      let c = self.peek(0)
      if is_ascii_hexdigit(c) {
        ignore(self.advance())
      } else if c == '\'' && is_ascii_hexdigit(self.peek(1)) {
        ignore(self.advance()) // 分隔符不进值
      } else {
        break
      }
    }
    if self.pos == hex_start {
      self.err("十六进制数字格式错误", E1001_UnknownChar)
      return self.make_token(Unknown, "0x", start)
    }
    let hex_str = filter_sep(slice_string(self.bytes, hex_start, self.pos))
    match parse_u64(hex_str, 16UL) {
      Some(v) => val = v
      None => {
        self.err(
          "十六进制数值 0x\{hex_str} 超出可表示范围",
          E1006_UnsupportedFeature,
        )
        return self.make_token(Number, "0", start)
      }
    }
    // Binary（C23）: 0b + [01]，允许 ' 分隔符
  } else if self.peek(0) == '0' && (self.peek(1) == 'b' || self.peek(1) == 'B') {
    has_integer_prefix = true
    ignore(self.advance()) // '0'
    ignore(self.advance()) // 'b' / 'B'
    let bin_start = self.pos
    while !self.at_end() {
      let c = self.peek(0)
      if c == '0' || c == '1' {
        ignore(self.advance())
      } else if c == '\'' && (self.peek(1) == '0' || self.peek(1) == '1') {
        ignore(self.advance())
      } else {
        break
      }
    }
    if self.pos == bin_start {
      self.err("二进制数字格式错误", E1001_UnknownChar)
      return self.make_token(Unknown, "0b", start)
    }
    let bin_str = filter_sep(slice_string(self.bytes, bin_start, self.pos))
    match parse_u64(bin_str, 2UL) {
      Some(v) => val = v
      None => {
        self.err(
          "二进制数值 0b\{bin_str} 超出可表示范围",
          E1006_UnsupportedFeature,
        )
        return self.make_token(Number, "0", start)
      }
    }
    // Octal: 0[0-7]+
  } else if self.peek(0) == '0' && is_ascii_digit(self.peek(1)) {
    has_integer_prefix = true
    ignore(self.advance()) // '0'
    let oct_start = self.pos
    while !self.at_end() && self.peek(0) >= '0' && self.peek(0) <= '7' {
      ignore(self.advance())
    }
    // U1#7：08/9 残段消费并报错（此前拆成 "0"+"8" 零词法诊断，错误全错位）
    if self.peek(0) == '8' || self.peek(0) == '9' {
      while !self.at_end() && is_ascii_digit(self.peek(0)) {
        ignore(self.advance())
      }
      self.err(
        "八进制常量中含非法数字（八进制只能用 0~7）",
        E1006_UnsupportedFeature,
      )
      return self.make_token(Number, "0", start)
    }
    if self.pos > oct_start {
      let oct_str = slice_string(self.bytes, oct_start, self.pos)
      match parse_u64(oct_str, 8UL) {
        Some(v) => val = v
        None => {
          self.err(
            "八进制数值 0\{oct_str} 超出可表示范围",
            E1006_UnsupportedFeature,
          )
          return self.make_token(Number, "0", start)
        }
      }
    }
    // Decimal，允许 ' 分隔符（C23，如 1'000'000）
  } else {
    while !self.at_end() {
      let c = self.peek(0)
      if is_ascii_digit(c) {
        ignore(self.advance())
      } else if c == '\'' && is_ascii_digit(self.peek(1)) {
        ignore(self.advance())
      } else {
        break
      }
    }
  }

  // 浮点检查必须在十进制数字扫描之后（peek 停在 '.' 或 'e' 上）；
  // 指数部分（C89）：float.h 的 DBL_EPSILON=2.22e-16 等宏依赖
  let dot_float = !has_integer_prefix &&
    self.peek(0) == '.' &&
    (is_ascii_digit(self.peek(1)) || self.peek(1) == 'f' || self.peek(1) == 'F')
  let exp_float = !has_integer_prefix &&
    (self.peek(0) == 'e' || self.peek(0) == 'E') &&
    (
      is_ascii_digit(self.peek(1)) ||
      (
        (self.peek(1) == '+' || self.peek(1) == '-') &&
        is_ascii_digit(self.peek(2))
      )
    )
  if dot_float || exp_float {
    if dot_float {
      ignore(self.advance()) // '.'
      while !self.at_end() && is_ascii_digit(self.peek(0)) {
        ignore(self.advance())
      }
    }
    // 指数部分可在小数点之后链式出现（2.2e-3），以当前 peek 重新判定
    if (self.peek(0) == 'e' || self.peek(0) == 'E') &&
      (
        is_ascii_digit(self.peek(1)) ||
        (
          (self.peek(1) == '+' || self.peek(1) == '-') &&
          is_ascii_digit(self.peek(2))
        )
      ) {
      ignore(self.advance()) // 'e' / 'E'
      if self.peek(0) == '+' || self.peek(0) == '-' {
        ignore(self.advance())
      }
      while !self.at_end() && is_ascii_digit(self.peek(0)) {
        ignore(self.advance())
      }
    }
    if self.peek(0) == 'f' || self.peek(0) == 'F' {
      ignore(self.advance())
    }
    let text = slice_string(self.bytes, start, self.pos)
    return self.make_token(FloatLiteral, text, start) // text = 源拼写（8.2-1 口径）
  }

  // Decimal 值在 suffix 解析前求出
  if !has_integer_prefix {
    let text = filter_sep(slice_string(self.bytes, start, self.pos))
    // U1#7：超 u64 十进制此前静默变 0；口径对齐 hex/bin 报 E1006，返回 0 继续收诊断
    match parse_u64(text, 10UL) {
      Some(v) => val = v
      None => {
        self.err(
          "整数常量 \{text} 超出可表示范围",
          E1006_UnsupportedFeature,
        )
        val = 0UL
      }
    }
  }

  // Suffix: [Uu][Ll]? or [Ll][Uu]?
  let mut has_u = false
  let mut has_l = false
  if self.peek(0) == 'U' || self.peek(0) == 'u' {
    has_u = true
    ignore(self.advance())
    if self.peek(0) == 'L' || self.peek(0) == 'l' {
      has_l = true
      ignore(self.advance())
      if self.peek(0) == 'L' || self.peek(0) == 'l' {
        ignore(self.advance())
      }
    }
  } else if self.peek(0) == 'L' || self.peek(0) == 'l' {
    has_l = true
    ignore(self.advance())
    if self.peek(0) == 'L' || self.peek(0) == 'l' {
      ignore(self.advance())
    }
    if self.peek(0) == 'U' || self.peek(0) == 'u' {
      has_u = true
      ignore(self.advance())
    }
  }

  let val_str = u64_to_string(val)
  if has_u && has_l {
    // unsigned long (long)——按值域升级：u32 内 unsigned，i64 内 long long，
    // (i64::MAX, u64::MAX] 以 64 位位模式承载为 unsigned long long（E1 B 档）
    if fits_u32(val) {
      self.make_token(UnsignedLiteral, val_str, start)
    } else if fits_i64(val) {
      self.make_token(LongLiteral, val_str, start)
    } else {
      self.make_token(UnsignedLiteral, val_str, start)
    }
  } else if has_u {
    if fits_u32(val) {
      self.make_token(UnsignedLiteral, val_str, start)
    } else if fits_i64(val) {
      self.make_token(LongLiteral, val_str, start)
    } else {
      self.make_token(UnsignedLiteral, val_str, start)
    }
  } else if has_l {
    if !fits_i64(val) {
      // C 标准十进制常量溢出有符号域时转 unsigned long long（LLONG_MIN 量值
      // 9223372036854775808LL；配合一元负号即得 LLONG_MIN 位模式）
      self.make_token(UnsignedLiteral, val_str, start)
    } else {
      self.make_token(LongLiteral, val_str, start)
    }
  } else if has_integer_prefix {
    if fits_i32(val) {
      self.make_token(Number, val_str, start)
    } else if fits_u32(val) {
      self.make_token(UnsignedLiteral, val_str, start)
    } else if fits_i64(val) {
      self.make_token(LongLiteral, val_str, start)
    } else {
      self.err(
        "整数常量 \{val_str} 超出可表示范围",
        E1006_UnsupportedFeature,
      )
      self.make_token(Number, "0", start)
    }
    // Decimal 无后缀：不自动升 unsigned
  } else if fits_i32(val) {
    self.make_token(Number, val_str, start)
  } else if fits_i64(val) {
    self.make_token(LongLiteral, val_str, start)
  } else {
    self.err(
      "整数常量 \{val_str} 超出可表示范围",
      E1006_UnsupportedFeature,
    )
    self.make_token(Number, "0", start)
  }
}

///|
/// i32 值域判定（Rust `i32::try_from(val).is_ok()`——无后缀默认 int 的判据）。
fn fits_i32(val : UInt64) -> Bool {
  val <= 2147483647UL
}

///|
/// 滤除数字分隔符 `'`（值解析文本口径）。
fn filter_sep(s : String) -> String {
  let buf = StringBuilder()
  for c in s {
    if c != '\'' {
      buf <+ "\{c}"
    }
  }
  buf.to_string()
}
