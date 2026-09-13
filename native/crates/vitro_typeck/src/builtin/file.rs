use super::*;

impl TypeChecker {
    pub(crate) fn check_builtin_fopen(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("fopen 需要两个参数（路径和模式）", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            for (i, arg) in args.iter_mut().enumerate() {
                let arg_type = self.resolve_expr_type(arg);
                if !arg_type.is_pointer() && !arg_type.is_array() {
                    self.report_error(
                        &format!("fopen 第 {} 个参数必须是字符串", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                }
            }
        }
        Type::pointer_to(Type::void())
    }

    pub(crate) fn check_builtin_fread(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 4 {
            self.report_error(
                "fread 需要四个参数（缓冲区、元素大小、元素数量、文件指针）",
                loc,
                ErrorCode::E3028_BuiltInArgCount,
            );
        } else {
            let buf_type = self.resolve_expr_type(&mut args[0]);
            if !buf_type.is_pointer() && !buf_type.is_array() {
                self.report_error("fread 第一个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for i in 1..3 {
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&Type::int(), &t, loc) {
                    self.report_error(
                        &format!("fread 第 {} 个参数必须是 int", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &Type::int());
                }
            }
            let stream_type = self.resolve_expr_type(&mut args[3]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fread 第四个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fwrite(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 4 {
            self.report_error(
                "fwrite 需要四个参数（缓冲区、元素大小、元素数量、文件指针）",
                loc,
                ErrorCode::E3028_BuiltInArgCount,
            );
        } else {
            let buf_type = self.resolve_expr_type(&mut args[0]);
            if !buf_type.is_pointer() && !buf_type.is_array() {
                self.report_error("fwrite 第一个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for i in 1..3 {
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&Type::int(), &t, loc) {
                    self.report_error(
                        &format!("fwrite 第 {} 个参数必须是 int", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &Type::int());
                }
            }
            let stream_type = self.resolve_expr_type(&mut args[3]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fwrite 第四个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fclose(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "fclose", loc) {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fclose 参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_feof(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "feof", loc) {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("feof 参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fgetc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "fgetc", loc) {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fgetc 参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fputc(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 2 {
            self.report_error("fputc 需要两个参数（字符和文件指针）", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let c_type = self.resolve_expr_type(&mut args[0]);
            if !self.check_assignable(&Type::int(), &c_type, loc) {
                self.report_error("fputc 第一个参数必须是 int", loc, ErrorCode::E3029_BuiltInArgType);
            } else {
                insert_implicit_cast(&mut args[0], &Type::int());
            }
            let stream_type = self.resolve_expr_type(&mut args[1]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fputc 第二个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_fseek(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 3 {
            self.report_error(
                "fseek 需要三个参数（文件指针、偏移量、起始位置）",
                loc,
                ErrorCode::E3028_BuiltInArgCount,
            );
        } else {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("fseek 第一个参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for i in 1..3 {
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&Type::int(), &t, loc) {
                    self.report_error(
                        &format!("fseek 第 {} 个参数必须是 int", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &Type::int());
                }
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_ftell(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "ftell", loc) {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("ftell 参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::int()
    }

    pub(crate) fn check_builtin_rewind(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if self.builtin_check_count(args, 1, "rewind", loc) {
            let stream_type = self.resolve_expr_type(&mut args[0]);
            if !stream_type.is_pointer() && !matches!(stream_type.kind(), TypeKind::Int) {
                self.report_error("rewind 参数必须是文件指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::void()
    }
}
