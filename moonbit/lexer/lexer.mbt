///|
/// facade 再导出：公共类型归属 `lexer/token` 公共包（MoonBit 包组织规则：
/// internal 不持有公共类型、facade 与实现包不环），本 facade 以 `@lexer.X`
/// 形态对外暴露并保留方法归属加载。`SourceProvider`（trait）与 `SourceError`
/// 以 `@token.SourceProvider` / `@token.SourceError` 原路径使用（trait 不参与
/// type re-export）。
pub using @token {
  type Token,
  type TokenType,
  type LexError,
  type LexWarning,
  type MacroDef,
  type TraceEntry,
  type LineMap,
  type LineSpan,
  type RawResult,
  type LexResult,
}

///|
/// L1：字符级原始词法（无行拼接、无指令消费、无宏展开；`#` 产出 Hash/HashHash；
/// BOM 不剥离——与 Rust oracle `tokenize_raw` 出口同构）。
pub fn tokenize_raw(src : String, cpp_mode? : Bool = false) -> @token.RawResult {
  @scanner.scan_raw(@utf8.encode(src), cpp_mode~, file_id=0)
}

///|
/// L2：完整管线（默认仅存根 provider——wasm-gc 单出口形态；自定义 include
/// 显式报 `UnsupportedHost`，见 token 包 SourceProvider 注释）。
pub fn tokenize(src : String, cpp_mode? : Bool = false) -> @token.LexResult {
  let pp_state : @pp.Pp[@host.StubsProvider] = @pp.Pp::new(
    provider=@host.stubs_provider(),
    cpp_mode~,
    base_dir=".",
  )
  let tokens = pp_state.run_pp(src, emit_eof=true)
  pp_state.finish_pp(tokens)
}

///|
/// L2（注入宿主 provider）：差分驱动 / 上层宿主注入真实文件内容用。
/// `base_dir` 为 quote-include 候选链的源文件目录。
pub fn[P : @token.SourceProvider] tokenize_with(
  src : String,
  provider~ : P,
  cpp_mode? : Bool = false,
  base_dir? : String = ".",
) -> @token.LexResult {
  let pp_state : @pp.Pp[P] = @pp.Pp::new(provider~, cpp_mode~, base_dir~)
  let tokens = pp_state.run_pp(src, emit_eof=true)
  pp_state.finish_pp(tokens)
}

///|
/// L2（注入内存文件表）：`files` 为"绝对规范路径 → 内容"映射（差分驱动把
/// 语料目录平铺文件注入，对齐 Rust 真实 fs 的 quote 候选链语义；上层宿主
/// 亦可经此注入任意文件内容——wasm 形态的 include 能力载体）。
pub fn tokenize_with_vfs(
  src : String,
  files~ : Map[String, String],
  cpp_mode? : Bool = false,
  base_dir? : String = ".",
) -> @token.LexResult {
  let provider = @host.vfs_provider(files~)
  let pp_state : @pp.Pp[@host.VfsProvider] = @pp.Pp::new(
    provider~,
    cpp_mode~,
    base_dir~,
  )
  let tokens = pp_state.run_pp(src, emit_eof=true)
  pp_state.finish_pp(tokens)
}
