#![forbid(unsafe_code)]

pub mod bytecode_libc_loader;
pub mod context;
pub mod core;
pub use core::VIS_EVENT_QUEUE_LIMIT;
pub use vitro_runtime::MAX_ARRAY_SNAPSHOT_ELEMENTS;
pub mod host_funcs;

pub use vitro_runtime::bytecode_libc_index;
pub use vitro_runtime::bytecode_libc_sig;
pub use vitro_runtime::host_func_id;
pub use vitro_runtime::instruction;
pub use vitro_runtime::opcode;
pub mod jit_templates;
pub mod jit_trace;
pub mod snapshot;
pub mod vfs;

pub use context::VmContext;
