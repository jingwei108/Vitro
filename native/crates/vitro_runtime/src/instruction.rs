use super::opcode::OpCode;

pub use vitro_shared::source_loc::SourceLoc;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Instruction {
    pub op: OpCode,
    pub operand: i32,
    pub loc: SourceLoc,
}

impl Instruction {
    pub fn new(op: OpCode, operand: i32, loc: SourceLoc) -> Self {
        Self { op, operand, loc }
    }
}
