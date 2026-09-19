///|
/// 类型 mangle 名（照搬 Rust `Type::mangle_name_into`——模板实例化名的
/// 组成部分；语义冻结，改一字符即 S4 差分锚红）。
///
/// 形态：`Pointer(Array(int, dims=[2,3]))` → `p_a2_3_int`
/// （E5 黄金串，与 Rust ast_unit_test 同断言——见 ast_wbtest）。
///
/// Typeof 形态输出 `typeof({:?})`（Rust Debug 格式）——本包不照搬 Debug
/// 排版（MoonBit derive(Debug) 形态不同），以 `typeof(<expr>)` 显式标记；
/// **两侧差异登记**：含 Typeof 的 mangle 串不参与 S4 对拍（mangled 名
/// 集合锚过滤 typeof 形态）。
pub fn Type::mangle_name(self : Type) -> String {
  let buf = StringBuilder()
  self.mangle_name_into(buf)
  buf.to_string()
}

///|
/// 追加式 mangle（Rust `mangle_name_into` 同构——避免 O(n²) 临时串；
/// E5 黄金串即本函数在预置前缀上的追加语义）。
pub fn Type::mangle_name_into(self : Type, buf : StringBuilder) -> Unit {
  match self {
    Void(_) => buf.write_string("void")
    Int(is_unsigned, _) =>
      if is_unsigned {
        buf.write_string("unsigned_int")
      } else {
        buf.write_string("int")
      }
    Char(is_unsigned, _) =>
      if is_unsigned {
        buf.write_string("unsigned_char")
      } else {
        buf.write_string("char")
      }
    Float(_) => buf.write_string("float")
    Double(_) => buf.write_string("double")
    LongLong(is_unsigned, _) =>
      if is_unsigned {
        buf.write_string("unsigned_long_long")
      } else {
        buf.write_string("long_long")
      }
    Pointer(pointee, _) => {
      buf.write_string("p_")
      pointee.mangle_name_into(buf)
    }
    Array(element, _, dims, _, _, _) => {
      buf.write_string("a")
      for i, d in dims {
        if i > 0 {
          buf.write_string("_")
        }
        buf.write_string("\{d}")
      }
      buf.write_string("_")
      element.mangle_name_into(buf)
    }
    Function(return_type, param_types, _, _) => {
      buf.write_string("fn_")
      return_type.mangle_name_into(buf)
      buf.write_string("_")
      for i, pt in param_types {
        if i > 0 {
          buf.write_string("_")
        }
        pt.mangle_name_into(buf)
      }
    }
    Struct(name, _) => {
      buf.write_string("struct_")
      buf.write_string(name)
    }
    Union(name, _) => {
      buf.write_string("union_")
      buf.write_string(name)
    }
    Class(name, _) => {
      buf.write_string("class_")
      buf.write_string(name)
    }
    Reference(base, is_const) => {
      if is_const {
        buf.write_string("const_ref_")
      } else {
        buf.write_string("ref_")
      }
      base.mangle_name_into(buf)
    }
    RValueRef(base) => {
      buf.write_string("rref_")
      base.mangle_name_into(buf)
    }
    Auto => buf.write_string("auto")
    Typeof(_, _, _) => buf.write_string("typeof(<expr>)")
    TemplateId(base, args, _) => {
      buf.write_string(base)
      buf.write_string("__")
      for i, a in args {
        if i > 0 {
          buf.write_string("__")
        }
        a.mangle_name_into(buf)
      }
    }
  }
}

///|
/// 模板实参 mangle（Rust `TemplateArg::mangle_name_into` 同构；
/// Expr 形态以 `<expr>` 标记——同 Typeof 的差异登记）。
pub fn TemplateArg::mangle_name_into(
  self : TemplateArg,
  buf : StringBuilder,
) -> Unit {
  match self {
    TArg(ty) => ty.mangle_name_into(buf)
    TInt(v) => buf.write_string("\{v}")
    TExpr(_) => buf.write_string("<expr>")
  }
}
