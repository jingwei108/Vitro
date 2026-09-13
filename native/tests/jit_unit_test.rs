#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};

fn compile_and_run(source: &str) -> Result<(i32, Vec<String>, (i32, i32)), String> {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        if session.is_null() {
            return Err("Failed to create session".to_string());
        }

        let src = CString::new(source).map_err(|e| e.to_string())?;
        let compile_ret = vitro_native::capi::vitro_compile(session, src.as_ptr() as *const c_char);
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
            let out_str = String::from_utf8_lossy(&buf);
            for line in out_str.lines() {
                if !line.is_empty() {
                    outputs.push(line.to_string());
                }
            }
        }

        let err_ptr = vitro_native::capi::vitro_get_runtime_error(session);
        let runtime_err = if err_ptr.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string())
        };

        let mut traces_compiled = 0;
        let mut steps_accelerated = 0;
        vitro_native::capi::vitro_get_jit_stats(session, &mut traces_compiled, &mut steps_accelerated);

        vitro_native::capi::vitro_session_destroy(session);

        if let Some(e) = runtime_err {
            if !e.is_empty() {
                return Err(format!("Runtime error: {}", e));
            }
        }

        Ok((run_ret, outputs, (traces_compiled, steps_accelerated)))
    }
}

#[test]
fn test_jit_simple_loop() {
    let source = r#"
#include <stdio.h>
int main() {
    int sum = 0;
    for (int i = 0; i < 200; i++) {
        sum = sum + 1;
    }
    printf("%d", sum);
    return 0;
}
"#;
    let (ret, outputs, jit_stats) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("\n");
    // vitro_get_output 是展示视图（程序 stdout + 引擎附注拼接），无法整体 eq；
    // starts_with 是该通道上可用的最强断言——contains("200") 对错值 "-200"
    // 同样为真（弱断言曾放过 JIT 错值），而 "-200..." 不以 "200" 开头。
    assert!(output.starts_with("200"), "Program stdout should start with 200, got: {:?}", output);
    assert!(jit_stats.0 > 0, "JIT should have triggered, stats: {:?}", jit_stats);
}

#[test]
fn test_jit_no_loop() {
    let source = r#"
#include <stdio.h>
int main() {
    printf("hello");
    return 0;
}
"#;
    let (ret, outputs, jit_stats) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("\n");
    assert!(output.contains("hello"), "Output should contain hello, got: {}", output);
    assert!(jit_stats.0 == 0, "JIT should not trigger without loop, stats: {:?}", jit_stats);
}

#[test]
fn test_jit_array_sum() {
    let source = r#"
#include <stdio.h>
int main() {
    int arr[10] = {1,2,3,4,5,6,7,8,9,10};
    int sum = 0;
    for (int i = 0; i < 10; i++) {
        sum = sum + arr[i];
    }
    printf("%d", sum);
    return 0;
}
"#;
    let (ret, outputs, jit_stats) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("\n");
    assert!(output.contains("55"), "Output should contain 55, got: {}", output);
    // 循环只有 10 次，小于阈值 100，不应触发 JIT
    assert!(
        jit_stats.0 == 0,
        "JIT should not trigger for short loop, stats: {:?}",
        jit_stats
    );
}

// 嵌套循环 trace 录制优化：确保只捕获以 start_ip 为目标的 backward jump，
// 并在条件跳转退出循环时正确设置 end_ip，避免无限重复执行。
#[test]
fn test_jit_nested_loop() {
    let source = r#"
#include <stdio.h>
int main() {
    int sum = 0;
    for (int i = 0; i < 20; i++) {
        for (int j = 0; j < 20; j++) {
            sum = sum + 1;
        }
    }
    printf("%d", sum);
    return 0;
}
"#;
    let (ret, outputs, jit_stats) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("\n");
    // 同 test_jit_simple_loop：展示视图通道用 starts_with（"-400" 不以 "400" 开头）
    assert!(output.starts_with("400"), "Program stdout should start with 400, got: {:?}", output);
    // 内层循环 backward jump 目标被命中 400 次（>阈值 100），JIT 应触发并正确加速
    assert!(jit_stats.0 > 0, "JIT should trigger for nested loops, stats: {:?}", jit_stats);
}
