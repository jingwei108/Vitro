///|
/// E1 emitter 全变体覆盖锚（审阅 P2-2：66 变体中此前仅十几个被测试
/// 执行；本测试构造含全部 Type 17 / Expr 26 / Stmt 16 / decl 族成员的
/// 程序，逐变体断言输出含其特征键——零覆盖分支清零）。
test "e1_dump_all_variants_covered" {
  let loc = @source.SourceLoc::new(1, 1)
  // ---- Type 17 全变体（经 Cast 目标 / 字段类型 / 泛型关联等挂入）----
  let t_all : Array[Type] = [
    Void(true),
    Int(true, true),
    Char(true, true),
    Float(true),
    Double(true),
    LongLong(true, true),
    Pointer(Int(false, false), true),
    Array(Char(false, false), 2, [2], true, true, [
      Literal(1, loc, Int(false, false)),
    ]),
    Function(Void(false), [Int(false, false)], true, true),
    Struct("S", true),
    Union("U", true),
    Class("C", true),
    Reference(Int(false, false), true),
    RValueRef(Int(false, false)),
    Auto,
    TemplateId(
      "V",
      [
        TArg(Int(false, false)),
        TInt(3),
        TExpr(Literal(1, loc, Int(false, false))),
      ],
      true,
    ),
    Typeof(Literal(1, loc, Int(false, false)), true, true),
  ]
  // ---- Expr 26 全变体 ----
  let i_ = Literal(1, loc, Int(false, false))
  let e_all : Array[Expr] = [
    Binary(Add, i_, i_, loc, Int(false, false)),
    Unary(Neg, i_, loc, Int(false, false)),
    i_,
    FloatLiteral(1.5, loc, Double(false)),
    LongLiteral(2L, loc, LongLong(false, false)),
    StringLiteral("s", loc, Pointer(Char(false, false), false)),
    Identifier("x", loc, Int(false, false)),
    Call("f", [i_], loc, Int(false, false)),
    CallPtr(
      Identifier("fp", loc, Int(false, false)),
      [i_],
      loc,
      Int(false, false),
    ),
    Index(i_, i_, loc, Int(false, false)),
    Member(i_, "m", loc, Int(false, false)),
    Expr::Assign(AssignOp::Assign, i_, i_, loc, Int(false, false)),
    Ternary(i_, i_, i_, loc, Int(false, false)),
    Sizeof(Some(Int(false, false)), None, loc, Int(false, false)),
    Sizeof(None, Some(i_), loc, Int(false, false)),
    Alignof(Some(Int(false, false)), None, loc, Int(false, false)),
    Cast(i_, Double(false), loc, Double(false)),
    InitList(
      [{ designators: [Field("f"), Index(i_)], value: i_, }],
      loc,
      Int(false, false),
    ),
    Offsetof(Struct("S", false), "f", loc, Int(false, false)),
    Generic(i_, [(Int(false, false), i_)], Some(i_), loc, Int(false, false)),
    CompoundLiteral(Int(false, false), i_, loc, Int(false, false)),
    This(loc, Int(false, false)),
    MemberCall(i_, "run", [i_], true, Some("mangled_x"), loc, Int(false, false)),
    New(
      Int(false, false),
      Some(i_),
      Some(i_),
      loc,
      Pointer(Int(false, false), false),
    ),
    Delete(i_, true, loc, Void(false)),
    Lambda(
      [ByValue("x"), ByReference("y"), Implicit],
      [],
      Block([], loc),
      7U,
      loc,
      Int(false, false),
    ),
    Move(i_, loc, Int(false, false)),
  ]
  // ---- Stmt 16 全变体 ----
  let s_all : Array[Stmt] = [
    Block([], loc),
    VarDecl(
      Int(false, false),
      "v",
      Some(i_),
      [{ var_type: Int(false, false), name: "w", init: Some(i_), }],
      true,
      loc,
    ),
    Expr(i_, loc),
    If(i_, Block([], loc), Some(Block([], loc)), loc),
    While(i_, Block([], loc), loc),
    DoWhile(Block([], loc), i_, loc),
    For(Some(Expr(i_, loc)), Some(i_), [i_], Block([], loc), loc),
    Return(Some(i_), loc),
    Break(loc),
    Continue(loc),
    Switch(i_, Block([], loc), loc),
    Case(Some(i_), Block([], loc), loc),
    Goto("lbl", loc),
    Label("lbl", Block([], loc), loc),
    RangeFor("x", Int(false, false), i_, Block([], loc), loc),
    Try(
      Block([], loc),
      [
        {
          param_type: Int(false, false),
          param_name: Some("e"),
          body: Block([], loc),
          loc,
        },
      ],
      loc,
    ),
  ]
  // ---- decl 族（C++ 类 / 模板 / 指派符等）----
  let param : Param = {
    ty: Int(false, false),
    name: "p",
    loc,
    default: Some(i_),
  }
  let class_decl : ClassDecl = {
    loc,
    name: "K",
    base: Some("B"),
    members: [
      Field("f", Int(false, false), Public, true),
      Method(
        "m",
        Int(false, false),
        [param],
        Some(Block([], loc)),
        true,
        Private,
        true,
        true,
      ),
      Constructor([param], Some(Block([], loc)), true, Protected, true),
      Destructor(Some(Block([], loc)), Public, true),
      NestedStruct(
        { loc, name: "N", fields: [{ ty: Int(false, false), name: "nf", }], },
        Public,
      ),
      NestedClass(
        {
          loc,
          name: "NC",
          base: None,
          members: [],
          vtable: Some({ entries: [("m", Int(false, false))], }),
        },
        Public,
      ),
    ],
    vtable: Some({ entries: [("m", Int(false, false))], }),
  }
  let tmpl_decl : TemplateDecl = {
    loc,
    params: [TypeParam("T", loc), NonType("N", Int(false, false), loc)],
    decl: TFunc({
      loc,
      return_type: Int(false, false),
      name: "tf",
      params: [param],
      body: Some(Block([], loc)),
      is_static: false,
      is_extern: false,
      source_file: "",
      is_variadic: false,
    }),
  }
  let tmpl_class : TemplateDecl = {
    loc,
    params: [TypeParam("T", loc)],
    decl: TClass(class_decl),
  }
  // 组装：类型经 Cast 挂 Expr；Expr/Stmt 挂函数体；decl 族挂根集合
  let stmts : Array[Stmt] = []
  for t in t_all {
    stmts.push(Expr(Cast(i_, t, loc, Int(false, false)), loc))
  }
  for e in e_all {
    stmts.push(Expr(e, loc))
  }
  for s in s_all {
    stmts.push(s)
  }
  let f : FuncDecl = {
    loc,
    return_type: Int(false, false),
    name: "all",
    params: [param],
    body: Some(Block(stmts, loc)),
    is_static: false,
    is_extern: false,
    source_file: "",
    is_variadic: true,
  }
  let prog : ProgramNode = {
    structs: [
      { loc, name: "S0", fields: [{ ty: Int(false, false), name: "a", }], },
    ],
    unions: [{ loc, name: "U0", fields: [], }],
    globals: [
      {
        loc,
        ty: Int(false, false),
        name: "g",
        init: Some(i_),
        is_static: false,
        is_extern: true,
        source_file: "f.c",
      },
    ],
    funcs: [f],
    classes: [class_decl],
    templates: [tmpl_decl, tmpl_class],
    template_instantiations: [
      { loc, base: "tf", args: [TArg(Int(false, false)), TInt(1)], },
    ],
  }
  let json = ast_dump_json(prog)
  // ---- Type 17 键 ----
  for
    key in [
      "\"Void\"", "\"Int\"", "\"Char\"", "\"Float\"", "\"Double\"", "\"LongLong\"",
      "\"Pointer\"", "\"Array\"", "\"Function\"", "\"Struct\"", "\"Union\"", "\"Class\"",
      "\"Reference\"", "\"RValueRef\"", "\"Auto\"", "\"TemplateId\"", "\"Typeof\"",
    ] {
    guard json.contains(key) else { fail("Type 键缺失 \{key}") }
  }
  // Auto 无字段变体 → 裸串 "Auto"
  assert_true(json.contains("\"target_type\":\"Auto\""))
  // ---- Expr 26 键 ----
  for
    key in [
      "\"Binary\"", "\"Unary\"", "\"Literal\"", "\"FloatLiteral\"", "\"LongLiteral\"",
      "\"StringLiteral\"", "\"Identifier\"", "\"Call\"", "\"CallPtr\"", "\"Index\"",
      "\"Member\"", "\"Assign\"", "\"Ternary\"", "\"Sizeof\"", "\"Alignof\"", "\"Cast\"",
      "\"InitList\"", "\"Offsetof\"", "\"Generic\"", "\"CompoundLiteral\"", "\"This\"",
      "\"MemberCall\"", "\"New\"", "\"Delete\"", "\"Lambda\"", "\"Move\"",
    ] {
    guard json.contains(key) else { fail("Expr 键缺失 \{key}") }
  }
  // ---- Stmt 16 键 ----
  for
    key in [
      "\"Block\"", "\"VarDecl\"", "\"Expr\"", "\"If\"", "\"While\"", "\"DoWhile\"",
      "\"For\"", "\"Return\"", "\"Break\"", "\"Continue\"", "\"Switch\"", "\"Case\"",
      "\"Goto\"", "\"Label\"", "\"RangeFor\"", "\"Try\"",
    ] {
    guard json.contains(key) else { fail("Stmt 键缺失 \{key}") }
  }
  // ---- 隐藏映射锚（MoonBit 名 ≠ Rust 名、只在 emitter 翻译——审阅 P2-2）----
  guard json.contains("{\"Type\":{\"name\":\"T\"") else {
    fail("TypeParam 映射")
  }
  guard json.contains("{\"NonType\":{\"name\":\"N\"") else {
    fail("NonType 映射")
  }
  guard json.contains("{\"Type\":") else { fail("TArg 映射") }
  guard json.contains("{\"Expr\":") else { fail("TExpr 映射") }
  guard json.contains("{\"Int\":3}") else { fail("TInt 映射") }
  guard json.contains("{\"Func\":") else { fail("TFunc 映射") }
  guard json.contains("{\"Class\":{\"loc\"") else { fail("TClass 映射") }
  // ---- decl 族特征键 ----
  for
    key in [
      "\"resolved_mangled\":\"mangled_x\"", "\"unique_id\":7", "\"designators\"",
      "\"Field\":\"f\"", "\"Index\":", "\"associations\"", "\"default\":", "\"vtable\"",
      "\"entries\"", "\"NestedStruct\"", "\"NestedClass\"", "\"Constructor\"", "\"Destructor\"",
      "\"Method\"", "\"is_explicit\"", "\"template_instantiations\"", "\"base\":\"B\"",
      "\"catches\"", "\"param_name\":\"e\"", "\"is_variadic\":true", "\"source_file\":\"f.c\"",
    ] {
    guard json.contains(key) else { fail("decl 特征缺失 \{key}") }
  }
}
