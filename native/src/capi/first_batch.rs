//! capi 第一批（2026-09-11 SharpTutor API 评审定稿后的实现）。
//!
//! 契约（见 `docs/current/06-出口与协议/CAPI评审回复与实现状态.md` §1）：
//! - 复杂返回一律 **JSON 字符串 + rust-alloc 所有权**：调用方负责用
//!   [`vitro_free_string`] 释放，禁止 `free()` / `delete`；
//! - 所有入口经 `catch_unwind`，panic 不跨 C 边界（V-P0-3 模式推广）；
//! - 状态码：`0 = 成功 / 负数 = 入参错误或会话无效 / 正数 = 领域状态`（1 = trap、2 = waiting_input）；
//! - Session 句柄**非线程安全**，跨线程访问须调用方自行同步；
//! - 输入输出 UTF-8；程序输出以 `\n` 结行。
//!
//! 版本化承诺：**加函数 = minor，改签名/语义 = major**（`vitro_abi_version`）。

use crate::session::Session;
use crate::session_api;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

/// capi 签名契约版本。
///
/// 1.1.0（E-P1-5）：新增纯程序输出出口 `vitro_get_output_length` 语义保持不变（展示视图），
/// 追加 `vitro_get_program_output_length` / `vitro_get_program_output` /
/// `vitro_get_engine_notes_length` / `vitro_get_engine_notes` / `vitro_get_program_output_delta`
/// —— 按"加函数 = minor"承诺升 minor。
///
/// 1.2.0（2026-09-13，D5 收尾重构）：追加 `vitro_get_compile_errors_length`，
/// 供驱动侧定长缓冲精确读取编译错误，消除变长窗口扫描的越界读依赖。
///
/// 1.3.0（2026-09-13，W0-3 契约止血）：`vitro_get_capabilities_json` 所有权
/// 由"进程级静态指针、勿释放"改为"rust-alloc、`vitro_free_string` 释放"，
/// 对齐书面契约（全部 JSON 函数 rust-alloc 所有权）。旧实现下按契约释放
/// 即 UAF；契约测试 `capi_string_ownership_contract_test` 锚定新语义
/// （两次调用返回不同指针 + 各自 free）。行为契约变更 → minor。
///
/// 2.0.0（2026-09-14，项目更名 Vitro）：全部 C ABI 导出符号前缀
/// `cide_*` → `vitro_*`（41 个入口），二进制名 `cide_native.dll` →
/// `vitro_native.dll`。符号面 breaking → major。语义与 JSON 帧格式
/// （wire format v0.1）零变化；消费方迁移 = 纯符号改名 + 重链接。
pub const VITRO_ABI_VERSION: &str = "2.0.0";

/// 把 Rust 字符串的所有权交给调用方（rust-alloc）。
fn owned_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn json_string<T: serde::Serialize>(v: &T) -> *mut c_char {
    match serde_json::to_string(v) {
        Ok(s) => owned_c_string(s),
        Err(e) => owned_c_string(format!("{{\"ok\":false,\"error\":\"JSON 序列化失败：{}\"}}", e)),
    }
}

fn err_json(msg: impl Into<String>) -> *mut c_char {
    json_string(&serde_json::json!({ "ok": false, "error": msg.into() }))
}

/// 入口 panic 护栏：panic 不跨 C 边界，统一返回 fallback。
/// `pub(crate)`：mod.rs 的第二批入口（W0-3 起）同样要求全覆盖
/// （契约："所有入口经 catch_unwind"）。
pub(crate) fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => fallback,
    }
}

/// 读取 C 字符串（null 与非法 UTF-8 均返回 None）。
fn cstr_opt(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p).to_str().ok().map(|s| s.to_owned()) }
}

// ─── 版本与字符串所有权 ───────────────────────────────────────────────────────

#[no_mangle]
/// 返回 capi 签名契约版本（rust-alloc，须用 `vitro_free_string` 释放）。
pub extern "C" fn vitro_abi_version() -> *mut c_char {
    owned_c_string(VITRO_ABI_VERSION.to_string())
}

#[no_mangle]
/// 返回引擎版本串：crate 版本（+ 构建期注入的 git hash，若可用）。
pub extern "C" fn vitro_engine_version() -> *mut c_char {
    owned_c_string(engine_version_string())
}

/// 引擎版本串的 Rust 侧单源（`vitro_engine_version` 与 `capabilities.engine_version` 共用）。
///
/// 消费方可用它做**产物新鲜度自检**：串中的短哈希应等于构建所用提交；
/// 回放/影子验证据此 fail fast，避免在陈旧二进制上拿到假绿。
pub fn engine_version_string() -> String {
    match option_env!("VITRO_GIT_HASH") {
        Some(hash) => format!("{} ({})", env!("CARGO_PKG_VERSION"), hash),
        None => env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[no_mangle]
/// 释放本库返回的字符串（rust-alloc 所有权契约的唯一释放入口）。
///
/// # Safety
/// - `p` 必须来自本库返回的、尚未释放的字符串指针；`p` 为 null 时安全无操作。
/// - 不得对同一指针调用两次，也不得用 `free()` / `delete` 释放。
pub unsafe extern "C" fn vitro_free_string(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}

// ─── 会话信息 ────────────────────────────────────────────────────────────────

#[no_mangle]
/// 错误码表机器可读导出（下游需求清单 B1）。
///
/// 返回 **rust-alloc** 的 JSON 字符串，调用方负责用 [`vitro_free_string`] 释放。
/// 形状：`{"catalog":[{code,code_str,lang,category,emoji,title,explanation,common_causes[]}]}`，
/// 按 `code` 升序（跨构建可差分）。静态元数据；按具体源码行生成的 `fix_suggestion`
/// 见 `compile_json` 诊断字段。
///
/// # Safety
/// 无入参；返回指针必须经 `vitro_free_string` 释放，禁止 `free()`。
pub unsafe extern "C" fn vitro_get_error_catalog_json() -> *mut c_char {
    guard(ptr::null_mut(), || {
        match CString::new(crate::session_api::error_catalog_json()) {
            Ok(c) => c.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    })
}

#[no_mangle]
/// 最近一次错误，JSON：`{"kind":"compile|runtime|none","message":"..."}`。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_last_error(s: *mut Session) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        let session = &*s;
        if !session.runtime.error.is_empty() {
            json_string(&serde_json::json!({ "kind": "runtime", "message": session.runtime.error }))
        } else if !session.compile.errors.is_empty() {
            json_string(&serde_json::json!({ "kind": "compile", "message": session.compile.errors }))
        } else {
            json_string(&serde_json::json!({ "kind": "none", "message": "" }))
        }
    })
}

// ─── 编译 ────────────────────────────────────────────────────────────────────

#[no_mangle]
/// 编译当前会话的编译单元，返回诊断 JSON：
/// `{"ok":bool,"diagnostics":[{code,error_code,severity,line,column,end_line,end_column,message,fix_suggestion,filename}]}`。
///
/// `severity` 枚举：`error` / `warning` / `hint` / `info`。
/// `end_line` / `end_column` 为精确跨度（需动三处错误结构体），Phase 1 先给
/// "起点 + 1"退化值——schema 先带字段，不欠债。
///
/// 语义实现见 [`crate::session_api::compile`]（与 `vitro_cli serve` 共用）。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_compile_json(s: *mut Session) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        json_string(&session_api::compile(&mut *s))
    })
}

// ─── 运行 ────────────────────────────────────────────────────────────────────

#[no_mangle]
/// 全速运行并返回结果 JSON：
/// `{"ok":bool,"status":"finished|trap|waiting_input|not_compiled","return_value":n,"trap":"...","waiting_input":bool,"steps_executed":n}`。
///
/// `trap` 内的错误码（E 码）为领域状态，原样透传。
/// 语义实现见 [`crate::session_api::run`]。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_run_json(s: *mut Session) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        json_string(&session_api::run(&mut *s))
    })
}

// ─── 输出游标增量 ─────────────────────────────────────────────────────────────

#[no_mangle]
/// 返回自 `cursor`（字节偏移）起的输出增量：
/// `{"delta":"...","cursor":<新游标>,"total":<总字节>}`。
///
/// 与 serve 事件流共享同一底层实现（三出口一套语义）。游标越界按末尾处理；
/// 落入多字节字符中间时前移到下一个字符边界，保证 `delta` 为合法 UTF-8。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_get_output_delta(s: *mut Session, cursor: c_int) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        json_string(&session_api::output_delta(&*s, cursor))
    })
}

#[no_mangle]
/// 返回自 `cursor`（字节偏移）起的**纯程序 stdout** 增量（E-P1-5）：
/// `{"delta":"...","cursor":<新游标>,"total":<总字节>,"stream":"stdout"}`。
///
/// 与 `vitro_get_output_delta`（展示视图，含引擎附注）的区别：这里只含学生程序自己写到
/// stdout 的字节，**不含**"程序运行完成，返回值：N"、泄露报告与教学提示。判分、与 Clang
/// golden 比对、第三方消费方应使用本函数，不要再对展示视图做正则清洗。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_get_program_output_delta(s: *mut Session, cursor: c_int) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        json_string(&session_api::output_delta_on(&*s, cursor, "stdout"))
    })
}

// ─── 会话级配置（运行时保险丝契约）─────────────────────────────────────────────

#[no_mangle]
/// 设置步数上限（教学内容 = 可控地撞上限并拿到教学 trap，而非无限等待）。
/// 返回 0 成功；-1 表示入参或会话无效。
///
/// 会话尚未编译时同样生效（配置保存在会话上；此前 `if let Some(vm)` 写法会**静默丢弃**
/// 并返回"成功"—— 见 `Session::set_max_steps`）。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_set_max_steps(s: *mut Session, max_steps: c_int) -> c_int {
    guard(-1, || {
        if s.is_null() || max_steps <= 0 {
            return -1;
        }
        (*s).set_max_steps(max_steps);
        0
    })
}

#[no_mangle]
/// 设置调用深度上限（V-P1-10）。下限 16 层兜底；返回 0 成功，-1 表示入参或会话无效。
///
/// 会话尚未编译时同样生效（见 `Session::set_call_depth_limit`）。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_set_call_depth_limit(s: *mut Session, depth: c_int) -> c_int {
    guard(-1, || {
        if s.is_null() || depth <= 0 {
            return -1;
        }
        (*s).set_call_depth_limit(depth as usize);
        0
    })
}

#[no_mangle]
/// 判分确定性模式开关（Phase 1 最小形态：`time()` / `clock()` 固定返回 0）。
///
/// 与 Phase 3 的"完整 step 派生伪时钟"分层：本开关服务**判分可复现**，
/// 后者服务时间旅行重放确定性。返回 0 成功，-1 表示会话无效。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_set_deterministic(s: *mut Session, on: c_int) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        let session = &mut *s;
        session.runtime.deterministic = on != 0;
        0
    })
}

#[no_mangle]
/// 读取判分确定性模式当前状态（1 = 开，0 = 关，-1 = 会话无效）。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_get_deterministic(s: *mut Session) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        if (*s).runtime.deterministic {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
/// 设置堆**隔离区字节预算**（堆决议 §1「隔离预算可调，写进会话配置」）。
///
/// 语义（配合 [`堆有界隔离决议.md`] 的 bump + 有界隔离）：
/// - 默认 `256KB`（堆上限 1/4）；`free` 的块进 FIFO 隔离区，地址在窗口内不复用；
/// - 超预算时 FIFO 驱逐最老块归还 `free_list` 复用 —— 合法 churn 循环因此不撞内存墙；
/// - `budget = 0` 表示**关闭隔离**：free 后地址立即可复用（教学对照用，代价是
///   UAF / Double-Free 检测窗口消失）；
/// - 入参被裁剪到堆上限（1MB）：传超大值等价于"整个堆都是隔离区"，语义安全。
///
/// 返回 0 成功；-1 表示会话无效或 `budget_bytes` 为负。
///
/// [`堆有界隔离决议.md`]: ../../../docs/current/06-出口与协议/堆有界隔离决议.md
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_set_quarantine_budget(s: *mut Session, budget_bytes: c_int) -> c_int {
    guard(-1, || {
        if s.is_null() || budget_bytes < 0 {
            return -1;
        }
        let session = &mut *s;
        session.memory.quarantine_budget = budget_bytes.min(crate::vm::core::MEM_SIZE as c_int);
        0
    })
}

#[no_mangle]
/// 读取当前堆隔离区预算（字节）。返回 -1 表示会话无效。
///
/// 与 [`vitro_set_quarantine_budget`] 成对：调用方可据此确认会话配置（判分/回放
/// 场景需要保证两边预算一致）。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_get_quarantine_budget(s: *mut Session) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        (*s).memory.quarantine_budget
    })
}

/// 供测试与内部消费：把诊断 severity 数值映射为枚举字符串。
///
/// 实现收敛到 [`crate::session_api::severity_name`]（口径单一来源）。
pub use crate::session_api::severity_name;

// ─── 断点与单步（统一模式引擎接入 `Session`）─────────────────────────────────
// 主计划 Phase 1 的"语言中立 Rust 层提炼"在本批落到了最小可用面：
// `UnifiedEngine` 由 `Session` 持有，三出口（capi / serve / cli）共用同一会话。

#[no_mangle]
/// 初始化统一模式（时间旅行）会话：装载 VM → 重建运行时 → 建立初始检查点。
/// 必须在 `vitro_step_next_json` / `vitro_get_step_payloads_json` 之前调用。
///
/// 返回 0 成功；-1 会话无效；-2 尚未编译成功。
/// 语义实现见 [`crate::session_api::step_begin`]。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_step_begin(s: *mut Session) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        session_api::step_begin(&mut *s)
    })
}

#[no_mangle]
/// 单步推进一次并返回该步的 **StepPayload JSON**（`AutoStepResult` 形态）：
/// `{"payloads":[...],"finished":bool,"trapped":bool,"waiting_input":bool,"paused":bool,`
/// `"current_line":n,"trap_message":...,"cache_start_step":n}`。
///
/// 命中断点时为 `paused=true`（断点在 `VitroVM` 层判定）。
/// 语义实现见 [`crate::session_api::step_next`]。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_step_next_json(s: *mut Session) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        match session_api::step_next(&mut *s) {
            Ok(v) => json_string(&v),
            Err(e) => err_json(e),
        }
    })
}

#[no_mangle]
/// 取 `[start, end)` 步号区间的 StepPayload 数组：
/// `{"payloads":[...],"cache_start_step":n,"max_collected_step":n}`。
/// 区间会被裁剪到当前 frameCache 窗口内（窗口语义：2000 帧、超出丢最早 20%）。
///
/// 语义实现见 [`crate::session_api::payloads`]。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
pub unsafe extern "C" fn vitro_get_step_payloads_json(s: *mut Session, start: c_int, end: c_int) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return err_json("会话指针为空");
        }
        match session_api::payloads(&*s, start, end) {
            Ok(v) => json_string(&v),
            Err(e) => err_json(e),
        }
    })
}

#[no_mangle]
/// 设置断点行号集合（入参为 JSON 整数数组，如 `[3, 7]`；空数组 `[]` 清空断点）。
/// **应在 `vitro_step_begin` 之后调用**——`step_begin` 会重建 VM 并清空断点。
/// 返回 0 成功；-1 表示会话无效或 JSON 非法。
///
/// # Safety
/// - `s` 必须是 `vitro_session_create` 返回且未销毁的有效句柄。
/// - `lines_json` 若非空，必须指向以 NUL 结尾的有效 UTF-8 JSON 字符串。
pub unsafe extern "C" fn vitro_set_breakpoints(s: *mut Session, lines_json: *const c_char) -> c_int {
    guard(-1, || {
        if s.is_null() {
            return -1;
        }
        let Some(text) = cstr_opt(lines_json) else {
            return -1;
        };
        let lines: Vec<i32> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return -1,
        };
        session_api::set_breakpoints(&mut *s, &lines)
    })
}
