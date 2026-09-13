//! 跨 compiler/session/vm 共享的基础类型与工具函数。
//!
//! 设计原则：
//! - `source_loc` 不依赖任何上层模块，供 `compiler::ast` 和 `vm::instruction` re-export。
//! - `func_meta` / `symbol` / `type_utils` 等运行时共享类型已下沉到 `vitro_runtime` crate，
//!   本模块仅做 re-export 以保持既有路径兼容。

pub use vitro_runtime::{func_meta, symbol, type_utils};

pub use vitro_shared::source_loc::SourceLoc;
pub use func_meta::FuncMeta;
pub use symbol::Symbol;
pub use type_utils::base_kind;
