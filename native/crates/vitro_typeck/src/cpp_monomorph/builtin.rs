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
        match base {
            "vitro_vec" => {
                let (mangled, new_class) = self.synthesize_vec_class(elem_ty, loc);
                self.register_single_class_layout(&mangled, &new_class);
                Some((mangled, new_class))
            }
            "vitro_list" => {
                let (mangled, new_class) = self.synthesize_list_class(elem_ty, loc);
                self.register_single_class_layout(&mangled, &new_class);
                Some((mangled, new_class))
            }
            _ => None,
        }
    }
}
