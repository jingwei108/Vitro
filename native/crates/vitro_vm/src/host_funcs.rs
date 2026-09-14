// NOTE: Host 函数已按功能拆分到 vm/host/*.rs；utils.rs 集中共享工具，未来如需进一步解耦可继续拆分。
use super::core::VitroVM;
use super::host_func_id;
use crate::context::VmContext;
use vitro_runtime::MemoryRegionData;

pub(crate) use super::core::{ArrayConstructionGuard, FreedRegionInfo, NULL_TRAP_SIZE};
pub(crate) use vitro_runtime::FreeBlock;
pub(crate) use vitro_shared::SourceLoc;

#[path = "host/file.rs"]
mod file;
#[path = "host/io.rs"]
mod io;
#[path = "host/math.rs"]
mod math;
#[path = "host/memory.rs"]
mod memory;
#[path = "host/misc.rs"]
mod misc;
#[path = "host/string.rs"]
mod string;
#[path = "host/utils.rs"]
mod utils;
pub use file::*;
pub use io::*;
pub use math::*;
pub use memory::*;
pub use misc::*;
pub use string::*;
pub(crate) use utils::*;

pub fn execute_host_func(vm: &mut VitroVM, session: &mut VmContext<'_>, id: u32) {
    match id {
        host_func_id::OUTPUT => host_output(vm, session),
        host_func_id::STEP => host_step(vm, session),
        host_func_id::MALLOC => host_malloc(vm, session),
        host_func_id::FREE => host_free(vm, session),
        host_func_id::PRINTF_N => host_printf_n(vm, session),
        host_func_id::SCANF_N => host_scanf_n(vm, session),
        host_func_id::STRLEN => host_strlen(vm, session),
        host_func_id::STRCPY => host_strcpy(vm, session),
        host_func_id::STRCMP => host_strcmp(vm, session),
        host_func_id::GETCHAR => host_getchar(vm, session),
        host_func_id::PUTCHAR => host_putchar(vm, session),
        host_func_id::RAND => host_rand(vm, session),
        host_func_id::SRAND => host_srand(vm, session),
        host_func_id::MEMSET => host_memset(vm, session),
        host_func_id::EXIT => host_exit(vm, session),
        host_func_id::STRCAT => host_strcat(vm, session),
        host_func_id::ATOI => host_atoi(vm, session),
        host_func_id::ABS => host_abs(vm, session),
        host_func_id::ISDIGIT => host_isdigit(vm, session),
        host_func_id::ISALPHA => host_isalpha(vm, session),
        host_func_id::ISLOWER => host_islower(vm, session),
        host_func_id::ISUPPER => host_isupper(vm, session),
        host_func_id::TOLOWER => host_tolower(vm, session),
        host_func_id::TOUPPER => host_toupper(vm, session),
        host_func_id::ISSPACE => host_isspace(vm, session),
        host_func_id::ISALNUM => host_isalnum(vm, session),
        host_func_id::ISPRINT => host_isprint(vm, session),
        host_func_id::ISCNTRL => host_iscntrl(vm, session),
        host_func_id::ISXDIGIT => host_isxdigit(vm, session),
        host_func_id::STRNCPY => host_strncpy(vm, session),
        host_func_id::MEMCPY => host_memcpy(vm, session),
        host_func_id::MEMMOVE => host_memmove(vm, session),
        host_func_id::FPRINTF => host_fprintf_n(vm, session),
        host_func_id::REALLOC => host_realloc(vm, session),
        host_func_id::QSORT => host_qsort(vm, session),
        host_func_id::FOPEN => host_fopen(vm, session),
        host_func_id::FREAD => host_fread(vm, session),
        host_func_id::FWRITE => host_fwrite(vm, session),
        host_func_id::FCLOSE => host_fclose(vm, session),
        host_func_id::FEOF => host_feof(vm, session),
        host_func_id::FGETS => host_fgets(vm, session),
        host_func_id::FPUTS => host_fputs(vm, session),
        host_func_id::SIN => host_sin(vm, session),
        host_func_id::COS => host_cos(vm, session),
        host_func_id::SQRT => host_sqrt(vm, session),
        host_func_id::POW => host_pow(vm, session),
        host_func_id::ATAN => host_atan(vm, session),
        host_func_id::LOG => host_log(vm, session),
        host_func_id::EXP => host_exp(vm, session),
        host_func_id::STRDUP => host_strdup(vm, session),
        host_func_id::UNGETC => host_ungetc(vm, session),
        host_func_id::PUTS => host_puts(vm, session),
        host_func_id::CALLOC => host_calloc(vm, session),
        host_func_id::BSEARCH => host_bsearch(vm, session),
        host_func_id::SPRINTF => host_sprintf(vm, session),
        host_func_id::SNPRINTF => host_snprintf(vm, session),
        host_func_id::SSCANF => host_sscanf(vm, session),
        host_func_id::FGETC => host_fgetc(vm, session),
        host_func_id::FPUTC => host_fputc(vm, session),
        host_func_id::FSEEK => host_fseek(vm, session),
        host_func_id::FTELL => host_ftell(vm, session),
        host_func_id::REWIND => host_rewind(vm, session),
        host_func_id::STRNCAT => host_strncat(vm, session),
        host_func_id::STRNCMP => host_strncmp(vm, session),
        host_func_id::MEMCMP => host_memcmp(vm, session),
        host_func_id::STRCHR => host_strchr(vm, session),
        host_func_id::STRRCHR => host_strrchr(vm, session),
        host_func_id::STRSTR => host_strstr(vm, session),
        host_func_id::MEMCHR => host_memchr(vm, session),
        host_func_id::ATOF => host_atof(vm, session),
        host_func_id::ATOL => host_atol(vm, session),
        host_func_id::TAN => host_tan(vm, session),
        host_func_id::LOG10 => host_log10(vm, session),
        host_func_id::FABS => host_fabs(vm, session),
        host_func_id::CEIL => host_ceil(vm, session),
        host_func_id::FLOOR => host_floor(vm, session),
        host_func_id::ROUND => host_round(vm, session),
        host_func_id::FMOD => host_fmod(vm, session),
        host_func_id::ISGRAPH => host_isgraph(vm, session),
        host_func_id::ISPUNCT => host_ispunct(vm, session),
        host_func_id::ISBLANK => host_isblank(vm, session),
        host_func_id::ASIN => host_asin(vm, session),
        host_func_id::ACOS => host_acos(vm, session),
        host_func_id::ATAN2 => host_atan2(vm, session),
        host_func_id::SINH => host_sinh(vm, session),
        host_func_id::COSH => host_cosh(vm, session),
        host_func_id::TANH => host_tanh(vm, session),
        host_func_id::LLABS => host_llabs(vm, session),
        host_func_id::ABORT => host_abort(vm, session),
        host_func_id::STRTOL => host_strtol(vm, session),
        host_func_id::STRTOD => host_strtod(vm, session),
        host_func_id::STRERROR => host_strerror(vm, session),
        host_func_id::FFLUSH => host_fflush(vm, session),
        host_func_id::PERROR => host_perror(vm, session),
        host_func_id::CLEARERR => host_clearerr(vm, session),
        host_func_id::TIME => host_time(vm, session),
        host_func_id::CLOCK => host_clock(vm, session),
        host_func_id::ASSERT_FAIL => host_vitro_assert_fail(vm, session),
        host_func_id::SET_ARRAY_GUARD => host_set_array_guard(vm, session),
        host_func_id::CLEAR_ARRAY_GUARD => host_clear_array_guard(vm, session),
        host_func_id::REMOVE => host_remove(vm, session),
        host_func_id::RENAME => host_rename(vm, session),
        host_func_id::STRPBRK => host_strpbrk(vm, session),
        host_func_id::STRSPN => host_strspn(vm, session),
        host_func_id::STRCSPN => host_strcspn(vm, session),
        host_func_id::VA_START => host_va_start(vm, session),
        host_func_id::VA_ARG => host_va_arg(vm, session),
        host_func_id::VA_END => host_va_end(vm, session),
        host_func_id::VA_COPY => host_va_copy(vm, session),
        host_func_id::UNREACHABLE => host_unreachable(vm, session),
        _ => {}
    }
}

fn host_step(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let line = vm.pop() as i32;
    session.runtime.current_line = line;
    session.runtime.push_trace(vitro_runtime::TraceEntryData {
        line,
        operation: "step".to_string(),
    });
}

fn host_output(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let val = vm.pop();
    session.runtime.push_stdout(format!("{}\n", val));
}
