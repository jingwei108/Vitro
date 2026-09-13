use super::*;

impl TypeChecker {
    pub(crate) fn check_builtin_strlen(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "strlen", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "strlen", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_strcpy(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 2, "strcpy", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "strcpy", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strcpy", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_strdup(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "strdup", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "strdup", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_strcmp(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 2, "strcmp", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "strcmp", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strcmp", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_strcat(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 2, "strcat", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "strcat", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strcat", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_atoi(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "atoi", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "atoi", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_strncat(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error("strncat 需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "strncat", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strncat", loc);
            self.builtin_check_int(&mut args[2], 2, "strncat", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_strncmp(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error("strncmp 需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "strncmp", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strncmp", loc);
            self.builtin_check_int(&mut args[2], 2, "strncmp", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_memcmp(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error("memcmp 需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "memcmp", loc);
            self.builtin_check_pointer(&mut args[1], 1, "memcmp", loc);
            self.builtin_check_int(&mut args[2], 2, "memcmp", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_strchr(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("strchr 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "strchr", loc);
            self.builtin_check_int(&mut args[1], 1, "strchr", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_strrchr(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("strrchr 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "strrchr", loc);
            self.builtin_check_int(&mut args[1], 1, "strrchr", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_strstr(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("strstr 需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "strstr", loc);
            self.builtin_check_pointer(&mut args[1], 1, "strstr", loc);
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_memchr(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error("memchr 需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "memchr", loc);
            self.builtin_check_int(&mut args[1], 1, "memchr", loc);
            self.builtin_check_int(&mut args[2], 2, "memchr", loc);
        }
        Type::pointer_to(Type::void())
    }

    pub(crate) fn check_builtin_atof(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "atof", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "atof", loc);
        }
        Type::double()
    }
    pub(crate) fn check_builtin_atol(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "atol", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "atol", loc);
        }
        Type::long_long()
    }
}
