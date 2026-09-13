//! R1 内存边界回归测试（重构计划批次 R1，2026-09-11）。
//!
//! 覆盖验收线：
//! - R1 ① 动态堆起点：`heap_base = max(HEAP_START, align4(global_data_end))`，
//!   "大全局 + malloc" 不再静默压坏全局数据（AGENTS.md 已知限制销项）；
//! - 三方挤压：大全局 + malloc 失败明确返回 NULL（note 通道提示）/ 深递归明确 trap；
//! - R1 ③ argv 编址修复：argv 自 GLOBAL_REGION_LIMIT 向下分配，
//!   不再落在 GLOBAL_START 与全局数据重叠（`global_count` 恒 0 的预存 bug）；
//! - R1 ②/⑤ 全局区容量上限单源（GLOBAL_REGION_LIMIT）+ 大全局编译 warning。
//!
//! 防线哲学：测试不是为了标榜通过率，而是诚实地发现自己可能存在的问题。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};

/// 与 crash_regression_tests.rs 相同的 C API 驱动方式（独立复制以避免跨测试文件依赖）。
fn compile_and_run(source: &str) -> Result<(i32, Vec<String>), String> {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        if session.is_null() {
            return Err("Failed to create session".to_string());
        }

        let fname = CString::new("main.c").map_err(|e| e.to_string())?;
        let src = CString::new(source).map_err(|e| e.to_string())?;
        vitro_native::capi::vitro_compile_unit(session, fname.as_ptr() as *const c_char, src.as_ptr() as *const c_char);
        let compile_ret = vitro_native::capi::vitro_compile_all(session);
        if compile_ret != 0 {
            let err_ptr = vitro_native::capi::vitro_get_compile_errors(session);
            let err_msg = if err_ptr.is_null() {
                "Unknown compile error".to_string()
            } else {
                std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string()
            };
            vitro_native::capi::vitro_session_destroy(session);
            return Err(err_msg);
        }

        let run_ret = vitro_native::capi::vitro_run(session);

        let mut outputs = Vec::new();
        let out_len = vitro_native::capi::vitro_get_output_length(session);
        if out_len > 0 {
            let mut buf = vec![0u8; out_len as usize + 1];
            vitro_native::capi::vitro_get_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            let out_str = String::from_utf8_lossy(&buf[..out_len as usize]);
            for line in out_str.lines() {
                outputs.push(line.to_string());
            }
        }

        let err_ptr = vitro_native::capi::vitro_get_runtime_error(session);
        let runtime_err = if err_ptr.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string())
        };

        let _ = run_ret;
        vitro_native::capi::vitro_session_destroy(session);

        if let Some(e) = runtime_err {
            if !e.is_empty() {
                return Err(format!("Runtime error: {}", e));
            }
        }
        Ok((run_ret, outputs))
    }
}

// ===================== R1 ①：动态堆起点 =====================

/// 大全局（32KB，越过 HEAP_START=20KB）+ malloc：全局数据必须完好无损。
///
/// 旧引擎 heap_offset 写死 0x5000，malloc 从 20KB 处 bump 直接覆盖 big[] 的
/// 后半段——校验和必然错误。新引擎堆起点上移至全局数据之后。
#[test]
fn test_large_globals_plus_malloc_data_intact() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
static int big[8000];
int main() {
    for (int i = 0; i < 8000; i++) big[i] = i * 3;
    int *p = (int *)malloc(100 * sizeof(int));
    if (p == 0) { printf("MALLOC_NULL\n"); return 1; }
    for (int i = 0; i < 100; i++) p[i] = i + 1;
    long long gsum = 0, psum = 0;
    for (int i = 0; i < 8000; i++) gsum += big[i];
    for (int i = 0; i < 100; i++) psum += p[i];
    printf("gsum=%lld psum=%lld\n", gsum, psum);
    free(p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    let (_, outputs) = match result {
        Ok(v) => v,
        Err(e) => panic!("应编译运行成功，实际: {}", e),
    };
    // Σ i*3 (i=0..7999) = 3 * 7999*8000/2 = 95988000；Σ (i+1) = 5050
    assert!(
        outputs.iter().any(|l| l.contains("gsum=95988000 psum=5050")),
        "全局数据被堆分配破坏或 malloc 失败，输出: {:?}",
        outputs
    );
}

/// 三方挤压（一）：大全局 + malloc 失败 —— 明确返回 NULL（教学 note 走独立通道），
/// 且全局数据完好，不静默损坏。
#[test]
fn test_large_globals_malloc_exhaustion_returns_null() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
static int big[15000];
int main() {
    big[0] = 42;
    big[14999] = 7;
    int *p = (int *)malloc(1048576);
    if (p == 0) {
        printf("NULL_OK big0=%d biglast=%d\n", big[0], big[14999]);
        return 0;
    }
    printf("UNEXPECTED_ALLOC\n");
    return 1;
}
"#;
    let result = compile_and_run(src);
    let (_, outputs) = match result {
        Ok(v) => v,
        Err(e) => panic!("应编译运行成功，实际: {}", e),
    };
    assert!(
        outputs.iter().any(|l| l.contains("NULL_OK big0=42 biglast=7")),
        "malloc 耗尽应返回 NULL 且全局数据完好，输出: {:?}",
        outputs
    );
}

/// 三方挤压（二）：深递归 —— 明确的教学 trap（调用深度上限 / 栈溢出），不崩溃。
#[test]
fn test_deep_recursion_traps_cleanly() {
    let src = r#"
#include <stdio.h>
int depth(int n) { return 1 + depth(n + 1); }
int main() { printf("%d\n", depth(0)); return 0; }
"#;
    let result = compile_and_run(src);
    match result {
        Err(msg) => {
            assert!(
                msg.contains("调用深度") || msg.contains("栈溢出"),
                "应报告调用深度/栈溢出教学 trap，实际: {}",
                msg
            );
        }
        Ok((_, outputs)) => panic!("无限递归应产生明确 trap 而非成功运行，输出: {:?}", outputs),
    }
}

// ===================== R1 ③：argv 编址修复 =====================

/// main(argc, argv) + 全局变量：argv 不得与全局数据重叠编址。
///
/// 旧实现 argv 指针数组落在 `GLOBAL_START + global_count*4`（global_count 恒 0），
/// 压在 libc 预留段/用户全局数据上。新实现自 GLOBAL_REGION_LIMIT 向下分配。
#[test]
fn test_argv_does_not_overlap_globals() {
    let src = r#"
#include <stdio.h>
static int tag = 77;
int main(int argc, char *argv[]) {
    printf("argc=%d argv1=%s tag=%d\n", argc, argv[1], tag);
    return 0;
}
"#;
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());

        // 直接注入会话级 argv（等价于 vitro_set_argv）
        (*session).runtime.argc = 2;
        (*session).runtime.argv = vec!["prog".to_string(), "hello".to_string()];

        let fname = CString::new("main.c").unwrap();
        let src_c = CString::new(src).unwrap();
        vitro_native::capi::vitro_compile_unit(session, fname.as_ptr() as *const c_char, src_c.as_ptr() as *const c_char);
        assert_eq!(vitro_native::capi::vitro_compile_all(session), 0, "应编译成功");

        let _ = vitro_native::capi::vitro_run(session);

        let out_len = vitro_native::capi::vitro_get_output_length(session);
        let mut buf = vec![0u8; out_len as usize + 1];
        vitro_native::capi::vitro_get_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
        let out_str = String::from_utf8_lossy(&buf[..out_len as usize]).to_string();
        vitro_native::capi::vitro_session_destroy(session);

        assert!(
            out_str.contains("argc=2 argv1=hello tag=77"),
            "argv 与全局数据应互不干扰，实际输出: {:?}",
            out_str
        );
    }
}

// ===================== R1 ②/⑤：全局区容量单源 + 编译 warning =====================

/// 全局数据越过 GLOBAL_REGION_LIMIT 上限 → 编译失败（fail loud，而非旧引擎的
/// "大全局无 malloc 时静默可行"）。上限口径 = 全局区容量（GLOBAL_REGION_LIMIT
/// - GLOBAL_START - BYTECODE_LIBC 预留），与字符串字面量共用同一判据。
#[test]
fn test_global_region_over_limit_fails_compile() {
    // 16000 int = 64000 字节 + libc 预留 1024 > 上限（0xF000 = 61440 可用字节）
    let src = r#"
static int big[16000];
int main() { big[0] = 1; return big[0]; }
"#;
    let result = compile_and_run(src);
    match result {
        Err(msg) => assert!(
            msg.contains("全局数据区容量不足"),
            "应报告全局数据区容量不足，实际: {}",
            msg
        ),
        Ok(_) => panic!("超过全局区上限应编译失败而非静默放行"),
    }
}

/// R1 ⑤：全局数据越过 HEAP_START（但不超上限）→ 编译 warning 提示堆起点上移。
#[test]
fn test_large_globals_compile_warning() {
    // 8000 int = 32000 字节，越过 HEAP_START=20480
    let src = r#"
static int big[8000];
int main() { big[0] = 1; return big[0]; }
"#;
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());
        let fname = CString::new("main.c").unwrap();
        let src_c = CString::new(src).unwrap();
        vitro_native::capi::vitro_compile_unit(session, fname.as_ptr() as *const c_char, src_c.as_ptr() as *const c_char);
        assert_eq!(vitro_native::capi::vitro_compile_all(session), 0, "应编译成功");

        let has_layout_warning = (*session).compile.diagnostics.iter().any(|d| {
            d.severity == 1 && d.message.contains("堆起点") && d.message.contains("上移")
        });
        vitro_native::capi::vitro_session_destroy(session);
        assert!(has_layout_warning, "大全局应产生堆起点上移 warning");
    }
}

// ===================== 布局函数单元测试（vitro_runtime） =====================

#[test]
fn test_compute_heap_base_layout_rules() {
    use vitro_runtime::{align4, argv_region_footprint, compute_heap_base, GLOBAL_REGION_LIMIT, GLOBAL_START, HEAP_START};

    // 上限常量：与旧字符串判据 MEM_SIZE/16 一致，不收紧存量行为
    assert_eq!(GLOBAL_REGION_LIMIT, vitro_runtime::MEM_SIZE / 16);
    assert_eq!(GLOBAL_REGION_LIMIT, 0x1_0000);

    // 无全局数据 / 无 argv → 静态 HEAP_START（行为不变锚）
    assert_eq!(compute_heap_base(0, 0, &[]), HEAP_START);

    // 大全局（32KB）→ 堆起点 = align4(global_data_end)
    let end = GLOBAL_START + 33024;
    assert_eq!(compute_heap_base(end, 0, &[]), align4(end));

    // 未对齐的全局末端 → 4 字节对齐
    assert_eq!(compute_heap_base(GLOBAL_START + 33025, 0, &[]), align4(GLOBAL_START + 33025));

    // 有 argv → 堆起点必须越过 argv 顶界（GLOBAL_REGION_LIMIT）
    assert_eq!(compute_heap_base(GLOBAL_START + 2048, 2, &["a".into(), "bb".into()]), GLOBAL_REGION_LIMIT);

    // argv 占用：argc*4 指针槽 + Σ(len+1)，4 字节对齐（2*4 + 2 + 3 = 13 → 16）
    assert_eq!(argv_region_footprint(2, &["a".into(), "bb".into()]), 16);
    assert_eq!(argv_region_footprint(0, &["a".into()]), 0);
    assert_eq!(argv_region_footprint(3, &["x".into()]), align4(3 * 4 + 2));
}
