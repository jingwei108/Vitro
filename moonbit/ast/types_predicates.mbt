///|
/// 类型谓词与消费面取值（S3 消费驱动补齐——此前 T4 按"消费面函数待 S4/S5
/// 按需补"延后；parser 片是首个消费者，语义照搬 Rust oracle
/// `vitro_ast/src/types.rs` 的同名方法，含 C++ 分支）。
///
/// 不引入 `TypeKind` 枚举：其 16 变体与 `Type` 的 17 构造器大面积同名，
/// 枚举入包会污染裸构造器名空间（Type::Array 与 TypeKind::Array 歧义，
/// S1 手册陷阱 4 的实测复现）。当前消费点（is_array / ctor-init 判定 /
/// enum 底层类型判定 / compute_type_size 尺寸表）全部直写模式匹配；
/// S4 typeck 若需大面积 kind 消费再独立裁定形态。

///|
/// 核心名称（照搬 Rust `Type::name()`：Struct/Union/Class/TemplateId 返回
/// 原始名；Pointer/Array/Reference/RValueRef 递归取内层；基础类型返回
/// 关键字；Typeof 返回 "typeof"）。
pub fn Type::name(self : Type) -> String {
  match self {
    Struct(name, _) | Union(name, _) => name
    Pointer(pointee, _) => pointee.name()
    Array(element, _, _, _, _, _) => element.name()
    Void(_) => "void"
    Int(_, _) => "int"
    Char(_, _) => "char"
    Float(_) => "float"
    Double(_) => "double"
    LongLong(_, _) => "long long"
    Function(_, _, _, _) => "fn"
    Class(name, _) => name
    Reference(base, _) => base.name()
    RValueRef(base) => base.name()
    Auto => "auto"
    Typeof(_, _, _) => "typeof"
    TemplateId(base, _, _) => base
  }
}

///|
/// 数组尺寸（非数组返回 0）。
pub fn Type::array_size(self : Type) -> Int {
  match self {
    Array(_, size, _, _, _, _) => size
    _ => 0
  }
}

///|
/// 维度表（非数组返回空）。
pub fn Type::dims(self : Type) -> Array[Int] {
  match self {
    Array(_, _, dims, _, _, _) => dims
    _ => []
  }
}

///|
/// VLA 判定（仅 Array 形态可真）。
pub fn Type::is_vla(self : Type) -> Bool {
  match self {
    Array(_, _, _, _, is_vla, _) => is_vla
    _ => false
  }
}

///|
/// 顶层 const 判定（照搬 Rust `is_const`：每变体各自的 is_const 位；
/// RValueRef/Auto 无 const 位恒 false）。
pub fn Type::is_const(self : Type) -> Bool {
  match self {
    Void(is_const) => is_const
    Int(_, is_const) => is_const
    Char(_, is_const) => is_const
    Float(is_const) => is_const
    Double(is_const) => is_const
    LongLong(_, is_const) => is_const
    Pointer(_, is_const) => is_const
    Array(_, _, _, is_const, _, _) => is_const
    Function(_, _, is_const, _) => is_const
    Struct(_, is_const) => is_const
    Union(_, is_const) => is_const
    Class(_, is_const) => is_const
    Reference(_, is_const) => is_const
    RValueRef(_) => false
    Auto => false
    TemplateId(_, _, is_const) => is_const
    Typeof(_, is_const, _) => is_const
  }
}

///|
/// 置/清顶层 const（返回新值；照搬 Rust `set_const` 的逐变体改位语义，
/// Rust 为原地修改，MoonBit 值形态返回新 Type）。
pub fn Type::set_const(self : Type, value : Bool) -> Type {
  match self {
    Void(_) => Void(value)
    Int(is_unsigned, _) => Int(is_unsigned, value)
    Char(is_unsigned, _) => Char(is_unsigned, value)
    Float(_) => Float(value)
    Double(_) => Double(value)
    LongLong(is_unsigned, _) => LongLong(is_unsigned, value)
    Pointer(pointee, _) => Pointer(pointee, value)
    Array(element, size, dims, _, is_vla, vla_dims) =>
      Array(element, size, dims, value, is_vla, vla_dims)
    Function(ret, params, _, variadic) => Function(ret, params, value, variadic)
    Struct(name, _) => Struct(name, value)
    Union(name, _) => Union(name, value)
    Class(name, _) => Class(name, value)
    Reference(base, _) => Reference(base, value)
    RValueRef(base) => RValueRef(base)
    Auto => Auto
    TemplateId(base, args, _) => TemplateId(base, args, value)
    Typeof(expr, _, unqual) => Typeof(expr, value, unqual)
  }
}

///|
/// `is_array`（照搬 Rust kind 口径）。
pub fn Type::is_array(self : Type) -> Bool {
  match self {
    Array(_, _, _, _, _, _) => true
    _ => false
  }
}

///|
/// 数组下标一次后的类型（照搬 Rust `subscript_type`：一维返回元素类型；
/// 多维去掉首维重算 size（非正维按 1 计）；vla_dims 同步去首；非数组
/// 返回自身）。
pub fn Type::subscript_type(self : Type) -> Type {
  if !self.is_array() {
    return self
  }
  match self {
    Array(element, _, dims, is_const, is_vla, vla_dims) =>
      if dims.length() <= 1 {
        element
      } else {
        let new_dims : Array[Int] = []
        for i in 1..<dims.length() {
          new_dims.push(dims[i])
        }
        let mut new_size = 1
        for d in new_dims {
          new_size *= if d > 0 { d } else { 1 }
        }
        let new_vla_dims : Array[Expr] = []
        if !vla_dims.is_empty() {
          for i in 1..<vla_dims.length() {
            new_vla_dims.push(vla_dims[i])
          }
        }
        Array(element, new_size, new_dims, is_const, is_vla, new_vla_dims)
      }
    _ => self
  }
}

///|
/// 递归取数组最内层元素类型（非数组返回自身；照搬 Rust）。
pub fn Type::base_element_type(self : Type) -> Type {
  match self {
    Array(element, _, _, _, _, _) => element.base_element_type()
    _ => self
  }
}

///|
/// 元素总数（照搬 Rust `total_elements`：非数组 1；含负维且 size>0 时
/// 返回 size；否则按维乘积（非正维按 1）；空维返回 size（>0）或 1）。
pub fn Type::total_elements(self : Type) -> Int {
  if !self.is_array() {
    return 1
  }
  let dims = self.dims()
  if !dims.is_empty() {
    let has_negative = dims.exists(fn(d) { d < 0 })
    if has_negative && self.array_size() > 0 {
      return self.array_size()
    }
    let mut product = 1
    for d in dims {
      product *= if d > 0 { d } else { 1 }
    }
    product
  } else if self.array_size() > 0 {
    self.array_size()
  } else {
    1
  }
}

///|
/// 根据类型定义计算字节大小（S3 补齐入包——Rust 归属 `vitro_ast::lib`，
/// parser 的 `eval_enum_const` 消费（空表场景）触发；与 Rust 同签名的
/// 完整实现，S4 typeck 复用）。内建：void=0 / int=4 / char=1 / float=4 /
/// double=longlong=8 / 指针=函数=引用=4（32 位教学 VM 口径）；数组 =
/// total_elements × 基元素；struct=Σ字段 / union=max 字段（环返回 0，
/// 真诊断在 typeck E3072）；class 查 class_size_map；Auto/TemplateId=0。
pub fn compute_type_size(
  ty : Type,
  struct_defs : Map[String, Array[StructField]],
  union_defs : Map[String, Array[StructField]],
  class_size_map : Map[String, Int],
) -> Int {
  compute_type_size_impl(ty, struct_defs, union_defs, class_size_map, Set([]))
}

///|
fn compute_type_size_impl(
  ty : Type,
  struct_defs : Map[String, Array[StructField]],
  union_defs : Map[String, Array[StructField]],
  class_size_map : Map[String, Int],
  visiting : Set[String],
) -> Int {
  match ty {
    Void(_) => 0
    Int(_, _) => 4
    Char(_, _) => 1
    Float(_) => 4
    Double(_) | LongLong(_, _) => 8
    Pointer(_, _) | Function(_, _, _, _) => 4
    Array(_, _, _, _, _, _) => {
      if ty.is_vla() {
        // VLA 变量本身在栈上按指针存
        return 4
      }
      let elem_count = ty.total_elements()
      let base_elem = ty.base_element_type()
      let elem_size = compute_type_size_impl(
        base_elem, struct_defs, union_defs, class_size_map, visiting,
      )
      elem_count * elem_size
    }
    Struct(name, _) => {
      if visiting.contains(name) {
        return 0 // 循环包含：防无限递归（真诊断在 typeck E3072）
      }
      ignore(visiting.add(name))
      let size = match struct_defs.get(name) {
        Some(fields) => {
          let mut sum = 0
          for f in fields {
            sum += compute_type_size_impl(
              f.ty,
              struct_defs,
              union_defs,
              class_size_map,
              visiting,
            )
          }
          sum
        }
        None => 0
      }
      // Rust 同款"退出时 remove"：visiting 是当前展开路径，兄弟字段共享
      visiting.remove(name)
      size
    }
    Union(name, _) => {
      if visiting.contains(name) {
        return 0
      }
      ignore(visiting.add(name))
      let size = match union_defs.get(name) {
        Some(fields) => {
          let mut mx = 0
          for f in fields {
            let s = compute_type_size_impl(
              f.ty,
              struct_defs,
              union_defs,
              class_size_map,
              visiting,
            )
            if s > mx {
              mx = s
            }
          }
          mx
        }
        None => 0
      }
      visiting.remove(name)
      size
    }
    Class(name, _) =>
      match class_size_map.get(name) {
        Some(s) => s
        None => 0
      }
    Reference(_, _) | RValueRef(_) => 4 // 引用底层是指针
    Auto => 0 // typeck 替换前不应出现在 codegen
    TemplateId(_, _, _) => 0 // 应先单态化为 Class
    Typeof(_, _, _) => 0 // 推导类型布局在 typeck 后——parser 空表场景不可达
  }
}
