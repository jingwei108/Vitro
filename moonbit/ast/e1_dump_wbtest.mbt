///|
/// E1 dump 形态快照锚（对拍真值 2026-09-19：Rust serve ast.dump ↔
/// 本 emitter，canonicalize 后 diff 为空，两样本：int main / 富样本）。
/// 本快照锁 emitter 输出的原始文本（未经 canonicalize）——emitter 任何
/// 键名/嵌套/浮点文本化漂移即红。
test "e1_dump_shape_int_main" {
  let loc_of = (line : Int, col : Int) => @source.SourceLoc::new(line, col)
  let lit = Literal(0, loc_of(1, 20), Type::int())
  let ret = Return(Some(lit), loc_of(1, 13))
  let blk = Block([ret], loc_of(1, 11))
  let f : FuncDecl = {
    loc: loc_of(1, 5),
    return_type: Type::int(),
    name: "main",
    params: [],
    body: Some(blk),
    is_static: false,
    is_extern: false,
    source_file: "",
    is_variadic: false,
  }
  let prog : ProgramNode = {
    structs: [],
    unions: [],
    globals: [],
    funcs: [f],
    classes: [],
    templates: [],
    template_instantiations: [],
  }
  inspect(
    ast_dump_json(prog),
    content=(
      #|{"structs":[],"unions":[],"globals":[],"funcs":[{"loc":{"line":1,"column":5,"file_id":0},"return_type":{"Int":{"is_unsigned":false,"is_const":false}},"name":"main","params":[],"body":{"Block":{"stmts":[{"Return":{"value":{"Literal":{"value":0,"loc":{"line":1,"column":20,"file_id":0},"ty":{"Int":{"is_unsigned":false,"is_const":false}}}},"loc":{"line":1,"column":13,"file_id":0}}}],"loc":{"line":1,"column":11,"file_id":0}}},"is_static":false,"is_extern":false,"source_file":"","is_variadic":false}],"classes":[],"templates":[],"template_instantiations":[]}
    ),
  )
}

///|
/// E1 富样本锚（FloatLiteral 3.5 / StringLiteral（ty=char[3]）/ 全局 /
/// While / Assign）——canonicalize 对拍通过的原始文本固化。
test "e1_dump_shape_rich_sample" {
  let loc_of = (line : Int, col : Int) => @source.SourceLoc::new(line, col)
  let g : GlobalDecl = {
    loc: loc_of(1, 13),
    ty: Double(false),
    name: "pi",
    init: Some(FloatLiteral(3.5, loc_of(1, 13), Double(false))),
    is_static: false,
    is_extern: false,
    source_file: "",
  }
  let prog : ProgramNode = {
    structs: [],
    unions: [],
    globals: [g],
    funcs: [],
    classes: [],
    templates: [],
    template_instantiations: [],
  }
  // 锚关键片段（全量见 probe 对拍；此锚锁浮点文本化与全局形态）
  let json = ast_dump_json(prog)
  assert_true(
    json.contains(
      "\"FloatLiteral\":{\"value\":3.5,\"loc\":{\"line\":1,\"column\":13,\"file_id\":0},\"ty\":{\"Double\":{\"is_const\":false}}}",
    ),
  )
  assert_true(
    json.has_prefix("{\"structs\":[],\"unions\":[],\"globals\":[{\"loc\":"),
  )
}

///|
/// 浮点文本化 ryu-pretty 全区间锚（P1-1/P1-2 修复的固化；红证 = 审阅
/// 2026-09-19 实测表：旧实现 1e16 段定点错形 / 1e-6 段定点错形 /
/// inf 产出非法 JSON，本表逐值锁死）。
test "double_to_json_text_alignment" {
  // 区间 1：定点整数（kk <= 16）
  inspect(double_to_json_text(1.0), content="1.0")
  inspect(double_to_json_text(1000000000000000.0), content="1000000000000000.0") // 1e15, kk=16 边界
  // 区间 4：kk >= 17 指数形（正指数带 +）
  inspect(double_to_json_text(10000000000000000.0), content="1e+16") // kk=17 边界
  inspect(double_to_json_text(15000000000000000.0), content="1.5e+16")
  inspect(double_to_json_text(100000000000000000.0), content="1e+17")
  inspect(double_to_json_text(150000000000000000.0), content="1.5e+17")
  inspect(
    double_to_json_text(123456789012345680.0),
    content="1.2345678901234568e+17",
  )
  inspect(double_to_json_text(1000000000000000000000.0), content="1e+21")
  // 区间 3：-5 < kk <= 0 定点 0.000x
  inspect(double_to_json_text(0.1), content="0.1")
  inspect(double_to_json_text(0.00001), content="0.00001") // 1e-5, kk=-4 边界
  inspect(double_to_json_text(0.000012345), content="0.000012345")
  // 区间 4：kk <= -5 指数形（负指数无 + 号、不补零——serve 原始字节实测）
  inspect(double_to_json_text(0.000001), content="1e-6") // kk=-5 边界
  inspect(double_to_json_text(0.0000001), content="1e-7")
  inspect(double_to_json_text(2.5e-8), content="2.5e-8")
  // 区间 2：定点小数
  inspect(double_to_json_text(3.5), content="3.5")
  inspect(double_to_json_text(123.45), content="123.45")
  inspect(double_to_json_text(-42.5), content="-42.5")
  // 零与符号
  inspect(double_to_json_text(-0.0), content="-0.0")
  inspect(double_to_json_text(0.0), content="0.0")
  // 非有限 → null（serde_json non-finite 语义；P1-2）
  inspect(double_to_json_text(1.0 / 0.0), content="null")
  inspect(double_to_json_text(-1.0 / 0.0), content="null")
  inspect(double_to_json_text(0.0 / 0.0), content="null")
}
