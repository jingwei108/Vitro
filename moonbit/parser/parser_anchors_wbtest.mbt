///|
/// S3 白盒锚（形态改造面的单点验证——黑盒对拍之外的构造性证据）：
/// 1. 深度参数语义：壳挂点的计数口径与 Rust 共享计数器同构
/// 2. 回滚全量快照：A5 第 4 类缺口的结构性消除
///（wbtest 不携带 lexer 依赖——token 数组手工构造）

///|
/// 手工 token（裸最小面）。
fn tok(ty : @token.TokenType, text : String, line : Int, col : Int) -> @token.Token {
  { ty, text, loc: { line, column: col, file_id: 0 }, byte_off: -1, real_line: -1 }
}

fn num_tokens(n : Int) -> Array[@token.Token] {
  let toks = []
  for i in 0..<n {
    toks.push(tok(Number, i.to_string(), 1, 1 + i * 2))
  }
  toks.push(tok(Eof, "", 1, 1 + n * 2))
  toks
}

///|
/// 深度壳检查边界：depth = MAX 时立即拒绝并跳尾（Rust enter_depth 同构）。
test "wb depth shell rejects at max" {
  let q = Parser::with_mode(num_tokens(4), false)
  let before = q.errors.length()
  ignore(q.parse_assign(MAX_PARSE_DEPTH))
  inspect(q.errors.length() - before, content="1")
  inspect(q.errors[before].code, content="1006")
  // 跳尾副作用：pos = len-1
  inspect(q.pos, content="4")
}

///|
/// depth = MAX-1 通过检查，body 内下一层壳（收 MAX）拒绝。
test "wb depth shell boundary off by one" {
  let q = Parser::with_mode(num_tokens(4), false)
  ignore(q.parse_assign(MAX_PARSE_DEPTH - 1))
  assert_true(q.errors.length() > 0)
  inspect(q.errors[0].code, content="1006")
}

///|
/// 回滚快照含 typedef_names（A5 结构性消除验证）。
test "wb rollback covers typedef table" {
  let q = Parser::with_mode(num_tokens(1), false)
  let rb = q.save()
  q.typedef_names.set("ProbeT", @ast.Type::int())
  inspect(q.typedef_names.contains("ProbeT"), content="true")
  q.restore(rb)
  inspect(q.typedef_names.contains("ProbeT"), content="false")
}

///|
/// 回滚快照含 current_class / next_lambda_id / errors / anonymous_structs。
test "wb rollback covers class stack and lambda id" {
  let q = Parser::with_mode(num_tokens(1), false)
  let rb = q.save()
  q.current_class.push("Outer")
  q.next_lambda_id = 7
  q.errors.push({ message: "x", line: 1, column: 1, code: 1 })
  q.anonymous_structs.push({ loc: @source.SourceLoc::new(1, 1), name: "S", fields: [] })
  q.restore(rb)
  inspect(q.current_class.length(), content="0")
  inspect(q.next_lambda_id, content="0")
  inspect(q.errors.length(), content="0")
  inspect(q.anonymous_structs.length(), content="0")
}

///|
/// FILE 预定义登记（Rust with_mode 同款）。
test "wb predefined FILE typedef" {
  let q = Parser::with_mode(num_tokens(1), false)
  inspect(q.typedef_names.contains("FILE"), content="true")
}
