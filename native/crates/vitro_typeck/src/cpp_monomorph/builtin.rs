use super::*;

impl TypeChecker {
    // =========================================================================
    // 内置容器（vector/list）对类类型模板实参的合成实例化
    // =========================================================================

    /// 若 base 是内置容器模板且模板实参是类类型，合成一个走普通类模板路径的
    /// 容器实例化。POD 实参仍走原有预编译 Bytecode Libc 路径。
    pub(crate) fn try_synthesize_builtin_container_class(
        &mut self,
        base: &str,
        args: &[Type],
        loc: &SourceLoc,
    ) -> Option<(String, ClassDecl)> {
        if args.len() != 1 {
            return None;
        }
        let elem_ty = &args[0];
        if !matches!(elem_ty, Type::Class { .. }) {
            return None;
        }
        // U3#8：查重前置——第二次遇到同 (base, elem) 时此前仍无条件合成并
        // register_single_class_layout，后者对已注册名报 E3002"类重复定义"
        //（合法代码被拒：第二个 vitro_vec<Foo>；错误级联后常显形为 E3023）。
        // 已注册 = 返回已存在的 mangled（Some + 空哑 decl 不入 pending——
        // 由调用方 push 点的查重兜住），不重复合成、不重复注册。
        let mangled_probe = Self::mangle_template_name(base, std::slice::from_ref(&TemplateArg::Type(elem_ty.clone())));
        let already = self.classes.contains_key(&mangled_probe) || self.structs.contains_key(&mangled_probe);
        match base {
            "vitro_vec" => {
                if already {
                    return Some((mangled_probe.clone(), Self::placeholder_class(&mangled_probe, *loc)));
                }
                let (mangled, new_class) = self.synthesize_vec_class(elem_ty, loc);
                self.register_single_class_layout(&mangled, &new_class);
                Some((mangled, new_class))
            }
            "vitro_list" => {
                if already {
                    return Some((mangled_probe.clone(), Self::placeholder_class(&mangled_probe, *loc)));
                }
                let (mangled, new_class) = self.synthesize_list_class(elem_ty, loc);
                self.register_single_class_layout(&mangled, &new_class);
                Some((mangled, new_class))
            }
            _ => None,
        }
    }
}
