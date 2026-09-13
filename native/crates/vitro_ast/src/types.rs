//! AST 类型系统定义。

use std::fmt::Write;

use super::expr::Expr;

/// 类型种类枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TypeKind {
    #[default]
    Void,
    Int,
    Char,
    Float,
    Double,
    LongLong,
    Pointer,
    Array,
    Struct,
    Union,
    Function,
    // === C++ 新增 ===
    Class,
    Reference,
    RValueRef,
    Auto,
    TemplateId,
}

/// 类型节点。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Type {
    Void {
        is_const: bool,
    },
    Int {
        is_unsigned: bool,
        is_const: bool,
    },
    Char {
        is_unsigned: bool,
        is_const: bool,
    },
    Float {
        is_const: bool,
    },
    Double {
        is_const: bool,
    },
    LongLong {
        is_unsigned: bool,
        is_const: bool,
    },
    Pointer {
        pointee: Box<Type>,
        is_const: bool,
    },
    Array {
        element: Box<Type>,
        array_size: i32,
        dims: Vec<i32>,
        is_const: bool,
        is_vla: bool,
        vla_dims: Vec<Box<Expr>>,
    },
    Function {
        return_type: Box<Type>,
        param_types: Vec<Type>,
        is_const: bool,
        is_variadic: bool,
    },
    Struct {
        name: String,
        is_const: bool,
    },
    Union {
        name: String,
        is_const: bool,
    },
    // === C++ 新增 ===
    Class {
        name: String,
        is_const: bool,
    },
    Reference {
        base: Box<Type>,
        is_const: bool,
    },
    RValueRef {
        base: Box<Type>,
    },
    Auto,
    TemplateId {
        base: String,
        args: Vec<super::decl::TemplateArg>,
        is_const: bool,
    },
    Typeof {
        expr: Box<Expr>,
        is_const: bool,
        /// C23 typeof_unqual（E1）：推导结果需剥离顶层限定符（const 等）
        unqual: bool,
    },
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Type::Void { is_const: a }, Type::Void { is_const: b }) => a == b,
            (Type::Int { is_unsigned: a1, is_const: a2 }, Type::Int { is_unsigned: b1, is_const: b2 }) => {
                a1 == b1 && a2 == b2
            }
            (Type::Char { is_unsigned: a1, is_const: a2 }, Type::Char { is_unsigned: b1, is_const: b2 }) => {
                a1 == b1 && a2 == b2
            }
            (Type::Float { is_const: a }, Type::Float { is_const: b }) => a == b,
            (Type::Double { is_const: a }, Type::Double { is_const: b }) => a == b,
            (Type::LongLong { is_unsigned: a1, is_const: a2 }, Type::LongLong { is_unsigned: b1, is_const: b2 }) => {
                a1 == b1 && a2 == b2
            }
            (Type::Pointer { pointee: a, is_const: a2 }, Type::Pointer { pointee: b, is_const: b2 }) => {
                a == b && a2 == b2
            }
            (
                Type::Array {
                    element: a,
                    array_size: a2,
                    dims: a3,
                    is_const: a4,
                    is_vla: a5,
                    ..
                },
                Type::Array {
                    element: b,
                    array_size: b2,
                    dims: b3,
                    is_const: b4,
                    is_vla: b5,
                    ..
                },
            ) => a == b && a2 == b2 && a3 == b3 && a4 == b4 && a5 == b5,
            (
                Type::Function {
                    return_type: a,
                    param_types: a2,
                    is_const: a3,
                    is_variadic: a4,
                },
                Type::Function {
                    return_type: b,
                    param_types: b2,
                    is_const: b3,
                    is_variadic: b4,
                },
            ) => a == b && a2 == b2 && a3 == b3 && a4 == b4,
            (Type::Struct { name: a, is_const: a2 }, Type::Struct { name: b, is_const: b2 }) => a == b && a2 == b2,
            (Type::Union { name: a, is_const: a2 }, Type::Union { name: b, is_const: b2 }) => a == b && a2 == b2,
            // === C++ 新增 ===
            (Type::Class { name: a, is_const: a2 }, Type::Class { name: b, is_const: b2 }) => a == b && a2 == b2,
            (Type::Reference { base: a, is_const: a2 }, Type::Reference { base: b, is_const: b2 }) => {
                a == b && a2 == b2
            }
            (Type::RValueRef { base: a }, Type::RValueRef { base: b }) => a == b,
            (Type::Auto, Type::Auto) => true,
            (Type::Typeof { .. }, Type::Typeof { .. }) => false,
            (
                Type::TemplateId {
                    base: a,
                    args: a2,
                    is_const: a3,
                },
                Type::TemplateId {
                    base: b,
                    args: b2,
                    is_const: b3,
                },
            ) => a == b && a2 == b2 && a3 == b3,
            _ => false,
        }
    }
}

impl Eq for Type {}

impl Default for Type {
    fn default() -> Self {
        Type::Void { is_const: false }
    }
}

impl Type {
    pub fn int() -> Self {
        Type::Int {
            is_unsigned: false,
            is_const: false,
        }
    }
    pub fn unsigned_int() -> Self {
        Type::Int {
            is_unsigned: true,
            is_const: false,
        }
    }
    pub fn char() -> Self {
        Type::Char {
            is_unsigned: false,
            is_const: false,
        }
    }
    pub fn float() -> Self {
        Type::Float { is_const: false }
    }
    pub fn double() -> Self {
        Type::Double { is_const: false }
    }
    pub fn long_long() -> Self {
        Type::LongLong {
            is_unsigned: false,
            is_const: false,
        }
    }
    pub fn void() -> Self {
        Type::Void { is_const: false }
    }
    pub fn pointer_to(pointee: Type) -> Self {
        Type::Pointer {
            pointee: Box::new(pointee),
            is_const: false,
        }
    }
    pub fn array_of(element: Type, dims: Vec<i32>) -> Self {
        let array_size = if dims.is_empty() {
            0
        } else {
            dims.iter().map(|&d| if d > 0 { d } else { 1 }).product()
        };
        Type::Array {
            element: Box::new(element),
            array_size,
            dims,
            is_const: false,
            is_vla: false,
            vla_dims: vec![],
        }
    }
    pub fn struct_type(name: impl Into<String>) -> Self {
        Type::Struct {
            name: name.into(),
            is_const: false,
        }
    }
    pub fn union_type(name: impl Into<String>) -> Self {
        Type::Union {
            name: name.into(),
            is_const: false,
        }
    }
    pub fn function(return_type: Type, param_types: Vec<Type>) -> Self {
        Type::Function {
            return_type: Box::new(return_type),
            param_types,
            is_const: false,
            is_variadic: false,
        }
    }
    pub fn function_pointer(return_type: Type, param_types: Vec<Type>) -> Self {
        Type::Pointer {
            pointee: Box::new(Type::Function {
                return_type: Box::new(return_type),
                param_types,
                is_const: false,
                is_variadic: false,
            }),
            is_const: false,
        }
    }

    /// Generate a mangled name for this type (used in template instantiation).
    pub fn mangle_name(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.mangle_name_into(&mut buf);
        buf
    }

    /// Append the mangled name for this type into the provided buffer.
    /// Avoids the O(n²) temporary String allocations of the recursive `mangle_name()`.
    pub fn mangle_name_into(&self, buf: &mut String) {
        match self {
            Type::Void { .. } => buf.push_str("void"),
            Type::Int { is_unsigned, .. } => {
                if *is_unsigned {
                    buf.push_str("unsigned_int")
                } else {
                    buf.push_str("int")
                }
            }
            Type::Char { is_unsigned, .. } => {
                if *is_unsigned {
                    buf.push_str("unsigned_char")
                } else {
                    buf.push_str("char")
                }
            }
            Type::Float { .. } => buf.push_str("float"),
            Type::Double { .. } => buf.push_str("double"),
            Type::LongLong { is_unsigned, .. } => {
                if *is_unsigned {
                    buf.push_str("unsigned_long_long")
                } else {
                    buf.push_str("long_long")
                }
            }
            Type::Pointer { pointee, .. } => {
                buf.push_str("p_");
                pointee.mangle_name_into(buf);
            }
            Type::Array { element, dims, .. } => {
                buf.push('a');
                for (i, d) in dims.iter().enumerate() {
                    if i > 0 {
                        buf.push('_');
                    }
                    let _ = write!(buf, "{}", d);
                }
                buf.push('_');
                element.mangle_name_into(buf);
            }
            Type::Function { return_type, param_types, .. } => {
                buf.push_str("fn_");
                return_type.mangle_name_into(buf);
                buf.push('_');
                for (i, pt) in param_types.iter().enumerate() {
                    if i > 0 {
                        buf.push('_');
                    }
                    pt.mangle_name_into(buf);
                }
            }
            Type::Struct { name, .. } => {
                buf.push_str("struct_");
                buf.push_str(name);
            }
            Type::Union { name, .. } => {
                buf.push_str("union_");
                buf.push_str(name);
            }
            Type::Class { name, .. } => {
                buf.push_str("class_");
                buf.push_str(name);
            }
            Type::Reference { base, is_const } => {
                if *is_const {
                    buf.push_str("const_ref_")
                } else {
                    buf.push_str("ref_")
                }
                base.mangle_name_into(buf);
            }
            Type::RValueRef { base } => {
                buf.push_str("rref_");
                base.mangle_name_into(buf);
            }
            Type::Auto => buf.push_str("auto"),
            Type::Typeof { expr, .. } => {
                // write! to an in-memory String is infallible.
                let _ = write!(buf, "typeof({:?})", expr);
            }
            Type::TemplateId { base, args, .. } => {
                buf.push_str(base);
                buf.push_str("__");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str("__");
                    }
                    a.mangle_name_into(buf);
                }
            }
        }
    }

    // 兼容访问器
    pub fn kind(&self) -> TypeKind {
        match self {
            Type::Void { .. } => TypeKind::Void,
            Type::Int { .. } => TypeKind::Int,
            Type::Char { .. } => TypeKind::Char,
            Type::Float { .. } => TypeKind::Float,
            Type::Double { .. } => TypeKind::Double,
            Type::LongLong { .. } => TypeKind::LongLong,
            Type::Pointer { .. } => TypeKind::Pointer,
            Type::Array { .. } => TypeKind::Array,
            Type::Function { .. } => TypeKind::Function,
            Type::Struct { .. } => TypeKind::Struct,
            Type::Union { .. } => TypeKind::Union,
            // === C++ 新增 ===
            Type::Class { .. } => TypeKind::Class,
            Type::Reference { .. } => TypeKind::Reference,
            Type::RValueRef { .. } => TypeKind::RValueRef,
            Type::Auto => TypeKind::Auto,
            Type::Typeof { .. } => TypeKind::Auto,
            Type::TemplateId { .. } => TypeKind::TemplateId,
        }
    }

    /// 返回类型的核心名称。对 Struct/Union 返回原始名称；对 Pointer/Array 递归返回；
    /// 对基础类型返回关键字。返回值的生命周期与 self 绑定。
    pub fn name(&self) -> &str {
        match self {
            Type::Struct { name, .. } | Type::Union { name, .. } => name.as_str(),
            Type::Pointer { pointee, .. } => pointee.name(),
            Type::Array { element, .. } => element.name(),
            Type::Void { .. } => "void",
            Type::Int { .. } => "int",
            Type::Char { .. } => "char",
            Type::Float { .. } => "float",
            Type::Double { .. } => "double",
            Type::LongLong { .. } => "long long",
            Type::Function { .. } => "fn",
            // === C++ 新增 ===
            Type::Class { name, .. } => name.as_str(),
            Type::Reference { base, .. } => base.name(),
            Type::RValueRef { base, .. } => base.name(),
            Type::Auto => "auto",
            Type::Typeof { .. } => "typeof",
            Type::TemplateId { base, .. } => base.as_str(),
        }
    }

    pub fn array_size(&self) -> i32 {
        match self {
            Type::Array { array_size, .. } => *array_size,
            _ => 0,
        }
    }

    pub fn dims(&self) -> &[i32] {
        match self {
            Type::Array { dims, .. } => dims.as_slice(),
            _ => &[],
        }
    }

    pub fn is_vla(&self) -> bool {
        match self {
            Type::Array { is_vla, .. } => *is_vla,
            _ => false,
        }
    }

    pub fn is_unsigned(&self) -> bool {
        match self {
            Type::Int { is_unsigned, .. } | Type::Char { is_unsigned, .. } | Type::LongLong { is_unsigned, .. } => {
                *is_unsigned
            }
            _ => false,
        }
    }

    pub fn is_const(&self) -> bool {
        match self {
            Type::Void { is_const } => *is_const,
            Type::Int { is_const, .. } => *is_const,
            Type::Char { is_const, .. } => *is_const,
            Type::Float { is_const } => *is_const,
            Type::Double { is_const } => *is_const,
            Type::LongLong { is_const, .. } => *is_const,
            Type::Pointer { is_const, .. } => *is_const,
            Type::Array { is_const, .. } => *is_const,
            Type::Function { is_const, .. } => *is_const,
            Type::Struct { is_const, .. } => *is_const,
            Type::Union { is_const, .. } => *is_const,
            // === C++ 新增 ===
            Type::Class { is_const, .. } => *is_const,
            Type::Reference { is_const, .. } => *is_const,
            Type::RValueRef { .. } => false,
            Type::Auto => false,
            Type::Typeof { is_const, .. } => *is_const,
            Type::TemplateId { is_const, .. } => *is_const,
        }
    }

    pub fn set_const(&mut self, value: bool) {
        match self {
            Type::Void { is_const } => *is_const = value,
            Type::Int { is_const, .. } => *is_const = value,
            Type::Char { is_const, .. } => *is_const = value,
            Type::Float { is_const } => *is_const = value,
            Type::Double { is_const } => *is_const = value,
            Type::LongLong { is_const, .. } => *is_const = value,
            Type::Pointer { is_const, .. } => *is_const = value,
            Type::Array { is_const, .. } => *is_const = value,
            Type::Function { is_const, .. } => *is_const = value,
            Type::Struct { is_const, .. } => *is_const = value,
            Type::Union { is_const, .. } => *is_const = value,
            // === C++ 新增 ===
            Type::Class { is_const, .. } => *is_const = value,
            Type::Reference { is_const, .. } => *is_const = value,
            _ => {}
        }
    }

    pub fn is_scalar(&self) -> bool {
        matches!(
            self.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        )
    }
    pub fn is_pointer(&self) -> bool {
        matches!(self.kind(), TypeKind::Pointer)
    }
    pub fn is_function_pointer(&self) -> bool {
        matches!(self, Type::Pointer { pointee, .. } if matches!(pointee.as_ref(), Type::Function { .. }))
    }
    pub fn is_array(&self) -> bool {
        matches!(self.kind(), TypeKind::Array)
    }
    pub fn is_struct(&self) -> bool {
        matches!(self.kind(), TypeKind::Struct)
    }
    pub fn is_union(&self) -> bool {
        matches!(self.kind(), TypeKind::Union)
    }
    pub fn is_class(&self) -> bool {
        matches!(self.kind(), TypeKind::Class)
    }
    pub fn is_void(&self) -> bool {
        matches!(self.kind(), TypeKind::Void)
    }
    pub fn is_auto(&self) -> bool {
        matches!(self, Type::Auto)
    }
    pub fn is_reference(&self) -> bool {
        matches!(self, Type::Reference { .. })
    }
    pub fn is_const_reference(&self) -> bool {
        matches!(self, Type::Reference { is_const: true, .. })
    }
    pub fn is_rvalue_ref(&self) -> bool {
        matches!(self, Type::RValueRef { .. })
    }
    pub fn reference_base(&self) -> Option<&Type> {
        match self {
            Type::Reference { base, .. } | Type::RValueRef { base, .. } => Some(base),
            Type::Typeof { .. } => None,
            _ => None,
        }
    }

    /// 递归获取数组的最内层元素类型。对非数组类型返回自身克隆。
    pub fn innermost_element_type(&self) -> Self {
        match self {
            Type::Array { element, .. } => element.innermost_element_type(),
            _ => self.clone(),
        }
    }

    pub fn total_elements(&self) -> i32 {
        if !self.is_array() {
            return 1;
        }
        let dims = self.dims();
        if !dims.is_empty() {
            let has_negative = dims.iter().any(|&d| d < 0);
            if has_negative && self.array_size() > 0 {
                return self.array_size();
            }
            dims.iter().map(|&d| if d > 0 { d } else { 1 }).product()
        } else if self.array_size() > 0 {
            self.array_size()
        } else {
            1
        }
    }

    pub fn subscript_type(&self) -> Self {
        if !self.is_array() {
            return self.clone();
        }
        match self {
            Type::Array {
                element,
                dims,
                is_const,
                is_vla,
                vla_dims,
                ..
            } => {
                if dims.len() <= 1 {
                    *element.clone()
                } else {
                    let mut new_dims = dims.clone();
                    new_dims.remove(0);
                    let new_array_size = new_dims.iter().map(|&d| if d > 0 { d } else { 1 }).product();
                    let mut new_vla_dims = vla_dims.clone();
                    if !new_vla_dims.is_empty() {
                        new_vla_dims.remove(0);
                    }
                    Type::Array {
                        element: element.clone(),
                        array_size: new_array_size,
                        dims: new_dims,
                        is_const: *is_const,
                        is_vla: *is_vla,
                        vla_dims: new_vla_dims,
                    }
                }
            }
            _ => self.clone(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Void { .. } => write!(f, "void"),
            Type::Int { .. } => write!(f, "int"),
            Type::Char { .. } => write!(f, "char"),
            Type::Float { .. } => write!(f, "float"),
            Type::Double { .. } => write!(f, "double"),
            Type::LongLong { .. } => write!(f, "long long"),
            Type::Struct { name, .. } => write!(f, "struct {}", name),
            Type::Union { name, .. } => write!(f, "union {}", name),
            Type::Pointer { pointee, .. } => write!(f, "{}*", pointee),
            Type::Array {
                element,
                dims,
                array_size,
                is_vla,
                ..
            } => {
                write!(f, "{}", element)?;
                if *is_vla {
                    for _ in dims {
                        write!(f, "[*]")?;
                    }
                    Ok(())
                } else if !dims.is_empty() {
                    for d in dims {
                        write!(f, "[{}]", d)?;
                    }
                    Ok(())
                } else if *array_size > 0 {
                    write!(f, "[{}]", array_size)
                } else {
                    write!(f, "[]")
                }
            }
            Type::Function { return_type, param_types, .. } => {
                write!(f, "{} (*)(", return_type)?;
                for (i, p) in param_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ")")
            }
            // === C++ 新增 ===
            Type::Class { name, .. } => write!(f, "class {}", name),
            Type::Reference { base, is_const } => {
                if *is_const {
                    write!(f, "const ")?;
                }
                write!(f, "{}&", base)
            }
            Type::RValueRef { base } => write!(f, "{}&&", base),
            Type::Auto => write!(f, "auto"),
            Type::Typeof { expr, .. } => write!(f, "typeof({:?})", expr),
            Type::TemplateId { base, args, .. } => {
                write!(f, "{}<", base)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ">")
            }
        }
    }
}

// Type 的 serde 由 #[derive(Serialize, Deserialize)] 自动生成嵌套 JSON 格式。
// 本项目处于开发期，无需兼容旧 flat 格式。
