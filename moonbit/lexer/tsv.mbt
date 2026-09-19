///|
/// L1/L2 token TSV emitter（差分对拍锚点出口——B 级锚，勘察 ⑧）。
///
/// 格式（两侧逐字节一致的契约，Rust 侧 `vitro_cli dump-tokens` 同构）：
/// - L1（raw）：`<index>\t<Ty>\t<text 转义>\t<line>\t<col>\t<byte_off>`
/// - L2（pp 后）：`<index>\t<Ty>\t<text 转义>\t<line>\t<col>`
/// - 末行：`count=<n>\terrors=<n>\twarnings=<n>`
/// - text 转义：`\` → `\\`、换行 → `\n`、制表 → `\t`、回车 → `\r`
///   （其余字符原样，含非 ASCII——两侧源与转义表同构，逐字节可比）。
///
/// 口径（勘察 8.2 三条固化）：
/// 1. `text` 为**规范化值**（整数十进制 / 字符串解码值 / 字符数值 / 浮点源拼写）；
/// 2. `col` = 复刻口径 `column - text 的 UTF-8 字节数`（已知偏差故意保留换可比，
///    无缺陷坐标用 byte_off）；
/// 3. L2 位置钉宏调用点（与 Rust 展开器行为一致）。
pub fn dump_raw_tsv(result : @token.RawResult) -> String {
  let buf = StringBuilder()
  for i in 0..<result.tokens.length() {
    let t = result.tokens[i]
    buf <+
      "\{i}\t\{t.ty.to_name()}\t\{escape_tsv_text(t.text)}\t\{t.loc.line}\t\{t.loc.column}\t\{t.byte_off}\n"
  }
  buf <+
    "count=\{result.tokens.length() - 1}\terrors=\{result.errors.length()}\twarnings=0\n"
  buf.to_string()
}

///|
pub fn dump_lex_tsv(result : @token.LexResult) -> String {
  let buf = StringBuilder()
  for i in 0..<result.tokens.length() {
    let t = result.tokens[i]
    buf <+
      "\{i}\t\{t.ty.to_name()}\t\{escape_tsv_text(t.text)}\t\{t.loc.line}\t\{t.loc.column}\n"
  }
  buf <+
    "count=\{result.tokens.length() - 1}\terrors=\{result.errors.length()}\twarnings=\{result.warnings.length()}\n"
  buf.to_string()
}

///|
fn escape_tsv_text(text : String) -> String {
  let buf = StringBuilder()
  for c in text {
    match c {
      '\\' => buf <+ "\\\\"
      '\n' => buf <+ "\\n"
      '\t' => buf <+ "\\t"
      '\r' => buf <+ "\\r"
      _ => buf <+ "\{c}"
    }
  }
  buf.to_string()
}
