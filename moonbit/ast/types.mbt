///|
/// 类型节点（17 变体，照搬 Rust oracle `vitro_ast/src/types.rs`）。
///
/// 契约：
/// - C 子集 11 变体（Void..Function + Struct/Union）+ C++ 6 变体
///   （Class/Reference/RValueRef/Auto/TemplateId/Typeof）；
/// - `Array.array_size` 由 dims 派生（`Type::array_of` 单点计算，消费方
///   禁自算——Rust 侧同构）；
/// - `vla_dims` 只在 VLA 形态携带（判等**显式忽略**，见 types_eq.mbt——
///   表达式维度不参与类型判等，Rust PartialEq 同语义）；
/// - `Typeof` 判等恒不等（typeof 表达式不做结构判等），自反即 false——
///   调用方须先用 typeck 归一再比较。
///
/// 未照搬面（消费驱动，登记）：kind()/name()/is_* 系列、total_elements/
/// subscript_type/innermost_element_type、set_const——待 S4/S5 消费侧
/// 落地时补（本包首批只含构造 / 判等 / 渲染 / mangle / depth / dump）。
pub(all) enum Type {
  Void(Bool) // is_const
  Int(Bool, Bool) // is_unsigned, is_const
  Char(Bool, Bool) // is_unsigned, is_const
  Float(Bool) // is_const
  Double(Bool) // is_const
  LongLong(Bool, Bool) // is_unsigned, is_const
  Pointer(Type, Bool) // pointee, is_const
  Array(Type, Int, Array[Int], Bool, Bool, Array[Expr]) // element, array_size, dims, is_const, is_vla, vla_dims
  Function(Type, Array[Type], Bool, Bool) // return_type, param_types, is_const, is_variadic
  Struct(String, Bool) // name, is_const
  Union(String, Bool) // name, is_const
  // === C++ 新增 ===
  Class(String, Bool) // name, is_const
  Reference(Type, Bool) // base, is_const
  RValueRef(Type) // base
  Auto
  TemplateId(String, Array[TemplateArg], Bool) // base, args, is_const
  Typeof(Expr, Bool, Bool) // expr, is_const, unqual（C23 typeof_unqual）
} derive(Debug)

///|
/// `int`。
pub fn Type::int() -> Type {
  Int(false, false)
}

///|
/// `unsigned int`。
pub fn Type::unsigned_int() -> Type {
  Int(true, false)
}

///|
/// `char`。
pub fn Type::char() -> Type {
  Char(false, false)
}

///|
/// `float`。
pub fn Type::float() -> Type {
  Float(false)
}

///|
/// `double`。
pub fn Type::double() -> Type {
  Double(false)
}

///|
/// `long long`。
pub fn Type::long_long() -> Type {
  LongLong(false, false)
}

///|
/// `void`。
pub fn Type::void_type() -> Type {
  Void(false)
}

///|
/// 指向 pointee 的指针。
pub fn Type::pointer_to(pointee : Type) -> Type {
  Pointer(pointee, false)
}

///|
/// element 的多维数组；`array_size` = 各维正值的乘积（空 dims 为 0，
/// 非正维按 1 计）——dims→size 派生的**单点**，消费方禁自算。
pub fn Type::array_of(element : Type, dims : Array[Int]) -> Type {
  let array_size = if dims.is_empty() {
    0
  } else {
    let mut product = 1
    for d in dims {
      product *= if d > 0 { d } else { 1 }
    }
    product
  }
  Array(element, array_size, dims, false, false, [])
}

///|
/// 具名 struct 类型。
pub fn Type::struct_type(name : String) -> Type {
  Struct(name, false)
}

///|
/// 具名 union 类型。
pub fn Type::union_type(name : String) -> Type {
  Union(name, false)
}

///|
/// 函数类型（非变参）。
pub fn Type::function(return_type : Type, param_types : Array[Type]) -> Type {
  Function(return_type, param_types, false, false)
}

///|
/// 函数指针（Pointer 包 Function 的惯用组合）。
pub fn Type::function_pointer(
  return_type : Type,
  param_types : Array[Type],
) -> Type {
  Pointer(Function(return_type, param_types, false, false), false)
}
