///|
/// 诊断严重级（三档，与 Rust oracle serve 帧 severity 值一致）。
pub(all) enum Severity {
  Error
  Warning
  Hint
} derive(Debug, Eq, Compare)

///|
/// serve 帧 / 显示用小写串（"error" / "warning" / "hint"）。
///
/// ```mbt check
/// test {
///   inspect(Severity::Warning.to_str(), content="warning")
///   inspect(Severity::Hint.to_str(), content="hint")
/// }
/// ```
pub fn Severity::to_str(self : Severity) -> String {
  match self {
    Error => "error"
    Warning => "warning"
    Hint => "hint"
  }
}

///|
/// 显示前缀（P3 段位语义：E / W / H——与 Rust code_prefix 同形单源）。
///
/// ```mbt check
/// test {
///   inspect(Severity::Warning.prefix(), content="W")
/// }
/// ```
pub fn Severity::prefix(self : Severity) -> String {
  match self {
    Error => "E"
    Warning => "W"
    Hint => "H"
  }
}

///|
/// 源语言域（is_cpp_mode 单源的类型载体——总计划 §5；46 处亲证分散）。
/// C = C 共通域（C++ 模式下的 C 子集同属）；Cpp = 仅 C++ 形态。
pub(all) enum SourceLang {
  C
  Cpp
} derive(Debug, Eq, Compare)

///|
/// 显示串（"c" / "cpp"）。
///
/// ```mbt check
/// test {
///   inspect(SourceLang::Cpp.to_str(), content="cpp")
/// }
/// ```
pub fn SourceLang::to_str(self : SourceLang) -> String {
  match self {
    C => "c"
    Cpp => "cpp"
  }
}
