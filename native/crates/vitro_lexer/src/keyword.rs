//! C/C++ 关键字识别。

use super::token::TokenType;

/// C++ 关键字查找。
pub fn cpp_keyword_type(text: &str) -> Option<TokenType> {
    match text {
        "class" => Some(TokenType::Class),
        "public" => Some(TokenType::Public),
        "private" => Some(TokenType::Private),
        "protected" => Some(TokenType::Protected),
        "this" => Some(TokenType::This),
        "using" => Some(TokenType::Using),
        "namespace" => Some(TokenType::Namespace),
        "virtual" => Some(TokenType::Virtual),
        "override" => Some(TokenType::Override),
        "explicit" => Some(TokenType::Explicit),
        "friend" => Some(TokenType::Friend),
        "template" => Some(TokenType::Template),
        "typename" => Some(TokenType::Typename),
        "static_cast" => Some(TokenType::StaticCast),
        "const_cast" => Some(TokenType::ConstCast),
        "reinterpret_cast" => Some(TokenType::ReinterpretCast),
        "new" => Some(TokenType::New),
        "delete" => Some(TokenType::Delete),
        "nullptr" => Some(TokenType::Null),
        _ => None,
    }
}

/// C 关键字查找。
pub fn keyword_type(text: &str) -> Option<TokenType> {
    match text {
        "int" => Some(TokenType::Int),
        "void" => Some(TokenType::Void),
        "char" => Some(TokenType::Char),
        "if" => Some(TokenType::If),
        "else" => Some(TokenType::Else),
        "while" => Some(TokenType::While),
        "do" => Some(TokenType::Do),
        "for" => Some(TokenType::For),
        "return" => Some(TokenType::Return),
        "break" => Some(TokenType::Break),
        "continue" => Some(TokenType::Continue),
        "struct" => Some(TokenType::Struct),
        "union" => Some(TokenType::Union),
        "sizeof" => Some(TokenType::Sizeof),
        "offsetof" => Some(TokenType::Offsetof),
        "_Generic" => Some(TokenType::Generic),
        "switch" => Some(TokenType::Switch),
        "case" => Some(TokenType::Case),
        "default" => Some(TokenType::Default),
        "typedef" => Some(TokenType::Typedef),
        "enum" => Some(TokenType::Enum),
        "unsigned" => Some(TokenType::Unsigned),
        "long" => Some(TokenType::Long),
        "short" => Some(TokenType::Short),
        "signed" => Some(TokenType::Signed),
        "const" => Some(TokenType::Const),
        "extern" => Some(TokenType::Extern),
        "volatile" => Some(TokenType::Volatile),
        "inline" => Some(TokenType::Inline),
        "restrict" => Some(TokenType::Restrict),
        "register" => Some(TokenType::Register),
        "auto" => Some(TokenType::Auto),
        "_Bool" => Some(TokenType::Bool),
        "bool" => Some(TokenType::Bool),
        "float" => Some(TokenType::Float),
        "double" => Some(TokenType::Double),
        "goto" => Some(TokenType::Goto),
        // C23（C11 起 _Alignof）：对齐查询（E1）
        "alignof" => Some(TokenType::Alignof),
        "_Alignof" => Some(TokenType::Alignof),
        "NULL" => Some(TokenType::Null),
        "null" => Some(TokenType::Null),
        // C23（E3）：空指针常量，与 NULL 同路径（void* 空）
        "nullptr" => Some(TokenType::Null),
        // C23（E3）：constexpr 对象——教学子集按 const 语义处理（差异入 spec §2.12）
        "constexpr" => Some(TokenType::Const),
        _ => None,
    }
}
