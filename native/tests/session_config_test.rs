#![allow(clippy::unwrap_used, clippy::expect_used)]

//! 会话级保险丝的存活与回显（2026-09-11）。
//!
//! 起因：复核 PR 清单「条目 5 堆决议第三道墙」时发现 —— **第二道墙（步数保险丝）对全速
//! 运行的程序从未生效**：`setup_vm` 里硬编码了 `vm.set_max_steps(10_000_000)`，每次 run
//! 都把会话配置抹掉（实测设 2000 步的程序一路跑到 16 万步、直到撞 1MB 堆墙才停）。
//! 既有用例 `test_second_wall_max_steps_fuse` 只断言"消息含步数超限"，1000 万步也能满足，
//! 因此长期掩盖了该缺陷 —— 本文件补上"配置值本身必须生效且可回显"的断言。

use vitro_native::capi;
use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;
use std::ffi::{c_char, CString};

/// 只分配不释放的自旋程序：会先撞步数保险丝（步数上限很小时），而非 1MB 堆墙。
const SPIN_SRC: &str =
    "#include <stdlib.h>\nint main(){ for(;;){ char *p=(char*)malloc(64); p[0]=0; } return 0; }\n";

#[test]
fn test_max_steps_survives_run_and_actually_applies() {
    let mut s = Session::default();
    s.set_max_steps(2000);
    s.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: SPIN_SRC.to_string(),
    }];
    assert!(
        session_api::compile(&mut s)["ok"].as_bool().unwrap_or(false),
        "用例程序应能编译"
    );

    let r = session_api::run(&mut s);
    assert_eq!(r["status"], "trap", "应撞步数保险丝：{r}");
    assert_eq!(
        r["steps_executed"], 2000,
        "应在**配置的** 2000 步处停下（而不是默认的 1000 万）：{r}"
    );
    assert!(
        r["trap"].as_str().unwrap_or("").contains("步数超过限制"),
        "trap 应为步数超限教学提示：{r}"
    );
    assert_eq!(
        s.vm.as_ref().map(|v| v.max_steps()),
        Some(2000),
        "会话级配置必须跨 run 存活"
    );
}

#[test]
fn test_call_depth_limit_survives_run() {
    let mut s = Session::default();
    s.set_call_depth_limit(48);
    s.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: "int main(){ return 0; }\n".to_string(),
    }];
    let _ = session_api::compile(&mut s);
    let _ = session_api::run(&mut s);
    assert_eq!(
        s.vm.as_ref().map(|v| v.call_depth_limit()),
        Some(48),
        "调用深度上限同样必须跨 run 存活"
    );
}

#[test]
fn test_config_echoes_fuse_settings() {
    // 配置可写也必须可读：此前 `config()` 不含 max_steps / call_depth_limit，
    // 消费方无法确认保险丝是否真的落到 VM 上（正是这个不透明掩盖了上面的缺陷）。
    let mut s = Session::default();
    s.set_max_steps(1234);
    s.set_call_depth_limit(64);
    let cfg = session_api::config(&s);
    assert_eq!(cfg["max_steps"], 1234, "config() 应回显步数上限：{cfg}");
    assert_eq!(cfg["call_depth_limit"], 64, "config() 应回显调用深度上限：{cfg}");
}

#[test]
fn test_capi_set_max_steps_before_compile_is_effective() {
    // 会话尚未编译时设置也必须生效：此前 capi 写成 `if let Some(vm) { .. }` 并返回 0，
    // 会话无 VM 时配置被静默丢弃（返回"成功"）。
    unsafe {
        let session = capi::vitro_session_create();
        assert!(!session.is_null());
        assert_eq!(capi::vitro_set_max_steps(session, 500), 0, "设置应返回成功");

        let fname = CString::new("main.c").unwrap();
        let src = CString::new(SPIN_SRC).unwrap();
        capi::vitro_compile_unit(
            session,
            fname.as_ptr() as *const c_char,
            src.as_ptr() as *const c_char,
        );
        assert_eq!(capi::vitro_compile_all(session), 0, "用例程序应能编译");

        let _ = capi::vitro_run(session);
        let err_ptr = capi::vitro_get_runtime_error(session);
        let err = if err_ptr.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string()
        };
        assert!(
            err.contains("步数超过限制"),
            "编译前设置的步数上限必须生效（而不是被静默丢弃）：{err}"
        );

        capi::vitro_session_destroy(session);
    }
}
