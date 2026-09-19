///|
/// 双坐标预留结构（契约 §2 第 2 条）：全文件字节偏移 + 两种列量纲。
///
/// - `byte_off`：全文件 UTF-8 字节偏移（0-based）——主坐标，引擎内部
///   计算与寻址的唯一量纲；
/// - `col_scalar`：行内 Unicode scalar 字符列（1-based）——Rust oracle
///   词法路径现状语义；
/// - `col_utf16`：行内 UTF-16 code unit 列（1-based）——JS 生态列语义
///   （前端 / 协议消费方）。
///
/// 三值同源派生：任一列值可由 `byte_off` + 行首信息重算（见
/// [Pos::from_byte_off]）。禁止互相直接换算（量纲混算禁令的落点）。
pub(all) struct Pos {
  byte_off : Int
  col_scalar : Int
  col_utf16 : Int
} derive(Debug, Eq)

///|
/// 从行内容与全文件偏移派生三坐标。
///
/// `line_bytes` 为该行起点到目标位置（不含）的 UTF-8 字节序列；
/// `line_start_off` 为该行首字节的全文件偏移。起始列取进入扫描前
/// 的快照（禁"扫描后计数 − 长度"回推，契约 §2 第 3 条）。
///
/// ```mbt check
/// test {
///   // "int 中"：4 个 ASCII 后跟 3 字节汉字。byte_off=4 指向"中"首字节，
///   // 此前 4 个 scalar / 4 个 UTF-16 unit → 两列同为 5。
///   let line : Bytes = [0x69, 0x6e, 0x74, 0x20, 0xe4, 0xb8, 0xad]
///   let pos = Pos::from_byte_off(line, 0, 4)
///   inspect(pos.col_scalar, content="5")
///   inspect(pos.col_utf16, content="5")
/// }
/// ```
pub fn Pos::from_byte_off(
  line_bytes : Bytes,
  line_start_off : Int,
  byte_off : Int,
) -> Pos {
  let rel = byte_off - line_start_off
  // 逐字节解码 UTF-8 统计 scalar 数与 UTF-16 code unit 数：
  // 前导字节决定本码点宽度；> 0xFFFF 的码点占 2 个 UTF-16 unit（代理对）。
  let mut scalar = 0
  let mut utf16 = 0
  let mut i = 0
  let len = line_bytes.length()
  while i < rel && i < len {
    let b = line_bytes[i].to_int()
    let width = if b < 0x80 {
      1
    } else if b < 0xE0 {
      2
    } else if b < 0xF0 {
      3
    } else {
      4
    }
    scalar += 1
    utf16 += if width == 4 { 2 } else { 1 }
    i += width
  }
  { byte_off, col_scalar: scalar + 1, col_utf16: utf16 + 1, }
}
