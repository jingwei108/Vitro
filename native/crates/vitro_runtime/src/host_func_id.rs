//! Host Function ID 统一常量定义
//!
//! 编译期（bytecode_gen.rs）和运行期（host_funcs.rs）共用此映射，
//! 避免新增 host 函数时两处 ID 不一致。

pub const OUTPUT: u32 = 0;
pub const STEP: u32 = 1;
pub const MALLOC: u32 = 2;
pub const FREE: u32 = 3;
pub const PRINTF_N: u32 = 15;
pub const SCANF_N: u32 = 21;
pub const STRLEN: u32 = 30;
pub const STRCPY: u32 = 31;
pub const STRCMP: u32 = 32;
pub const GETCHAR: u32 = 33;
pub const PUTCHAR: u32 = 34;
pub const RAND: u32 = 35;
pub const SRAND: u32 = 36;
pub const MEMSET: u32 = 37;
pub const EXIT: u32 = 38;
pub const STRCAT: u32 = 39;
pub const ATOI: u32 = 40;
pub const ABS: u32 = 41;
pub const ISDIGIT: u32 = 42;
pub const ISALPHA: u32 = 43;
pub const ISLOWER: u32 = 44;
pub const ISUPPER: u32 = 45;
pub const TOLOWER: u32 = 46;
pub const TOUPPER: u32 = 47;
pub const ISSPACE: u32 = 48;
pub const ISALNUM: u32 = 49;
pub const ISPRINT: u32 = 53;
pub const ISCNTRL: u32 = 54;
pub const ISXDIGIT: u32 = 55;
pub const STRNCPY: u32 = 56;
pub const MEMCPY: u32 = 57;
pub const MEMMOVE: u32 = 58;
pub const FPRINTF: u32 = 50;
pub const REALLOC: u32 = 51;
pub const QSORT: u32 = 52;
// VFS-backed 文件 I/O host 函数常量（已在 host_funcs.rs / vfs.rs 完整实现）。
pub const FOPEN: u32 = 60;
pub const FREAD: u32 = 61;
pub const FWRITE: u32 = 62;
pub const FCLOSE: u32 = 63;
pub const FEOF: u32 = 64;
pub const FGETS: u32 = 65;
pub const FPUTS: u32 = 66;

// math.h
pub const SIN: u32 = 70;
pub const COS: u32 = 71;
pub const SQRT: u32 = 72;
pub const POW: u32 = 73;
pub const ATAN: u32 = 74;
pub const LOG: u32 = 75;
pub const EXP: u32 = 76;
pub const STRDUP: u32 = 77;
pub const UNGETC: u32 = 78;
pub const PUTS: u32 = 79;
pub const CALLOC: u32 = 80;
pub const BSEARCH: u32 = 81;
pub const SPRINTF: u32 = 82;
pub const SNPRINTF: u32 = 83;
pub const SSCANF: u32 = 84;

// VFS I/O 扩展
pub const FGETC: u32 = 85;
pub const FPUTC: u32 = 86;
pub const FSEEK: u32 = 87;
pub const FTELL: u32 = 88;
pub const REWIND: u32 = 89;

// 字符串/内存扩展
pub const STRNCAT: u32 = 90;
pub const STRNCMP: u32 = 91;
pub const MEMCMP: u32 = 92;
pub const STRCHR: u32 = 93;
pub const STRRCHR: u32 = 94;
pub const STRSTR: u32 = 95;
pub const MEMCHR: u32 = 96;

// 转换扩展
pub const ATOF: u32 = 97;
pub const ATOL: u32 = 98;

// 数学扩展
pub const TAN: u32 = 99;
pub const LOG10: u32 = 100;
pub const FABS: u32 = 101;
pub const CEIL: u32 = 102;
pub const FLOOR: u32 = 103;
pub const ROUND: u32 = 104;
pub const FMOD: u32 = 105;

// ctype 补全
pub const ISGRAPH: u32 = 106;
pub const ISPUNCT: u32 = 107;
pub const ISBLANK: u32 = 108;

// math 补全
pub const ASIN: u32 = 109;
pub const ACOS: u32 = 110;
pub const ATAN2: u32 = 111;
pub const SINH: u32 = 112;
pub const COSH: u32 = 113;
pub const TANH: u32 = 114;

// stdlib 补全
pub const LLABS: u32 = 115;
pub const ABORT: u32 = 116;
pub const STRTOL: u32 = 117;
pub const STRTOD: u32 = 118;
pub const STRERROR: u32 = 119;

// stdio 补全
pub const FFLUSH: u32 = 120;
pub const PERROR: u32 = 121;
pub const CLEARERR: u32 = 122;

// time.h
pub const TIME: u32 = 123;
pub const CLOCK: u32 = 124;
pub const ASSERT_FAIL: u32 = 125;

// C++ new[] 构造守卫（构造失败时回滚释放内存）
pub const SET_ARRAY_GUARD: u32 = 131;
pub const CLEAR_ARRAY_GUARD: u32 = 132;

// stdio 扩展
pub const REMOVE: u32 = 126;
pub const RENAME: u32 = 127;

// string 扩展
pub const STRPBRK: u32 = 128;
pub const STRSPN: u32 = 129;
pub const STRCSPN: u32 = 130;

// stdarg.h 变参支持
pub const VA_START: u32 = 140;
pub const VA_ARG: u32 = 141;
pub const VA_END: u32 = 142;
pub const VA_COPY: u32 = 143;
pub const UNREACHABLE: u32 = 144;

/// 已由 Bytecode Libc 覆盖的纯计算函数。
/// 这些函数不再走 CallHost 路径，而是走 Bytecode Libc 的固定索引 Call。
/// 诊断敏感的函数（strcpy、printf、malloc 等）继续保留 Host Func 路径。
pub const BYTECODE_LIBC_PURE_FUNCS: &[&str] = &[
    "isdigit", "isalpha", "islower", "isupper", "tolower", "toupper", "isspace", "isalnum", "isprint", "iscntrl",
    "isxdigit", "abs", "strlen", "strcmp",
];

/// 判断函数是否已由 Bytecode Libc 覆盖（纯计算函数）。
pub fn is_bytecode_libc_pure(name: &str) -> bool {
    BYTECODE_LIBC_PURE_FUNCS.contains(&name)
}

/// 将用户代码中的函数名解析为 host function ID。
/// 包含别名映射（如 `print_int` → `OUTPUT`, `printf` → `PRINTF_N`）。
/// 已由 Bytecode Libc 覆盖的纯计算函数返回 None，确保生成 Call 而非 CallHost。
pub fn by_user_name(name: &str) -> Option<u32> {
    if is_bytecode_libc_pure(name) {
        return None;
    }
    match name {
        "print_int" | "__vitro_output" => Some(OUTPUT),
        "__vitro_step" => Some(STEP),
        "malloc" => Some(MALLOC),
        "free" => Some(FREE),
        "printf" => Some(PRINTF_N),
        "scanf" => Some(SCANF_N),
        "strlen" => Some(STRLEN),
        "strcpy" => Some(STRCPY),
        "strcmp" => Some(STRCMP),
        "getchar" => Some(GETCHAR),
        "putchar" => Some(PUTCHAR),
        "rand" => Some(RAND),
        "srand" => Some(SRAND),
        "memset" => Some(MEMSET),
        "exit" => Some(EXIT),
        "strcat" => Some(STRCAT),
        "atoi" => Some(ATOI),
        "abs" => Some(ABS),
        "isdigit" => Some(ISDIGIT),
        "isalpha" => Some(ISALPHA),
        "islower" => Some(ISLOWER),
        "isupper" => Some(ISUPPER),
        "tolower" => Some(TOLOWER),
        "toupper" => Some(TOUPPER),
        "isspace" => Some(ISSPACE),
        "isalnum" => Some(ISALNUM),
        "isprint" => Some(ISPRINT),
        "iscntrl" => Some(ISCNTRL),
        "isxdigit" => Some(ISXDIGIT),
        "strncpy" => Some(STRNCPY),
        "memcpy" => Some(MEMCPY),
        "memmove" => Some(MEMMOVE),
        "fprintf" => Some(FPRINTF),
        "realloc" => Some(REALLOC),
        "qsort" => Some(QSORT),
        "fopen" => Some(FOPEN),
        "fread" => Some(FREAD),
        "fwrite" => Some(FWRITE),
        "fclose" => Some(FCLOSE),
        "feof" => Some(FEOF),
        "fgets" => Some(FGETS),
        "fputs" => Some(FPUTS),
        "sin" => Some(SIN),
        "cos" => Some(COS),
        "sqrt" => Some(SQRT),
        "pow" => Some(POW),
        "atan" => Some(ATAN),
        "log" => Some(LOG),
        "exp" => Some(EXP),
        "strdup" => Some(STRDUP),
        "ungetc" => Some(UNGETC),
        "puts" => Some(PUTS),
        "calloc" => Some(CALLOC),
        "bsearch" => Some(BSEARCH),
        "sprintf" => Some(SPRINTF),
        "snprintf" => Some(SNPRINTF),
        "sscanf" => Some(SSCANF),
        "fgetc" => Some(FGETC),
        "fputc" => Some(FPUTC),
        "fseek" => Some(FSEEK),
        "ftell" => Some(FTELL),
        "rewind" => Some(REWIND),
        "strncat" => Some(STRNCAT),
        "strncmp" => Some(STRNCMP),
        "memcmp" => Some(MEMCMP),
        "strchr" => Some(STRCHR),
        "strrchr" => Some(STRRCHR),
        "strstr" => Some(STRSTR),
        "memchr" => Some(MEMCHR),
        "atof" => Some(ATOF),
        "atol" => Some(ATOL),
        "tan" => Some(TAN),
        "log10" => Some(LOG10),
        "fabs" => Some(FABS),
        "ceil" => Some(CEIL),
        "floor" => Some(FLOOR),
        "round" => Some(ROUND),
        "fmod" => Some(FMOD),
        "isgraph" => Some(ISGRAPH),
        "ispunct" => Some(ISPUNCT),
        "isblank" => Some(ISBLANK),
        "asin" => Some(ASIN),
        "acos" => Some(ACOS),
        "atan2" => Some(ATAN2),
        "sinh" => Some(SINH),
        "cosh" => Some(COSH),
        "tanh" => Some(TANH),
        "labs" => Some(ABS),
        "llabs" => Some(LLABS),
        "abort" => Some(ABORT),
        "strtol" => Some(STRTOL),
        "strtod" => Some(STRTOD),
        "strerror" => Some(STRERROR),
        "fflush" => Some(FFLUSH),
        "perror" => Some(PERROR),
        "clearerr" => Some(CLEARERR),
        "time" => Some(TIME),
        "clock" => Some(CLOCK),
        "__vitro_assert_fail" => Some(ASSERT_FAIL),
        "remove" => Some(REMOVE),
        "rename" => Some(RENAME),
        "strpbrk" => Some(STRPBRK),
        "strspn" => Some(STRSPN),
        "strcspn" => Some(STRCSPN),
        "__vitro_va_start" => Some(VA_START),
        "__vitro_va_arg" => Some(VA_ARG),
        "__vitro_va_end" => Some(VA_END),
        "__vitro_va_copy" => Some(VA_COPY),
        "unreachable" => Some(UNREACHABLE),
        _ => None,
    }
}

/// 判断名称是否为内置宿主函数（供 TypeChecker 使用）。
/// 包含仍走 Host 路径的函数和已切换为 Bytecode Libc 的纯计算函数。
pub fn is_builtin(name: &str) -> bool {
    by_user_name(name).is_some() || is_bytecode_libc_pure(name)
}
