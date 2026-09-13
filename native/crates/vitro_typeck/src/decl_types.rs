//! 类型解析辅助（D16 拆分：自 `decl.rs` 移出）——typeof / auto / 限定符处理。
//!
//! 全部为 `TypeChecker` 的关联函数（无 self），与 `decl.rs` 的声明检查主流程
//! 共生但可独立演进；R4 批次按 <800 非空行规约拆出。

use super::TypeChecker;
use vitro_ast::Type;

impl TypeChecker {
    pub(crate) fn type_has_auto(ty: &Type) -> bool {
        match ty {
            Type::Auto => true,
            Type::Pointer { pointee, .. } => Self::type_has_auto(pointee),
            Type::Reference { base, .. } => Self::type_has_auto(base),
            Type::RValueRef { base, .. } => Self::type_has_auto(base),
            Type::Array { element, .. } => Self::type_has_auto(element),
            _ => false,
        }
    }

    pub(crate) fn type_has_typeof(ty: &Type) -> bool {
        match ty {
            Type::Typeof { .. } => true,
            Type::Pointer { pointee, .. } => Self::type_has_typeof(pointee),
            Type::Reference { base, .. } => Self::type_has_typeof(base),
            Type::RValueRef { base, .. } => Self::type_has_typeof(base),
            Type::Array { element, .. } => Self::type_has_typeof(element),
            _ => false,
        }
    }

    /// C23 typeof_unqual（E1）：剥离**顶层**限定符（const / volatile 语义在
    /// 教学子集中仅建模 is_const）。嵌套层（如指针的 pointee）保持不变——
    /// 这正是 typeof_unqual 与 typeof 的唯一区别。
    pub(crate) fn strip_top_level_qualifiers(ty: &Type) -> Type {
        let mut t = ty.clone();
        t.set_const(false);
        t
    }

    pub(crate) fn resolve_typeof_in_type(ty: &Type, replacement: Type) -> Type {
        match ty {
            Type::Typeof { is_const, unqual, .. } => {
                let mut t = replacement;
                if *unqual {
                    // C23 typeof_unqual（E1）：剥离顶层限定符后再套 is_const
                    t = Self::strip_top_level_qualifiers(&t);
                }
                t.set_const(*is_const);
                t
            }
            Type::Pointer { pointee, is_const } => Type::Pointer {
                pointee: Box::new(Self::resolve_typeof_in_type(pointee, replacement)),
                is_const: *is_const,
            },
            Type::Reference { base, is_const } => Type::Reference {
                base: Box::new(Self::resolve_typeof_in_type(base, replacement)),
                is_const: *is_const,
            },
            Type::RValueRef { base } => Type::RValueRef {
                base: Box::new(Self::resolve_typeof_in_type(base, replacement)),
            },
            Type::Array {
                element,
                array_size,
                dims,
                is_const,
                is_vla,
                vla_dims,
            } => Type::Array {
                element: Box::new(Self::resolve_typeof_in_type(element, replacement)),
                array_size: *array_size,
                dims: dims.clone(),
                is_const: *is_const,
                is_vla: *is_vla,
                vla_dims: vla_dims.clone(),
            },
            _ => ty.clone(),
        }
    }

    pub(crate) fn replace_auto_in_type(ty: &Type, replacement: Type) -> Type {
        match ty {
            Type::Auto => replacement,
            Type::Pointer { pointee, is_const } => Type::Pointer {
                pointee: Box::new(Self::replace_auto_in_type(pointee, replacement)),
                is_const: *is_const,
            },
            Type::Reference { base, is_const } => Type::Reference {
                base: Box::new(Self::replace_auto_in_type(base, replacement)),
                is_const: *is_const,
            },
            Type::RValueRef { base } => Type::RValueRef {
                base: Box::new(Self::replace_auto_in_type(base, replacement)),
            },
            Type::Array {
                element,
                array_size,
                dims,
                is_const,
                is_vla,
                vla_dims,
            } => Type::Array {
                element: Box::new(Self::replace_auto_in_type(element, replacement)),
                array_size: *array_size,
                dims: dims.clone(),
                is_const: *is_const,
                is_vla: *is_vla,
                vla_dims: vla_dims.clone(),
            },
            _ => ty.clone(),
        }
    }
}
