#![allow(clippy::unwrap_used, clippy::expect_used)]

//! U0#5 实锤回归锚：`cide_set_argv` 负 argc。
//!
//! 修复前：`Vec::with_capacity(argc as usize)` 把负 argc 绕回 `usize::MAX`
//! → 分配器 capacity overflow panic（被入口 guard 吞成静默无操作——
//! guard 兜住进程，但 panic 本身是缺陷：调用方拿到无意义静默）。
//! 修复后：argc < 0 直接忽略本次调用（会话 argc/argv 不动），零 panic。
//! 观测手段：panic hook 计数（guard 吞掉的 panic 也会过 hook，可精确断言）。

use std::ffi::c_char;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn negative_argc_is_rejected_without_panic() {
    static PANICS: AtomicUsize = AtomicUsize::new(0);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {
        PANICS.fetch_add(1, Ordering::SeqCst);
    }));
    let result = std::panic::catch_unwind(|| unsafe {
        let s = cide_native::capi::cide_session_create();
        assert!(!s.is_null());
        let argv: [*const c_char; 2] = [c"prog".as_ptr(), c"-v".as_ptr()];
        cide_native::capi::cide_set_argv(s, -5, argv.as_ptr());
        // 负 argc 被忽略后会话仍可用（正常 argc 继续工作）
        cide_native::capi::cide_set_argv(s, 1, argv.as_ptr());
        cide_native::capi::cide_session_destroy(s);
    });
    std::panic::set_hook(prev);
    assert!(result.is_ok(), "负 argc 不得 abort/panic 逃逸");
    assert_eq!(
        PANICS.load(Ordering::SeqCst),
        0,
        "负 argc 不得触发内部 panic（guard 吞掉的 capacity overflow 也是缺陷）"
    );
}
