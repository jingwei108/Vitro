///|
/// 内置宏表（照搬 `macros.rs` + `macro_table.rs::builtin` 全量语义）。
/// 纯数据表驱动（勘察 ⑤-8：521 行手写 token 构造改表驱动）——对象宏以
/// `(名, TokenType, text)` 三元组登记；`va_*` 三条函数式宏为显式 token 列表
///（body token 的 `(ty, text)` 逐项照搬，宏体不经 number() 重扫——负号数字
/// 如 INT_MIN 的 "-2147483648" 是**单个 Number token**，text 带负号，parser
/// 直接按文本取值，`macros.rs:195` 口径注释）。
fn builtin_macro_table() -> Map[String, @token.MacroDef] {
  let m : Map[String, @token.MacroDef] = Map([])
  // (name, ty, text) —— stdio/stdlib/limits/stdbool 常量宏
  let object_macros : Array[(String, @token.TokenType, String)] = [
    ("stdout", Number, "1"),
    ("stderr", Number, "2"),
    ("EOF", Number, "-1"),
    ("stdin", Number, "0"),
    ("EXIT_SUCCESS", Number, "0"),
    ("EXIT_FAILURE", Number, "1"),
    ("RAND_MAX", Number, "32767"),
    ("SEEK_SET", Number, "0"),
    ("SEEK_CUR", Number, "1"),
    ("SEEK_END", Number, "2"),
    ("INT_MAX", Number, "2147483647"),
    ("INT_MIN", Number, "-2147483648"),
    ("LONG_MAX", Number, "2147483647"),
    ("LONG_MIN", Number, "-2147483648"),
    ("CHAR_BIT", Number, "8"),
    ("SCHAR_MIN", Number, "-128"),
    ("SCHAR_MAX", Number, "127"),
    ("UCHAR_MAX", Number, "255"),
    ("SHRT_MIN", Number, "-32768"),
    ("SHRT_MAX", Number, "32767"),
    ("USHRT_MAX", Number, "65535"),
    ("UINT_MAX", UnsignedLiteral, "4294967295"),
    ("ULONG_MAX", UnsignedLiteral, "4294967295"),
    ("LLONG_MAX", LongLiteral, "9223372036854775807"),
    ("LLONG_MIN", LongLiteral, "-9223372036854775808"),
    ("ULLONG_MAX", UnsignedLiteral, "18446744073709551615"),
    ("true", Number, "1"),
    ("false", Number, "0"),
  ]
  for pair in object_macros {
    let (name, ty, text) = pair
    m[name] = @token.MacroDef::Object(body=[synth_token(ty, text)])
  }
  // E2 预定义宏族：__STDC_VERSION__ 报 202311L 名义锚点（把版本当能力探测的
  // 病根由三层配套解决，见 Rust macro_table.rs 注释）；__VITRO_SUBSET__ 引擎探测
  m["__STDC_VERSION__"] = @token.MacroDef::Object(body=[
    synth_token(Number, "202311L"),
  ])
  m["__VITRO_SUBSET__"] = @token.MacroDef::Object(body=[
    synth_token(Number, "1"),
  ])
  // stdarg.h：标准写法 → 内部 host func 调用（token 序列照 macros.rs 逐项搬）
  m["va_start"] = @token.MacroDef::Function(params=["ap", "last"], body=[
    synth_token(Identifier, "__vitro_va_start"),
    synth_token(LParen, "("),
    synth_token(Ampersand, "&"),
    synth_token(LParen, "("),
    synth_token(Identifier, "ap"),
    synth_token(RParen, ")"),
    synth_token(Comma, ","),
    synth_token(Ampersand, "&"),
    synth_token(LParen, "("),
    synth_token(Identifier, "last"),
    synth_token(RParen, ")"),
    synth_token(Comma, ","),
    synth_token(Sizeof, "sizeof"),
    synth_token(LParen, "("),
    synth_token(Identifier, "last"),
    synth_token(RParen, ")"),
    synth_token(RParen, ")"),
  ])
  m["va_arg"] = @token.MacroDef::Function(params=["ap", "type"], body=[
    synth_token(Star, "*"),
    synth_token(LParen, "("),
    synth_token(Identifier, "type"),
    synth_token(Star, "*"),
    synth_token(RParen, ")"),
    synth_token(Identifier, "__vitro_va_arg"),
    synth_token(LParen, "("),
    synth_token(Ampersand, "&"),
    synth_token(LParen, "("),
    synth_token(Identifier, "ap"),
    synth_token(RParen, ")"),
    synth_token(Comma, ","),
    synth_token(Sizeof, "sizeof"),
    synth_token(LParen, "("),
    synth_token(Identifier, "type"),
    synth_token(RParen, ")"),
    synth_token(RParen, ")"),
  ])
  m["va_end"] = @token.MacroDef::Function(params=["ap"], body=[
    synth_token(Identifier, "__vitro_va_end"),
    synth_token(LParen, "("),
    synth_token(Ampersand, "&"),
    synth_token(LParen, "("),
    synth_token(Identifier, "ap"),
    synth_token(RParen, ")"),
    synth_token(RParen, ")"),
  ])
  m
}

///|
/// 合成 token（内置宏体专用——line/column/file_id 全 0，Rust 同构）。
fn synth_token(ty : @token.TokenType, text : String) -> @token.Token {
  {
    ty,
    text,
    loc: { line: 0, column: 0, file_id: 0, },
    byte_off: 0,
    real_line: 0,
  }
}

///|
/// 宏体逐 token 相等比较（文本与类型都一致才算"相同定义"——W1018 判据）。
fn bodies_identical(a : Array[@token.Token], b : Array[@token.Token]) -> Bool {
  if a.length() != b.length() {
    return false
  }
  for i in 0..<a.length() {
    if a[i].ty != b[i].ty || a[i].text != b[i].text {
      return false
    }
  }
  true
}

///|
/// 宏参数表相等（区分对象式/函数式——同型才比参数）。
fn macro_defs_equivalent(a : @token.MacroDef, b : @token.MacroDef) -> Bool {
  // W1018 判据复刻 Rust 无分型口径（S2 审阅 P2）：Object vs Function(params=[])
  // 在 Rust 视角 params 同为空——body 相同即"相同定义"静默放行（此前跨型
  // 直接判不等价，`FOO 1` 后 `FOO() 1` 误报遮蔽）
  let params_of = (d : @token.MacroDef) => {
    match d {
      Object(..) => []
      Function(params~, ..) => params
    }
  }
  match (a, b) {
    (Object(body~), Object(body=body2)) => bodies_identical(body, body2)
    (Function(params~, body~), Function(params=p2, body=body2)) =>
      params == p2 && bodies_identical(body, body2)
    (Object(body~), Function(body=body2, ..))
    | (Function(body~, ..), Object(body=body2)) =>
      params_of(a).length() == 0 &&
      params_of(b).length() == 0 &&
      bodies_identical(body, body2)
  }
}

///|
/// `#define`（W1018 遮蔽警告文案逐字照搬；**无 4096 静默上限**——勘察 ③-9
/// 修复：Rust 超限静默丢弃 `#define`，零诊断指向真因）。
fn[P] Pp::define_macro(
  self : Pp[P],
  name : String,
  def : @token.MacroDef,
  line : Int,
) -> Unit {
  match self.macros.get(name) {
    Some(existing) =>
      if !macro_defs_equivalent(existing, def) {
        self.warnings.push({
          message: "宏 '\{name}' 被重复定义且宏体不同（遮蔽了先前的定义）。若非有意为之，请先 #undef 或改名",
          loc: { line, column: 0, file_id: self.current_file_id(), },
          code: W1018_MacroShadowing,
        })
      }
    None => ()
  }
  self.macros[name] = def
}
