//! C++ 扩展字节码生成子模块
//!
//! 将 BytecodeGen 中与 C++ RAII、类成员、构造函数、RangeFor、变量初始化相关的专属逻辑集中到这里，
//! 降低 `codegen/mod.rs` 与 `codegen/stmt.rs` 的认知负荷并明确 C/C++ 边界。
// TODO(#D10): parser/mod.rs parse_program 中的 class/template 顶层分发继续下沉。

mod member;
mod raii;
mod range_for;
mod var_decl;
