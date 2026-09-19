///|
/// 词法单元类型——116 变体，命名与顺序照搬 Rust oracle `vitro_lexer::TokenType`
/// （`native/crates/vitro_lexer/src/token.rs`，tag `rust-oracle-freeze`）。
///
/// S2 契约决策（登记）：勘察报告 ⑤-4 曾建议分层（Keyword/Punct/Literal），本实现
/// 维持 flat 116 变体——理由：L1/L2 token TSV 差分锚要求 `to_name` 与 Rust Debug
/// 输出逐字节一致，flat 直接照搬消除一层映射噪声；S1 diag 包 137 臂穷尽 match
/// 已验证此规模的工程可行性。穷尽 match 由编译器强制（F7）。
///
/// `to_name` 输出与 Rust `format!("{:?}", ty)`（变体名）一致——TSV 的 Ty 字段
/// 两侧逐字节可比的依据。
pub(all) enum TokenType {
  Int
  Void
  Char
  If
  Else
  While
  Do
  For
  Return
  Break
  Continue
  Struct
  Union
  Sizeof
  Offsetof
  Alignof
  Generic
  Switch
  Case
  Default
  Typedef
  Enum
  Unsigned
  Long
  Short
  Signed
  Const
  Extern
  Float
  Double
  Volatile
  Inline
  Restrict
  Register
  Auto
  Bool
  Goto
  Null
  // === C++ 新增关键字 ===
  Class
  Public
  Private
  Protected
  This
  Using
  Namespace
  Virtual
  Override
  Explicit
  Friend
  Template
  Typename
  StaticCast
  ConstCast
  ReinterpretCast
  New
  Delete
  // === C++ 新增标点 ===
  ColonColon
  ArrowStar
  DotStar
  Identifier
  Number
  UnsignedLiteral
  FloatLiteral
  LongLiteral
  CharLiteral
  String
  Plus
  Minus
  Star
  Slash
  Percent
  Eq
  Ne
  Lt
  Le
  Gt
  Ge
  AndAnd
  OrOr
  Not
  Assign
  PlusAssign
  MinusAssign
  StarAssign
  SlashAssign
  PercentAssign
  AndAssign
  OrAssign
  XorAssign
  ShlAssign
  ShrAssign
  Ampersand
  BitOr
  BitXor
  BitNot
  Shl
  Shr
  Increment
  Decrement
  Semicolon
  Comma
  LParen
  RParen
  LBrace
  RBrace
  LBracket
  RBracket
  Dot
  Ellipsis
  Arrow
  Colon
  Question
  Eof
  Unknown
  /// E2：`#`（仅宏体内作字符串化操作符；其余上下文按预处理指令消费）
  Hash
  /// E2：`##`（宏体内 token 拼接操作符）
  HashHash
} derive(Eq, @debug.Debug)

///|
/// 变体名——与 Rust `TokenType` 的 Debug 输出逐字节一致（L1/L2 TSV 的 Ty 字段）。
pub fn TokenType::to_name(self : TokenType) -> String {
  match self {
    Int => "Int"
    Void => "Void"
    Char => "Char"
    If => "If"
    Else => "Else"
    While => "While"
    Do => "Do"
    For => "For"
    Return => "Return"
    Break => "Break"
    Continue => "Continue"
    Struct => "Struct"
    Union => "Union"
    Sizeof => "Sizeof"
    Offsetof => "Offsetof"
    Alignof => "Alignof"
    Generic => "Generic"
    Switch => "Switch"
    Case => "Case"
    Default => "Default"
    Typedef => "Typedef"
    Enum => "Enum"
    Unsigned => "Unsigned"
    Long => "Long"
    Short => "Short"
    Signed => "Signed"
    Const => "Const"
    Extern => "Extern"
    Float => "Float"
    Double => "Double"
    Volatile => "Volatile"
    Inline => "Inline"
    Restrict => "Restrict"
    Register => "Register"
    Auto => "Auto"
    Bool => "Bool"
    Goto => "Goto"
    Null => "Null"
    Class => "Class"
    Public => "Public"
    Private => "Private"
    Protected => "Protected"
    This => "This"
    Using => "Using"
    Namespace => "Namespace"
    Virtual => "Virtual"
    Override => "Override"
    Explicit => "Explicit"
    Friend => "Friend"
    Template => "Template"
    Typename => "Typename"
    StaticCast => "StaticCast"
    ConstCast => "ConstCast"
    ReinterpretCast => "ReinterpretCast"
    New => "New"
    Delete => "Delete"
    ColonColon => "ColonColon"
    ArrowStar => "ArrowStar"
    DotStar => "DotStar"
    Identifier => "Identifier"
    Number => "Number"
    UnsignedLiteral => "UnsignedLiteral"
    FloatLiteral => "FloatLiteral"
    LongLiteral => "LongLiteral"
    CharLiteral => "CharLiteral"
    String => "String"
    Plus => "Plus"
    Minus => "Minus"
    Star => "Star"
    Slash => "Slash"
    Percent => "Percent"
    Eq => "Eq"
    Ne => "Ne"
    Lt => "Lt"
    Le => "Le"
    Gt => "Gt"
    Ge => "Ge"
    AndAnd => "AndAnd"
    OrOr => "OrOr"
    Not => "Not"
    Assign => "Assign"
    PlusAssign => "PlusAssign"
    MinusAssign => "MinusAssign"
    StarAssign => "StarAssign"
    SlashAssign => "SlashAssign"
    PercentAssign => "PercentAssign"
    AndAssign => "AndAssign"
    OrAssign => "OrAssign"
    XorAssign => "XorAssign"
    ShlAssign => "ShlAssign"
    ShrAssign => "ShrAssign"
    Ampersand => "Ampersand"
    BitOr => "BitOr"
    BitXor => "BitXor"
    BitNot => "BitNot"
    Shl => "Shl"
    Shr => "Shr"
    Increment => "Increment"
    Decrement => "Decrement"
    Semicolon => "Semicolon"
    Comma => "Comma"
    LParen => "LParen"
    RParen => "RParen"
    LBrace => "LBrace"
    RBrace => "RBrace"
    LBracket => "LBracket"
    RBracket => "RBracket"
    Dot => "Dot"
    Ellipsis => "Ellipsis"
    Arrow => "Arrow"
    Colon => "Colon"
    Question => "Question"
    Eof => "Eof"
    Unknown => "Unknown"
    Hash => "Hash"
    HashHash => "HashHash"
  }
}
