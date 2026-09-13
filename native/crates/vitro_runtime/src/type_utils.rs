use vitro_ast::{Type, TypeKind};

/// 解引用一层指针/数组，得到其元素（或指向）的标量类型。
pub fn base_kind(ty: &Type) -> TypeKind {
    match ty {
        Type::Pointer { pointee, .. } => pointee.kind(),
        Type::Array { element, .. } => base_kind(element),
        _ => ty.kind(),
    }
}

/// 只解引用一层，用于数组元素类型判断和单级指针解引用。
pub fn immediate_base_kind(ty: &Type) -> TypeKind {
    match ty {
        Type::Pointer { pointee, .. } => pointee.kind(),
        Type::Array { element, .. } => element.kind(),
        _ => ty.kind(),
    }
}

/// 生成 C/C++ 风格的可读类型名（教学输出 / 第三方消费方使用）。
///
/// 此前 `StepPayload.local_vars[].ty_name` 直接 `format!("{:?}", ty)`，把引擎内部枚举
/// 结构泄漏到用户可见输出：`Int { is_unsigned: false, is_const: false }`（P2-7c）。
/// 本函数是类型名的**单一来源**，例：
/// `int` / `unsigned int` / `const char*` / `int[5]` / `int[3][4]` / `struct Node` /
/// `Foo`（C++ 类）/ `int&` / `int (*)(int, ...)`。
pub fn type_display_name(ty: &Type) -> String {
    fn cst(is_const: bool) -> &'static str {
        if is_const {
            "const "
        } else {
            ""
        }
    }
    match ty {
        Type::Void { is_const } => format!("{}void", cst(*is_const)),
        Type::Int { is_unsigned, is_const } => format!(
            "{}{}",
            cst(*is_const),
            if *is_unsigned { "unsigned int" } else { "int" }
        ),
        Type::Char { is_unsigned, is_const } => format!(
            "{}{}",
            cst(*is_const),
            if *is_unsigned { "unsigned char" } else { "char" }
        ),
        Type::Float { is_const } => format!("{}float", cst(*is_const)),
        Type::Double { is_const } => format!("{}double", cst(*is_const)),
        Type::LongLong { is_unsigned, is_const } => format!(
            "{}{}",
            cst(*is_const),
            if *is_unsigned {
                "unsigned long long"
            } else {
                "long long"
            }
        ),
        Type::Pointer { pointee, is_const } => {
            if *is_const {
                format!("{}* const", type_display_name(pointee))
            } else {
                format!("{}*", type_display_name(pointee))
            }
        }
        Type::Array {
            element,
            array_size,
            dims,
            is_vla,
            ..
        } => {
            let base = type_display_name(element);
            if !dims.is_empty() && dims.iter().all(|d| *d > 0) {
                let mut s = base;
                for d in dims {
                    s.push_str(&format!("[{}]", d));
                }
                s
            } else if *is_vla || *array_size <= 0 {
                format!("{}[]", base)
            } else {
                format!("{}[{}]", base, array_size)
            }
        }
        Type::Function {
            return_type,
            param_types,
            is_variadic,
            ..
        } => {
            let mut params: Vec<String> = param_types.iter().map(type_display_name).collect();
            if *is_variadic {
                params.push("...".to_string());
            }
            format!("{} (*)({})", type_display_name(return_type), params.join(", "))
        }
        Type::Struct { name, is_const } => format!("{}struct {}", cst(*is_const), name),
        Type::Union { name, is_const } => format!("{}union {}", cst(*is_const), name),
        Type::Class { name, is_const } => format!("{}{}", cst(*is_const), name),
        Type::Reference { base, is_const } => format!("{}{}&", cst(*is_const), type_display_name(base)),
        Type::RValueRef { base } => format!("{}&&", type_display_name(base)),
        Type::Auto => "auto".to_string(),
        Type::TemplateId { base, args, is_const } => {
            if args.is_empty() {
                format!("{}{}", cst(*is_const), base)
            } else {
                let rendered: Vec<String> = args.iter().map(|a| format!("{:?}", a)).collect();
                format!("{}{}<{}>", cst(*is_const), base, rendered.join(", "))
            }
        }
        Type::Typeof { is_const, .. } => format!("{}typeof(...)", cst(*is_const)),
    }
}
