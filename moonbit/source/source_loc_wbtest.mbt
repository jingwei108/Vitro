///|
/// 契约 §1：column 主坐标 = 行内 UTF-8 字节偏移 + 1（1-based 字节列）。
/// 三字段构造与 file_id 语义。
test "sourceloc_fields_and_file_id" {
  let loc = SourceLoc::new(3, 17)
  inspect(loc.line, content="3")
  inspect(loc.column, content="17")
  inspect(loc.file_id, content="0")
  let ext = SourceLoc::with_file(1, 6, 2)
  assert_false(ext.is_user_file())
  assert_true(ext.file_id == 2)
}

///|
/// 契约：显示形式 `line:column`（与 Rust oracle 诊断前缀同形）。
test "sourceloc_display" {
  inspect(SourceLoc::new(2, 13).display(), content="2:13")
  inspect(SourceLoc::with_file(1, 6, 9).display(), content="1:6")
}

///|
/// Ord 全序：line 主序 → column 次序 → file_id 末序（诊断排序的依据）。
test "sourceloc_ord_line_then_column_then_file" {
  let a = SourceLoc::with_file(1, 5, 0)
  let b = SourceLoc::new(1, 10)
  let c = SourceLoc::new(2, 1)
  let d = SourceLoc::with_file(1, 5, 3)
  assert_true(a < b)
  assert_true(b < c)
  assert_true(a < d) // 同 line/column，file_id 0（用户文件）排在 3（外部）前
}

///|
/// Hash：可作 Map 键（去重诊断位置的标准用法）。
test "sourceloc_hash_map_key" {
  let seen : Map[SourceLoc, Int] = Map([])
  let k1 = SourceLoc::new(1, 5)
  let k2 = SourceLoc::new(1, 5)
  seen[k1] = 1
  seen[k2] = 2 // 相等键覆盖
  inspect(seen.length(), content="1")
  inspect(seen[k1], content="2")
}

///|
/// Default：零值 0:0:0（file_id=0）——与 Rust `#[derive(Default)]` **同形**
/// （审阅 P3 勘误：原注释自称"不同/不复制"系旧意图残留，断言值即 0/0/0）。
/// 0 行 0 列不是合法 1-based 坐标——零值仅作结构初始化默认，消费方不得
/// 将其当作有效位置使用。
test "sourceloc_default_is_zero" {
  let d : SourceLoc = Default::default()
  inspect(d.line, content="0")
  inspect(d.column, content="0")
  inspect(d.file_id, content="0")
}

///|
/// 契约 §2 第 2 条核心：三种量纲在同一 byte_off 上的列值分歧。
/// "😀中"（行首）：emoji 4 字节 / 1 scalar / 2 UTF-16 unit，"中" 3 字节。
/// byte_off=4 指向"中"：scalar 列 = 2，UTF-16 列 = 3，字节列 = 5。
test "pos_three_units_diverge_on_emoji" {
  // 😀 = F0 9F 98 80，中 = E4 B8 AD
  let line : Bytes = [0xf0, 0x9f, 0x98, 0x80, 0xe4, 0xb8, 0xad]
  let pos = Pos::from_byte_off(line, 0, 4)
  inspect(pos.byte_off, content="4")
  inspect(pos.col_scalar, content="2")
  inspect(pos.col_utf16, content="3")
}

///|
/// 纯 ASCII：三种量纲同值（Rust oracle ASCII 全对的等价面）。
test "pos_ascii_all_units_agree" {
  let line : Bytes = [0x69, 0x6e, 0x74, 0x20, 0x61]
  let pos = Pos::from_byte_off(line, 0, 4)
  inspect(pos.col_scalar, content="5")
  inspect(pos.col_utf16, content="5")
}

///|
/// line_start_off 非零（多行文件）：rel = byte_off − line_start_off。
test "pos_with_nonzero_line_start" {
  let line : Bytes = [0x20, 0x20, 0x2a] // "  *"
  let pos = Pos::from_byte_off(line, 100, 103)
  inspect(pos.col_scalar, content="4")
  inspect(pos.col_utf16, content="4")
}

///|
/// byte_off 指向行首：列恒为 1（三量纲一致）。
test "pos_line_start_column_one" {
  let line : Bytes = [0xe4, 0xb8, 0xad]
  let pos = Pos::from_byte_off(line, 0, 0)
  inspect(pos.col_scalar, content="1")
  inspect(pos.col_utf16, content="1")
}
