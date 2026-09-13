use vitro_ast::*;

use super::super::{AccessSpec, MethodSig, TypeChecker};

impl TypeChecker {
    /// 将类外方法定义（`void Foo::bar() { ... }`、`Foo::Foo() { ... }`）合并到对应
    /// ClassDecl 的成员声明中，避免产生重复的函数符号，同时让方法体能访问类字段。
    pub(crate) fn merge_out_of_line_method_definitions(&mut self, program: &mut ProgramNode) {
        let mut merged_indices = std::collections::HashSet::new();
        for (i, f) in program.funcs.iter().enumerate() {
            if f.body.is_none() {
                continue;
            }

            // 普通方法: ClassName__methodName(...)
            if let Some((class_name, method_name)) = f.name.split_once("__") {
                if !class_name.is_empty() && !method_name.is_empty() && !method_name.starts_with("ctor__") {
                    if let Some(c) = program.classes.iter_mut().find(|c| c.name == class_name) {
                        for member in &mut c.members {
                            if let ClassMember::Method { name, body, .. } = member {
                                if name == method_name && body.is_none() {
                                    *body = f.body.clone();
                                    merged_indices.insert(i);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // 构造函数: __ctor__ClassName 或 __ctor__ClassName__N
            if f.name.starts_with("__ctor__") {
                let rest = &f.name["__ctor__".len()..];
                let class_name = rest.split("__").next().unwrap_or(rest);
                if let Some(c) = program.classes.iter_mut().find(|c| c.name == class_name) {
                    for member in &mut c.members {
                        if let ClassMember::Constructor { body, .. } = member {
                            if body.is_none() {
                                *body = f.body.clone();
                                merged_indices.insert(i);
                                break;
                            }
                        }
                    }
                }
            }
        }

        program.funcs = program
            .funcs
            .drain(..)
            .enumerate()
            .filter(|(i, _)| !merged_indices.contains(i))
            .map(|(_, f)| f)
            .collect();
    }

    pub(crate) fn get_class_field_type_with_access(
        &self,
        class_name: &str,
        field_name: &str,
    ) -> (Option<Type>, Option<AccessSpec>) {
        let sym = match self.classes.get(class_name) {
            Some(s) => s,
            None => return (None, None),
        };
        for (fty, fname, faccess) in &sym.fields {
            if fname == field_name {
                return (Some(fty.clone()), Some(*faccess));
            }
        }
        for (fty, fname, faccess) in &sym.static_fields {
            if fname == field_name {
                return (Some(fty.clone()), Some(*faccess));
            }
        }
        (None, None)
    }

    pub(crate) fn find_class_method_sigs(&self, class_name: &str, method_name: &str) -> Option<Vec<MethodSig>> {
        let sym = self.classes.get(class_name)?;
        sym.methods.get(method_name).cloned()
    }

    /// 类型 → mangled 名后缀片段（**单源**；D1 修复 2026-09-12）。
    ///
    /// 此前成员函数 mangled 名只带**参数个数**（`{Class}__{method}__{arity}`），
    /// 同参数个数仅类型不同的重载（`show(int)` / `show(double)`）会撞名 —— 定义处
    /// 后写覆盖先写、调用点静默派发到错误实现，最终运行时"栈下溢"trap。
    /// 现在把每个参数类型编码进名字，保证不同签名 → 不同符号。
    ///
    /// 编码（ASCII、稳定、可读）：`v`空/`i`int/`u`unsigned int/`l`long long/`c`char/
    /// `f`float/`d`double/`P`+pointee/`R`+base/`S`+类名/`A`+元素。const 不参与
    /// （顶层 const 不影响重载决议，与 C++ 一致）。
    pub(crate) fn type_mangle_suffix(ty: &Type) -> String {
        match ty {
            Type::Void { .. } => "v".to_string(),
            Type::Int { is_unsigned, .. } => if *is_unsigned { "u" } else { "i" }.to_string(),
            Type::Char { .. } => "c".to_string(),
            Type::Float { .. } => "f".to_string(),
            Type::Double { .. } => "d".to_string(),
            Type::LongLong { .. } => "l".to_string(),
            Type::Pointer { pointee, .. } => format!("P{}", Self::type_mangle_suffix(pointee)),
            Type::Reference { base, .. } | Type::RValueRef { base } => {
                format!("R{}", Self::type_mangle_suffix(base))
            }
            Type::Array { element, .. } => format!("A{}", Self::type_mangle_suffix(element)),
            Type::Struct { name, .. } | Type::Union { name, .. } | Type::Class { name, .. } => {
                format!("S{}", name)
            }
            Type::Function { return_type, param_types, .. } => {
                let ps: String = param_types.iter().map(Self::type_mangle_suffix).collect();
                format!("F{}{}", Self::type_mangle_suffix(return_type), ps)
            }
            Type::TemplateId { base, args, .. } => {
                let as_: String = args
                    .iter()
                    .map(|a| match a {
                        vitro_ast::TemplateArg::Type(t) => Self::type_mangle_suffix(t),
                        vitro_ast::TemplateArg::Int(n) => format!("I{}", n),
                        vitro_ast::TemplateArg::Expr(_) => "E".to_string(),
                    })
                    .collect();
                format!("T{}{}", base, as_)
            }
            Type::Auto => "a".to_string(),
            Type::Typeof { .. } => "y".to_string(),
        }
    }

    /// 成员函数 mangled 名单源（D1）：`{Class}__{method}` 单签名 / `{Class}__{method}__{类型编码}` 多签名。
    ///
    /// **定义处（`check_class_methods` / `load_class`）与调用处（`resolve_method_overload`）
    /// 必须共用本函数**，否则符号名不一致会导致派发失败。
    /// `has_overloads` 语义：该 `{method}` 名下是否有多于一个签名（决定是否带类型后缀，
    /// 保持既有单签名场景的短名兼容——不破坏已落库的字节码/回放断言）。
    pub(crate) fn method_mangled_name(
        class_name: &str,
        method_name: &str,
        param_types: &[Type],
        has_overloads: bool,
    ) -> String {
        if !has_overloads {
            return format!("{}__{}", class_name, method_name);
        }
        let suffix: String = param_types.iter().map(Self::type_mangle_suffix).collect();
        format!("{}__{}__{}", class_name, method_name, suffix)
    }

    /// Resolve a non-constructor method overload from the given class.
    /// Returns the matching signature and the mangled function name to call.
    pub(crate) fn resolve_method_overload(
        &self,
        class_name: &str,
        method_name: &str,
        arg_types: &[Type],
    ) -> Option<(MethodSig, String)> {
        let sigs = self.find_class_method_sigs(class_name, method_name)?;
        let mut best: Option<(MethodSig, usize)> = None;
        for sig in &sigs {
            if sig.param_types.len() < arg_types.len() {
                continue;
            }
            if sig.param_types.len() > arg_types.len() {
                let has_defaults = sig.param_defaults.iter().skip(arg_types.len()).all(|d| d.is_some());
                if !has_defaults {
                    continue;
                }
            }
            let mut score = 0usize;
            let mut ok = true;
            for (param, arg) in sig.param_types.iter().zip(arg_types.iter()) {
                let s = self.overload_match_score(param, arg);
                if s == 0 {
                    ok = false;
                    break;
                }
                score += s;
            }
            if !ok {
                continue;
            }
            match &best {
                None => best = Some((sig.clone(), score)),
                Some((_, cur)) if score > *cur => best = Some((sig.clone(), score)),
                _ => {}
            }
        }
        best.map(|(sig, _)| {
            // D1：mangled 名带**完整参数类型编码**（含 this），与定义处
            // （`check_class_methods` / `load_class`）共用 `method_mangled_name`。
            // 此前只带 arity，同参数量仅类型不同的重载撞名 → 静默错派发 → 运行时栈下溢。
            let mangled = Self::method_mangled_name(class_name, method_name, &sig.param_types, sigs.len() > 1);
            (sig, mangled)
        })
    }

    /// Non-reporting compatibility score for overload resolution.
    fn overload_match_score(&self, param: &Type, arg: &Type) -> usize {
        if param == arg {
            return 3;
        }
        // Reference binding
        if let Type::Reference { base, .. } = param {
            if base.as_ref() == arg {
                return 3;
            }
            if base.kind() == arg.kind()
                && matches!(
                    base.kind(),
                    TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
                )
            {
                return 2;
            }
            if let Type::Pointer { pointee: pb, .. } = base.as_ref() {
                if let Type::Pointer { pointee: pa, .. } = arg {
                    if pb == pa || matches!(pb.as_ref(), Type::Void { .. }) {
                        return 2;
                    }
                }
            }
            return 0;
        }
        // Numeric promotion / conversion
        if matches!(
            param.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        ) && matches!(
            arg.kind(),
            TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong
        ) {
            return 2;
        }
        // Pointer / array compatibility
        if param.is_pointer() && (arg.is_pointer() || arg.is_array()) {
            return 2;
        }
        // Class type match
        if param.is_class() && arg.is_class() && param.name() == arg.name() {
            return 3;
        }
        // RValue reference binding to class
        if let Type::RValueRef { base } = param {
            if arg.is_class() && base.as_ref() == arg {
                return 2;
            }
        }
        0
    }
}
