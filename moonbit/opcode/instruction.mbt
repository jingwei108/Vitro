///|
/// 字节码指令（L0：op + operand + loc 三元组，照搬 Rust oracle
/// `vitro_runtime/src/instruction.rs`）。
///
/// operand 为单 i32 槽位：常量池索引 / 跳转目标 / 立即值，语义随 op 而定
/// （语义校验待 S5 发射侧实证后落地，见 opcode.mbt 包注释）。
///
/// derive 能力扩张登记（审阅 P3）：Rust 侧 Instruction 仅 Debug/Clone/Copy/
/// serde，本侧多 derive(Eq)（Map 键去重用法所需）——值语义惯例扩张，非
/// 行为分歧；序列化仍归上层。
pub(all) struct Instruction {
  op : OpCode
  operand : Int
  loc : @source.SourceLoc
} derive(Debug, Eq)

///|
/// 构造指令（与 Rust `Instruction::new` 同形）。
///
/// ```mbt check
/// test {
///   let ins = Instruction::new(Nop, 0, @source.SourceLoc::new(1, 1))
///   inspect(ins.op.code(), content="0")
/// }
/// ```
pub fn Instruction::new(
  op : OpCode,
  operand : Int,
  loc : @source.SourceLoc,
) -> Instruction {
  { op, operand, loc, }
}
