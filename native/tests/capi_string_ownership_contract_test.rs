#![allow(clippy::unwrap_used, clippy::expect_used)]

//! C ABI 字符串所有权契约测试（W0-3，U0#7，裁定 §13.3 W0-3）。
//!
//! 书面契约（`CAPI评审回复与实现状态.md` §横切契约落实）：**全部 JSON 函数为
//! rust-alloc 所有权（`cide_free_string` 释放）**。下游按此契约编程——返回
//! 静态指针的"优化"会让按契约 free 的下游直接 UAF，比缺函数更糟。
//!
//! 红留痕（2026-09-13）：`cide_get_capabilities_json` 曾返回 `OnceLock` 静态
//! 指针（注释写"无需释放"）——与书面契约矛盾。本测试的
//! `each_json_call_returns_fresh_owned_pointer` 在修复前红（两次调用返回同一
//! 指针）；修复（每次 clone 出独立 rust-alloc 缓冲，ABI 1.3.0）后绿。
//!
//! 三种所有权的完整分区见 `native/include/cide_capi.h` 分区注释：
//! ① rust-alloc 必须由 `cide_free_string` 释放（`*_json` 族、`cide_*_version`、
//!   `cide_last_error`、`cide_get_output_delta`）；
//! ② 会话租借、勿 free、下次调用失效（`cide_get_compile_errors`、
//!   `cide_get_runtime_error`）；
//! ③ 调用方缓冲拷贝（`cide_get_output` 族，len+buf 出参模式）。

use std::ffi::{CStr, CString};

unsafe fn take_and_free(p: *mut std::ffi::c_char) -> String {
    assert!(!p.is_null(), "JSON 出口不得返回 NULL（正常路径）");
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    cide_native::capi::cide_free_string(p);
    s
}

#[test]
fn each_json_call_returns_fresh_owned_pointer() {
    unsafe {
        // 契约：每次调用返回独立的 rust-alloc 缓冲。返回静态指针时两次调用
        // 地址相同（红），且按契约各自 free 会 double-free。
        let p1 = cide_native::capi::cide_get_capabilities_json() as *mut std::ffi::c_char;
        let p2 = cide_native::capi::cide_get_capabilities_json() as *mut std::ffi::c_char;
        assert_ne!(p1, p2, "capabilities_json 两次调用必须返回不同指针（rust-alloc 契约），得到同一静态指针");
        let s1 = take_and_free(p1);
        let s2 = take_and_free(p2);
        assert!(!s1.is_empty() && s1 == s2, "两次内容应一致且非空");
        assert!(s1.contains("\"abi_version\"") || s1.contains("memory_model") || s1.len() > 100,
            "内容应是能力清单 JSON：{}", &s1[..s1.len().min(80)]);

        // 其余 rust-alloc 出口同契约
        let a = cide_native::capi::cide_abi_version();
        let b = cide_native::capi::cide_engine_version();
        let c = cide_native::capi::cide_get_error_catalog_json();
        assert_ne!(a, std::ptr::null_mut());
        assert_ne!(b, std::ptr::null_mut());
        assert_ne!(c, std::ptr::null_mut());
        let _ = take_and_free(a);
        let _ = take_and_free(b);
        let catalog = take_and_free(c);
        assert!(catalog.contains("catalog"), "错误码表应含 catalog 字段");
    }
}

#[test]
fn json_outputs_survive_free_and_next_call() {
    // 会话级 JSON 出口：取串 → free → 会话继续可用 → 再取再 free（全周期契约）
    unsafe {
        let s = cide_native::capi::cide_session_create();
        assert!(!s.is_null());

        let src = CString::new("int main(){return 0;}").unwrap();
        assert_eq!(cide_native::capi::cide_compile(s, src.as_ptr()), 0);

        let j1 = cide_native::capi::cide_compile_json(s);
        let _ = take_and_free(j1);
        // free 后会话仍可用
        assert_eq!(cide_native::capi::cide_run(s), 0);
        let j2 = cide_native::capi::cide_compile_json(s);
        let body = take_and_free(j2);
        assert!(body.contains("\"ok\"") || body.contains("ok"), "compile_json 应含 ok 字段：{}", &body[..body.len().min(80)]);

        let rj = cide_native::capi::cide_run_json(s);
        let _ = take_and_free(rj);

        // step 链路
        assert_eq!(cide_native::capi::cide_step_begin(s), 0);
        let nj = cide_native::capi::cide_step_next_json(s);
        let _ = take_and_free(nj);
        let pj = cide_native::capi::cide_get_step_payloads_json(s, 0, 10);
        let _ = take_and_free(pj);

        let od = cide_native::capi::cide_get_output_delta(s, 0);
        let _ = take_and_free(od);

        cide_native::capi::cide_session_destroy(s);
    }
}

#[test]
fn free_string_is_null_safe() {
    unsafe { cide_native::capi::cide_free_string(std::ptr::null_mut()) }
}
