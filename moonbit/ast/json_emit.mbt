///|
/// E1 锚出口：AST → JSON（与 Rust serde externally-tagged 形态同构）。
///
/// 显式 emitter 纪律（B 级锚 / P7 落注释）：禁 ToJson 派生直拼——
/// 键名/嵌套形态/浮点文本化在此单点对齐 Rust serde：
/// - 变体 → `{"Variant":{字段...}}`；无字段变体 → `"Variant"` 裸串；
/// - Option → `null` / 值；Box 透明；元组 → 数组；
/// - 字符串 JSON 转义（" \ \n \r \t）；
/// - **浮点文本化对齐 serde_json/ryu**（F9 登记差异的根治点）：
///   整值小数补 `.0`（1 → `1.0`）、`-0.0` 保符号——MoonBit `to_string`
///   分别输出 `1` / `0`（实测），不经本函数禁入 dump。
///
/// 键序与本包字段声明序一致；canonicalize 归一后与 serde 侧对齐
/// （键排序 / 数字保形 / 转义统一由 Go canonicalizer 单点保证）。
pub fn ast_dump_json(program : ProgramNode) -> String {
  let buf = StringBuilder()
  emit_program(program, buf)
  buf.to_string()
}

///|
fn emit_program(p : ProgramNode, buf : StringBuilder) -> Unit {
  buf.write_string("{\"structs\":")
  emit_array(p.structs, buf, (d, b) => emit_struct_decl(d, b))
  buf.write_string(",\"unions\":")
  emit_array(p.unions, buf, (d, b) => emit_struct_decl(d, b))
  buf.write_string(",\"globals\":")
  emit_array(p.globals, buf, (d, b) => emit_global_decl(d, b))
  buf.write_string(",\"funcs\":")
  emit_array(p.funcs, buf, (f, b) => emit_func_decl(f, b))
  buf.write_string(",\"classes\":")
  emit_array(p.classes, buf, (c, b) => emit_class_decl(c, b))
  buf.write_string(",\"templates\":")
  emit_array(p.templates, buf, (t, b) => emit_template_decl(t, b))
  buf.write_string(",\"template_instantiations\":")
  emit_array(p.template_instantiations, buf, (t, b) => emit_template_inst(t, b))
  buf.write_string("}")
}

///|
fn[T] emit_array(
  xs : Array[T],
  buf : StringBuilder,
  f : (T, StringBuilder) -> Unit,
) -> Unit {
  buf.write_string("[")
  for i, x in xs {
    if i > 0 {
      buf.write_string(",")
    }
    f(x, buf)
  }
  buf.write_string("]")
}

///|
fn emit_loc(loc : @source.SourceLoc, buf : StringBuilder) -> Unit {
  buf.write_string(
    "{\"line\":\{loc.line},\"column\":\{loc.column},\"file_id\":\{loc.file_id}}",
  )
}

///|
fn emit_json_str(s : String, buf : StringBuilder) -> Unit {
  buf.write_string("\"")
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
  buf.write_string("\"")
}

///|
/// 浮点文本化（serde_json/ryu-pretty 全区间对齐，2026-09-19 审阅 P1-1/P1-2 修复）。
///
/// 区间规则（ryu pretty：d 为最短有效数字串，v = d × 10^e10，
/// kk = len(d) + e10）：
/// 1. 0 <= e10 && kk <= 16 → 定点整数补 .0（1e15 → 1000000000000000.0）；
/// 2. 0 < kk <= 16 → 定点小数（123.45）；
/// 3. -5 < kk <= 0 → 0.000x（1e-5 → 0.00001）；
/// 4. 其余 → 指数 d.ddde±n（1e16 → 1e+16；1e-6 → 1e-6，负指数无 + 号、
///    不补零——serve 原始字节实测，非 Python 重排形态）。
/// 边界实证：kk=16 定点 / kk=17 指数；kk=-4 定点 / kk=-5 指数。
///
/// 非有限（±Infinity / NaN）→ null（serde_json 对 non-finite f64 的序列化
/// 语义——P1-2：此前输出 Infinity.0 / NaN 为非法 JSON）。
/// -0.0 保符号；0.0 → 0.0。
pub fn double_to_json_text(v : Double) -> String {
  // 非有限判定：NaN 自反不等 + 与 ±inf 相等（1/0 与 -1/0 产生正负 inf）
  if v != v || v == 1.0 / 0.0 || v == -1.0 / 0.0 {
    return "null"
  }
  if v == 0.0 {
    // 1/-0 = -inf 判负零（MoonBit to_string 会丢符号——实测）
    if 1.0 / v < 0.0 {
      return "-0.0"
    }
    return "0.0"
  }
  let neg = v < 0.0
  let raw = v.to_string()
  let s = if neg { raw[1:].to_owned() } else { raw }
  // 解析 to_string 形态：int[.frac][eE±exp] → (digits, e10)
  let buf_digits = StringBuilder()
  let mut e10 = 0
  let mut phase = 0 // 0=整数部 1=小数部 2=指数部
  let exp_txt = StringBuilder()
  for c in s {
    if phase == 2 {
      exp_txt.write_char(c)
      continue
    }
    if c == '.' {
      phase = 1
      continue
    }
    if c == 'e' || c == 'E' {
      phase = 2
      continue
    }
    buf_digits.write_char(c)
    if phase == 1 {
      e10 -= 1
    }
  }
  // 显式指数合并
  let et = exp_txt.to_string()
  if et != "" {
    let eneg = et.has_prefix("-")
    let edigits = et.replace(old="-", new="").replace(old="+", new="")
    let ev = try! @string.parse_int(edigits)
    e10 += if eneg { -ev } else { ev }
  }
  let mut digits = buf_digits.to_string()
  // 前导零剥离（e10 不变——"000001" 与 "1" 整数值同）；
  // 尾零剥离（e10 递增），至少留 1 位。
  let n_lead = {
    let mut i = 0
    while i < digits.length() - 1 && digits[i] == '0' {
      i += 1
    }
    i
  }
  digits = digits[n_lead:].to_owned()
  let n_trail = {
    let mut i = 0
    while i < digits.length() - 1 && digits[digits.length() - 1 - i] == '0' {
      i += 1
    }
    i
  }
  if n_trail > 0 {
    let keep = digits.length() - n_trail
    digits = digits[0:keep].to_owned()
    e10 += n_trail
  }
  let kk = digits.length() + e10
  let sign = if neg { "-" } else { "" }
  if e10 >= 0 && kk <= 16 {
    // 区间 1：定点整数 + .0
    let out = StringBuilder()
    out.write_string(sign)
    out.write_string(digits)
    for i = 0; i < e10; i = i + 1 {
      out.write_string("0")
    }
    out.write_string(".0")
    out.to_string()
  } else if kk > 0 && kk <= 16 {
    // 区间 2：定点小数（插点位置 len-kk）
    let out = StringBuilder()
    let split = kk
    out.write_string(sign)
    out.write_string(digits[0:split].to_owned())
    out.write_string(".")
    out.write_string(digits[split:].to_owned())
    out.to_string()
  } else if kk > -5 && kk <= 0 {
    // 区间 3：0.000x
    let out = StringBuilder()
    out.write_string(sign)
    out.write_string("0.")
    for i = 0; i < -kk; i = i + 1 {
      out.write_string("0")
    }
    out.write_string(digits)
    out.to_string()
  } else {
    // 区间 4：指数形 d[.ddd]e±(kk-1)
    let out = StringBuilder()
    out.write_string(sign)
    out.write_string(digits[0:1].to_owned())
    if digits.length() > 1 {
      out.write_string(".")
      out.write_string(digits[1:].to_owned())
    }
    out.write_string("e")
    let e = kk - 1
    if e >= 0 {
      out.write_string("+")
      out.write_string("\{e}")
    } else {
      out.write_string("\{e}")
    }
    out.to_string()
  }
}

///|
fn emit_bool(b : Bool, buf : StringBuilder) -> Unit {
  buf.write_string(if b { "true" } else { "false" })
}

// ---- Type ----

///|
fn emit_type(t : Type, buf : StringBuilder) -> Unit {
  match t {
    Void(is_const) => {
      buf.write_string("{\"Void\":{\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Int(is_unsigned, is_const) => {
      buf.write_string("{\"Int\":{\"is_unsigned\":")
      emit_bool(is_unsigned, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Char(is_unsigned, is_const) => {
      buf.write_string("{\"Char\":{\"is_unsigned\":")
      emit_bool(is_unsigned, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Float(is_const) => {
      buf.write_string("{\"Float\":{\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Double(is_const) => {
      buf.write_string("{\"Double\":{\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    LongLong(is_unsigned, is_const) => {
      buf.write_string("{\"LongLong\":{\"is_unsigned\":")
      emit_bool(is_unsigned, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Pointer(pointee, is_const) => {
      buf.write_string("{\"Pointer\":{\"pointee\":")
      emit_type(pointee, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Array(element, array_size, dims, is_const, is_vla, vla_dims) => {
      buf.write_string("{\"Array\":{\"element\":")
      emit_type(element, buf)
      buf.write_string(",\"array_size\":\{array_size},\"dims\":")
      emit_array(dims, buf, (d, b) => b.write_string("\{d}"))
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string(",\"is_vla\":")
      emit_bool(is_vla, buf)
      buf.write_string(",\"vla_dims\":")
      emit_array(vla_dims, buf, (e, b) => emit_expr(e, b))
      buf.write_string("}}")
    }
    Function(return_type, param_types, is_const, is_variadic) => {
      buf.write_string("{\"Function\":{\"return_type\":")
      emit_type(return_type, buf)
      buf.write_string(",\"param_types\":")
      emit_array(param_types, buf, (t, b) => emit_type(t, b))
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string(",\"is_variadic\":")
      emit_bool(is_variadic, buf)
      buf.write_string("}}")
    }
    Struct(name, is_const) => {
      buf.write_string("{\"Struct\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Union(name, is_const) => {
      buf.write_string("{\"Union\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Class(name, is_const) => {
      buf.write_string("{\"Class\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Reference(base, is_const) => {
      buf.write_string("{\"Reference\":{\"base\":")
      emit_type(base, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    RValueRef(base) => {
      buf.write_string("{\"RValueRef\":{\"base\":")
      emit_type(base, buf)
      buf.write_string("}}")
    }
    Auto => buf.write_string("\"Auto\"")
    TemplateId(base, args, is_const) => {
      buf.write_string("{\"TemplateId\":{\"base\":")
      emit_json_str(base, buf)
      buf.write_string(",\"args\":")
      emit_array(args, buf, (a, b) => emit_template_arg(a, b))
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Typeof(expr, is_const, unqual) => {
      buf.write_string("{\"Typeof\":{\"expr\":")
      emit_expr(expr, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string(",\"unqual\":")
      emit_bool(unqual, buf)
      buf.write_string("}}")
    }
  }
}

///|
fn emit_template_arg(a : TemplateArg, buf : StringBuilder) -> Unit {
  match a {
    TArg(t) => {
      buf.write_string("{\"Type\":")
      emit_type(t, buf)
      buf.write_string("}")
    }
    TExpr(e) => {
      buf.write_string("{\"Expr\":")
      emit_expr(e, buf)
      buf.write_string("}")
    }
    TInt(v) => buf.write_string("{\"Int\":\{v}}")
  }
}

// ---- 运算子（单元枚举 → 裸串）----

///|
fn emit_binary_op(op : BinaryOp, buf : StringBuilder) -> Unit {
  let s = match op {
    Add => "Add"
    Sub => "Sub"
    Mul => "Mul"
    Div => "Div"
    Mod => "Mod"
    Eq => "Eq"
    Ne => "Ne"
    Lt => "Lt"
    Le => "Le"
    Gt => "Gt"
    Ge => "Ge"
    And => "And"
    Or => "Or"
    BitAnd => "BitAnd"
    BitOr => "BitOr"
    BitXor => "BitXor"
    Shl => "Shl"
    Shr => "Shr"
    Comma => "Comma"
  }
  buf.write_string("\"")
  buf.write_string(s)
  buf.write_string("\"")
}

///|
fn emit_unary_op(op : UnaryOp, buf : StringBuilder) -> Unit {
  let s = match op {
    Neg => "Neg"
    Not => "Not"
    BitNot => "BitNot"
    Addr => "Addr"
    Deref => "Deref"
    PreInc => "PreInc"
    PreDec => "PreDec"
    PostInc => "PostInc"
    PostDec => "PostDec"
  }
  buf.write_string("\"")
  buf.write_string(s)
  buf.write_string("\"")
}

///|
fn emit_assign_op(op : AssignOp, buf : StringBuilder) -> Unit {
  let s = match op {
    Assign => "Assign"
    AddAssign => "AddAssign"
    SubAssign => "SubAssign"
    MulAssign => "MulAssign"
    DivAssign => "DivAssign"
    ModAssign => "ModAssign"
    AndAssign => "AndAssign"
    OrAssign => "OrAssign"
    XorAssign => "XorAssign"
    ShlAssign => "ShlAssign"
    ShrAssign => "ShrAssign"
  }
  buf.write_string("\"")
  buf.write_string(s)
  buf.write_string("\"")
}
