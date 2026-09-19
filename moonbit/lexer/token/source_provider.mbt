///|
/// 源码读取错误。**显式区分**"文件不存在"与"宿主不支持"（勘察 H-5 修复：
/// Rust 侧 `std::fs::read_to_string(...).ok()` 在 wasm 下静默吞错，自定义
/// include 整体失效且零诊断——本侧 checked error 强制两条路径都可诊断）。
pub(all) suberror SourceError {
  NotFound(path~ : String)
  UnsupportedHost(path~ : String, reason~ : String)
}

///|
/// 宿主源码提供者（勘察 ⑤-3：单出口 wasm-gc 无文件系统，include 能力经本
/// trait 注入；不可用能力必须显式报错，不得静默降级）。
///
/// 实现（internal/host）：`StubsProvider`（默认——仅内嵌标准库存根，自定义
/// include 显式报 `UnsupportedHost`）；`VfsProvider`（内存文件表——差分驱动
/// 与上层宿主注入真实文件内容用）。
pub(open) trait SourceProvider {
  /// 按路径读取源文本。路径为"包含者目录相对路径"解析后的候选绝对形态
  ///（由 pp/resolver 拼好），provider 只负责查表。
  fn read(Self, path : String) -> String raise SourceError
}
