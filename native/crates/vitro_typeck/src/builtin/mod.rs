use super::*;

mod file;
mod io;
mod math;
mod memory;
mod string;

impl TypeChecker {
    pub(crate) fn is_builtin_func(&self, name: &str) -> bool {
        vitro_runtime::host_func_id::is_builtin(name)
            || vitro_runtime::bytecode_libc_index::BYTECODE_LIBC_ALL_FUNCS.contains(&name)
    }

    pub(crate) fn visit_call(&mut self, name: &str, args: &mut Vec<Expr>, loc: &SourceLoc) -> Type {
        match name {
            "malloc" => self.check_builtin_malloc(args, loc),
            "free" => self.check_builtin_free(args, loc),
            "print_int" | "__vitro_output" | "__vitro_step" => self.check_builtin_print_int(args, loc, name),
            "printf" => self.check_builtin_printf(args, loc),
            "scanf" => self.check_builtin_scanf(args, loc),
            "strlen" => self.check_builtin_strlen(args, loc),
            "strcpy" => self.check_builtin_strcpy(args, loc),
            "strcmp" => self.check_builtin_strcmp(args, loc),
            "strdup" => self.check_builtin_strdup(args, loc),
            "getchar" => self.check_builtin_getchar(args, loc),
            "putchar" => self.check_builtin_putchar(args, loc),
            "ungetc" => self.check_builtin_ungetc(args, loc),
            "rand" => self.check_builtin_rand(args, loc),
            "srand" => self.check_builtin_srand(args, loc),
            "memset" => self.check_builtin_memset(args, loc),
            "exit" => self.check_builtin_exit(args, loc),
            "strcat" => self.check_builtin_strcat(args, loc),
            "atoi" => self.check_builtin_atoi(args, loc),
            "fopen" => self.check_builtin_fopen(args, loc),
            "fread" => self.check_builtin_fread(args, loc),
            "fwrite" => self.check_builtin_fwrite(args, loc),
            "fclose" => self.check_builtin_fclose(args, loc),
            "feof" => self.check_builtin_feof(args, loc),
            "fgets" => self.check_builtin_fgets(args, loc),
            "fputs" => self.check_builtin_fputs(args, loc),
            "fprintf" => self.check_builtin_fprintf(args, loc),
            "realloc" => self.check_builtin_realloc(args, loc),
            "qsort" => {
                if self.funcs.contains_key(name) {
                    self.check_user_func(name, args, loc)
                } else {
                    self.check_builtin_qsort(args, loc)
                }
            }
            "puts" => self.check_builtin_puts(args, loc),
            "calloc" => self.check_builtin_calloc(args, loc),
            "bsearch" => self.check_builtin_bsearch(args, loc),
            "sprintf" => self.check_builtin_sprintf(args, loc),
            "snprintf" => self.check_builtin_snprintf(args, loc),
            "sscanf" => self.check_builtin_sscanf(args, loc),
            "fgetc" => self.check_builtin_fgetc(args, loc),
            "fputc" => self.check_builtin_fputc(args, loc),
            "fseek" => self.check_builtin_fseek(args, loc),
            "ftell" => self.check_builtin_ftell(args, loc),
            "rewind" => self.check_builtin_rewind(args, loc),
            "strncat" => self.check_builtin_strncat(args, loc),
            "strncmp" => self.check_builtin_strncmp(args, loc),
            "memcmp" => self.check_builtin_memcmp(args, loc),
            "strchr" => self.check_builtin_strchr(args, loc),
            "strrchr" => self.check_builtin_strrchr(args, loc),
            "strstr" => self.check_builtin_strstr(args, loc),
            "memchr" => self.check_builtin_memchr(args, loc),
            "atof" => self.check_builtin_atof(args, loc),
            "atol" => self.check_builtin_atol(args, loc),
            "tan" => self.check_builtin_tan(args, loc),
            "log10" => self.check_builtin_log10(args, loc),
            "fabs" => self.check_builtin_fabs(args, loc),
            "ceil" => self.check_builtin_ceil(args, loc),
            "floor" => self.check_builtin_floor(args, loc),
            "round" => self.check_builtin_round(args, loc),
            "fmod" => self.check_builtin_fmod(args, loc),
            "std__move" => {
                if args.len() != 1 {
                    self.report_error("std::move 预期 1 个参数", loc, ErrorCode::E3037_FuncArgCount);
                    return Type::int();
                }
                let arg_ty = self.resolve_expr_type(&mut args[0]);
                Type::RValueRef { base: Box::new(arg_ty) }
            }
            // math.h functions are resolved through stub declarations in funcs
            _ => self.check_user_func(name, args, loc),
        }
    }

    pub(crate) fn parse_format_specs(fmt: &str) -> Vec<(char, String)> {
        let mut specs = Vec::new();
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(&next) = chars.peek() {
                    if next == '%' {
                        chars.next(); // skip %%
                        continue;
                    }
                }
                // skip flags
                while let Some(&c) = chars.peek() {
                    if c == '-' || c == '+' || c == ' ' || c == '#' || c == '0' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                // skip width
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '*' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                // skip precision
                if let Some(&'.') = chars.peek() {
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '*' {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                // length modifier
                let mut len_mod = String::new();
                if let Some(&c) = chars.peek() {
                    if c == 'l' || c == 'h' || c == 'L' || c == 'z' || c == 'j' || c == 't' {
                        chars.next();
                        len_mod.push(c);
                        if c == 'l' || c == 'h' {
                            if let Some(&c2) = chars.peek() {
                                if c2 == c {
                                    chars.next();
                                    len_mod.push(c2);
                                }
                            }
                        }
                    }
                }
                if let Some(&spec) = chars.peek() {
                    chars.next();
                    specs.push((spec, len_mod));
                }
            }
        }
        specs
    }

    pub(crate) fn check_printf_format(&mut self, fmt: &str, args: &mut [Expr], loc: &SourceLoc) {
        let specs = Self::parse_format_specs(fmt);
        if specs.len() != args.len() {
            self.report_error(
                &format!("printf 格式说明符数量（{}）与参数数量（{}）不匹配", specs.len(), args.len()),
                loc,
                ErrorCode::E3032_PrintfArgType,
            );
        }
        for (i, ((spec, len_mod), arg)) in specs.iter().zip(args.iter_mut()).enumerate() {
            let arg_type = self.resolve_expr_type(arg);
            let ok = match (*spec, len_mod.as_str()) {
                ('d' | 'i' | 'u' | 'x' | 'X' | 'o' | 'n', "") => {
                    matches!(arg_type.kind(), TypeKind::Int | TypeKind::Char)
                }
                ('d' | 'i' | 'u' | 'x' | 'X' | 'o', "l" | "ll") => {
                    matches!(arg_type.kind(), TypeKind::LongLong | TypeKind::Int)
                }
                ('f' | 'e' | 'g' | 'E' | 'G', "") => {
                    matches!(arg_type.kind(), TypeKind::Float | TypeKind::Double)
                }
                ('f' | 'e' | 'g' | 'E' | 'G', "l") | ('F', "") => {
                    matches!(arg_type.kind(), TypeKind::Double | TypeKind::Float)
                }
                ('c', "") => {
                    matches!(arg_type.kind(), TypeKind::Int | TypeKind::Char)
                }
                ('s', "") => arg_type.is_pointer() || arg_type.is_array(),
                ('p', "") => arg_type.is_pointer() || arg_type.is_array(),
                _ => true, // unknown spec, be permissive
            };
            if !ok {
                self.report_error(
                    &format!("printf 格式 '%{}' 与第 {} 个参数类型 '{}' 不匹配", spec, i + 2, arg_type),
                    loc,
                    ErrorCode::E3062_PrintfFormatMismatch,
                );
            }
        }
    }

    pub(crate) fn check_scanf_format(&mut self, fmt: &str, args: &mut [Expr], loc: &SourceLoc) {
        let specs = Self::parse_format_specs(fmt);
        if specs.len() != args.len() {
            self.report_error(
                &format!("scanf 格式说明符数量（{}）与参数数量（{}）不匹配", specs.len(), args.len()),
                loc,
                ErrorCode::E3035_ScanfArgType,
            );
        }
        for (i, ((spec, len_mod), arg)) in specs.iter().zip(args.iter_mut()).enumerate() {
            let arg_type = self.resolve_expr_type(arg);
            // scanf args must be pointers
            let pointee = if let Type::Pointer { pointee, .. } = &arg_type {
                Some((**pointee).clone())
            } else if let Type::Array { element, .. } = &arg_type {
                Some((**element).clone())
            } else {
                None
            };
            let ok = match (*spec, len_mod.as_str()) {
                ('d' | 'i' | 'u' | 'x' | 'X' | 'o' | 'n', "") => pointee
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind(), TypeKind::Int | TypeKind::Char)),
                ('d' | 'i' | 'u' | 'x' | 'X' | 'o', "l" | "ll") => pointee
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind(), TypeKind::LongLong | TypeKind::Int)),
                ('f' | 'e' | 'g' | 'E' | 'G', "") => pointee
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind(), TypeKind::Float | TypeKind::Int)),
                ('f' | 'e' | 'g' | 'E' | 'G', "l") | ('F', "") => pointee
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind(), TypeKind::Double | TypeKind::Float | TypeKind::Int)),
                ('c', "") => pointee
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind(), TypeKind::Char | TypeKind::Int)),
                ('s', "") => arg_type.is_pointer() || arg_type.is_array(),
                ('p', "") => arg_type.is_pointer() || arg_type.is_array(),
                _ => true,
            };
            if !ok {
                self.report_error(
                    &format!("scanf 格式 '%{}' 与第 {} 个参数类型 '{}' 不匹配", spec, i + 2, arg_type),
                    loc,
                    ErrorCode::E3063_ScanfFormatMismatch,
                );
            }
        }
    }

    // ---------- 内建函数检查器辅助方法 ----------

    fn builtin_check_count(&mut self, args: &[Expr], expected: usize, name: &str, loc: &SourceLoc) -> bool {
        if args.len() != expected {
            self.report_error(
                &format!("{} 需要{}个参数", name, expected),
                loc,
                ErrorCode::E3028_BuiltInArgCount,
            );
            false
        } else {
            true
        }
    }

    fn builtin_check_pointer(&mut self, arg: &mut Expr, idx: usize, name: &str, loc: &SourceLoc) {
        let arg_type = self.resolve_expr_type(arg);
        if !arg_type.is_pointer() && !arg_type.is_array() {
            self.report_error(
                &format!("{} 的第 {} 个参数必须是指针或数组", name, idx + 1),
                loc,
                ErrorCode::E3029_BuiltInArgType,
            );
        }
    }

    fn builtin_check_int(&mut self, arg: &mut Expr, idx: usize, name: &str, loc: &SourceLoc) {
        let expected = Type::int();
        let arg_type = self.resolve_expr_type(arg);
        if !self.check_assignable(&expected, &arg_type, loc) {
            self.report_error(
                &format!("{} 的第 {} 个参数必须是 int", name, idx + 1),
                loc,
                ErrorCode::E3029_BuiltInArgType,
            );
        } else {
            insert_implicit_cast(arg, &expected);
        }
    }

    // ---------- 内建函数检查器 ----------

    fn check_builtin_qsort(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 4 {
            self.report_error("qsort 需要四个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let base_type = self.resolve_expr_type(&mut args[0]);
            if !base_type.is_pointer() && !base_type.is_array() {
                self.report_error("qsort 第一个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for i in 1..3 {
                let t = self.resolve_expr_type(&mut args[i]);
                if !self.check_assignable(&Type::int(), &t, loc) {
                    self.report_error(
                        &format!("qsort 第 {} 个参数必须是 int", i + 1),
                        loc,
                        ErrorCode::E3029_BuiltInArgType,
                    );
                } else {
                    insert_implicit_cast(&mut args[i], &Type::int());
                }
            }
            let compar_type = self.resolve_expr_type(&mut args[3]);
            if !matches!(compar_type.kind(), TypeKind::Int) && !compar_type.is_pointer() {
                self.report_error("qsort 第四个参数必须是函数指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::void()
    }

    fn check_builtin_bsearch(&mut self, args: &mut [Expr], loc: &SourceLoc) -> Type {
        if args.len() != 5 {
            self.report_error("bsearch 需要五个参数", loc, ErrorCode::E3028_BuiltInArgCount);
        } else {
            let key_type = self.resolve_expr_type(&mut args[0]);
            if !key_type.is_pointer() && !key_type.is_array() {
                self.report_error("bsearch 第一个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            let base_type = self.resolve_expr_type(&mut args[1]);
            if !base_type.is_pointer() && !base_type.is_array() {
                self.report_error("bsearch 第二个参数必须是指针或数组", loc, ErrorCode::E3029_BuiltInArgType);
            }
            for (i, arg) in args.iter_mut().enumerate().take(4).skip(2) {
                self.builtin_check_int(arg, i, "bsearch", loc);
            }
            let compar_type = self.resolve_expr_type(&mut args[4]);
            if !matches!(compar_type.kind(), TypeKind::Int) && !compar_type.is_pointer() {
                self.report_error("bsearch 第五个参数必须是函数指针", loc, ErrorCode::E3029_BuiltInArgType);
            }
        }
        Type::pointer_to(Type::void())
    }
}
