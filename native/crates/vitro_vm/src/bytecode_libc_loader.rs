//! Bytecode Libc 运行时加载器
//!
//! 加载构建期预编译的 Bytecode Libc 产物（`bytecode_libc_data.json`），
//! 供 `setup_vm` 在 VM 初始化时拼接代码和函数表。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::instruction::Instruction;
use vitro_runtime::FuncMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeLibcArtifact {
    pub version: u32,
    pub code_len: usize,
    pub code: Vec<Instruction>,
    pub func_table: HashMap<String, FuncMeta>,
    pub func_index: HashMap<String, i32>,
    pub globals_init_32: Vec<(u32, i32)>,
    pub globals_init_64: Vec<(u32, u64)>,
    pub string_data: Vec<(u32, String)>,
    pub f64_constants: Vec<f64>,
    pub i64_constants: Vec<i64>,
    pub globals_size: u32,
}

/// 加载预编译的 Bytecode Libc 产物。
///
/// 产物由 `scripts/precompile_bytecode_libc`（Go，2026-09-18 起；原 `.py` 同日退役）在构建期生成，
/// 并提交到版本控制。日常构建直接嵌入，无需重新编译。
/// Bytecode Libc 预编译产物的文件标识。
///
/// 加载后所有 libc 指令的 `SourceLoc.file_id` 会被置为该值，
/// 以便 VM 在执行热力图中区分用户代码与标准库代码。
pub const BYTECODE_LIBC_FILE_ID: i32 = 1;

pub fn load_artifact() -> BytecodeLibcArtifact {
    let json = include_str!("bytecode_libc_data.json");
    let mut artifact: BytecodeLibcArtifact = match serde_json::from_str(json) {
        Ok(artifact) => artifact,
        Err(e) => panic!(
            "Failed to parse bytecode_libc_data.json: {}. \
             Run: go run ./scripts/precompile_bytecode_libc",
            e
        ),
    };

    // 将 libc 指令标记为外部文件，避免污染用户代码覆盖率统计。
    for inst in &mut artifact.code {
        inst.loc.file_id = BYTECODE_LIBC_FILE_ID;
    }

    artifact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_artifact() {
        let artifact = load_artifact();
        assert_eq!(artifact.version, 1);
        assert!(!artifact.code.is_empty());
        assert!(!artifact.func_index.is_empty());
    }
}
