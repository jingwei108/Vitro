//! C++ 扩展类型检查子模块
//!
//! 将 TypeChecker 中与 C++ 类、方法、Lambda 相关的专属逻辑集中到这里，
//! 降低 `typeck/mod.rs` 的认知负荷并明确 C/C++ 边界。
// TODO(#D10): 后续将现有的 cpp_class_layout.rs / cpp_overload.rs / cpp_monomorph.rs /
// cpp_container.rs / cpp_auto.rs 逐步归入本目录，统一 C++ typeck 模块边界。

mod lambda;
mod methods;
