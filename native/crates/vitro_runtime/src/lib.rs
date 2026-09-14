//! Vitro 运行时共享数据结构。
//!
//! 包含 VM 与会话（Session）共享的状态类型：运行时状态、内存状态、符号表、
//! 函数元数据等。该 crate 依赖 `vitro_ast` 与 `vitro_shared`，供 `vitro_vm` 与
//! `vitro_native` 共同使用，避免 `vm/` 与 `session.rs` 之间的循环依赖。
//!
//! 注意：本 crate 中不带 `Data` 后缀的类型用于 VM 内部；带 `Data` 后缀的类型
//! 是不含 `#[frb]` 的基础数据结构，`vitro_native` 会在此基础上定义带 `#[frb]`
//! 的 Dart 绑定包装类型。

pub mod bytecode_libc_index;
pub mod bytecode_libc_sig;
pub mod func_meta;
pub mod host_func_id;
pub mod instruction;
pub mod memory_state;
pub mod opcode;
pub mod output;
pub mod runtime_state;
pub mod symbol;
pub mod type_utils;
pub mod unified_types;

pub use func_meta::{FuncMeta, LocalBuffer};
pub use memory_state::{
    align4, argv_region_footprint, build_heap_stats, compute_heap_base, find_region_by_addr, fragmentation_rate,
    FreeBlock, HeapStatsData, MemoryFragmentData, MemoryRegionData, MemoryState, DEFAULT_QUARANTINE_BUDGET,
    GLOBAL_REGION_LIMIT, GLOBAL_START, HEAP_START, MAX_STACK_DEPTH, MEM_SIZE, NULL_TRAP_SIZE, SNAPSHOT_INTERVAL,
    STACK_START,
};
pub use output::{OutputChunk, OutputKind, OutputLog, OUTPUT_LOG_BUDGET};
pub use runtime_state::{
    ExecutionHeatmap, InputMode, RuntimeState, TraceEntryData, VariableSnapshotData, VisEventData,
    TRACE_LIMIT,
};
pub use symbol::Symbol;
pub use type_utils::{base_kind, immediate_base_kind, type_display_name};
pub use unified_types::{AccessedVarData, ArraySnapshotData, PointerSnapshotData, PointerStatusData, MAX_ARRAY_SNAPSHOT_ELEMENTS};

/// 编译单元：一次编译的单个源文件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompileUnit {
    pub filename: String,
    pub source: String,
}

/// 代码文件：与 `CompileUnit` 语义相近，用于前端传递。
#[derive(Debug, Clone)]
pub struct CodeFile {
    pub filename: String,
    pub source: String,
}
