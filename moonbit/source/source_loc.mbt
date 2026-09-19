///|
/// 源码位置（line / column / file_id 三字段）。
///
/// # 列单位契约（S1 T1 冻结，输入 = 《列号口径冻结》§2）
///
/// 1. **主坐标 = 字节列**：`column` 语义为行内 UTF-8 字节偏移 + 1
///    （1-based，与 Rust oracle 现状列显示对齐）。单值、全路径唯一语义
///    （词法 / 解析 / 诊断共用），禁止与字符量纲混算。
/// 2. **双坐标预留**：需要字符列 / UTF-16 列的消费方（前端 / 协议）使用
///    [Pos]（byte_off + col_scalar + col_utf16 三值同源派生），不用本结构
///    的 column 换算——本结构不携带换算所需信息。
/// 3. **禁止"扫描后计数 − 长度"回推**：起始列必须取进入扫描前的快照
///    （Rust oracle `make_token` 量纲混算根因：字符计数 − 字节数）。
/// 4. **词法 +1 现状不复刻**：报错列指向非法字符本体，不继承 Rust oracle
///    "advance 后报错"的 +1 偏移——差分时此类形状列入已登记差异。
///
/// # file_id 语义
///
/// 0 = 用户主文件（默认）；非 0 区分标准库 / 预编译字节码 / 头文件等
/// 外部源码位置，避免外部行号污染覆盖率统计。
pub(all) struct SourceLoc {
  line : Int
  column : Int
  file_id : Int
} derive(Debug, Eq, Compare, Hash, Default) // 序列化归 vitro/engine/protocol（L8）——L0 零依赖不引 JSON

///|
/// 用户主文件（file_id = 0）的源码位置。
///
/// ```mbt check
/// test {
///   let loc = SourceLoc::new(3, 17)
///   inspect(loc.line, content="3")
///   inspect(loc.file_id, content="0")
/// }
/// ```
pub fn SourceLoc::new(line : Int, column : Int) -> SourceLoc {
  { line, column, file_id: 0, }
}

///|
/// 指定文件标识的源码位置。
///
/// ```mbt check
/// test {
///   let loc = SourceLoc::with_file(1, 6, 2)
///   inspect(loc.file_id, content="2")
/// }
/// ```
pub fn SourceLoc::with_file(
  line : Int,
  column : Int,
  file_id : Int,
) -> SourceLoc {
  { line, column, file_id, }
}

///|
/// 位置是否属于用户主文件（file_id = 0）。
///
/// ```mbt check
/// test {
///   assert_true(SourceLoc::new(1, 1).is_user_file())
///   assert_false(SourceLoc::with_file(1, 1, 3).is_user_file())
/// }
/// ```
pub fn SourceLoc::is_user_file(self : SourceLoc) -> Bool {
  self.file_id == 0
}

///|
/// `line:column` 显示形式（不含文件标识；与 Rust oracle 诊断前缀
/// `"{}:{}"` 同形）。
///
/// ```mbt check
/// test {
///   inspect(SourceLoc::new(2, 13).display(), content="2:13")
/// }
/// ```
pub fn SourceLoc::display(self : SourceLoc) -> String {
  "\{self.line}:\{self.column}"
}
