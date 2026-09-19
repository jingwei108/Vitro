///|
/// E4 锚出口（error_catalog JSON）——与 Rust `error_catalog::export_json`
/// 同形：`{"catalog":[{...},...]}`，码升序，字段序
/// code / code_str / lang / category / emoji / title / explanation / common_causes。
///
/// 显式 emitter 纪律（B 级锚）：两侧手写序列化，禁一侧 serde 一侧 ToJson
/// 派生直拼——字段序与转义在此单点可控。
///
/// lang 值域对齐 Rust 目录导出（"c" / "c++"）：SourceLang::Cpp 的
/// 协议标识 "cpp"（is_cpp_mode 单源）与此处的展示值 "c++" 是两个
/// 语义层，emitter 内显式映射，不共用 to_str。
///
/// category 值域：1xxx=词法 / 2xxx=语法 / 3xxx=语义 / 4xxx=C++。
pub fn export_catalog_json() -> String {
  let buf = StringBuilder()
  buf.write_string("{\"catalog\":[")
  let codes = catalog_codes()
  for i, code in codes {
    if i > 0 {
      buf.write_string(",")
    }
    let op = ErrorCode::from_code(code).unwrap()
    let e = op.catalog().unwrap()
    buf.write_string("{\"code\":\{e.code}")
    buf.write_string(",\"code_str\":\"\{op.display_code()}\"")
    buf.write_string(",\"lang\":\"\{export_lang_of(e.code)}\"")
    buf.write_string(",\"category\":\"\{category_of(e.code)}\"")
    buf.write_string(",\"emoji\":\"\{json_escape(e.emoji)}\"")
    buf.write_string(",\"title\":\"\{json_escape(e.title)}\"")
    buf.write_string(",\"explanation\":\"\{json_escape(e.explanation)}\"")
    buf.write_string(",\"common_causes\":[")
    for j, cause in e.common_causes {
      if j > 0 {
        buf.write_string(",")
      }
      buf.write_string("\"\{json_escape(cause)}\"")
    }
    buf.write_string("]}")
  }
  buf.write_string("]}")
  buf.to_string()
}

///|
/// lang 导出值（Rust lang_of_code 对齐：1000..=3999 "c" / 4000.. "c++"）。
fn export_lang_of(code : Int) -> String {
  if code >= 4000 {
    "c++"
  } else {
    "c"
  }
}

///|
/// category 导出值（Rust category_of_code 对齐）。
fn category_of(code : Int) -> String {
  match code / 1000 {
    1 => "词法"
    2 => "语法"
    3 => "语义"
    4 => "C++"
    _ => "其它"
  }
}

///|
/// JSON 转义（Rust json_escape 同集：" \ \n \r \t；不引 serde——显式 emitter）。
fn json_escape(s : String) -> String {
  let buf = StringBuilder()
  for c in s {
    match c {
      '"' => buf.write_string("\\\"")
      '\\' => buf.write_string("\\\\")
      '\n' => buf.write_string("\\n")
      '\r' => buf.write_string("\\r")
      '\t' => buf.write_string("\\t")
      _ => buf.write_string("\{c}")
    }
  }
  buf.to_string()
}
