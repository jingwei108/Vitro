///|
/// 教学卡片条目（L1 感知层：emoji + 标题 + 解释 + 常见原因清单）。
/// 数据由 scripts/gen_diag 从 Rust oracle error_catalog 生成
/// （catalog_gen.mbt），本类型为手写契约。
pub(all) struct CatalogEntry {
  code : Int
  emoji : String
  title : String
  explanation : String
  common_causes : Array[String]
} derive(Debug, Eq)

///|
/// 构造（生成代码专用入口；外部构造走同一契约）。
pub fn CatalogEntry::of(
  code : Int,
  emoji : String,
  title : String,
  explanation : String,
  common_causes : Array[String],
) -> CatalogEntry {
  { code, emoji, title, explanation, common_causes, }
}
