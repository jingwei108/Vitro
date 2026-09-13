use super::TypeChecker;
use vitro_ast::{Expr, FuncDecl, SourceLoc, Type, TypeKind, UnaryOp};
use vitro_shared::ErrorCode;

/// 根据 (from, to) 类型对判断是否允许隐式转换，并返回转换后的目标类型。
pub(crate) fn implicit_cast_target(from: &Type, to: &Type) -> Option<Type> {
    use TypeKind::*;
    // Reference types do not participate in implicit scalar conversions
    if matches!(from.kind(), Reference | RValueRef) || matches!(to.kind(), Reference | RValueRef) {
        return None;
    }
    match (from.kind(), to.kind()) {
        (Int | Char | Float | LongLong, Double) => Some(Type::double()),
        (Double, Int) => Some(Type::Int {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (Double, Char) => Some(Type::char()),
        (Double, Float) => Some(Type::float()),
        (Double, LongLong) => Some(Type::LongLong {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (Int | Char | LongLong, Float) => Some(Type::float()),
        (Float, Int) => Some(Type::Int {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (Float, Char) => Some(Type::char()),
        (Float, LongLong) => Some(Type::LongLong {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (Int | Char, LongLong) => Some(Type::LongLong {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (LongLong, Int) => Some(Type::Int {
            is_unsigned: to.is_unsigned(),
            is_const: false,
        }),
        (LongLong, Char) => Some(Type::char()),
        _ => None,
    }
}

pub(crate) fn insert_implicit_cast(expr: &mut Expr, target: &Type) {
    let current_ty = expr.ty().clone();
    if target.kind() == TypeKind::Double && matches!(expr, Expr::FloatLiteral { .. }) {
        expr.set_ty(Type::double());
        return;
    }
    if let Some(target_ty) = implicit_cast_target(&current_ty, target) {
        let loc = *expr.loc();
        let old = std::mem::take(expr);
        *expr = Expr::Cast {
            expr: Box::new(old),
            target_type: target_ty.clone(),
            loc,
            ty: target_ty,
        };
    }
}

/// 对可变参数实参应用默认实参提升（C 标准）。
/// 当前处理：float -> double，char -> int。
pub(crate) fn apply_default_argument_promotions(expr: &mut Expr) {
    let ty = expr.ty().clone();
    match ty.kind() {
        TypeKind::Float => insert_implicit_cast(expr, &Type::double()),
        TypeKind::Char => insert_implicit_cast(expr, &Type::int()),
        _ => {}
    }
}

impl TypeChecker {
    pub(crate) fn is_int(&self, t: &Type) -> bool {
        matches!(t.kind(), TypeKind::Int | TypeKind::Char)
    }

    pub(crate) fn is_scalar(&self, t: &Type) -> bool {
        matches!(
            t.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        )
    }

    pub(crate) fn is_comparable(&self, a: &Type, b: &Type) -> bool {
        if matches!(a.kind(), TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double)
            && matches!(b.kind(), TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double)
        {
            return true;
        }
        if matches!(a.kind(), TypeKind::Pointer) && matches!(b.kind(), TypeKind::Pointer) {
            return true;
        }
        if matches!(a.kind(), TypeKind::Pointer) && matches!(b.kind(), TypeKind::Array) {
            return true;
        }
        if matches!(a.kind(), TypeKind::Array) && matches!(b.kind(), TypeKind::Pointer) {
            return true;
        }
        if matches!(a.kind(), TypeKind::Pointer) && matches!(b.kind(), TypeKind::Int) {
            return true;
        }
        if matches!(a.kind(), TypeKind::Int) && matches!(b.kind(), TypeKind::Pointer) {
            return true;
        }
        false
    }

    fn check_array_pointer_assignable(&mut self, target: &Type, value: &Type, _loc: &SourceLoc) -> bool {
        if matches!(target.kind(), TypeKind::Pointer) && matches!(value.kind(), TypeKind::Array) {
            if let (Type::Pointer { pointee: t_pointee, .. }, Type::Array { element: v_element, .. }) = (target, value)
            {
                if t_pointee.as_ref() == v_element.as_ref() {
                    return true;
                }
                // Multidimensional array decay: int arr[3][3] -> int (*)[3]
                if t_pointee.as_ref() == &value.subscript_type() {
                    return true;
                }
            }
        }
        if matches!(target.kind(), TypeKind::Array) && matches!(value.kind(), TypeKind::Pointer) {
            if let (Type::Array { element: t_element, .. }, Type::Pointer { pointee: v_pointee, .. }) = (target, value)
            {
                if t_element == v_pointee {
                    return true;
                }
            }
        }
        if matches!(target.kind(), TypeKind::Array) && matches!(value.kind(), TypeKind::Array) {
            if let (Type::Array { element: t_element, .. }, Type::Array { element: v_element, .. }) = (target, value) {
                if t_element == v_element {
                    let check_count = target.dims().len().min(value.dims().len());
                    let mut dims_compatible = true;
                    for i in 0..check_count {
                        if target.dims()[i] > 0 && target.dims()[i] != value.dims()[i] {
                            dims_compatible = false;
                            break;
                        }
                    }
                    if dims_compatible {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn check_function_pointer_assignable(&mut self, target: &Type, value: &Type, loc: &SourceLoc) -> bool {
        if target.is_function_pointer() && value.is_function_pointer() {
            if let (Type::Pointer { pointee: t_pointee, .. }, Type::Pointer { pointee: v_pointee, .. }) =
                (target, value)
            {
                if let (
                    Type::Function {
                        return_type: t_ret,
                        param_types: t_params,
                        ..
                    },
                    Type::Function {
                        return_type: v_ret,
                        param_types: v_params,
                        ..
                    },
                ) = (t_pointee.as_ref(), v_pointee.as_ref())
                {
                    if t_params.len() == v_params.len() {
                        let params_compatible = t_params.iter().zip(v_params.iter()).all(|(a, b)| a == b);
                        if params_compatible && t_ret == v_ret {
                            return true;
                        }
                    }
                }
            }
            self.report_warning(
                "函数指针类型不完全匹配，赋值可能存在风险",
                loc,
                ErrorCode::W3053_ImplicitScalarConversion,
            );
            return true;
        }
        if target.is_pointer() && value.is_function_pointer() {
            return true;
        }
        if target.is_function_pointer() && value.is_pointer() {
            self.report_warning("将通用指针赋值给函数指针，建议显式转换", loc, ErrorCode::W3055_VoidPointerCast);
            return true;
        }
        false
    }

    fn check_scalar_assignable(&mut self, target: &Type, value: &Type, loc: &SourceLoc) -> bool {
        if !matches!(
            target.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        ) {
            return false;
        }
        if !matches!(
            value.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        ) {
            return false;
        }
        // 警告可能丢失精度的情况
        if matches!(target.kind(), TypeKind::Char)
            && matches!(
                value.kind(),
                TypeKind::Int | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
            )
        {
            // W0-4：初始化器路径的 char 窄化由调用方判定无损时豁免
            // （字符常量 / char 值域内整常量；见 `is_char_safe_initializer`）。
            if !self.char_narrow_suppress {
                self.report_warning(
                    "被隐式转换为 char，可能会丢失精度。",
                    loc,
                    ErrorCode::W3053_ImplicitScalarConversion,
                );
            }
        }
        if matches!(target.kind(), TypeKind::Int)
            && matches!(value.kind(), TypeKind::Float | TypeKind::Double | TypeKind::LongLong)
        {
            self.report_warning(
                &format!("{} 被隐式转换为 int，可能会丢失精度。", value),
                loc,
                ErrorCode::W3053_ImplicitScalarConversion,
            );
        }
        if matches!(target.kind(), TypeKind::Float) && matches!(value.kind(), TypeKind::Double | TypeKind::LongLong) {
            self.report_warning(
                "double 被隐式转换为 float，可能会丢失精度。",
                loc,
                ErrorCode::W3053_ImplicitScalarConversion,
            );
        }
        // 提示安全的隐式提升
        if matches!(target.kind(), TypeKind::Int) && matches!(value.kind(), TypeKind::Char) {
            self.report_hint("char 被隐式提升为 int。", loc, ErrorCode::H3057_ImplicitConversionHint);
        }
        if matches!(target.kind(), TypeKind::Float) && matches!(value.kind(), TypeKind::Int | TypeKind::Char) {
            let src = if matches!(value.kind(), TypeKind::Char) {
                "char"
            } else {
                "int"
            };
            self.report_hint(
                &format!("{} 被隐式提升为 float。", src),
                loc,
                ErrorCode::H3057_ImplicitConversionHint,
            );
        }
        if matches!(target.kind(), TypeKind::Double)
            && matches!(
                value.kind(),
                TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::LongLong
            )
        {
            let src = match value.kind() {
                TypeKind::Char => "char",
                TypeKind::Float => "float",
                TypeKind::LongLong => "long long",
                _ => "int",
            };
            self.report_hint(
                &format!("{} 被隐式提升为 double。", src),
                loc,
                ErrorCode::H3057_ImplicitConversionHint,
            );
        }
        true
    }

    /// `value_pointee*` 是否可**隐式**赋给 `target_pointee*`：即 C++ 向上转型
    /// （派生类指针 → 基类指针）。
    ///
    /// 只沿单继承链回溯（教学子集范围），带步数上限防环。多继承在本子集中不支持
    /// （`E4005`），因此单链判定足够。
    fn is_upcast(&self, target_pointee: &Type, value_pointee: &Type) -> bool {
        let (Type::Class { name: base_name, .. }, Type::Class { name: derived_name, .. }) =
            (target_pointee, value_pointee)
        else {
            return false;
        };
        let mut cur: &str = derived_name.as_str();
        for _ in 0..32 {
            let Some(sym) = self.classes.get(cur) else {
                return false;
            };
            match sym.base.as_deref() {
                Some(b) if b == base_name => return true,
                Some(b) => cur = b,
                None => return false,
            }
        }
        false
    }

    fn check_pointer_assignable(&mut self, target: &Type, value: &Type, loc: &SourceLoc) -> bool {
        if matches!(target.kind(), TypeKind::Pointer) && matches!(value.kind(), TypeKind::Int) {
            self.report_warning(
                "整数被隐式转换为指针。建议确保这是有意义的地址值（如 NULL = 0）。",
                loc,
                ErrorCode::W3054_IntToPointerCast,
            );
            return true;
        }
        // C 标准：任意指针或数组都可以隐式转换为 void*（Phase D 补齐 Host 函数所需）
        if matches!(target.kind(), TypeKind::Pointer) {
            if let Type::Pointer { pointee, .. } = target {
                if matches!(pointee.as_ref(), Type::Void { .. })
                    && matches!(value.kind(), TypeKind::Pointer | TypeKind::Array)
                {
                    self.report_hint("具体指针类型被隐式转换为 void*。", loc, ErrorCode::H3057_ImplicitConversionHint);
                    return true;
                }
            }
        }
        if matches!(target.kind(), TypeKind::Pointer) && matches!(value.kind(), TypeKind::Pointer) {
            if let (
                Type::Pointer {
                    is_const: t_const,
                    pointee: t_pointee,
                },
                Type::Pointer {
                    pointee: v_pointee,
                    is_const: v_const,
                },
            ) = (target, value)
            {
                if matches!(v_pointee.as_ref(), Type::Void { .. }) {
                    self.report_hint("void* 被隐式转换为具体指针类型。", loc, ErrorCode::H3057_ImplicitConversionHint);
                } else if t_pointee != v_pointee {
                    // P1-6：C++ 向上转型（`Derived* → Base*`）隐式允许，不得报"不兼容/截断"
                    // —— Clang++ 在 `-Wall -Wextra` 下对此零警告，它是多态的基础写法。
                    // 此前复用标量转换码 W3053（建议文案为"可能导致数据截断"），把多态
                    // 基础讲成了危险操作。
                    if !self.is_upcast(t_pointee, v_pointee) {
                        self.report_warning(
                            &format!(
                                "指针类型不兼容：{}* ← {}*，需要显式转换（向上转型 Derived* → Base* 除外）。",
                                t_pointee.name(),
                                v_pointee.name()
                            ),
                            loc,
                            ErrorCode::W3067_PointerTypeMismatch,
                        );
                    }
                }
                if *v_const && !*t_const {
                    self.report_warning(
                        "将 const 指针赋值给非 const 指针，可能通过后者修改 const 数据。",
                        loc,
                        ErrorCode::W3053_ImplicitScalarConversion,
                    );
                }
            }
            return true;
        }
        false
    }

    pub(crate) fn check_assignable(&mut self, target: &Type, value: &Type, loc: &SourceLoc) -> bool {
        if target == value {
            return true;
        }
        // Reference type compatibility (basic type check, lvalue/rvalue checked at use site)
        if let Type::Reference {
            base: t_base,
            is_const: t_const,
        } = target
        {
            if let Type::Reference {
                base: v_base,
                is_const: v_const,
            } = value
            {
                // Reference to reference: only const& can bind to const&
                if t_base == v_base && (*t_const || !*v_const) {
                    return true;
                }
                return false;
            }
            // Non-reference value binding to reference: check base type compatibility
            if t_base.as_ref() == value {
                return true;
            }
            // const Class& 可以绑定到非 const Class 对象（按类名比较）。
            if let (Type::Class { name: t_name, .. }, Type::Class { name: v_name, .. }) = (t_base.as_ref(), value) {
                if t_name == v_name {
                    return true;
                }
            }
            if t_base.kind() == value.kind()
                && matches!(
                    t_base.kind(),
                    TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
                )
            {
                return true;
            }
            if let Type::Pointer { pointee: t_pt, .. } = t_base.as_ref() {
                if let Type::Pointer { pointee: v_pt, .. } = value {
                    if t_pt == v_pt || matches!(t_pt.as_ref(), Type::Void { .. }) {
                        return true;
                    }
                }
            }
            return false;
        }
        // RValueRef value binding to class type: implicit move construction
        if let Type::RValueRef { base: v_base } = value {
            if target.is_class() && v_base.as_ref() == target {
                return true;
            }
        }
        if let Type::RValueRef { base: t_base } = target {
            if t_base.as_ref() == value {
                return true;
            }
            if self.check_scalar_assignable(t_base, value, loc) {
                return true;
            }
            return false;
        }
        if self.check_array_pointer_assignable(target, value, loc) {
            return true;
        }
        if self.check_function_pointer_assignable(target, value, loc) {
            return true;
        }
        if self.check_scalar_assignable(target, value, loc) {
            return true;
        }
        if self.check_pointer_assignable(target, value, loc) {
            return true;
        }
        false
    }

    /// 尝试隐式实例化函数模板，返回 (mangled_name, FuncDecl) 但不注册到 program。
    pub(crate) fn try_instantiate_template(
        &mut self,
        name: &str,
        arg_types: &[Type],
    ) -> Option<(String, Option<FuncDecl>)> {
        self.try_monomorphize_func(name, arg_types)
    }

    /// 判断表达式是否为左值（可被取地址的表达式）。
    pub(crate) fn is_lvalue(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Identifier { .. } => true,
            Expr::Member { .. } => true,
            Expr::Index { .. } => true,
            Expr::Unary { op: UnaryOp::Deref, .. } => true,
            Expr::Call { .. } | Expr::CallPtr { .. } => expr.ty().is_reference() || expr.ty().is_rvalue_ref(),
            _ => false,
        }
    }

    pub(crate) fn get_struct_field_type(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        let sym = self.structs.get(struct_name)?;
        for (fty, fname) in &sym.fields {
            if fname == field_name {
                return Some(fty.clone());
            }
        }
        None
    }

    pub(crate) fn get_union_field_type(&self, union_name: &str, field_name: &str) -> Option<Type> {
        let sym = self.unions.get(union_name)?;
        for (fty, fname) in &sym.fields {
            if fname == field_name {
                return Some(fty.clone());
            }
        }
        None
    }

    pub(crate) fn expr_involves_array_or_pointer(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Index { .. } => true,
            Expr::Identifier { name, .. } => self
                .lookup_var(name)
                .map(|s| s.ty.is_array() || s.ty.is_pointer())
                .unwrap_or(false),
            Expr::Binary { left, right, .. } => {
                self.expr_involves_array_or_pointer(left) || self.expr_involves_array_or_pointer(right)
            }
            Expr::Unary { operand, .. } => self.expr_involves_array_or_pointer(operand),
            Expr::Assign { left, right, .. } => {
                self.expr_involves_array_or_pointer(left) || self.expr_involves_array_or_pointer(right)
            }
            Expr::Ternary {
                cond, then_branch, else_branch, ..
            } => {
                self.expr_involves_array_or_pointer(cond)
                    || self.expr_involves_array_or_pointer(then_branch)
                    || self.expr_involves_array_or_pointer(else_branch)
            }
            _ => false,
        }
    }
}
