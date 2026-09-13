use super::*;

impl TypeChecker {
    /// 尝试实例化类模板。返回 (mangled_name, ClassDecl) 若成功。
    pub(crate) fn try_monomorphize_class(&mut self, base: &str, args: &[TemplateArg]) -> Option<(String, ClassDecl)> {
        // 内置容器 + 类类型实参：合成走普通类模板路径的实例化。
        let type_only_args: Vec<Type> = args
            .iter()
            .filter_map(|a| match a {
                TemplateArg::Type(ty) => Some(ty.clone()),
                _ => None,
            })
            .collect();
        if let Some(result) = self.try_synthesize_builtin_container_class(base, &type_only_args, &SourceLoc::default())
        {
            return Some(result);
        }
        let template = self.templates.get(base)?.clone();
        let class_decl = match &template.decl {
            Templateable::Class(c) => c.as_ref(),
            _ => return None,
        };

        if template.params.is_empty() || template.params.len() != args.len() {
            return None;
        }

        let mut type_map: HashMap<String, Type> = HashMap::new();
        let mut value_map: HashMap<String, i32> = HashMap::new();
        for (tp, arg) in template.params.iter().zip(args.iter()) {
            match (tp, arg) {
                (TemplateParam::Type { name, .. }, TemplateArg::Type(ty)) => {
                    type_map.insert(name.clone(), ty.clone());
                }
                (TemplateParam::NonType { name, .. }, TemplateArg::Int(v)) => {
                    value_map.insert(name.clone(), *v);
                }
                (TemplateParam::NonType { name, .. }, TemplateArg::Expr(expr)) => {
                    let v = self.evaluate_constexpr(expr, &value_map)?;
                    value_map.insert(name.clone(), v);
                }
                _ => return None,
            }
        }

        let mangled = Self::mangle_template_name(base, args);
        if self.classes.contains_key(&mangled) || self.structs.contains_key(&mangled) {
            return None; // Already instantiated
        }

        let mut new_class = class_decl.clone();
        new_class.name = mangled.clone();
        self.replace_template_types_in_class(&mut new_class, &type_map, &value_map);

        // Register layout immediately so Pass 3 can resolve members
        self.register_single_class_layout(&mangled, &new_class);

        Some((mangled, new_class))
    }

    fn replace_template_types_in_class(
        &self,
        class: &mut ClassDecl,
        type_map: &HashMap<String, Type>,
        value_map: &HashMap<String, i32>,
    ) {
        // Replace types in base class name? For now, base is not a template param in our subset
        for member in &mut class.members {
            match member {
                ClassMember::Field { ty, .. } => {
                    *ty = self.replace_template_type(ty, type_map, value_map);
                }
                ClassMember::Method { ret, params, body, .. } => {
                    *ret = self.replace_template_type(ret, type_map, value_map);
                    for p in params.iter_mut() {
                        p.ty = self.replace_template_type(&p.ty, type_map, value_map);
                    }
                    if let Some(ref mut b) = body {
                        self.replace_template_types_in_stmt(b, type_map, value_map);
                    }
                }
                ClassMember::Constructor { params, body, .. } => {
                    for p in params.iter_mut() {
                        p.ty = self.replace_template_type(&p.ty, type_map, value_map);
                    }
                    if let Some(ref mut b) = body {
                        self.replace_template_types_in_stmt(b, type_map, value_map);
                    }
                }
                ClassMember::Destructor { body, .. } => {
                    if let Some(ref mut b) = body {
                        self.replace_template_types_in_stmt(b, type_map, value_map);
                    }
                }
                ClassMember::NestedStruct { decl, .. } => {
                    for field in &mut decl.fields {
                        field.ty = self.replace_template_type(&field.ty, type_map, value_map);
                    }
                }
                ClassMember::NestedClass { decl, .. } => {
                    self.replace_template_types_in_class(decl, type_map, value_map);
                }
            }
        }
    }
}
