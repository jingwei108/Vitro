use crate::session::*;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::slice;

use crate::engine::compile_pipeline::run_multi_file_pipeline;
use crate::engine::session_ops::execute_run;

/// capi 第一批（2026-09-11 SharpTutor 评审定稿）：版本/错误/JSON 诊断与运行结果/
/// 输出游标增量/会话级保险丝（max_steps、call_depth_limit、deterministic）。
mod first_batch;
pub use first_batch::*;

/// 将 C 字符串指针安全转换为 Rust &str。
///
/// 内部完成 null 检查，可作为 safe 函数调用。
fn cstr_to_str(s: *const c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s).to_str().ok().map(|s| s.to_owned()) }
}

#[no_mangle]
pub extern "C" fn vitro_session_create() -> *mut Session {
    guard(ptr::null_mut(), || Box::into_raw(Box::new(Session::default())))
}

#[no_mangle]
/// vitro_session_destroy 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_session_destroy(s: *mut Session) {
    guard((), || {
        if !s.is_null() {
            drop(Box::from_raw(s));
        }
    })
}

#[no_mangle]
/// vitro_compile 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `source` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
/// - `source` 若非空，必须指向以 null 结尾的有效 UTF-8 字符串。
pub unsafe extern "C" fn vitro_compile(s: *mut Session, source: *const c_char) -> c_int {
    guard(-1, || {
        if s.is_null() || source.is_null() {
            return -1;
        }
        let session = &mut *s;
        let src = match cstr_to_str(source) {
            Some(v) => v,
            None => return -1,
        };
        session.compile.compile_units.clear();
        session.compile.compile_units.push(CompileUnit {
            filename: "main.c".to_string(),
            source: src.to_string(),
        });
        vitro_compile_all(s)
    })
}

#[no_mangle]
/// vitro_compile_unit 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `filename`, `source` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
/// - `filename`, `source` 若非空，必须指向以 null 结尾的有效 UTF-8 字符串。
pub unsafe extern "C" fn vitro_compile_unit(s: *mut Session, filename: *const c_char, source: *const c_char) -> c_int {
    guard(-1, || {
        if s.is_null() || filename.is_null() || source.is_null() {
            return -1;
        }
        let session = &mut *s;
        let fname = match cstr_to_str(filename) {
            Some(v) => v,
            None => return -1,
        };
        let src = match cstr_to_str(source) {
            Some(v) => v,
            None => return -1,
        };
        session.compile.compile_units.push(CompileUnit {
            filename: fname.to_string(),
            source: src.to_string(),
        });
        0
    })
}

#[no_mangle]
/// vitro_compile_all 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_compile_all(s: *mut Session) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        let session = &mut *s;

        let units = session.compile.compile_units.clone();
        if run_multi_file_pipeline(session, units, false).is_err() {
            return -1;
        }
        0
    })
}

#[no_mangle]
/// vitro_get_compile_errors 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_compile_errors(s: *mut Session) -> *const c_char {
    guard(ptr::null(), || {
        if s.is_null() {
            return ptr::null();
        }
        let session = &mut *s;
        if session.compile.errors.is_empty() {
            return ptr::null();
        }
        match CString::new(session.compile.errors.clone()) {
            Ok(cstring) => {
                let ptr = cstring.as_ptr();
                session.compile.last_errors_cstring = Some(cstring);
                ptr
            }
            Err(_) => ptr::null(),
        }
    })
}

#[no_mangle]
/// vitro_get_compile_errors_length 的 C API 封装（ABI 1.2.0，加函数 = minor）。
///
/// 返回编译错误 JSON 的字节长度（不含 NUL 终止符），无错误时返回 0。
/// 与 `vitro_get_compile_errors` 配套：驱动侧可先取长度再用定长缓冲精确读取，
/// 不再依赖"NUL 终止字符串 + 变长窗口扫描"（对短于扫描窗口的分配是越界读）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_compile_errors_length(s: *mut Session) -> c_int {
    guard(0, || {
        if s.is_null() {
            return 0;
        }
        let session = &mut *s;
        session.compile.errors.len() as c_int
    })
}

#[no_mangle]
/// E2：引擎能力清单 JSON（机器可读真实能力；无状态）。
///
/// # 所有权（ABI 1.3.0 起对齐书面契约）
/// 返回 **rust-alloc 所有权**缓冲，调用方必须以 `vitro_free_string` 释放——
/// 与全部 `*_json` 出口同一契约（`CAPI评审回复与实现状态.md` §横切契约落实：
/// "全部 JSON 函数为 rust-alloc 所有权"）。历史上本函数曾返回 `OnceLock`
/// 静态指针（"无需释放"），下游按书面契约 free 即 UAF——W0-3 修复为每次
/// clone 出独立缓冲（序列化本身仍只做一次，进程级缓存）。
///
/// # Safety
/// 返回指针指向本次调用独占的 NUL 结尾 UTF-8 串；两次调用返回不同指针；
/// 释放后指针失效。返回 NULL 仅在分配失败时。
pub unsafe extern "C" fn vitro_get_capabilities_json() -> *mut c_char {
    guard(ptr::null_mut(), || {
        static CAPS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let cached =
            CAPS.get_or_init(|| serde_json::to_string(&crate::session_api::capabilities()).unwrap_or_default());
        CString::new(cached.as_str())
            .map(std::ffi::CString::into_raw)
            .unwrap_or(std::ptr::null_mut())
    })
}

#[no_mangle]
/// 设置命令行参数（供 `main(int argc, char *argv[])` 使用）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `argv` 必须是长度为 `argc` 的 C 字符串指针数组，每个指针指向有效的以 NUL 结尾的字符串。
/// - 字符串内容在调用期间必须保持有效；本函数会复制其内容到 Session 中。
pub unsafe extern "C" fn vitro_set_argv(s: *mut Session, argc: c_int, argv: *const *const c_char) {
    guard((), || {
        if s.is_null() || argv.is_null() {
            return;
        }
        // U0#5 实锤：负 argc 曾直接 `argc as usize` 进 with_capacity——负数绕回
        // usize::MAX，分配器 capacity overflow panic（被入口 guard 吞成静默）。
        // 参数域契约：argc < 0 属调用方错误，忽略本次调用（会话 argc/argv 不动）。
        if argc < 0 {
            return;
        }
        let session = &mut *s;
        let mut args = Vec::with_capacity(argc as usize);
        for i in 0..argc as isize {
            let ptr = *argv.offset(i);
            if ptr.is_null() {
                args.push(String::new());
            } else {
                args.push(std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string());
            }
        }
        session.runtime.argc = argc;
        session.runtime.argv = args;
    })
}

#[no_mangle]
/// 设置输入模式：0 为交互式（默认），非 0 为批量模式。
/// 批量模式下 getchar 在输入耗尽后立即返回 EOF，不进入等待状态。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_set_input_mode(s: *mut Session, is_batch: c_int) {
    guard((), || {
        if s.is_null() {
            return;
        }
        let session = &mut *s;
        session.runtime.input_mode = if is_batch != 0 {
            crate::session::InputMode::Batch
        } else {
            crate::session::InputMode::Interactive
        };
    })
}

#[no_mangle]
/// vitro_run 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_run(s: *mut Session) -> c_int {
    guard(-1, || {
        if s.is_null() || !(*s).compile.compiled {
            if !s.is_null() {
                (*s).runtime.error = "程序尚未编译。请先编译代码。".to_string();
            }
            return -1;
        }
        let session = &mut *s;
        match execute_run(session) {
            Ok((_, true)) => 2,
            Ok((_, false)) => 0,
            Err(_) => -1,
        }
    })
}

#[no_mangle]
/// vitro_get_runtime_error 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_runtime_error(s: *mut Session) -> *const c_char {
    guard(ptr::null(), || {
        if s.is_null() {
            return ptr::null();
        }
        let session = &mut *s;
        if session.runtime.error.is_empty() {
            return ptr::null();
        }
        match CString::new(session.runtime.error.clone()) {
            Ok(cstring) => {
                let ptr = cstring.as_ptr();
                session.runtime.last_error_cstring = Some(cstring);
                ptr
            }
            Err(_) => ptr::null(),
        }
    })
}

#[no_mangle]
/// vitro_set_input 的 C API 封装。
///
/// 语义：**保留换行**的标准输入（C 的 stdin 是字节流，`getchar()` 需读到 `'\n'`）。
/// 拆分实现见 `RuntimeState::split_stdin`（capi / FRB / serve 共用同一口径）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `input` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
/// - `input` 若非空，必须指向以 null 结尾的有效 UTF-8 字符串。
pub unsafe extern "C" fn vitro_set_input(s: *mut Session, input: *const c_char) {
    guard((), || {
        if s.is_null() {
            return;
        }
        let session = &mut *s;
        let input_str = match cstr_to_str(input) {
            Some(v) => v,
            None => {
                session.runtime.input_lines.clear();
                session.runtime.input_index = 0;
                session.runtime.input_char_offset = 0;
                return;
            }
        };
        session.runtime.set_stdin(&input_str);
    })
}

#[no_mangle]
/// vitro_is_waiting_input 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_is_waiting_input(s: *mut Session) -> c_int {
    guard(0, || {
        if s.is_null() {
            return 0;
        }
        if (*s).runtime.waiting_input {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
/// vitro_provide_input_line 的 C API 封装。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `line` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
/// - `line` 若非空，必须指向以 null 结尾的有效 UTF-8 字符串。
pub unsafe extern "C" fn vitro_provide_input_line(s: *mut Session, line: *const c_char) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        let session = &mut *s;
        let line_str = match cstr_to_str(line) {
            Some(v) => v,
            None => return -1,
        };
        session.runtime.push_stdin_text(&line_str);
        // 关键顺序：**保留 `waiting_input=true`**，让后续 `vitro_run` → `execute_run` 走
        // resume 分支（`is_resume = session.runtime.waiting_input`）。此前在此清位会让
        // execute_run 误判为新一次运行 → `reset_runtime` + `setup_vm` 从 main 重跑，
        // 已产生的输出被重复打印、且首个 scanf 会读到本次新喂入的文本（而非上次遗留）。
        // 仅恢复 VM 暂停位：WaitingInput 时 host call 执行前 `ip -= 1` 且 VM 处于 paused，
        // 不 resume 则 vm.run 立即返回 paused、无法续跑。与 `session_api::input_feed` 同源。
        if let Some(ref mut vm) = session.vm {
            vm.resume();
        }
        0
    })
}

/// 把 Rust 字符串写入调用方缓冲区（按字节截断，末尾补 NUL）。
///
/// # Safety
/// - `buf` 若非空，必须指向至少 `max_len` 字节的有效可写内存。
unsafe fn write_c_buf(text: &str, buf: *mut c_char, max_len: c_int) {
    if buf.is_null() || max_len <= 0 {
        return;
    }
    let copy_len = text.len().min((max_len - 1) as usize);
    let slice = slice::from_raw_parts_mut(buf as *mut u8, copy_len);
    slice.copy_from_slice(&text.as_bytes()[..copy_len]);
    *buf.add(copy_len) = 0;
}

#[no_mangle]
/// vitro_get_output_length 的 C API 封装。
///
/// **展示视图**（程序 stdout + stderr + 引擎附注按写入顺序拼接）。需要与 Clang golden
/// 比对的纯净 stdout 请用 `vitro_get_program_output_length`（E-P1-5）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_output_length(s: *mut Session) -> c_int {
    guard(0, || {
        if s.is_null() {
            return 0;
        }
        // U2#3：O(1)（OutputLog 增量维护总量，废除全量拼接取长度）
        (*s).runtime.output.total_bytes() as c_int
    })
}

#[no_mangle]
/// 纯程序 stdout 的字节长度（不含引擎附注、不含 stderr）。
///
/// E-P1-5：这是 Shadow Verification / 判分场景的**唯一**合法输出来源；此前消费方只能
/// 对 `vitro_get_output` 做正则清洗，程序自己打印"程序运行完成，返回值：N"时会被误删。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_program_output_length(s: *mut Session) -> c_int {
    guard(0, || {
        if s.is_null() {
            return 0;
        }
        // U2#3：O(1)
        (*s).runtime.output.len_of(vitro_runtime::OutputKind::Stdout) as c_int
    })
}

#[no_mangle]
/// 引擎附注（运行完成提示 / 内存泄漏报告 / 教学安全提示）的字节长度。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
pub unsafe extern "C" fn vitro_get_engine_notes_length(s: *mut Session) -> c_int {
    guard(0, || {
        if s.is_null() {
            return 0;
        }
        // U2#3：O(1)
        (*s).runtime.output.len_of(vitro_runtime::OutputKind::Note) as c_int
    })
}

#[no_mangle]
/// vitro_get_output 的 C API 封装（展示视图，语义同 `vitro_get_output_length`）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `buf` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
pub unsafe extern "C" fn vitro_get_output(s: *mut Session, buf: *mut c_char, max_len: c_int) {
    guard((), || {
        if s.is_null() {
            return;
        }
        write_c_buf(&(*s).runtime.display(), buf, max_len);
    })
}

#[no_mangle]
/// 复制纯程序 stdout 到缓冲区（max_len 含 NUL 终止符）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `buf` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
pub unsafe extern "C" fn vitro_get_program_output(s: *mut Session, buf: *mut c_char, max_len: c_int) {
    guard((), || {
        if s.is_null() {
            return;
        }
        write_c_buf(&(*s).runtime.stdout(), buf, max_len);
    })
}

#[no_mangle]
/// 复制引擎附注到缓冲区（max_len 含 NUL 终止符）。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `buf` 若非空，必须指向足够大的有效内存区域，供函数写入结果。
pub unsafe extern "C" fn vitro_get_engine_notes(s: *mut Session, buf: *mut c_char, max_len: c_int) {
    guard((), || {
        if s.is_null() {
            return;
        }
        write_c_buf(&(*s).runtime.notes(), buf, max_len);
    })
}

#[no_mangle]
/// 获取最近一次运行后的 JIT 统计信息。
///
/// # Safety
/// - `s` 必须是由 `vitro_session_create` 返回的有效 `Session` 指针，且未被 `vitro_session_destroy` 销毁。
/// - `traces_compiled` 与 `steps_accelerated` 若非空，必须指向有效的 `c_int` 内存。
pub unsafe extern "C" fn vitro_get_jit_stats(
    s: *mut Session,
    traces_compiled: *mut c_int,
    steps_accelerated: *mut c_int,
) {
    guard((), || {
        if s.is_null() {
            return;
        }
        let session = &*s;
        let stats = session.vm.as_ref().map(|vm| vm.jit_stats()).cloned().unwrap_or_default();
        if !traces_compiled.is_null() {
            *traces_compiled = stats.traces_compiled as c_int;
        }
        if !steps_accelerated.is_null() {
            *steps_accelerated = stats.steps_accelerated as c_int;
        }
    })
}
