///|
test "tokenize basic define and expansion" {
  let r = @lexer.tokenize(
    "#define MAX(a, b) ((a) > (b) ? (a) : (b))\nint x = MAX(1, 2);\n",
  )
  let summary = r.tokens
    .map(t => "\{t.ty.to_name()}:\{t.text}@\{t.loc.line}:\{t.loc.column}")
    .join(" ")
  inspect(
    summary,
    content=(
      #|Int:int@2:1 Identifier:x@2:5 Assign:=@2:7 LParen:(@2:9 LParen:(@2:9 Number:1@2:9 RParen:)@2:9 Gt:>@2:9 LParen:(@2:9 Number:2@2:9 RParen:)@2:9 Question:?@2:9 LParen:(@2:9 Number:1@2:9 RParen:)@2:9 Colon::@2:9 LParen:(@2:9 Number:2@2:9 RParen:)@2:9 RParen:)@2:9 Semicolon:;@2:18 Eof:@3:1
    ),
  )
  assert_eq(r.errors.length(), 0)
}

///|
test "tokenize conditional and builtin macro" {
  let r = @lexer.tokenize(
    "#ifdef __STDC_VERSION__\nint a;\n#else\nint b;\n#endif\nint c = true;\n",
  )
  let summary = r.tokens.map(t => "\{t.ty.to_name()}:\{t.text}").join(" ")
  inspect(
    summary,
    content=(
      #|Int:int Identifier:a Semicolon:; Int:int Identifier:c Assign:= Number:1 Semicolon:; Eof:
    ),
  )
  assert_eq(r.errors.length(), 0)
  assert_eq(r.warnings.length(), 0)
}

///|
test "tokenize if arithmetic and skip" {
  let r = @lexer.tokenize(
    "#if 2 + 3 * 4 == 14\nint ok;\n#else\nint bad;\n#endif\n",
  )
  let summary = r.tokens.map(t => t.text).join(" ")
  inspect(
    summary,
    content=(
      #|int ok ; 
    ),
  )
}

///|
test "tokenize include stub and once" {
  let r = @lexer.tokenize(
    "#include <stdio.h>\n#include <stdio.h>\nint main() { printf(\"hi\"); return 0; }\n",
  )
  // 存根展开：stdio.h 内容 token + 主文件 token；include-once 使第二次跳过
  let texts = r.tokens.map(t => t.text)
  assert_eq(r.errors.length(), 0)
  // printf 声明来自存根（typedef/声明 token 化后进入流）
  assert_true(texts.contains("printf"))
  assert_eq(r.warnings.length(), 0)
}

///|
test "tokenize object macro chain and self reference" {
  let r = @lexer.tokenize("#define A B\n#define B A\nint x = A;\n")
  // 自引用停止（蓝漆）：A→B→A 停，产出 Identifier A
  let tail = r.tokens[r.tokens.length() - 3]
  inspect(tail.ty.to_name(), content="Identifier")
  inspect(tail.text, content="A")
  assert_eq(r.errors.length(), 0)
}

///|
test "tokenize unknown directive diagnosed" {
  // 修复 ⑥-11：未知指令显式诊断（Rust 静默跳过）
  let r = @lexer.tokenize("#foobar 1\nint x;\n")
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1006")
}

///|
test "tokenize depth fuse on object macro chain" {
  // U1#6 红锚：5000 层不同名链必须熔断（E1017）而非栈溢出
  let buf = StringBuilder()
  for i in 0..<5000 {
    buf <+ "#define A\{i} A\{i + 1}\n"
  }
  buf <+ "int x = A0;\n"
  let r = @lexer.tokenize(buf.to_string())
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1017")
}

///|
test "tokenize elifdef c23" {
  // 修复 ⑥-11：#elifdef 正确选分支（Rust 静默跳过导致选错）
  let r = @lexer.tokenize("#define FOO 1\n#elifdef_QUIRK\n")
  ignore(r)
}

///|
test "tokenize elifdef real" {
  let r = @lexer.tokenize(
    "#define FEATURE 1\n#ifdef BASE\nint a;\n#elifdef FEATURE\nint b;\n#else\nint c;\n#endif\n",
  )
  let summary = r.tokens.map(t => t.text).join(" ")
  inspect(
    summary,
    content=(
      #|int b ; 
    ),
  )
}

///|
test "tokenize multiline macro continuation" {
  // 修复 ⑥-4：`\` 续行（Rust 报 3 条无法识别的字符 '\'）
  let r = @lexer.tokenize(
    "#define SWAP(a, b) do { \\\n  int t = a; a = b; b = t; \\\n} while (0)\nSWAP(x, y);\n",
  )
  assert_eq(r.errors.length(), 0)
  // H01：大括号块宏体 + 调用后分号 → do{...}while(0) 包装
  let names = r.tokens.map(t => t.ty.to_name()).join(" ")
  assert_true(names.contains("Do"))
  assert_true(names.contains("While"))
}

///|
test "tokenize zero-param function macro" {
  // 修复 ⑥-9：#define FOO() 42 是函数式宏（Rust 误当对象宏导致 E3066 错位）
  let r = @lexer.tokenize("#define FOO() 42\nint x = FOO();\n")
  assert_eq(r.errors.length(), 0)
  let summary = r.tokens.map(t => t.text).join(" ")
  inspect(
    summary,
    content=(
      #|int x = 42 ; 
    ),
  )
}

///|
test "tokenize macro arity mismatch diagnosed" {
  // 修复 ⑥-10：参数个数不匹配显式诊断（Rust 静默保留 → E3023 错位）
  let r = @lexer.tokenize(
    "#define MAX(a, b) ((a)>(b)?(a):(b))\nint x = MAX(1);\n",
  )
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1005")
  // fail-soft：L2 token 流保留原样（与 Rust 一致——只有诊断不同）
  let texts = r.tokens.map(t => t.text).join(" ")
  inspect(
    texts,
    content=(
      #|int x = MAX ( 1 ) ; 
    ),
  )
}

///|
test "tokenize if bitwise and ternary full evaluator" {
  // 修复 ⑥-6：位运算/三目（Rust 报 E1014）
  let r1 = @lexer.tokenize("#if (6 & 3) == 2\nint a;\n#else\nint b;\n#endif\n")
  inspect(
    r1.tokens.map(t => t.text).join(" "),
    content=(
      #|int a ; 
    ),
  )
  let r2 = @lexer.tokenize(
    "#if (1 ? 2 : 3) == 2\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(
    r2.tokens.map(t => t.text).join(" "),
    content=(
      #|int a ; 
    ),
  )
  assert_eq(r2.errors.length(), 0)
}

///|
test "tokenize or normalization" {
  // 修复 ⑥-3：|| 短路归一 0/1（Rust `(2 || 0) == 1` 为假选错分支）
  let r = @lexer.tokenize("#if (2 || 0) == 1\nint a;\n#else\nint b;\n#endif\n")
  inspect(
    r.tokens.map(t => t.text).join(" "),
    content=(
      #|int a ; 
    ),
  )
  assert_eq(r.errors.length(), 0)
}

///|
test "tokenize shadowing warning W1018" {
  let r = @lexer.tokenize("#define M 1\n#define M 2\nint x = M;\n")
  assert_eq(r.warnings.length(), 1)
  inspect(@diag.ErrorCode::code(r.warnings[0].code), content="1018")
  inspect(
    r.tokens.map(t => t.text).join(" "),
    content=(
      #|int x = 2 ; 
    ),
  )
}

///|
test "tokenize side effect warning W1019" {
  let r = @lexer.tokenize("#define SQ(x) ((x)*(x))\nint y = SQ(i++);\n")
  assert_eq(r.warnings.length(), 1)
  inspect(@diag.ErrorCode::code(r.warnings[0].code), content="1019")
}

///|
test "tokenize trace render wire format" {
  let r = @lexer.tokenize("#define SQ(x) ((x)*(x))\nint y = SQ(3);\n")
  assert_eq(r.trace.length(), 1)
  inspect(r.trace[0].render(), content="SQ(3) ⇒ ( ( 3 ) * ( 3 ) )")
}

///|
test "tokenize stringize and paste" {
  let r = @lexer.tokenize("#define NAME(x) #x\nconst char* s = NAME(hello);\n")
  inspect(
    r.tokens.map(t => t.text).join(" "),
    content=(
      #|const char * s = hello ; 
    ),
  )
  let r2 = @lexer.tokenize("#define CAT(a, b) a##b\nint CAT(xy, zw) = 1;\n")
  let texts = r2.tokens.map(t => t.text).join(" ")
  assert_true(texts.contains("xyzw"))
}

///|
test "tokenize has include" {
  let r = @lexer.tokenize(
    "#if __has_include(<stdio.h>)\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(
    r.tokens.map(t => t.text).join(" "),
    content=(
      #|int a ; 
    ),
  )
  let r2 = @lexer.tokenize(
    "#if __has_include(<no_such.h>)\nint a;\n#else\nint b;\n#endif\n",
  )
  inspect(
    r2.tokens.map(t => t.text).join(" "),
    content=(
      #|int b ; 
    ),
  )
}

///|
test "tokenize unmatched endif and unclosed if" {
  let r = @lexer.tokenize("int x;\n#endif\n")
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1011")
  // 修复 ⑥-23：E1013 报在开启行（此处第 1 行）而非 EOF 行
  let r2 = @lexer.tokenize("#ifdef FOO\nint a;\nint b;\nint c;\n")
  assert_eq(r2.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r2.errors[0].code), content="1013")
  inspect(r2.errors[0].loc.line, content="1")
}

///|
test "tokenize custom include with vfs" {
  // VfsProvider 注入：quote include 走候选链（当前目录 "." → base_dir "src"）
  let files : Map[String, String] = { "src/myhdr.h": "int from_header;\n" }
  let provider = @host.vfs_provider(files~)
  let r = @lexer.tokenize_with(
    "#include \"myhdr.h\"\nint main;\n",
    provider~,
    base_dir="src",
  )
  assert_eq(r.errors.length(), 0)
  let texts = r.tokens.map(t => t.text).join(" ")
  assert_true(texts.contains("from_header"))
}

///|
test "tokenize stubs only provider rejects custom include explicitly" {
  // H-5 修复语义：默认 provider 下自定义 include 显式诊断（Rust 在 wasm 静默失效）
  let r = @lexer.tokenize("#include \"local.h\"\nint x;\n")
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1021")
}
