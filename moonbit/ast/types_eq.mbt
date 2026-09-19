///|
/// 类型判等（显式函数，照搬 Rust `impl PartialEq for Type` 的**全部**
/// 语义决策——含两处故意的"不等"）：
///
/// 1. **Typeof 自反即 false**：`(Typeof, Typeof) => false`——typeof 表达式
///    不做结构判等，调用方须先经 typeck 归一；
/// 2. **Array 判等忽略 vla_dims**：VLA 的表达式维度不参与判等
///    （Rust 侧 `..` 忽略同语义）——VLA 相等性只看 element/array_size/
///    dims/is_const/is_vla；
/// 3. 异变体恒 false（无兜底臂——增删变体即编译红，穷尽 match 纪律）。
pub fn type_eq(a : Type, b : Type) -> Bool {
  match (a, b) {
    (Void(ac), Void(bc)) => ac == bc
    (Int(au, ac), Int(bu, bc)) => au == bu && ac == bc
    (Char(au, ac), Char(bu, bc)) => au == bu && ac == bc
    (Float(ac), Float(bc)) => ac == bc
    (Double(ac), Double(bc)) => ac == bc
    (LongLong(au, ac), LongLong(bu, bc)) => au == bu && ac == bc
    (Pointer(ap, ac), Pointer(bp, bc)) => type_eq(ap, bp) && ac == bc
    (Array(ae, asize, adims, ac, avla, _), Array(be, bsize, bdims, bc, bvla, _)) =>
      type_eq(ae, be) &&
      asize == bsize &&
      int_array_eq(adims, bdims) &&
      ac == bc &&
      avla == bvla
    (Function(ar, aps, ac, av), Function(br, bps, bc, bv)) =>
      type_eq(ar, br) && type_array_eq(aps, bps) && ac == bc && av == bv
    (Struct(an, ac), Struct(bn, bc)) => an == bn && ac == bc
    (Union(an, ac), Union(bn, bc)) => an == bn && ac == bc
    // === C++ 新增 ===
    (Class(an, ac), Class(bn, bc)) => an == bn && ac == bc
    (Reference(ab, ac), Reference(bb, bc)) => type_eq(ab, bb) && ac == bc
    (RValueRef(ab), RValueRef(bb)) => type_eq(ab, bb)
    (Auto, Auto) => true
    (Typeof(_, _, _), Typeof(_, _, _)) => false
    (TemplateId(abase, aargs, ac), TemplateId(bbase, bargs, bc)) =>
      abase == bbase && template_arg_array_eq(aargs, bargs) && ac == bc
    _ => false
  }
}

///|
fn int_array_eq(a : Array[Int], b : Array[Int]) -> Bool {
  if a.length() != b.length() {
    return false
  }
  for i in 0..<a.length() {
    if a[i] != b[i] {
      return false
    }
  }
  true
}

///|
fn type_array_eq(a : Array[Type], b : Array[Type]) -> Bool {
  if a.length() != b.length() {
    return false
  }
  for i in 0..<a.length() {
    if !type_eq(a[i], b[i]) {
      return false
    }
  }
  true
}

///|
/// 模板实参判等（照搬 Rust `impl PartialEq for TemplateArg`：Type/Type
/// 递归 type_eq、Int/Int 数值比、**Expr 形态恒 false**——未求值表达式
/// 不参与判等）。
pub fn template_arg_eq(a : TemplateArg, b : TemplateArg) -> Bool {
  match (a, b) {
    (TArg(at), TArg(bt)) => type_eq(at, bt)
    (TInt(av), TInt(bv)) => av == bv
    _ => false
  }
}

///|
fn template_arg_array_eq(
  a : Array[TemplateArg],
  b : Array[TemplateArg],
) -> Bool {
  if a.length() != b.length() {
    return false
  }
  for i in 0..<a.length() {
    if !template_arg_eq(a[i], b[i]) {
      return false
    }
  }
  true
}
