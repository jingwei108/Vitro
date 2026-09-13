use super::*;

impl TypeChecker {
    pub(crate) fn check_builtin_rand(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        self.builtin_check_count(args, 0, "rand", loc);
        Type::int()
    }

    pub(crate) fn check_builtin_srand(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "srand", loc) {
            self.builtin_check_int(&mut args[0], 0, "srand", loc);
        }
        Type::void()
    }

    pub(crate) fn check_builtin_tan(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "tan", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("tan 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_log10(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "log10", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("log10 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_fabs(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "fabs", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("fabs 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_ceil(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "ceil", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("ceil 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_floor(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "floor", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("floor 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_round(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "round", loc) {
            let t = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::double(), &t, loc) {
                self.report_error("round 参数必须是 double", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::double());
            }
        }
        Type::double()
    }

    pub(crate) fn check_builtin_fmod(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("fmod 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            for i in 0..2 {
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&Type::double(), &t, loc) {
                    self.report_error(
                        &format!("fmod 第 {} 个参数必须是 double", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &Type::double());
                }
            }
        }
        Type::double()
    }
}
