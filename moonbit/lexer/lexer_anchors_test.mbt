///|
/// Rust `lexer_unit_test.rs` 57 用例迁移的**红锚补集**：其余形态已由
/// `lexer_test.mbt` 与 `scanner_wbtest.mbt` 覆盖（对照清单见 S2 执行记录）。
/// 本文件逐条对齐原锚用例名（U1#6 / U1#11 红锚族）。

///|
test "preprocessor depth fuse nested self reference" {
  // 同名自引用：蓝漆停止（C99 6.10.3.1）——有限展开零错误、不熔断不崩
  //（Rust 同构；深度保险丝防的是**不同名**链，见下一个测试）
  let r = @lexer.tokenize("#define ID(x) ID(ID(x))\nint y = ID(1);\n")
  assert_eq(r.errors.length(), 0)
}

///|
test "preprocessor byte budget large literal amplification" {
  // 原 test_preprocessor_byte_budget_large_literal_amplification：
  // 4KB **字符串**字面量 ×13 层翻倍 ≈ 33MB（16384 token 低于旧 token 预算
  // 但字节口径超 16MB 预算）——字节口径必须熔断（U1#6 红→绿锚）
  let big = "\"" + "a".repeat(4094) + "\""
  let buf = StringBuilder()
  for i in 0..<14 {
    buf <+ "#define S\{i}(x) S\{i + 1}(x) S\{i + 1}(x)\n"
  }
  buf <+ "#define S14(x) x\n"
  buf <+ "int v = S0(\{big});\n"
  let r = @lexer.tokenize(buf.to_string())
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1017")
}

///|
test "preprocessor normal nesting not affected by fuses" {
  // 原 test_preprocessor_normal_nesting_not_affected_by_fuses（反向锚）
  let src =
    #|#define A(x) ((x) + 1)
    #|#define B(x) A(A(x))
    #|#define C(x) B(B(x))
    #|int r = C(5);
  let r = @lexer.tokenize(src)
  assert_eq(r.errors.length(), 0)
  assert_eq(r.warnings.length(), 0)
}

///|
test "u11 include quote not found" {
  // 原 test_u11_include_quote_not_found：E1021 定位在 include 行
  let r = @lexer.tokenize_with_vfs(
    "#include \"no_such.h\"\nint x;\n",
    files=Map([]),
  )
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1021")
  inspect(r.errors[0].loc.line, content="1")
}

///|
test "u11 include angle not found" {
  let r = @lexer.tokenize("#include <stdio.h>\n#include <not_std.h>\nint x;\n")
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1021")
  inspect(r.errors[0].loc.line, content="2")
}

///|
test "u11 include angle custom rejected" {
  // 原 test_u11_include_angle_custom_rejected：<> 只匹配存根（H-3）
  let files : Map[String, String] = { "local.h": "int q;\n" }
  let r = @lexer.tokenize_with_vfs("#include <local.h>\nint x;\n", files~)
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1021")
}

///|
test "u11 has include uses includer dir" {
  // 原 test_u11_has_include_uses_includer_dir：头内探测**同目录** inner.h
  //（外层目录无 inner.h——包含者目录优先语义）
  let files : Map[String, String] = {
    "nest/outer.h": "#if __has_include(\"inner.h\")\nint have_inner;\n#else\nint no_inner;\n#endif\n",
    "nest/inner.h": "int inner_token;\n",
  }
  let r = @lexer.tokenize_with_vfs(
    "#include \"nest/outer.h\"\n",
    files~,
    base_dir=".",
  )
  assert_eq(r.errors.length(), 0)
  let texts = r.tokens.map(t => t.text).join(" ")
  assert_true(texts.contains("have_inner"))
}

///|
test "u11 has include angle stub only" {
  // 原 test_u11_has_include_angle_stub_only：<> 探测不搜文件系统
  let files : Map[String, String] = { "local.h": "int q;\n" }
  let vfs = @lexer.tokenize_with_vfs(
    "#if __has_include(<local.h>)\nint a;\n#else\nint b;\n#endif\n",
    files~,
  )
  inspect(vfs.tokens.map(t => t.text).join(" "), content="int b ; ")
}

///|
test "u11 header extra endif localized" {
  // 原 test_u11_header_extra_endif_localized：头内多余 #endif 不得弹掉
  // 包含者条件组（E1011 定位报错，主文件结构不被破坏）
  let files : Map[String, String] = {
    "bad.h": "#ifdef INNER\nint i;\n#endif\n#endif\n",
  }
  let r = @lexer.tokenize_with_vfs(
    "#define M 1\n#include \"bad.h\"\n#ifdef M\nint kept;\n#endif\n",
    files~,
  )
  let e1011 = r.errors.filter(e => @diag.ErrorCode::code(e.code) == 1011)
  assert_eq(e1011.length(), 1)
  // 主文件 kept 保留（条件组未被头破坏）
  assert_true(r.tokens.map(t => t.text).join(" ").contains("kept"))
}

///|
test "u11 header unclosed if autoclosed" {
  // 原 test_u11_header_unclosed_if_autoclosed：头内未闭合 #if 在边界闭合
  //（E1013），主文件后续不被吞
  let files : Map[String, String] = {
    "open.h": "#ifdef HIDDEN\nint hidden_line;\n",
  }
  let r = @lexer.tokenize_with_vfs(
    "#include \"open.h\"\nint main_line;\n",
    files~,
  )
  let e1013 = r.errors.filter(e => @diag.ErrorCode::code(e.code) == 1013)
  assert_eq(e1013.length(), 1)
  assert_true(r.tokens.map(t => t.text).join(" ").contains("main_line"))
}

///|
test "u11 include cycle 20 files" {
  // 原 test_u11_include_cycle_20_files：20 文件环检出（E1015）——
  // 环检测 visited 路径局部化（M2 修复）下的正确检出
  let files : Map[String, String] = Map([])
  for i in 0..<20 {
    let next = if i == 19 { "h0.h" } else { "h\{i + 1}.h" }
    files["h\{i}.h"] = "// link\n#include \"\{next}\"\nint c\{i};\n"
  }
  let r = @lexer.tokenize_with_vfs("#include \"h0.h\"\nint after;\n", files~)
  let e1015 = r.errors.filter(e => @diag.ErrorCode::code(e.code) == 1015)
  assert_eq(e1015.length(), 1)
  // include-once 断链兜底：h0 只进一次，after 保留
  assert_true(r.tokens.map(t => t.text).join(" ").contains("after"))
}

///|
test "u11 include depth fuse" {
  // 原 test_u11_include_depth_fuse：嵌套 64 层上限（E1015）
  let files : Map[String, String] = Map([])
  for i in 0..<70 {
    files["d\{i}.h"] = "#include \"d\{i + 1}.h\"\nint deep\{i};\n"
  }
  let r = @lexer.tokenize_with_vfs("#include \"d0.h\"\nint tail;\n", files~)
  let e1015 = r.errors.filter(e => @diag.ErrorCode::code(e.code) == 1015)
  assert_eq(e1015.length(), 1)
}

///|
test "preprocessor undef" {
  // 原 test_preprocessor_undef
  let r = @lexer.tokenize(
    "#define FOO 1\n#undef FOO\n#ifdef FOO\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(r.tokens.map(t => t.text).join(" "), content="int b ; ")
}

///|
test "lexer define inside skipped block" {
  // 原 test_lexer_define_inside_skipped_block：跳过区内 #define 不生效
  let r = @lexer.tokenize(
    "#if 0\n#define SKIPPED 1\n#endif\n#ifdef SKIPPED\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(r.tokens.map(t => t.text).join(" "), content="int b ; ")
}

///|
test "lexer conditional with comments" {
  // 原 test_lexer_conditional_with_comments：条件区内注释不干扰指令识别
  let src =
    #|#define X 1
    #|/* c */ #ifdef X /* c2 */
    #|int a; // tail
    #|#endif
  let r = @lexer.tokenize(src)
  inspect(r.tokens.map(t => t.text).join(" "), content="int a ; ")
}

///|
test "lexer nested skip inner" {
  // 原 test_lexer_nested_skip_inner：外层活跃内层跳过
  let r = @lexer.tokenize(
    "#define OUT 1\n#ifdef OUT\nint keep;\n#ifdef IN\nint drop;\n#endif\nint keep2;\n#endif\n",
  )
  let texts = r.tokens.map(t => t.text).join(" ")
  assert_true(texts.contains("keep2"))
  assert_true(!texts.contains("drop"))
}

///|
test "lexer multiline and error positions" {
  // 原 test_lexer_multiline：跨行行号记账
  let r = @lexer.tokenize("int a;\nint b;\nchar c = 'x';\n")
  let c_tok = r.tokens.filter(t => t.ty is CharLiteral)[0]
  inspect(c_tok.loc.line, content="3")
}

///|
test "macro chain trace and branch reason wire format" {
  // 原 test_preprocessor_branch_reason：#if 分支原因（CondBranch render）
  let r = @lexer.tokenize("#if 2 > 1\nint a;\n#endif\n")
  let cond = r.trace.filter(t => t is CondBranch(..))
  assert_eq(cond.length(), 1)
  inspect(cond[0].render(), content="#if `2 > 1` → 真")
}

///|
test "continuation splice keeps non-ascii prefix" {
  // S2 审阅 P1-1 红锚：非 ASCII 行尾 `\` 的续行拼接必须保留行前缀——
  // 此前 drop_last_char 按 UTF-16 长度切 UTF-8 字节，decode 抛错兜底空串，
  // 中文注释续行把注释吞成代码（ascii/cjk 同形状必须同结果）
  let bs = "\u{5c}"
  let ascii_r = @lexer.tokenize("// abc comment \{bs}\nint a = 1;\n")
  let cjk_r = @lexer.tokenize("// 中文注释续行 \{bs}\nint a = 1;\n")
  // 拼接后均为"整行注释"（int 行并入注释）——token 计数与文本必须一致
  inspect(ascii_r.tokens.length(), content="\{cjk_r.tokens.length()}")
  assert_eq(ascii_r.tokens.length(), cjk_r.tokens.length())
  assert_eq(ascii_r.errors.length(), cjk_r.errors.length())
}

///|
test "ternary lazy both branches" {
  // S2 审阅 P1-4 红锚：C 标准只求值被选中分支——`0 ? (1/0) : 1` 必须为真
  //（此前真分支无条件求值，除零误报 E1014）
  let r = @lexer.tokenize("#if 0 ? 1/0 : 1\nint a;\n#else\nint b;\n#endif\n")
  inspect(r.tokens.map(t => t.text).join(" "), content="int a ; ")
  // 反向：1 ? 2 : (1/0)——假分支不求值
  let r2 = @lexer.tokenize("#if 1 ? 2 : 1/0\nint a;\n#else\nint b;\n#endif\n")
  inspect(r2.tokens.map(t => t.text).join(" "), content="int a ; ")
}

///|
test "cond int64 overflow rejected loudly" {
  // S2 审阅 P1-5 红锚：超 i64 值域常量必须显式 E1014（Rust 口径）——
  // 此前 UInt64 累积 + 位型重解释，`#if ULLONG_MAX > 0` 得 -1 > 0 = 假，
  // 零诊断静默错值（Clang 语义为 u64 容纳得真——差异登记 D 清单）
  let r = @lexer.tokenize("#if ULLONG_MAX > 0\nint a;\n#else\nint b;\n#endif\n")
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1014")
  // i64 值域内不受影响
  let r2 = @lexer.tokenize(
    "#if 9223372036854775807 > 0\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(r2.tokens.map(t => t.text).join(" "), content="int a ; ")
  assert_eq(r2.errors.length(), 0)
}

///|
test "linemap and real line attribution" {
  // S2 审阅 P1-6 红锚：真实文件归属必须可用——
  // 此前 spans 恒缺主文件段且 lookup 在负行号区间上全错（探针实测
  // lookup(1/2/3) 全部 (1,2)）
  let files : Map[String, String] = {
    "a.h": "int from_header_1;\nint from_header_2;\n",
  }
  let r = @lexer.tokenize_with_vfs(
    "#include \"a.h\"\nint x = A;\n",
    files~,
    base_dir=".",
  )
  assert_eq(r.errors.length(), 0)
  // ① 头文件 token 的 file_id + real_line（文件内真实行号）
  let h1 = r.tokens.filter(t => t.text == "from_header_1")[0]
  let h2 = r.tokens.filter(t => t.text == "from_header_2")[0]
  inspect(h1.loc.file_id, content="1")
  inspect(h1.real_line, content="1")
  inspect(h2.real_line, content="2")
  // ② 主文件 token 的 real_line = 自身行号
  let x = r.tokens.filter(t => t.text == "x")[0]
  inspect(x.loc.file_id, content="0")
  inspect(x.real_line, content="2")
  // ③ LineMap：主文件行号归属主文件（铺底段）
  inspect(r.line_map.lookup(2), content="(0, 2)")
}

///|
test "w1018 cross-shape redefinition silent" {
  // S2 审阅 P2：W1018 判据复刻 Rust 无分型口径——`FOO 1` 后 `FOO() 1`
  // 在 Rust 视角 params 同为空 + body 同 → 静默；MoonBit 显式分型后误报
  let r = @lexer.tokenize("#define FOO 1\n#define FOO() 1\nint x = FOO();\n")
  assert_eq(r.warnings.length(), 0)
  assert_eq(r.errors.length(), 0)
  // 真正的遮蔽（body 不同）仍报
  let r2 = @lexer.tokenize("#define FOO 1\n#define FOO 2\nint x = FOO;\n")
  assert_eq(r2.warnings.length(), 1)
}

///|
test "unterminated string two diagnostics with following line" {
  // S2 审阅 P2：未闭合字符串——行后有换行时 Rust 报两条（不能跨行 + 未闭合），
  // 行级架构此前只报 1 条（errors 计数不一致）
  let r = @lexer.tokenize("const char* s = \"abc\nint x = 1;\n")
  assert_eq(r.errors.length(), 2)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1003")
  inspect(@diag.ErrorCode::code(r.errors[1].code), content="1002")
  // 源尾行（无后继换行）只报未闭合
  let r2 = @lexer.tokenize("const char* s = \"abc")
  assert_eq(r2.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r2.errors[0].code), content="1002")
}

///|
test "crlf string value keeps carriage return" {
  // S2 审阅 P2：CRLF 源的未闭合字符串值须含 \r（Rust 不剥；此前行剥离吞掉）
  let crlf = "const char* s = \"abc\r\nint y;\n"
  let r = @lexer.tokenize(crlf)
  let s_tok = r.tokens.filter(t => t.ty is String)[0]
  inspect(s_tok.text, content="abc\r")
}
