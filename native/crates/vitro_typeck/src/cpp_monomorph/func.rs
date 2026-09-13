use super::*;

impl TypeChecker {
    /// 尝试根据实参类型隐式实例化函数模板。
    /// 返回实例化后的 mangled 函数名（如 "max__int"），若无法匹配则返回 None。
    /// Try to monomorphize a function template. Returns (mangled_name, FuncDecl) if successful.
    /// Returns `Some((mangled_name, Some(func_decl)))` for a new instantiation,
    /// `Some((mangled_name, None))` if already instantiated,
    /// or `None` if the name is not a template or inference failed.
    pub(crate) fn try_monomorphize_func(
        &mut self,
        name: &str,
        arg_types: &[Type],
    ) -> Option<(String, Option<FuncDecl>)> {
        let template = self.templates.get(name)?.clone();
        let func_decl = match &template.decl {
            Templateable::Func(f) => f.as_ref(),
            _ => return None,
        };

        if template.params.is_empty() {
            return None;
        }

        // Derive template arguments from parameter types
        let mut type_map: HashMap<String, Type> = HashMap::new();
        for (param, arg_ty) in func_decl.params.iter().zip(arg_types.iter()) {
            self.infer_template_arg(&param.ty, arg_ty, &mut type_map);
        }
        // Also infer from return type if it's a template param (for cases like T foo())
        if let Some(first_arg) = arg_types.first() {
            self.infer_template_arg(&func_decl.return_type, first_arg, &mut type_map);
        }

        // Check all template params have been inferred.
        // Non-type template parameters are not inferred from argument types in this subset;
        // they require explicit instantiation (e.g. Foo<5>).
        let mut type_args: Vec<TemplateArg> = Vec::new();
        for tp in &template.params {
            match tp {
                TemplateParam::Type { name, .. } => {
                    let t = type_map.get(name)?; // Could not infer all params
                    type_args.push(TemplateArg::Type(t.clone()));
                }
                TemplateParam::NonType { .. } => {
                    return None; // Explicit instantiation required for NTTP
                }
            }
        }

        let mangled = Self::mangle_template_name(name, &type_args);
        if self.funcs.contains_key(&mangled) {
            return Some((mangled, None)); // Already instantiated
        }

        // Instantiate: deep clone and replace template params
        let mut new_func = func_decl.clone();
        new_func.name = mangled.clone();
        let value_map: HashMap<String, i32> = HashMap::new();
        self.replace_template_types_in_func(&mut new_func, &type_map, &value_map);

        self.funcs.insert(
            mangled.clone(),
            FuncSymbol {
                return_type: new_func.return_type.clone(),
                param_types: new_func.params.iter().map(|p| p.ty.clone()).collect(),
                is_variadic: new_func.is_variadic,
                param_defaults: new_func.params.iter().map(|p| p.default.clone()).collect(),
            },
        );

        Some((mangled, Some(new_func)))
    }

    /// Mangling rule: func__T1_T2_...
    /// 内置容器模板保持历史短名，与现有测试及标准库存根兼容。
    pub(crate) fn mangle_template_name(base: &str, args: &[TemplateArg]) -> String {
        if args.len() == 1 {
            if let TemplateArg::Type(Type::Int { .. }) = &args[0] {
                match base {
                    "vitro_vec" => return "vitro_vec_int".to_string(),
                    "vitro_list" => return "vitro_list_int".to_string(),
                    _ => {}
                }
            }
            if let TemplateArg::Type(Type::Float { .. }) = &args[0] {
                if base == "vitro_vec" {
                    return "vitro_vec_float".to_string();
                }
            }
            if let TemplateArg::Type(Type::Char { .. }) = &args[0] {
                match base {
                    "vitro_vec" => return "vitro_vec_char".to_string(),
                    "vitro_string" => return "vitro_string".to_string(),
                    _ => {}
                }
            }
        }
        let mut result = base.to_string();
        for a in args {
            result.push_str("__");
            a.mangle_name_into(&mut result);
        }
        result
    }

    /// Infer template argument by matching a formal parameter type against an actual type.
    fn infer_template_arg(&self, formal: &Type, actual: &Type, map: &mut HashMap<String, Type>) {
        match formal {
            Type::Class { name, .. }
                // Treat Class types with names not found in self.classes as template params
                // (Parser registers template params as Type::Class { name: param_name })
                if !self.classes.contains_key(name) && !self.structs.contains_key(name) =>
            {
                map.insert(name.clone(), actual.clone());
            }
            Type::Pointer { pointee, .. } => {
                match actual {
                    Type::Pointer { pointee: actual_pointee, .. } => {
                        self.infer_template_arg(pointee, actual_pointee, map);
                    }
                    Type::Array { element: actual_elem, .. } => {
                        // C array-to-pointer decay: T formal vs T[5] actual
                        self.infer_template_arg(pointee, actual_elem, map);
                    }
                    _ => {}
                }
            }
            Type::Array { element, .. } => {
                if let Type::Array { element: actual_elem, .. } = actual {
                    self.infer_template_arg(element, actual_elem, map);
                }
            }
            Type::Reference { base, .. } => {
                if let Type::Reference { base: actual_base, .. } = actual {
                    self.infer_template_arg(base, actual_base, map);
                }
            }
            _ => {}
        }
    }

    /// Recursively replace template parameter types in a function declaration.
    fn replace_template_types_in_func(
        &self,
        func: &mut FuncDecl,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) {
        func.return_type = self.replace_template_type(&func.return_type, type_map, value_map);
        for p in &mut func.params {
            p.ty = self.replace_template_type(&p.ty, type_map, value_map);
        }
        if let Some(ref mut body) = func.body {
            self.replace_template_types_in_stmt(body, type_map, value_map);
        }
    }
}
