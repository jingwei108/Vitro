use super::*;

impl TypeChecker {
    /// 尝试将内置容器方法调用（如 `v.push_back(x)`）降解为 C 风格函数调用。
    /// 若匹配成功，返回 `(host_func_name, addr_expr, extra_call_args, result_type)`。
    pub(crate) fn try_resolve_container_member_call(
        &mut self,
        class_name: &str,
        method: &str,
        object: &Expr,
        args: &mut [Expr],
        loc: &SourceLoc,
    ) -> Option<(String, Expr, Vec<Expr>, Type)> {
        let host_func = vitro_cpp_frontend::type_map::map_container_method(class_name, method)?;
        let layout = vitro_cpp_frontend::builtin_layout::builtin_class_layout(class_name)?;
        let method_sig = layout.methods.iter().find(|m| m.name == method)?;

        if args.len() != method_sig.params.len() {
            self.report_error(
                &format!(
                    "容器方法 '{}' 参数数量不匹配：期望 {}，实际 {}",
                    method,
                    method_sig.params.len(),
                    args.len()
                ),
                loc,
                ErrorCode::E3037_FuncArgCount,
            );
        } else {
            for (i, (arg, expected)) in args.iter_mut().zip(method_sig.params.iter()).enumerate() {
                let arg_type = self.resolve_expr_type(arg);
                if !self.check_assignable(expected, &arg_type, loc) {
                    self.report_error(
                        &format!("容器方法 '{}' 第 {} 个参数类型不匹配", method, i + 1),
                        loc,
                        ErrorCode::E3038_FuncArgType,
                    );
                } else {
                    insert_implicit_cast(arg, expected);
                }
            }
        }

        let obj_type = object.ty().clone();
        let addr_expr = if matches!(obj_type, Type::Pointer { .. }) {
            object.clone()
        } else {
            Expr::Unary {
                op: UnaryOp::Addr,
                operand: Box::new(object.clone()),
                loc: *loc,
                ty: Type::pointer_to(obj_type),
            }
        };
        let result_ty = method_sig.ret.clone();
        let call_args: Vec<Expr> = args.to_vec();
        Some((host_func.to_string(), addr_expr, call_args, result_ty))
    }
}
