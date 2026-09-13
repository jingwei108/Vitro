//! capi 第一批集成测试（对应 `CAPI评审回复与实现状态.md` §8 review 基线第 3 条：
//! 每个新函数带 capi 集成测试）。
//!
//! 契约验证点：JSON 字符串所有权（rust-alloc + `cide_free_string`）、状态码约定、
//! 诊断 severity/end 字段、运行结果字段、输出游标增量、会话级保险丝。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CStr, CString};

use cide_native::capi;
use cide_native::session::Session;

/// 读取 rust-alloc 字符串并按契约释放。
unsafe fn take_string(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(p).to_string_lossy().to_string();
    capi::cide_free_string(p);
    s
}

fn compile_session(source: &str) -> *mut Session {
    unsafe {
        let session = capi::cide_session_create();
        assert!(!session.is_null(), "session 创建失败");
        let fname = CString::new("main.c").unwrap();
        let src = CString::new(source).unwrap();
        capi::cide_compile_unit(session, fname.as_ptr(), src.as_ptr());
        // cide_compile_unit 只收集单元，需触发真实编译
        take_string(capi::cide_compile_json(session));
        session
    }
}

fn run_json(session: *mut Session) -> serde_json::Value {
    unsafe {
        let s = take_string(capi::cide_run_json(session));
        serde_json::from_str(&s).unwrap_or_else(|e| panic!("run_json 非法 JSON: {} / {}", e, s))
    }
}

/// 运行至结束并取出程序输出（含 Cide 追加的完成提示/泄漏报告）。
fn run_and_take_output(session: *mut Session) -> String {
    let v = run_json(session);
    assert_eq!(v["status"], "finished", "程序应正常结束: {}", v);
    unsafe {
        let d = take_string(capi::cide_get_output_delta(session, 0));
        let dv: serde_json::Value = serde_json::from_str(&d).unwrap();
        let text = dv["delta"].as_str().unwrap_or("").to_string();
        capi::cide_session_destroy(session);
        text
    }
}

// ─── 版本与字符串所有权 ───────────────────────────────────────────────────────

#[test]
fn test_abi_version_is_released_by_free_string() {
    unsafe {
        let v = take_string(capi::cide_abi_version());
        assert_eq!(v, capi::CIDE_ABI_VERSION, "ABI 版本串应与常量一致");
        // 再次调用确认指针所有权契约可重复使用（每次返回独立分配）
        let v2 = take_string(capi::cide_abi_version());
        assert_eq!(v2, v);
        // null 释放必须安全
        capi::cide_free_string(std::ptr::null_mut());
    }
}

#[test]
fn test_engine_version_non_empty() {
    unsafe {
        let v = take_string(capi::cide_engine_version());
        assert!(!v.is_empty(), "engine_version 不应为空");
    }
}

// ─── compile_errors_length（ABI 1.2.0）───────────────────────────────────────

#[test]
fn test_compile_errors_length_matches_string() {
    unsafe {
        // 干净会话：无编译错误 → length 0 且指针为 null
        let ok = compile_session("int main() { return 0; }");
        assert_eq!(capi::cide_get_compile_errors_length(ok), 0, "无错误应返回 0");
        assert!(capi::cide_get_compile_errors(ok).is_null(), "无错误应返回 null");
        capi::cide_session_destroy(ok);

        // 编译失败会话：length > 0 且与 NUL 终止串字节数一致（不含 NUL）
        let bad = compile_session("int main() { int x = ; }");
        let n = capi::cide_get_compile_errors_length(bad);
        assert!(n > 0, "编译错误时 length 应 > 0");
        let p = capi::cide_get_compile_errors(bad);
        assert!(!p.is_null(), "编译错误时应返回非 null 指针");
        let bytes = CStr::from_ptr(p).to_bytes();
        assert_eq!(bytes.len(), n as usize, "length 应等于不含 NUL 的字节长度");
        capi::cide_session_destroy(bad);
    }
}

// ─── last_error ──────────────────────────────────────────────────────────────

#[test]
fn test_last_error_none_when_clean() {
    let session = compile_session("int main(){ return 0; }\n");
    unsafe {
        let s = take_string(capi::cide_last_error(session));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["kind"], "none");
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_last_error_reports_runtime_trap() {
    let session = compile_session("#include <stdlib.h>\nint main(){ int *p = (int*)malloc(4); free(p); free(p); return 0; }\n");
    let r = run_json(session);
    assert_eq!(r["status"], "trap");
    unsafe {
        let s = take_string(capi::cide_last_error(session));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["kind"], "runtime");
        assert!(
            v["message"].as_str().unwrap_or("").contains("E3061"),
            "运行期错误应透传领域错误码: {}",
            v["message"]
        );
        capi::cide_session_destroy(session);
    }
}

// ─── compile_json ────────────────────────────────────────────────────────────

#[test]
fn test_compile_json_success_has_empty_or_warning_diagnostics() {
    let session = compile_session("int main(){ int x = 1; return x; }\n");
    unsafe {
        let s = take_string(capi::cide_compile_json(session));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ok"], true, "合法程序应编译成功: {}", s);
        assert!(v["diagnostics"].is_array(), "diagnostics 必须是数组");
        // 不得出现 error 级诊断
        if let Some(arr) = v["diagnostics"].as_array() {
            for d in arr {
                assert_ne!(d["severity"], "error", "成功编译不应有 error 级诊断: {}", d);
            }
        }
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_compile_json_error_reports_severity_and_end_columns() {
    let session = compile_session("int main(){ int x = ; return 0; }\n");
    unsafe {
        let s = take_string(capi::cide_compile_json(session));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ok"], false, "语法错误应编译失败");
        let arr = v["diagnostics"].as_array().expect("必须有诊断");
        assert!(!arr.is_empty(), "错误程序必须给出至少一条诊断");
        let d = &arr[0];
        assert_eq!(d["severity"], "error");
        assert!(d["line"].as_i64().unwrap_or(0) >= 1, "line 必须从 1 起");
        assert!(d["end_column"].as_i64().unwrap_or(0) > d["column"].as_i64().unwrap_or(0),
            "end_column 至少为起点 + 1（schema 先带字段）");
        assert!(d["code"].as_str().unwrap_or("").starts_with('E'), "code 应为 E 码形式");
        capi::cide_session_destroy(session);
    }
}

// ─── run_json ────────────────────────────────────────────────────────────────

#[test]
fn test_run_json_finished_reports_return_value_and_steps() {
    let session = compile_session("int main(){ return 7; }\n");
    // 先编译
    unsafe {
        take_string(capi::cide_compile_json(session));
    }
    let r = run_json(session);
    assert_eq!(r["ok"], true, "正常程序 ok 应为 true: {}", r);
    assert_eq!(r["status"], "finished");
    assert_eq!(r["return_value"], 7, "return_value 应透传 main 返回值");
    assert!(r["steps_executed"].as_i64().unwrap_or(0) > 0, "steps_executed 应大于 0");
    assert_eq!(r["waiting_input"], false);
    unsafe { capi::cide_session_destroy(session) };
}

#[test]
fn test_run_json_without_compile_reports_not_compiled() {
    unsafe {
        let session = capi::cide_session_create();
        let r = run_json(session);
        assert_eq!(r["status"], "not_compiled");
        assert_eq!(r["ok"], false);
        capi::cide_session_destroy(session);
    }
}

// ─── 输出游标增量 ─────────────────────────────────────────────────────────────

#[test]
fn test_output_delta_cursor_semantics() {
    let session = compile_session("#include <stdio.h>\nint main(){ printf(\"abc\"); return 0; }\n");
    unsafe {
        take_string(capi::cide_compile_json(session));
    }
    let r = run_json(session);
    assert_eq!(r["status"], "finished");

    unsafe {
        let first = take_string(capi::cide_get_output_delta(session, 0));
        let v: serde_json::Value = serde_json::from_str(&first).unwrap();
        let delta = v["delta"].as_str().unwrap_or("");
        assert!(delta.starts_with("abc"), "初始游标应包含程序输出: {}", delta);
        let cursor = v["cursor"].as_i64().unwrap();
        let total = v["total"].as_i64().unwrap();
        assert_eq!(cursor, total, "cursor 应等于总字节数");
        assert_eq!(total as usize, delta.len(), "游标为 0 时增量长度应等于总长度");

        // 再取一次：游标已到末尾，增量为空
        let again = take_string(capi::cide_get_output_delta(session, cursor as i32));
        let v2: serde_json::Value = serde_json::from_str(&again).unwrap();
        assert_eq!(v2["delta"], "", "游标之后的增量为空");
        assert_eq!(v2["cursor"], total);

        // 负游标按 0 处理
        let neg = take_string(capi::cide_get_output_delta(session, -5));
        let v3: serde_json::Value = serde_json::from_str(&neg).unwrap();
        assert_eq!(v3["delta"].as_str().unwrap_or(""), delta, "负游标应按 0 处理");

        capi::cide_session_destroy(session);
    }
}

// ─── 会话级保险丝 ─────────────────────────────────────────────────────────────

#[test]
fn test_set_max_steps_traps_infinite_loop() {
    let session = compile_session("int main(){ for(;;){} return 0; }\n");
    unsafe {
        take_string(capi::cide_compile_json(session));
        assert_eq!(capi::cide_set_max_steps(session, 1000), 0, "设置步数上限应成功");
    }
    let r = run_json(session);
    assert_eq!(r["status"], "trap", "死循环应被步数保险丝拦下: {}", r);
    assert!(
        r["trap"].as_str().unwrap_or("").contains("步数超过限制"),
        "trap 应说明步数超限: {}",
        r["trap"]
    );
    unsafe {
        // 非法入参
        assert_eq!(capi::cide_set_max_steps(std::ptr::null_mut(), 100), -1);
        assert_eq!(capi::cide_set_max_steps(session, 0), -1);
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_set_call_depth_limit_traps_deep_recursion() {
    let session = compile_session("int f(int n){ return f(n + 1); }\nint main(){ return f(0); }\n");
    unsafe {
        take_string(capi::cide_compile_json(session));
        assert_eq!(capi::cide_set_call_depth_limit(session, 50), 0);
    }
    let r = run_json(session);
    assert_eq!(r["status"], "trap", "无限递归应被调用深度保险丝拦下: {}", r);
    assert!(
        r["trap"].as_str().unwrap_or("").contains("调用深度超过上限"),
        "trap 应说明调用深度超限: {}",
        r["trap"]
    );
    unsafe { capi::cide_session_destroy(session) };
}

#[test]
fn test_set_deterministic_freezes_time() {
    // 非确定性下 time(NULL) 几乎不可能为 0；确定性下固定为 0。
    // 注意：不依赖 `#include <time.h>`（runtime_libc 的 time.h stub 当前会导致
    // "编译失败且无诊断"，见 CHANGELOG 已知问题），改用自带原型声明。
    let src = "#include <stdio.h>\nlong time(long *t);\nint main(){ printf(\"%d\", (int)time((long*)0)); return 0; }\n";
    let session = compile_session(src);
    unsafe {
        take_string(capi::cide_compile_json(session));
        assert_eq!(capi::cide_get_deterministic(session), 0, "默认关闭");
        assert_eq!(capi::cide_set_deterministic(session, 1), 0);
        assert_eq!(capi::cide_get_deterministic(session), 1);
    }
    let r = run_json(session);
    assert_eq!(r["status"], "finished", "确定性运行时不应 trap: {}", r);
    unsafe {
        let out = take_string(capi::cide_get_output_delta(session, 0));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let delta = v["delta"].as_str().unwrap_or("");
        assert!(
            delta.starts_with('0'),
            "确定性模式下 time() 必须固定为 0（输出以 0 起）: {}",
            delta
        );
        capi::cide_session_destroy(session);
    }
}

// ─── 断点与单步（统一模式引擎接入 Session）──────────────────────────────────────

#[test]
fn test_step_begin_requires_compiled_session() {
    unsafe {
        let session = capi::cide_session_create();
        assert_eq!(
            capi::cide_step_begin(session),
            -2,
            "未编译会话初始化统一模式应返回 -2"
        );
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_step_next_and_payload_schema_fields() {
    let session = compile_session(
        "#include <stdio.h>\nint main(){ int a = 1; int b = 2; printf(\"%d\", a + b); return 0; }\n",
    );
    unsafe {
        assert_eq!(capi::cide_step_begin(session), 0, "已编译会话应能初始化统一模式");
        for _ in 0..5 {
            let s = take_string(capi::cide_step_next_json(session));
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            assert!(v["payloads"].is_array(), "每步应返回 payload 数组: {}", s);
            assert_eq!(v["trapped"], false, "正常程序不应 trap: {}", s);
        }
        let s = take_string(capi::cide_get_step_payloads_json(session, 0, 100));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let arr = v["payloads"].as_array().expect("应有 payloads 数组");
        assert!(!arr.is_empty(), "累积步应有 payload: {}", s);
        let p = &arr[0];
        for key in [
            "step_index",
            "code_line",
            "func_name",
            "semantic_label",
            "local_vars",
            "call_stack",
            "accessed_vars",
            "array_snapshots",
            "pointer_snapshots",
        ] {
            assert!(p.get(key).is_some(), "StepPayload 缺 schema 字段 {}: {}", key, p);
        }
        // 窗口语义字段
        assert!(v.get("cache_start_step").is_some());
        assert!(v.get("max_collected_step").is_some());
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_set_breakpoints_pauses_on_hit() {
    let session = compile_session(
        "#include <stdio.h>\nint main(){\n    int a = 1;\n    printf(\"%d\", a);\n    return 0;\n}\n",
    );
    unsafe {
        assert_eq!(capi::cide_step_begin(session), 0);
        // 注意：必须在 step_begin 之后设置断点（step_begin 会重建 VM 并清空断点）
        let json = CString::new("[4]").unwrap();
        assert_eq!(capi::cide_set_breakpoints(session, json.as_ptr()), 0);

        let mut paused_seen = false;
        for _ in 0..200 {
            let s = take_string(capi::cide_step_next_json(session));
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            if v["paused"].as_bool().unwrap_or(false) {
                paused_seen = true;
                break;
            }
            if v["finished"].as_bool().unwrap_or(false) {
                break;
            }
        }
        assert!(paused_seen, "执行到断点行时应 paused=true");

        // 非法入参
        let bad = CString::new("not-json").unwrap();
        assert_eq!(capi::cide_set_breakpoints(session, bad.as_ptr()), -1);
        assert_eq!(capi::cide_set_breakpoints(std::ptr::null_mut(), json.as_ptr()), -1);
        capi::cide_session_destroy(session);
    }
}

// ─── 堆隔离预算（堆决议 §1：「隔离预算可调，写进会话配置」）────────────────────

/// 探针程序：free 后立即再 malloc 同尺寸，打印地址是否相同（`[1]` = 复用）。
const QUARANTINE_PROBE: &str = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(4);
    free(p);
    int *q = (int*)malloc(4);
    printf("[%d]", p == q);
    free(q);
    return 0;
}
"#;

#[test]
fn test_set_quarantine_budget_controls_address_reuse() {
    // 默认预算（256KB，有界隔离）：隔离窗口内地址不复用 → [0]
    let session = compile_session(QUARANTINE_PROBE);
    let out = run_and_take_output(session);
    assert!(
        out.contains("[0]"),
        "默认隔离预算下 free 后地址不应立即复用（有界隔离语义），实际输出: {}",
        out
    );

    // 预算 = 0（关闭隔离，教学对照用）：free 后地址立即可复用 → [1]
    let session = compile_session(QUARANTINE_PROBE);
    unsafe {
        assert_eq!(
            capi::cide_set_quarantine_budget(session, -1),
            -1,
            "负预算必须被拒绝"
        );
        assert_eq!(
            capi::cide_set_quarantine_budget(session, 0),
            0,
            "预算 0 表示关闭隔离，应被接受"
        );
        assert_eq!(
            capi::cide_get_quarantine_budget(session),
            0,
            "读回应与写入一致"
        );
    }
    let out = run_and_take_output(session);
    assert!(
        out.contains("[1]"),
        "预算为 0 时 free 后地址应立即复用，实际输出: {}",
        out
    );
}

#[test]
fn test_quarantine_budget_is_clamped_to_heap_limit() {
    let session = compile_session(QUARANTINE_PROBE);
    unsafe {
        assert_eq!(capi::cide_set_quarantine_budget(session, i32::MAX), 0);
        let budget = capi::cide_get_quarantine_budget(session);
        assert!(
            budget > 0 && budget <= 1024 * 1024,
            "预算应被裁剪到堆上限（1MB）内，实际 {}",
            budget
        );
        capi::cide_session_destroy(session);
    }
}

#[test]
fn test_quarantine_budget_setter_rejects_null_session() {
    unsafe {
        assert_eq!(capi::cide_set_quarantine_budget(std::ptr::null_mut(), 1024), -1);
        assert_eq!(capi::cide_get_quarantine_budget(std::ptr::null_mut()), -1);
    }
}
