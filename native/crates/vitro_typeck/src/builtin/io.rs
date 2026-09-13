use super::*;

impl TypeChecker {
    pub(crate) fn check_builtin_print_int(&mut self, args: &mut [Expr], loc: &SourceLoc, name: &str) -> Type {
        if args.len() != 1 {
            self.report_error(&format!("{} 需要一个参数", name), loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let expected = Type::int();
            let arg_type = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&expected, &arg_type, loc) {
                self.report_error(&format!("{} 参数必须是 int", name), loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &expected);
            }
        }
        Type::void()
    }

    /// 解析 printf/scanf 格式字符串，返回非 %% 的格式说明符列表。
    /// 每个元素为 (spec_char, length_mod) 例如 ('d', "") 或 ('f', "l")
    pub(crate) fn check_builtin_printf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.is_empty() {
            self.report_error("printf 至少需要 1 个参数（格式字符串）", loc, ErrorCode::E3030_PrintfArgCount);
        } else {
            let fmt_type = self.resolve_expr_type(&mut args[0]);
            if !fmt_type.is_pointer() && !fmt_type.is_array() {
                self.report_error("printf 的第一个参数必须是字符串", loc, ErrorCode::E3031_PrintfFirstArg);
            }
            // 如果格式字符串是字面量，进行格式-参数类型匹配检查
            let fmt_str = if let Expr::StringLiteral { ref value, .. } = args[0] {
                Some(value.clone())
            } else {
                None
            };
            if let Some(fmt) = fmt_str {
                self.check_printf_format(&fmt, &mut args[1..], loc);
            } else {
                // 非字面量格式字符串，只做粗略检查
                for (i, arg) in args.iter_mut().enumerate().skip(1) {
                    let arg_type = self.resolve_expr_type(arg);
                    if !self.is_scalar(&arg_type) && !arg_type.is_pointer() && !arg_type.is_array() {
                        self.report_error(
                            &format!("printf 的第 {} 个参数必须是 int、float、char 或指针", i + 1),
                            loc,
                            ErrorCode::E3032_PrintfArgType,
                        );
                    }
                }
            }
        }
        Type::void()
    }

    pub(crate) fn check_builtin_scanf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() < 2 {
            self.report_error(
                "scanf 至少需要 2 个参数（格式字符串和地址）",
                loc,
                ErrorCode::E3033_ScanfArgCount,
            );
        } else {
            let fmt_type = self.resolve_expr_type(&mut args[0]);
            if !fmt_type.is_pointer() && !fmt_type.is_array() {
                self.report_error("scanf 的第一个参数必须是字符串", loc, ErrorCode::E3034_ScanfFirstArg);
            }
            let fmt_str = if let Expr::StringLiteral { ref value, .. } = args[0] {
                Some(value.clone())
            } else {
                None
            };
            if let Some(fmt) = fmt_str {
                self.check_scanf_format(&fmt, &mut args[1..], loc);
            } else {
                for (i, arg) in args.iter_mut().enumerate().skip(1) {
                    let arg_type = self.resolve_expr_type(arg);
                    if !arg_type.is_pointer() && !arg_type.is_array() {
                        self.report_error(
                            &format!("scanf 的第 {} 个参数必须是指针", i + 1),
                            loc,
                            ErrorCode::E3035_ScanfArgType,
                        );
                    }
                }
            }
        }
        // C11 7.21.6.2：scanf 返回成功匹配并赋值的项数（输入失败时为 EOF）。
        // 此前声明为 void —— 教学代码 `int r = scanf("%d", &x);` 会报 E3004，
        // `while (scanf(...) != EOF)` 类写法完全不可用（条目 3，2026-09-11 修正）。
        Type::int()
    }

    pub(crate) fn check_builtin_getchar(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        self.builtin_check_count(args, 0, "getchar", loc);
        Type::int()
    }

    pub(crate) fn check_builtin_ungetc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 2, "ungetc", loc) {
            self.builtin_check_int(&mut args[0], 0, "ungetc", loc);
            self.builtin_check_int(&mut args[1], 1, "ungetc", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_putchar(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "putchar", loc) {
            self.builtin_check_int(&mut args[0], 0, "putchar", loc);
        }
        Type::void()
    }

    pub(crate) fn check_builtin_fprintf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() < 2 {
            self.report_error(
                "fprintf 至少需要 2 个参数（文件指针和格式字符串）",
                loc,
                ErrorCode::E3030_PrintfArgCount,
            );
        } else {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fprintf 的第一个参数必须是文件指针或整数", loc, ErrorCode::E3029_BuiltInArgType);
            }
            let fmt_type = self.resolve_expr_type(&mut args[1]);
            if !fmt_type.is_pointer() && !fmt_type.is_array() {
                self.report_error("fprintf 的第二个参数必须是字符串", loc, ErrorCode::E3031_PrintfFirstArg);
            }
            for (i, arg) in args.iter_mut().enumerate().skip(2) {
                let arg_type = self.resolve_expr_type(arg);
                if !self.is_scalar(&arg_type) && !arg_type.is_pointer() && !arg_type.is_array() {
                    self.report_error(
                        &format!("fprintf 的第 {} 个参数必须是 int、float、char 或指针", i + 1),
                        loc,
                        ErrorCode::E3032_PrintfArgType,
                    );
                }
            }
        }
        Type::void()
    }

    pub(crate) fn check_builtin_puts(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "puts", loc) {
            self.builtin_check_pointer(&mut args[0], 0, "puts", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_sprintf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() < 2 {
            self.report_error("sprintf 至少需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "sprintf", loc);
            self.builtin_check_pointer(&mut args[1], 1, "sprintf", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_snprintf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() < 3 {
            self.report_error("snprintf 至少需要三个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "snprintf", loc);
            self.builtin_check_int(&mut args[1], 1, "snprintf", loc);
            self.builtin_check_pointer(&mut args[2], 2, "snprintf", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_sscanf(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() < 2 {
            self.report_error("sscanf 至少需要两个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            self.builtin_check_pointer(&mut args[0], 0, "sscanf", loc);
            self.builtin_check_pointer(&mut args[1], 1, "sscanf", loc);
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fgets(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 3, "fgets", loc) {
            let buf_type = self.resolve_expr_type(&mut args[0]);
            if !buf_type.is_pointer() && !buf_type.is_array() {
                self.report_error("fgets 第一个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            let n_type = self.resolve_expr_type(&mut args[1]);
            if !self.check_assignable(&Type::int(), &n_type, loc) {
                self.report_error("fgets 第二个参数必须是 int", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[1], &Type::int());
            }
            let stream_type = self.resolve_expr_type(&mut args[2]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fgets 第三个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::pointer_to(Type::char())
    }

    pub(crate) fn check_builtin_fputs(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 2, "fputs", loc) {
            let s_type = self.resolve_expr_type(&mut args[0]);
            if !s_type.is_pointer() && !s_type.is_array() {
                self.report_error("fputs 第一个参数必须是字符串", loc, ErrorCode::E3029_BuiltInArgType);
            }
            let stream_type = self.resolve_expr_type(&mut args[1]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fputs 第二个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }
}
