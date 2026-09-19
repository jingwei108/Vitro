///|
/// 类型 → C 声明形渲染**单源**（`to_c_string`；对齐 Rust `Display for Type`
/// 语义——MoonBit 侧禁第二轨道（Rust 侧 Display + 散点手拼的双轨教训，
/// 总计划 §5 "渲染单源"））。
///
/// 形态例：
/// - `Pointer(Char)` → `char*`
/// - `Array(Int, dims=[2,3])` → `int[2][3]`；VLA → `int[*]`
/// - `Function(Int, [Int])` → `int (*)(int)`
/// - `Struct("S")` → `struct S`
pub fn Type::to_c_string(self : Type) -> String {
  let buf = StringBuilder()
  self.render_into(buf)
  buf.to_string()
}

///|
fn Type::render_into(self : Type, buf : StringBuilder) -> Unit {
  match self {
    Void(_) => buf.write_string("void")
    // Rust Display 现状：基础类型不显示 unsigned 修饰（信息丢失为已知
    // 现状——mangle 轨道才区分 unsigned_int）。单源照搬现状，不私改。
    Int(_, _) => buf.write_string("int")
    Char(_, _) => buf.write_string("char")
    Float(_) => buf.write_string("float")
    Double(_) => buf.write_string("double")
    LongLong(_, _) => buf.write_string("long long")
    Struct(name, _) => {
      buf.write_string("struct ")
      buf.write_string(name)
    }
    Union(name, _) => {
      buf.write_string("union ")
      buf.write_string(name)
    }
    Pointer(pointee, _) => {
      pointee.render_into(buf)
      buf.write_string("*")
    }
    Array(element, array_size, dims, _, is_vla, _) => {
      element.render_into(buf)
      if is_vla {
        for _ in dims {
          buf.write_string("[*]")
        }
      } else if dims.length() > 0 {
        for d in dims {
          buf.write_string("[\{d}]")
        }
      } else if array_size > 0 {
        buf.write_string("[\{array_size}]")
      } else {
        buf.write_string("[]")
      }
    }
    Function(return_type, param_types, _, _) => {
      return_type.render_into(buf)
      buf.write_string(" (*)(")
      for i, p in param_types {
        if i > 0 {
          buf.write_string(", ")
        }
        p.render_into(buf)
      }
      buf.write_string(")")
    }
    // === C++ 新增 ===
    Class(name, _) => {
      buf.write_string("class ")
      buf.write_string(name)
    }
    Reference(base, is_const) => {
      if is_const {
        buf.write_string("const ")
      }
      base.render_into(buf)
      buf.write_string("&")
    }
    RValueRef(base) => {
      base.render_into(buf)
      buf.write_string("&&")
    }
    Auto => buf.write_string("auto")
    Typeof(_, _, _) => buf.write_string("typeof(<expr>)")
    TemplateId(base, args, _) => {
      buf.write_string(base)
      buf.write_string("<")
      for i, a in args {
        if i > 0 {
          buf.write_string(", ")
        }
        match a {
          TArg(ty) => ty.render_into(buf)
          TInt(v) => buf.write_string("\{v}")
          TExpr(_) => buf.write_string("<expr>")
        }
      }
      buf.write_string(">")
    }
  }
}
