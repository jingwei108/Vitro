///|
/// 构造器与 array_size 单点派生：dims→size 乘积（非正维按 1）。
test "type_constructors_and_array_size" {
  inspect(Type::int().to_c_string(), content="int")
  inspect(Type::void_type().to_c_string(), content="void")
  inspect(Type::pointer_to(Type::char()).to_c_string(), content="char*")
  match Type::array_of(Type::int(), [2, 3]) {
    Array(_, size, dims, _, is_vla, _) => {
      assert_eq(size, 6)
      assert_eq(dims.length(), 2)
      assert_false(is_vla)
    }
    _ => fail("array_of 形态")
  }
  match Type::array_of(Type::int(), []) {
    Array(_, size, _, _, _, _) => assert_eq(size, 0)
    _ => fail("空 dims 形态")
  }
}

///|
/// to_c_string 渲染单源（对齐 Rust Display 现状口径）：
/// unsigned 基础类型**不**显示修饰（Rust 现状，注释登记）；
/// 多维数组 / VLA / 函数指针 / struct 形态。
test "to_c_string_shapes" {
  assert_eq(Type::array_of(Type::int(), [2, 3]).to_c_string(), "int[2][3]")
  assert_eq(Type::unsigned_int().to_c_string(), "int")
  assert_eq(Type::struct_type("S").to_c_string(), "struct S")
  assert_eq(Type::union_type("U").to_c_string(), "union U")
  // Rust Display 现状：Pointer 包 Function 渲染为 "int (*)(int)*"
  //（pointee 全渲染后缀 *——怪形态照搬，不私改；裸 Function 形态才无 *）
  assert_eq(
    Type::function_pointer(Type::int(), [Type::int()]).to_c_string(),
    "int (*)(int)*",
  )
  assert_eq(
    Type::function(Type::void_type(), [Type::int(), Type::char()]).to_c_string(),
    "void (*)(int, char)",
  )
  // VLA：dims 存在即 [*] 迭代
  match Type::array_of(Type::int(), [2]) {
    Array(element, _, dims, _, _, _) => {
      let vla = Array(element, 0, dims, false, true, [])
      assert_eq(vla.to_c_string(), "int[*]")
    }
    _ => fail("vla 基座")
  }
}

///|
/// 类型判等（types_eq.mbt 两处故意"不等"的裁定锚）：
/// 1. Typeof 自反即 false；
/// 2. Array 判等忽略 vla_dims（同 dims 不同 vla_dims 表达式仍相等）。
test "type_eq_deliberate_inequalities" {
  // 自反 sanity：常规类型自反为真
  assert_true(type_eq(Type::int(), Type::int()))
  assert_false(type_eq(Type::int(), Type::unsigned_int()))
  assert_false(type_eq(Type::int(), Type::char()))
  assert_false(type_eq(Type::int(), Type::void_type()))
  // 嵌套判等
  assert_true(
    type_eq(Type::pointer_to(Type::int()), Type::pointer_to(Type::int())),
  )
  assert_false(
    type_eq(Type::pointer_to(Type::int()), Type::pointer_to(Type::char())),
  )
  // Typeof vs Typeof 恒 false——**同一表达式也判不等**
  let e1 = Literal(1, @source.SourceLoc::new(1, 1), Type::int())
  let t1 = Typeof(e1, false, false)
  let t2 = Typeof(e1, false, false)
  assert_false(type_eq(t1, t2))
  // Array 忽略 vla_dims
  let a1 = Array(Type::int(), 2, [2], false, false, [])
  let dim_expr = Literal(2, @source.SourceLoc::new(1, 1), Type::int())
  let a2 = Array(Type::int(), 2, [2], false, false, [dim_expr])
  assert_true(type_eq(a1, a2))
  // dims 不等则不等
  let a3 = Array(Type::int(), 3, [3], false, false, [])
  assert_false(type_eq(a1, a3))
}

///|
/// 模板实参判等：Type/Type 递归、Int/Int 数值、Expr 形态恒 false。
test "template_arg_eq_semantics" {
  assert_true(template_arg_eq(TArg(Type::int()), TArg(Type::int())))
  assert_true(template_arg_eq(TInt(3), TInt(3)))
  assert_false(template_arg_eq(TInt(3), TInt(4)))
  assert_false(template_arg_eq(TArg(Type::int()), TInt(3)))
  let e = Literal(1, @source.SourceLoc::new(1, 1), Type::int())
  // Expr vs Expr 恒 false（同一表达式亦然——未求值不判等）
  assert_false(template_arg_eq(TExpr(e), TExpr(e)))
}

///|
/// E5 mangle 黄金串（与 Rust ast_unit_test::test_mangle_name_into_appends_to_buffer
/// 同断言）：预置 "prefix_" 前缀 + ptr(array(int,[2,3])).mangle_name_into
/// == "prefix_p_a2_3_int"。
test "mangle_golden_prefix_p_a2_3_int" {
  let buf = StringBuilder()
  buf.write_string("prefix_")
  Type::pointer_to(Type::array_of(Type::int(), [2, 3])).mangle_name_into(buf)
  assert_eq(buf.to_string(), "prefix_p_a2_3_int")
  // 追加语义同款（非前缀形态）
  assert_eq(
    Type::pointer_to(Type::array_of(Type::int(), [2, 3])).mangle_name(),
    "p_a2_3_int",
  )
}

///|
/// mangle 形态面：函数 / struct / unsigned 变体 / 模板实参。
test "mangle_shapes" {
  assert_eq(
    Type::function(Type::int(), [Type::char(), Type::int()]).mangle_name(),
    "fn_int_char_int",
  )
  assert_eq(Type::struct_type("Node").mangle_name(), "struct_Node")
  assert_eq(Type::unsigned_int().mangle_name(), "unsigned_int")
  assert_eq(
    Type::pointer_to(Type::unsigned_int()).mangle_name(),
    "p_unsigned_int",
  )
  let buf = StringBuilder()
  TInt(42).mangle_name_into(buf)
  assert_eq(buf.to_string(), "42")
}

///|
/// 深度语义（depth.rs 契约）：叶子 1；左结合链 n 项深度 n；
/// Block 语句序列不增深；嵌套 If 增深；Typeof 等深 Type 独立预算。
test "depth_semantics" {
  let loc = @source.SourceLoc::new(1, 1)
  // 叶子
  let leaf = Literal(1, loc, Type::int())
  assert_eq(expr_depth(leaf), 1)
  // 左结合链 a+a+a：Binary(Binary(a,a),a) 深 3
  let two = Binary(Add, leaf, leaf, loc, Type::int())
  let three = Binary(Add, two, leaf, loc, Type::int())
  assert_eq(expr_depth(three), 3)
  // Block(1)→Expr壳(2)→three(3)→two(4)→leaf(5)：包装壳逐层 +1（Rust 语义）
  let s = Expr(three, loc)
  let blk = Block([s, s, s], loc)
  assert_eq(stmt_depth(blk), 5)
  // 序列不增深：三条同深语句的 Block 不比单条深
  let single = Block([s], loc)
  assert_eq(stmt_depth(single), 5)
  // If：1 + max(cond=1, then=Block[Expr(leaf)]=3) = 4
  let inner = Block([Expr(leaf, loc)], loc) // 深 3
  let if1 = If(leaf, inner, None, loc)
  assert_eq(stmt_depth(if1), 4)
  // 一元链深 n+叶
  let neg = Unary(Neg, leaf, loc, Type::int())
  assert_eq(expr_depth(neg), 2)
}

///|
/// 类型深度（P1 语义）：`int[1][1]` 深度 = Array(1) + Int(2) = 2；
/// 指针链 n 层深 n+1；病态后缀链对 stmt 深度隐身（VarDecl 深度小而
/// type_depth 大）——两预算解耦的实证锚。
test "type_depth_and_budget_decoupling" {
  assert_eq(type_depth(Type::int()), 1)
  assert_eq(type_depth(Type::array_of(Type::int(), [1, 1])), 2)
  assert_eq(type_depth(Type::pointer_to(Type::pointer_to(Type::int()))), 3)
  // VarDecl{array×3} 语句深 1（无 init），类型深 4
  let loc = @source.SourceLoc::new(1, 1)
  let deep_ty = Array(Type::array_of(Type::int(), [1]), 1, [1], false, false, [])
  let vd = VarDecl(deep_ty, "a", None, [], false, loc)
  assert_eq(stmt_depth(vd), 1)
  // deep_ty = Array(array_of(int,[1])) 双层嵌套：类型深 3、语句深 1（解耦锚）
  assert_eq(stmt_type_depth(vd), 3)
}

///|
/// Expr::loc / Expr::ty 全变体访问器（单点等价 Rust expr_field! 宏）。
test "expr_accessors" {
  let loc = @source.SourceLoc::new(3, 7)
  let e = Binary(
    Add,
    Literal(1, loc, Type::int()),
    Literal(2, loc, Type::int()),
    loc,
    Type::int(),
  )
  assert_eq(e.loc().line, 3)
  assert_eq(e.loc().column, 7)
  assert_true(type_eq(e.ty(), Type::int()))
  let s = StringLiteral("hi", loc, Type::pointer_to(Type::char()))
  assert_true(type_eq(s.ty(), Type::pointer_to(Type::char())))
}
