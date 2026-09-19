///|
/// 字节码指令（L0：op + operand + loc 三元组，照搬 Rust oracle
/// `vitro_runtime/src/instruction.rs`）。
///
/// operand 为单 i32 槽位：常量池索引 / 跳转目标 / 立即值，语义随 op 而定
/// （语义校验待 S5 发射侧实证后落地，见 opcode.mbt 包注释）。
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
