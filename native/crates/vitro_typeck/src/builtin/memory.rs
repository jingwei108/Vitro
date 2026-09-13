use super::*;

impl TypeChecker {
    pub(crate) fn check_builtin_malloc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 1 {
            self.report_error("malloc 需要一个参数", loc, ErrorCode::E3024_MallocArgCount);
        } else {
            let expected = Type::int();
            let arg_type = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&expected, &arg_type, loc) {
                self.report_error("malloc 参数必须是 int", loc, ErrorCode::E3025_MallocArgType);
            } else {
                insert_implicit_cast(&mut args[0], &expected);
            }
        }
        Type::pointer_to(Type::void())
    }

    pub(crate) fn check_builtin_free(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "free", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "free", loc);
        }
        Type::void()
    }

    pub(crate) fn check_builtin_memset(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error("memset 需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let ptr_type = self.resolve_expr_type(&mut args[0]);
            if !ptr_type.is_pointer() && !ptr_type.is_array() {
                self.report_error("memset 第一个参数必须是指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for i in 1..3 {
                let expected = Type::int();
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&expected, &t, loc) {
                    self.report_error(
                        &format!("memset 的第 {} 个参数必须是 int", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &expected);
                }
            }
        }
        Type::pointer_to(Type::void())
    }

    pub(crate) fn check_builtin_exit(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "exit", loc) {
            self.builtin_check_int(&mut args[0], 0, "exit", loc);
        }
        Type::void()
    }

    pub(crate) fn check_builtin_realloc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("realloc 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let ptr_type = self.resolve_expr_type(&mut args[0]);
            if !ptr_type.is_pointer() && !matches!(ptr_type.kind(), TypeKind::Int) {
                self.report_error("realloc 第一个参数必须是指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
            let size_type = self.resolve_expr_type(&mut args[1]);
            if !self.check_assignable(&Type::int(), &size_type, loc) {
                self.report_error("realloc 第二个参数必须是 int", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[1], &Type::int());
            }
        }
        Type::pointer_to(Type::void())
    }

    pub(crate) fn check_builtin_calloc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("calloc 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            for (i, arg) in args.iter_mut().enumerate().take(2) {
                self.builtin_check_int(arg, i, "calloc", loc);
            }
        }
        Type::pointer_to(Type::void())
    }
}
