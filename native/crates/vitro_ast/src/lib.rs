//! Vitro AST 节点与类型系统定义。
//!
//! 从 `vitro_native::compiler::ast` 拆分而来，作为独立 crate 供编译器各阶段、VM、诊断等模块共享。

pub mod decl;
pub mod depth;
pub mod expr;
pub mod stmt;
pub mod types;

pub use vitro_shared::source_loc::SourceLoc;

pub use decl::{
    AccessSpec, CaptureMode, ClassDecl, ClassMember, FuncDecl, GlobalDecl, Param, ProgramNode, StructDecl, StructField,
    TemplateArg, TemplateDecl, TemplateInstantiation, TemplateParam, Templateable, VTable,
};
pub use expr::{AssignOp, BinaryOp, Designator, Expr, InitElement, UnaryOp};
pub use stmt::{CatchClause, Stmt};
pub use types::{Type, TypeKind};

use std::collections::HashMap;

/// 获取数组类型的最底层元素类型。
pub fn base_element_type(ty: &Type) -> &Type {
    match ty {
        Type::Array { element, .. } => base_element_type(element),
        _ => ty,
    }
}

/// 根据类型定义计算类型的字节大小。
/// 与 `bytecode_gen::type_size` 和 `compile_pipeline::type_size` 保持同一语义。
pub fn compute_type_size(
    ty: &Type,
    struct_defs: &HashMap<String, Vec<StructField>>,
    union_defs: &HashMap<String, Vec<StructField>>,
    class_size_map: &HashMap<String, i32>,
) -> i32 {
    // T-P0-8：自含/循环包含 struct（如 `struct S { struct S inner; }`）曾在此
    // 无限递归导致编译器栈溢出崩溃。visiting 记录当前展开路径，环出现时返回 0
    // 防崩；真正的诊断由 TypeChecker Pass 1（E3072）给出。
    let mut visiting = std::collections::HashSet::new();
    compute_type_size_impl(ty, struct_defs, union_defs, class_size_map, &mut visiting)
}

fn compute_type_size_impl(
    ty: &Type,
    struct_defs: &HashMap<String, Vec<StructField>>,
    union_defs: &HashMap<String, Vec<StructField>>,
    class_size_map: &HashMap<String, i32>,
    visiting: &mut std::collections::HashSet<String>,
) -> i32 {
    match ty.kind() {
        TypeKind::Void => 0,
        TypeKind::Int => 4,
        TypeKind::Char => 1,
        TypeKind::Float => 4,
        TypeKind::Double | TypeKind::LongLong => 8,
        TypeKind::Pointer | TypeKind::Function => 4,
        TypeKind::Array => {
            if ty.is_vla() {
                // VLA variable itself is stored as a pointer on stack
                return 4;
            }
            let elem_count = ty.total_elements();
            let base_elem = base_element_type(ty);
            let elem_size = compute_type_size_impl(base_elem, struct_defs, union_defs, class_size_map, visiting);
            elem_count * elem_size
        }
        TypeKind::Struct => {
            let name = ty.name().to_string();
            if !visiting.insert(name.clone()) {
                return 0; // 循环包含：防无限递归
            }
            let size = struct_defs
                .get(&name)
                .map(|f| {
                    f.iter()
                        .map(|field| {
                            compute_type_size_impl(&field.ty, struct_defs, union_defs, class_size_map, visiting)
                        })
                        .sum()
                })
                .unwrap_or(0);
            visiting.remove(&name);
            size
        }
        TypeKind::Union => {
            let name = ty.name().to_string();
            if !visiting.insert(name.clone()) {
                return 0; // 循环包含：防无限递归
            }
            let size = union_defs
                .get(&name)
                .map(|f| {
                    f.iter()
                        .map(|field| {
                            compute_type_size_impl(&field.ty, struct_defs, union_defs, class_size_map, visiting)
                        })
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            visiting.remove(&name);
            size
        }
        TypeKind::Class => class_size_map.get(ty.name()).copied().unwrap_or(0),
        TypeKind::Reference | TypeKind::RValueRef => 4, // reference is a pointer under the hood
        TypeKind::Auto => 0,                            // should not appear in codegen before TypeChecker replaces it
        TypeKind::TemplateId => 0,                      // should be resolved to Class before codegen
    }
}
