///|
test "scan_raw basic tokens" {
  let r = scan_raw(
    @utf8.encode(
      "int x = 42; // note\n/* blk */ if (x >= 'a') printf(\"hi\\n\");",
    ),
  )
  let summary = r.tokens
    .map(t => {
      "\{t.ty.to_name()}:\{t.text}@\{t.loc.line}:\{t.loc.column}+\{t.byte_off}"
    })
    .join(" | ")
  inspect(
    summary,
    content=(
      #|Int:int@1:1+0 | Identifier:x@1:5+4 | Assign:=@1:7+6 | Number:42@1:9+8 | Semicolon:;@1:11+10 | If:if@2:11+30 | LParen:(@2:14+33 | Identifier:x@2:15+34 | Ge:>=@2:17+36 | CharLiteral:97@2:20+39 | RParen:)@2:23+42 | Identifier:printf@2:25+44 | LParen:(@2:31+50 | String:hi
      #|@2:32+51 | RParen:)@2:38+57 | Semicolon:;@2:39+58 | Eof:@2:40+59
    ),
  )
  assert_eq(r.errors.length(), 0)
}

///|
test "scan_raw hex float suffix dispatch" {
  let r = scan_raw(
    @utf8.encode("0xFF 3.14f .5 1e5 0b1010 1'000 42ULL 8000000000LL"),
  )
  let summary = r.tokens.map(t => "\{t.ty.to_name()}:\{t.text}").join(" ")
  inspect(
    summary,
    content=(
      #|Number:255 FloatLiteral:3.14f FloatLiteral:.5 FloatLiteral:1e5 Number:10 Number:1000 UnsignedLiteral:42 LongLiteral:8000000000 Eof:
    ),
  )
  assert_eq(r.errors.length(), 0)
}

///|
test "scan_raw unknown char collects error" {
  let r = scan_raw(@utf8.encode("int $x;"))
  assert_eq(r.errors.length(), 1)
  inspect(@diag.ErrorCode::code(r.errors[0].code), content="1001")
  inspect(r.tokens[1].ty.to_name(), content="Unknown")
}

///|
test "scan_raw cpp mode and hash" {
  let r = scan_raw(@utf8.encode("a ## b # c"), cpp_mode=false)
  let summary = r.tokens.map(t => t.ty.to_name()).join(" ")
  inspect(summary, content="Identifier HashHash Identifier Hash Identifier Eof")
}
