#![allow(clippy::unwrap_used, clippy::expect_used)]

//! C ABI 字符串所有权契约测试（W0-3，U0#7，裁定 §13.3 W0-3）。
//!
//! 书面契约（`CAPI评审回复与实现状态.md` §横切契约落实）：**全部 JSON 函数为
//! rust-alloc 所有权（`vitro_free_string` 释放）**。下游按此契约编程——返回
//! 静态指针的"优化"会让按契约 free 的下游直接 UAF，比缺函数更糟。
//!
//! 红留痕（2026-09-13）：`vitro_get_capabilities_json` 曾返回 `OnceLock` 静态
//! 指针（注释写"无需释放"）——与书面契约矛盾。本测试的
//! `each_json_call_returns_fresh_owned_pointer` 在修复前红（两次调用返回同一
//! 指针）；修复（每次 clone 出独立 rust-alloc 缓冲，ABI 1.3.0）后绿。
//!
//! 三种所有权的完整分区见 `native/include/vitro_capi.h` 分区注释：
//! ① rust-alloc 必须由 `vitro_free_string` 释放（`*_json` 族、`vitro_*_version`、
//!   `vitro_last_error`、`vitro_get_output_delta`）；
//! ② 会话租借、勿 free、下次调用失效（`vitro_get_compile_errors`、
//!   `vitro_get_runtime_error`）；
//! ③ 调用方缓冲拷贝（`vitro_get_output` 族，len+buf 出参模式）。

use std::ffi::{CStr, CString};

unsafe fn take_and_free(p: *mut std::ffi::c_char) -> String {
    assert!(!p.is_null(), "JSON 出口不得返回 NULL（正常路径）");
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    vitro_native::capi::vitro_free_string(p);
    s
}

#[test]
fn each_json_call_returns_fresh_owned_pointer() {
    unsafe {
        // 契约：每次调用返回独立的 rust-alloc 缓冲。返回静态指针时两次调用
        // 地址相同（红），且按契约各自 free 会 double-free。
        let p1 = vitro_native::capi::vitro_get_capabilities_json() as *mut std::ffi::c_char;
        let p2 = vitro_native::capi::vitro_get_capabilities_json() as *mut std::ffi::c_char;
        assert_ne!(p1, p2, "capabilities_json 两次调用必须返回不同指针（rust-alloc 契约），得到同一静态指针");
        let s1 = take_and_free(p1);
        let s2 = take_and_free(p2);
        assert!(!s1.is_empty() && s1 == s2, "两次内容应一致且非空");
        assert!(s1.contains("\"abi_version\"") || s1.contains("memory_model") || s1.len() > 100,
            "内容应是能力清单 JSON：{}", &s1[..s1.len().min(80)]);

        // 其余 rust-alloc 出口同契约
        let a = vitro_native::capi::vitro_abi_version();
        let b = vitro_native::capi::vitro_engine_version();
        let c = vitro_native::capi::vitro_get_error_catalog_json();
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
        let s = vitro_native::capi::vitro_session_create();
        assert!(!s.is_null());

        let src = CString::new("int main(){return 0;}").unwrap();
        assert_eq!(vitro_native::capi::vitro_compile(s, src.as_ptr()), 0);

        let j1 = vitro_native::capi::vitro_compile_json(s);
        let _ = take_and_free(j1);
        // free 后会话仍可用
        assert_eq!(vitro_native::capi::vitro_run(s), 0);
        let j2 = vitro_native::capi::vitro_compile_json(s);
        let body = take_and_free(j2);
        assert!(body.contains("\"ok\"") || body.contains("ok"), "compile_json 应含 ok 字段：{}", &body[..body.len().min(80)]);

        let rj = vitro_native::capi::vitro_run_json(s);
        let _ = take_and_free(rj);

        // step 链路
        assert_eq!(vitro_native::capi::vitro_step_begin(s), 0);
        let nj = vitro_native::capi::vitro_step_next_json(s);
        let _ = take_and_free(nj);
        let pj = vitro_native::capi::vitro_get_step_payloads_json(s, 0, 10);
        let _ = take_and_free(pj);

        let od = vitro_native::capi::vitro_get_output_delta(s, 0);
        let _ = take_and_free(od);

        vitro_native::capi::vitro_session_destroy(s);
    }
}

#[test]
fn free_string_is_null_safe() {
    unsafe { vitro_native::capi::vitro_free_string(std::ptr::null_mut()) }
}

/// U2#13（2026-09-14）：buf 写入式出口（ABI 2.1.0）与既有指针出口的**语义
/// 等价**契约——跨语言 FFI 消费方（Go scripts）全部迁移到 buf 形态后，两形态
/// 必须永远返回相同内容（buf 版是零所有权转移的第三类出口：调用方缓冲拷贝）。
#[test]
fn buf_style_exits_match_pointer_exits() {
    unsafe {
        // ① abi_version：buf 版 == rust-alloc 版
        let owned = take_and_free(vitro_native::capi::vitro_abi_version());
        let mut buf = [0 as std::ffi::c_char; 64];
        let n = vitro_native::capi::vitro_abi_version_into(buf.as_mut_ptr(), 64);
        assert!(n > 0, "abi_version_into 应写入非空内容");
        let s = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        assert_eq!(s, owned, "abi_version 两形态内容必须一致");
        assert_eq!(n as usize, owned.len(), "返回字节数应等于实际长度（不含 NUL）");

        // ② engine_version
        let owned = take_and_free(vitro_native::capi::vitro_engine_version());
        let mut buf = [0 as std::ffi::c_char; 64];
        let n = vitro_native::capi::vitro_engine_version_into(buf.as_mut_ptr(), 64);
        assert!(n > 0);
        let s = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        assert_eq!(s, owned, "engine_version 两形态内容必须一致");

        // ③ 空缓冲/非法参数防御：返回 0 且不崩
        assert_eq!(vitro_native::capi::vitro_engine_version_into(std::ptr::null_mut(), 0), 0);
        assert_eq!(
            vitro_native::capi::vitro_engine_version_into(buf.as_mut_ptr(), -1),
            0,
            "负 max_len 必须安全返回 0"
        );
    }
}

/// runtime_error / compile_errors 的 buf 版与租借版等价（会话出口）。
#[test]
fn buf_style_session_exits_match_pointer_exits() {
    unsafe {
        let s = vitro_native::capi::vitro_session_create();
        assert!(!s.is_null());

        // 编译一个含错误的源（产生 compile error）
        let src = CString::new("int main() { return x; }").unwrap();
        let name = CString::new("e.c").unwrap();
        let _ = vitro_native::capi::vitro_compile_unit(s, name.as_ptr(), src.as_ptr());
        let _ = vitro_native::capi::vitro_compile_all(s);

        // compile_errors：租借版与 buf 版内容一致
        let p = vitro_native::capi::vitro_get_compile_errors(s);
        let borrowed = if p.is_null() {
            String::new()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        };
        let mut buf = vec![0 as std::ffi::c_char; 1 << 16];
        let n = vitro_native::capi::vitro_get_compile_errors_into(s, buf.as_mut_ptr(), buf.len() as i32);
        let via_buf = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        assert!(!via_buf.is_empty(), "含错误源应产出错误文本");
        assert_eq!(via_buf, borrowed, "compile_errors 两形态内容必须一致");
        assert_eq!(n as usize, borrowed.len());

        // runtime_error：未运行时两形态都为空
        let mut rbuf = [0 as std::ffi::c_char; 256];
        let rn = vitro_native::capi::vitro_get_runtime_error_into(s, rbuf.as_mut_ptr(), 256);
        assert_eq!(rn, 0, "无运行错误应返回 0");
        assert!(vitro_native::capi::vitro_get_runtime_error(s).is_null());

        vitro_native::capi::vitro_session_destroy(s);
    }
}
