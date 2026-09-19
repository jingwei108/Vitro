///|
/// T2 覆盖率断言基线：137 臂 / 77 卡片 / 60 无卡片（77 + 60 = 137，
/// 清单互斥互补）。源漂移由 gen_diag fail loud 兜底，本断言锁生成产物。
test "coverage_baseline_137_77_60" {
  let all = ErrorCode::all()
  assert_eq(all.length(), 137)
  assert_eq(catalog_codes().length(), 77)
  assert_eq(codes_without_catalog().length(), 60)
  // 互补无交集
  let set : Map[Int, Bool] = Map([])
  for c in catalog_codes() {
    set[c] = true
  }
  for c in codes_without_catalog() {
    assert_false(set.contains(c))
  }
}

///|
/// 无卡片码全部 None；有卡片码全部 Some 且内容非空。
test "catalog_presence_per_code" {
  for c in codes_without_catalog() {
    assert_true(ErrorCode::from_code(c).unwrap().catalog() is None)
  }
  for c in catalog_codes() {
    let e = ErrorCode::from_code(c).unwrap().catalog().unwrap()
    assert_true(e.title.length() > 0)
    assert_true(e.explanation.length() > 0)
  }
}

///|
/// P3 段位前缀红锚同形（Rust test_code_prefix_* 的 MoonBit 侧镜像）：
/// W/H 码显示串与 severity 一致；E 码与未知默认 E。
test "display_code_prefix_alignment" {
  assert_eq(W3053_ImplicitScalarConversion.display_code(), "W3053")
  assert_eq(W1018_MacroShadowing.display_code(), "W1018")
  assert_eq(W3067_PointerTypeMismatch.display_code(), "W3067")
  assert_eq(H3057_ImplicitConversionHint.display_code(), "H3057")
  assert_eq(E1006_UnsupportedFeature.display_code(), "E1006")
  assert_eq(E3060_UseAfterFree.display_code(), "E3060")
  assert_eq(E4022_TemplateRecursionDepthExceeded.display_code(), "E4022")
  assert_eq(Unknown.display_code(), "E0")
}

///|
/// severity 与 display 首字母互证（两侧独立来源：severity 来自白名单
/// 分类，display 首字母来自生成时前缀拼接——单源漂移即红）。
test "severity_display_first_char_agree" {
  for code in ErrorCode::all() {
    let expected = match code.severity() {
      Error => "E"
      Warning => "W"
      Hint => "H"
    }
    let d = code.display_code()
    assert_true(d.has_prefix(expected))
  }
}

///|
/// lang 段位：4xxx 全 Cpp（38 条），其余 C（99 条）。
test "lang_segment_4xxx_cpp" {
  let mut cpp = 0
  let mut c_lang = 0
  for code in ErrorCode::all() {
    match code.lang() {
      Cpp => {
        cpp += 1
        assert_true(code.code() >= 4000)
      }
      C => c_lang += 1
    }
  }
  assert_eq(cpp, 38)
  assert_eq(c_lang, 99)
}

///|
/// from_code 与 code 互逆（全量 137）；未知编号 None。
test "from_code_roundtrip_all" {
  for code in ErrorCode::all() {
    assert_eq(ErrorCode::from_code(code.code()).unwrap(), code)
  }
  assert_true(ErrorCode::from_code(9999) is None)
  assert_true(ErrorCode::from_code(3068) is None) // 3xxx 段空号
  assert_true(ErrorCode::from_code(3069) is None)
}

///|
/// 编号升序与唯一（all() 顺序契约——差分遍历确定性）。
test "all_codes_sorted_unique" {
  let mut prev = -1
  let seen : Map[Int, Bool] = Map([])
  for code in ErrorCode::all() {
    assert_true(code.code() > prev)
    assert_false(seen.contains(code.code()))
    seen[code.code()] = true
    prev = code.code()
  }
}

///|
/// 卡片内容抽查（转义正确性锚）：E1002 卡片中文与 emoji 原样。
test "catalog_content_escape_probe" {
  let e2 = E1002_UnterminatedString.catalog().unwrap()
  assert_eq(e2.title, "字符串未闭合")
  assert_eq(e2.emoji, "💬")
  let e3007 = E3007_StringInitNonCharArray.catalog().unwrap()
  assert_true(e3007.explanation.contains("char"))
}

///|
/// Severity / SourceLang 契约（to_str / prefix 显示面）。
test "severity_source_lang_display" {
  assert_eq(Severity::Error.to_str(), "error")
  assert_eq(Severity::Warning.to_str(), "warning")
  assert_eq(Severity::Hint.to_str(), "hint")
  assert_eq(SourceLang::C.to_str(), "c")
  assert_eq(SourceLang::Cpp.to_str(), "cpp")
}
