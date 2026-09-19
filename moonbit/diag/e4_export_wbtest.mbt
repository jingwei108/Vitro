///|
/// E4 出口形状：包裹结构 + 条目数 77（每条必含一个 common_causes 键）。
test "e4_export_shape_and_count" {
  let json = export_catalog_json()
  assert_true(json.has_prefix("{\"catalog\":["))
  assert_true(json.has_suffix("]}"))
  let hits = json.split("common_causes").length() - 1
  assert_eq(hits, 77)
}

///|
/// E4 首条逐字节锚（E1002——Rust 侧 export_json 的同条目对照）：
/// 字段序 / lang="c" / category="词法" / 转义形态。
test "e4_export_first_entry_bytes" {
  let json = export_catalog_json()
  // 首条即最小码 1002（catalog_codes 升序首位）
  let expect_head = "{\"catalog\":[{\"code\":1002,\"code_str\":\"E1002\",\"lang\":\"c\",\"category\":\"词法\",\"emoji\":\"💬\",\"title\":\"字符串未闭合\","
  assert_true(json.has_prefix(expect_head))
}

///|
/// E4 4xxx 段 lang="c++"、category="C++"（语言段位对齐锚）。
test "e4_export_cpp_segment" {
  let json = export_catalog_json()
  let probe = ErrorCode::from_code(4100).unwrap()
  assert_true(probe.catalog() is Some(_))
  // 4100 卡片存在，其导出片段必含 lang "c++"
  let frag = ",{\"code\":4100,\"code_str\":\"E4100\",\"lang\":\"c++\",\"category\":\"C++\","
  assert_true(json.contains(frag))
}

///|
/// W/H 码卡片码串前缀（P3 段位单源在 E4 出口的体现）：
/// W3051 有卡片（ArrayBoundOffByOne 在 catalog_codes 内），导出 code_str="W3051"。
test "e4_export_warn_prefix" {
  let json = export_catalog_json()
  assert_true(catalog_codes().contains(3051))
  let frag = "\"code\":3051,\"code_str\":\"W3051\""
  assert_true(json.contains(frag))
}

///|
/// json_escape 转义集（与 Rust json_escape 同集）。
test "json_escape_set" {
  inspect(json_escape("a\"b\\c\nd\te"), content="a\\\"b\\\\c\\nd\\te")
  inspect(json_escape("无转义"), content="无转义")
}
