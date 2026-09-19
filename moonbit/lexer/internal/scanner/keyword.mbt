///|
/// C 关键字查找（照搬 `keyword.rs:32-81`——43 条映射含同义拼写与 C23 锚定）。
fn keyword_type(text : String) -> @token.TokenType? {
  match text {
    "int" => Some(Int)
    "void" => Some(Void)
    "char" => Some(Char)
    "if" => Some(If)
    "else" => Some(Else)
    "while" => Some(While)
    "do" => Some(Do)
    "for" => Some(For)
    "return" => Some(Return)
    "break" => Some(Break)
    "continue" => Some(Continue)
    "struct" => Some(Struct)
    "union" => Some(Union)
    "sizeof" => Some(Sizeof)
    "offsetof" => Some(Offsetof)
    "_Generic" => Some(Generic)
    "switch" => Some(Switch)
    "case" => Some(Case)
    "default" => Some(Default)
    "typedef" => Some(Typedef)
    "enum" => Some(Enum)
    "unsigned" => Some(Unsigned)
    "long" => Some(Long)
    "short" => Some(Short)
    "signed" => Some(Signed)
    "const" => Some(Const)
    "extern" => Some(Extern)
    "volatile" => Some(Volatile)
    "inline" => Some(Inline)
    "restrict" => Some(Restrict)
    "register" => Some(Register)
    "auto" => Some(Auto)
    "_Bool" => Some(Bool)
    "bool" => Some(Bool)
    "float" => Some(Float)
    "double" => Some(Double)
    "goto" => Some(Goto)
    // C23（C11 起 _Alignof）：对齐查询（E1）
    "alignof" => Some(Alignof)
    "_Alignof" => Some(Alignof)
    "NULL" => Some(Null)
    "null" => Some(Null)
    // C23（E3）：空指针常量，与 NULL 同路径
    "nullptr" => Some(Null)
    // C23（E3）：constexpr 按 const 语义处理（差异入 spec §2.12）
    "constexpr" => Some(Const)
    _ => None
  }
}

///|
/// C++ 关键字查找（照搬 `keyword.rs:6-29`——18 条 + nullptr→Null）。
fn cpp_keyword_type(text : String) -> @token.TokenType? {
  match text {
    "class" => Some(Class)
    "public" => Some(Public)
    "private" => Some(Private)
    "protected" => Some(Protected)
    "this" => Some(This)
    "using" => Some(Using)
    "namespace" => Some(Namespace)
    "virtual" => Some(Virtual)
    "override" => Some(Override)
    "explicit" => Some(Explicit)
    "friend" => Some(Friend)
    "template" => Some(Template)
    "typename" => Some(Typename)
    "static_cast" => Some(StaticCast)
    "const_cast" => Some(ConstCast)
    "reinterpret_cast" => Some(ReinterpretCast)
    "new" => Some(New)
    "delete" => Some(Delete)
    "nullptr" => Some(Null)
    _ => None
  }
}
