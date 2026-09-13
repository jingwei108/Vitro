//! C++ 模板单态化与内置容器合成。
//!
//! 将原 `cpp_monomorph.rs` 按职责拆分为子模块：
//! - `func.rs`：函数模板单态化
//! - `class.rs`：类模板单态化
//! - `replace.rs`：模板参数类型替换（类型 / 语句 / 表达式）
//! - `synth.rs`：合成 AST 小工具
//! - `builtin.rs`：内置容器（`vitro_vec<T>` / `vitro_list<T>`）对类类型实参的合成实例化

pub(crate) use super::*;

mod builtin;
mod builtin_list;
mod builtin_vec;
mod class;
mod func;
mod replace;
mod synth;
