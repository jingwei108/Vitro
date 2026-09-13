//! VitroVM 核心执行引擎
//!
//! 由三个子模块组成：
//! - `state`：VM 状态、调用帧、快照、符号管理。
//! - `memory`：内存安全读写、脏页追踪、变量/数组快照。
//! - `executor`：字节码取指、译码、执行主循环。
//! - `trap`：错误格式化与 trap 处理。

pub use state::{
    AccessType, ArrayConstructionGuard, CallFrame, VitroVM, FreedRegionInfo, StepResult, VariableAccess, GLOBAL_START,
    HEAP_START, MAX_STACK_DEPTH, MEM_SIZE, NULL_TRAP_SIZE, SNAPSHOT_INTERVAL, STACK_START,
};

// Re-export types and constants used by `executor/` submodules via `use super::*`.
pub use crate::context::VmContext;
pub use crate::host_funcs::execute_host_func;
pub use crate::instruction::{Instruction, SourceLoc};
pub use crate::jit_templates::{execute_trace_bulk, CompiledTrace};
pub use crate::jit_trace::{JitStats, TraceRecorder, JIT_THRESHOLD};
pub use crate::opcode::OpCode;
pub use vitro_runtime::VisEventData;
pub use state::{FuncMeta, VMSymbol};
pub use std::sync::Arc;

pub mod executor;
pub mod memory;
pub mod snapshot;
pub mod state;
mod trap;

// executor 子模块内部通过 `use super::*;` 使用上述导出项。
