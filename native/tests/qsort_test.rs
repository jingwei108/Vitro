#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};

fn compile_and_run(source: &str) -> Result<(i32, Vec<String>), String> {
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

        // E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道），不再做文本清洗。
        let mut outputs = Vec::new();
        let out_len = vitro_native::capi::vitro_get_program_output_length(session);
        if out_len > 0 {
            let mut buf = vec![0u8; out_len as usize + 1];
            vitro_native::capi::vitro_get_program_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            let out_str = String::from_utf8_lossy(&buf[..out_len as usize]);
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

        vitro_native::capi::vitro_session_destroy(session);

        if let Some(e) = runtime_err {
            if !e.is_empty() {
                return Err(format!("Runtime error: {}", e));
            }
        }

        Ok((run_ret, outputs))
    }
}

#[test]
fn test_qsort_int_array() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(const void* a, const void* b) {
    int x = *(const int*)a;
    int y = *(const int*)b;
    return x - y;
}
int main() {
    int arr[] = {5, 2, 8, 1, 9, 3};
    qsort(arr, 6, sizeof(int), cmp);
    for (int i = 0; i < 6; i++) {
        printf("%d ", arr[i]);
    }
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("");
    assert_eq!(output.trim(), "1 2 3 5 8 9");
}

#[test]
fn test_qsort_byte_array() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(const void* a, const void* b) {
    return *(const char*)a - *(const char*)b;
}
int main() {
    char arr[] = "fedcba";
    qsort(arr, 6, sizeof(char), cmp);
    printf("%s", arr);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("");
    assert_eq!(output.trim(), "abcdef");
}

#[test]
fn test_qsort_larger_array() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(const void* a, const void* b) {
    return *(const int*)a - *(const int*)b;
}
int main() {
    int arr[100];
    for (int i = 0; i < 100; i++) {
        arr[i] = 100 - i;
    }
    qsort(arr, 100, sizeof(int), cmp);
    for (int i = 0; i < 99; i++) {
        if (arr[i] > arr[i + 1]) {
            printf("UNSORTED");
            return 1;
        }
    }
    printf("OK");
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("");
    assert_eq!(output.trim(), "OK");
}

#[test]
fn test_qsort_thousand_int_array() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(const void* a, const void* b) {
    return *(const int*)a - *(const int*)b;
}
int main() {
    int arr[1000];
    for (int i = 0; i < 1000; i++) {
        arr[i] = 1000 - i;
    }
    qsort(arr, 1000, sizeof(int), cmp);
    for (int i = 0; i < 999; i++) {
        if (arr[i] > arr[i + 1]) {
            printf("UNSORTED");
            return 1;
        }
    }
    printf("OK");
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run(source).expect("Should compile and run");
    assert_eq!(ret, 0);
    let output = outputs.join("");
    assert!(output.trim().starts_with("OK"), "期望输出以 OK 开头，实际：{}", output);
}
