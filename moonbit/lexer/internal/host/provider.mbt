///|
/// 默认 provider：仅内嵌标准库存根（14 个，见 `stubs_gen.mbt`）。
/// 自定义 `#include "..."` 显式报 `UnsupportedHost`——wasm-gc 单出口形态下
/// "自定义 include"能力不存在，宁可诊断也不静默跳过（勘察 H-5 修复语义）。
/// 存根优先于 provider 的 fallback 逻辑在 pp/resolver 层单源实现。
pub enum StubsProvider {
  Stubs
}

///|
pub impl @token.SourceProvider for StubsProvider with fn read(
  self,
  path : String,
) -> String raise @token.SourceError {
  match self {
    Stubs => ()
  }
  match stub_lookup(path) {
    Some(content) => content
    None =>
      raise @token.SourceError::UnsupportedHost(
        path~,
        reason="当前宿主未提供文件系统（仅内嵌标准库存根）；自定义头文件需注入 VfsProvider",
      )
  }
}

///|
/// 内存文件表 provider：差分驱动与上层宿主注入真实文件内容用。
/// 路径匹配为**精确字符串**（由 pp/resolver 拼好规范形态后查询）；
/// 未命中报 `NotFound`（与 Rust E1021 "找不到头文件" 对拍口径一致）。
pub(all) struct VfsProvider {
  files : Map[String, String]
}

///|
pub impl @token.SourceProvider for VfsProvider with fn read(self, path : String) -> String raise @token.SourceError {
  match self.files.get(path) {
    Some(content) => content
    None => raise @token.SourceError::NotFound(path~)
  }
}

///|
/// 构造入口（跨包 struct 构造经函数暴露——字段形态可演化）。
pub fn stubs_provider() -> StubsProvider {
  Stubs
}

///|
pub fn vfs_provider(files~ : Map[String, String]) -> VfsProvider {
  let v : VfsProvider = { files, }
  v
}
