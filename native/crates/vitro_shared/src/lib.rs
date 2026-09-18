//! Vitro 最基础共享类型与工具。
//!
//! 该 crate 不依赖任何其他 vitro_* crate，用于打破底层循环依赖。

pub mod cstring;
pub mod error_codes;
pub mod source_loc;

pub use cstring::{cstring_bytes, cstring_len};
pub use error_codes::ErrorCode;
pub use source_loc::SourceLoc;
