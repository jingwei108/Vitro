#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, c_int, CString};

// E-P1-5：`filter_outputs`（过滤"程序运行完成"附注）已删除——引擎按通道分离输出后，
// `compile_and_run*` 直接读纯 stdout，不需要也不允许再做文本清洗（清洗会在教学程序
// 自己打印同类文本时误删真实输出）。

fn compile_and_run(source: &str) -> Result<(i32, Vec<String>), String> {
    compile_and_run_detailed(source).map(|(ret, stdout, _notes)| (ret, stdout))
}

/// 与 `compile_and_run` 相同，但额外返回引擎附注（note 通道）。
///
/// E-P1-5：泄漏报告等附注不再混入 stdout，验证附注的测试必须显式读 note 通道。
fn compile_and_run_detailed(source: &str) -> Result<(i32, Vec<String>, Vec<String>), String> {
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

        // E-P1-5：纯程序 stdout；引擎附注单独从 note 通道读取。
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

        let mut notes = Vec::new();
        let notes_len = vitro_native::capi::vitro_get_engine_notes_length(session);
        if notes_len > 0 {
            let mut buf = vec![0u8; notes_len as usize + 1];
            vitro_native::capi::vitro_get_engine_notes(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            let notes_str = String::from_utf8_lossy(&buf[..notes_len as usize]);
            for line in notes_str.lines() {
                if !line.is_empty() {
                    notes.push(line.to_string());
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

        Ok((run_ret, outputs, notes))
    }
}

fn compile_and_run_with_input(source: &str, input: &str) -> Result<(i32, Vec<String>), String> {
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

        let input_cstr = CString::new(input).map_err(|e| e.to_string())?;
        vitro_native::capi::vitro_set_input(session, input_cstr.as_ptr() as *const c_char);

        let run_ret = vitro_native::capi::vitro_run(session);

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

fn compile_and_run_with_argv(source: &str, argv: &[&str]) -> Result<(i32, Vec<String>), String> {
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

        let c_argv: Vec<CString> = argv.iter().map(|s| CString::new(*s).unwrap()).collect();
        let ptrs: Vec<*const c_char> = c_argv.iter().map(|s| s.as_ptr()).collect();
        vitro_native::capi::vitro_set_argv(session, argv.len() as c_int, ptrs.as_ptr());

        let run_ret = vitro_native::capi::vitro_run(session);

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

// ============================================================================
// Control Flow
// ============================================================================

#[test]
fn test_e2e_do_while_loop() {
    let src = r#"
#include <stdio.h>
int main() {
    int i = 0;
    int sum = 0;
    do {
        sum = sum + i;
        i = i + 1;
    } while (i < 5);
    printf("%d", sum);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("10")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_break_continue() {
    let src = r#"
#include <stdio.h>
int main() {
    int sum = 0;
    for (int i = 0; i < 10; i = i + 1) {
        if (i == 3) continue;
        if (i == 7) break;
        sum = sum + i;
    }
    printf("%d", sum);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    // 0+1+2+4+5+6 = 18
    assert!(outputs.iter().any(|l| l.contains("18")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_nested_if() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 5;
    int y = 3;
    if (x > 3) {
        if (y > 2) {
            printf("both");
        } else {
            printf("x only");
        }
    } else {
        printf("none");
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("both")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_logical_operators() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 1;
    int b = 0;
    if (a && !b) printf("ok1");
    if (a || b) printf("ok2");
    if (!(a && b)) printf("ok3");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("ok1")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("ok2")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("ok3")), "Outputs: {:?}", outputs);
}

// ============================================================================
// Data & Types
// ============================================================================

#[test]
fn test_e2e_global_variables() {
    let src = r#"
#include <stdio.h>
int g = 42;
int main() {
    printf("%d", g);
    g = 100;
    printf("%d", g);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("100")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_string_char_array() {
    let src = r#"
#include <stdio.h>
int main() {
    char s[6] = "hello";
    printf("%s", s);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("hello")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_multi_variable_decl() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 1, b = 2, c = 3;
    printf("%d", a + b + c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("6")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_negative_numbers() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = -5;
    int b = -a;
    printf("%d %d", a, b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("-5")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("5")), "Outputs: {:?}", outputs);
}

// ============================================================================
// Functions
// ============================================================================

#[test]
fn test_e2e_multi_arg_function() {
    let src = r#"
#include <stdio.h>
int add3(int a, int b, int c) {
    return a + b + c;
}
int main() {
    printf("%d", add3(1, 2, 3));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("6")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_array_as_function_arg() {
    let src = r#"
#include <stdio.h>
int sum(int arr[], int n) {
    int s = 0;
    for (int i = 0; i < n; i = i + 1) {
        s = s + arr[i];
    }
    return s;
}
int main() {
    int a[3] = {1, 2, 3};
    printf("%d", sum(a, 3));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("6")), "Outputs: {:?}", outputs);
}

// ============================================================================
// Pointers
// ============================================================================

#[test]
fn test_e2e_pointer_arithmetic() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {10, 20, 30};
    int *p = arr;
    printf("%d", *(p + 1));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("20")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_null_pointer_trap() {
    let src = r#"
int main() {
    int *p = 0;
    return *p;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected null pointer trap, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("NULL") || err.contains("空指针") || err.contains("null"),
        "Error: {}",
        err
    );
}

// ============================================================================
// Runtime Traps
// ============================================================================

#[test]
fn test_e2e_div_by_zero_trap() {
    let src = r#"
int main() {
    int a = 10;
    int b = 0;
    return a / b;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected div-by-zero trap, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("除零") || err.contains("div") || err.contains("zero"),
        "Error: {}",
        err
    );
}

#[test]
fn test_e2e_infinite_loop_trap() {
    let src = r#"
int main() {
    while (1) {}
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected infinite loop trap, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("无限循环") || err.contains("步数超过限制") || err.contains("infinite"),
        "Error: {}",
        err
    );
}

// ============================================================================
// Arrays & Structs
// ============================================================================

#[test]
fn test_e2e_multidim_array_init() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[2][3] = {{1, 2, 3}, {4, 5, 6}};
    printf("%d", arr[1][2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("6")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_struct_init_list() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    struct Point p = {3, 4};
    printf("%d", p.x + p.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("7")), "Outputs: {:?}", outputs);
}

// ============================================================================
// Enum / Typedef
// ============================================================================

#[test]
fn test_e2e_enum() {
    let src = r#"
#include <stdio.h>
enum Color { RED, GREEN, BLUE };
int main() {
    enum Color c = GREEN;
    printf("%d", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_typedef() {
    let src = r#"
#include <stdio.h>
typedef int Integer;
int main() {
    Integer a = 42;
    printf("%d", a);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_sizeof() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    int arr[5];
    struct Point p;
    int *ptr;
    printf("%d ", sizeof(int));
    printf("%d ", sizeof(char));
    printf("%d ", sizeof(struct Point));
    printf("%d ", sizeof(arr));
    printf("%d ", sizeof(ptr));
    printf("%d", sizeof(p));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("4 1 8 20 4 8"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_char_literal() {
    let src = r#"
#include <stdio.h>
int main() {
    char c = 'A';
    printf("%d", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("65")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_block_comment() {
    let src = r#"
#include <stdio.h>
/* this is a block comment */
int main() {
    int a = 1;
    /* nested
       multiline
       comment */
    int b = 2;
    printf("%d", a + b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_type_qualifiers() {
    let src = r#"
#include <stdio.h>
int main() {
    const int a = 10;
    long b = 20;
    short c = 30;
    signed int d = 40;
    unsigned int e = 50;
    printf("%d", a + b + c + d + e);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("150")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_hex_literal() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 0x0A;
    int b = 0xFF;
    int c = 0x100;
    printf("%d %d %d", a, b, c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("10 255 256"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_escape_sequences() {
    let src = r#"
#include <stdio.h>
int main() {
    char s[10];
    s[0] = '\r';
    s[1] = '\a';
    s[2] = '\b';
    s[3] = '\f';
    s[4] = '\v';
    s[5] = '\x41';
    printf("%d %d %d %d %d %d", s[0], s[1], s[2], s[3], s[4], s[5]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("13 7 8 12 11 65"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_compound_assign_array() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {10, 20, 30};
    arr[0] += 5;
    arr[1] -= 5;
    arr[2] *= 2;
    printf("%d %d %d", arr[0], arr[1], arr[2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("15 15 60"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_compound_assign_deref() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 10;
    int *p = &x;
    *p += 5;
    printf("%d", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("15")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_compound_assign_member() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    struct Point p;
    p.x = 10;
    p.y = 20;
    p.x += 5;
    p.y -= 5;
    printf("%d %d", p.x, p.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("15 15"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_addr_of_array_index() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {10, 20, 30};
    int *p = &arr[1];
    printf("%d", *p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("20")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_addr_of_member() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    struct Point p;
    p.x = 42;
    int *px = &p.x;
    printf("%d", *px);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_global_struct_member() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
struct Point g;
int main() {
    g.x = 10;
    g.y = 20;
    printf("%d %d", g.x, g.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("10 20"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_inc_dec_complex() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    int arr[3] = {10, 20, 30};
    int a = arr[0]++;
    int b = ++arr[1];
    struct Point p;
    p.x = 5;
    p.y = 10;
    int c = p.x--;
    int d = --p.y;
    printf("%d %d %d %d %d %d", a, b, arr[0], arr[1], c, d);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("10 21 11 21 5 9"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_bitwise_ops() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;   // 0101
    int b = 3;   // 0011
    int c = a & b;
    int d = a | b;
    int e = a ^ b;
    int f = ~a;
    int g = a << 1;
    int h = a >> 1;
    printf("%d %d %d %d %d %d", c, d, e, f, g, h);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("1 7 6 -6 10 2"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_bitwise_precedence() {
    let src = r#"
#include <stdio.h>
int main() {
    int r = 1 | 2 & 4;
    printf("%d", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_ternary() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;
    int b = 3;
    int max = a > b ? a : b;
    int min = a < b ? a : b;
    int sign = a > 0 ? 1 : (a < 0 ? -1 : 0);
    printf("%d %d %d", max, min, sign);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("5 3 1"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_ptr_inc_dec() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {10, 20, 30};
    int *p = arr;
    int a = *p;
    p = p + 1;
    int b = *p;
    p++;
    int c = *p;
    p--;
    int d = *p;
    printf("%d %d %d %d", a, b, c, d);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("10 20 30 20"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_ptr_sub_ptr() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[5] = {10, 20, 30, 40, 50};
    int *p = &arr[4];
    int *q = &arr[1];
    int diff = p - q;
    printf("%d", diff);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_forward_decl() {
    let src = r#"
#include <stdio.h>
int add(int a, int b);
int main() {
    printf("%d", add(1, 2));
    return 0;
}
int add(int a, int b) {
    return a + b;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_strlen() {
    let src = r#"
#include <stdio.h>
int main() {
    char s[] = "hello";
    int len = strlen(s);
    printf("%d", len);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_strcpy() {
    let src = r#"
#include <stdio.h>
int main() {
    char src[] = "hello";
    char dest[10];
    strcpy(dest, src);
    printf("%s", dest);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("hello")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_strcmp() {
    let src = r#"
#include <stdio.h>
int main() {
    char a[] = "abc";
    char b[] = "abc";
    char c[] = "abd";
    int r1 = strcmp(a, b);
    int r2 = strcmp(a, c);
    printf("%d %d", r1, r2);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let output = outputs.join(" ");
    assert!(output.contains("0 -1"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_cast_malloc() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(sizeof(int));
    *p = 42;
    printf("%d", *p);
    free(p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_cast_pointer() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {1, 2, 3};
    char *c = (char*)arr;
    printf("%d", *c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
}

// ============================================================================
// Student classic error cases (compile-time & runtime)
// ============================================================================

#[test]
fn test_e2e_error_undeclared_variable() {
    let src = r#"
#include <stdio.h>
int main() {
    x = 5;
    printf("%d", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected compile error for undeclared variable");
    let err = result.unwrap_err();
    assert!(
        err.contains("x")
            || err.contains("undeclared")
            || err.contains("not declared")
            || err.contains("Unknown identifier"),
        "Error should mention undeclared variable x: {}",
        err
    );
}

#[test]
fn test_e2e_error_scanf_missing_ampersand() {
    let src = r#"
#include <stdio.h>
int main() {
    int a;
    scanf("%d", a);
    printf("%d", a);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected compile error for scanf without &");
    let err = result.unwrap_err();
    assert!(
        err.contains("scanf") || err.contains("type") || err.contains("pointer") || err.contains("int*"),
        "Error should mention type mismatch in scanf: {}",
        err
    );
}

#[test]
fn test_e2e_error_array_out_of_bounds() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[3] = {10, 20, 30};
    printf("%d", arr[5]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected runtime error for array out of bounds");
    let err = result.unwrap_err();
    assert!(
        err.contains("bounds")
            || err.contains("out of")
            || err.contains("overflow")
            || err.contains("memory")
            || err.contains("数组越界")
            || err.contains("越界"),
        "Error should mention bounds violation: {}",
        err
    );
}

#[test]
fn test_e2e_error_divide_by_zero() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;
    int b = 0;
    printf("%d", a / b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected runtime error for divide by zero");
    let err = result.unwrap_err();
    assert!(
        err.contains("zero")
            || err.contains("divide")
            || err.contains("arithmetic")
            || err.contains("除零")
            || err.contains("除以"),
        "Error should mention division by zero: {}",
        err
    );
}

// NOTE: Current TypeChecker does not enforce non-void functions to have a return
// statement on all paths, so this case compiles successfully.
// #[test]
// fn test_e2e_error_missing_return_non_void() { ... }

#[test]
fn test_e2e_typedef_anon_struct() {
    let src = r#"
#include <stdio.h>
typedef struct {
    int x;
    int y;
} Point;

int main() {
    Point p;
    p.x = 3;
    p.y = 4;
    printf("%d %d", p.x, p.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3 4")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_typedef_named_struct() {
    let src = r#"
#include <stdio.h>
typedef struct Vec2 {
    int x;
    int y;
} Vec2Alias;

int main() {
    Vec2Alias v;
    v.x = 10;
    v.y = 20;
    printf("%d %d", v.x, v.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("10 20")), "Outputs: {:?}", outputs);
}

// ============================================================================
// P0 Subset Extensions
// ============================================================================

#[test]
fn test_e2e_null_keyword() {
    let src = r#"
#include <stdio.h>
int main() {
    int *p = NULL;
    if (p == NULL) {
        printf("null");
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("null")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_rand_srand() {
    let src = r#"
#include <stdio.h>
int main() {
    srand(12345);
    int a = rand();
    int b = rand();
    printf("%d %d", a, b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    // Extract first two integers from the output line
    let line = outputs.join("");
    let nums: Vec<i32> = line
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i32>().unwrap())
        .collect();
    assert!(nums.len() >= 2, "Expected at least 2 numbers in output: {}", line);
    let a = nums[0];
    let b = nums[1];
    assert!((0..=32767).contains(&a), "rand out of range: {}", a);
    assert!((0..=32767).contains(&b), "rand out of range: {}", b);
    assert_ne!(a, b, "rand should produce different values");
}

#[test]
fn test_e2e_memset() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[5];
    memset(arr, 0, sizeof(arr));
    printf("%d %d %d", arr[0], arr[2], arr[4]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("0 0 0")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_exit() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("before");
    exit(42);
    printf("after");
    return 0;
}
"#;
    let result = compile_and_run_detailed(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_ret, stdout, notes) = result.unwrap();
    assert!(stdout.iter().any(|l| l.contains("before")), "stdout: {:?}", stdout);
    assert!(!stdout.iter().any(|l| l.contains("after")), "stdout: {:?}", stdout);
    // E-P1-5：运行完成提示是引擎附注，读 note 通道（不再混入 stdout）。
    assert!(notes.iter().any(|l| l.contains("返回值：42")), "notes: {:?}", notes);
}

#[test]
fn test_e2e_atoi() {
    let src = r#"
#include <stdio.h>
int main() {
    char s[] = "12345";
    int n = atoi(s);
    printf("%d", n);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("12345")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_strcat() {
    let src = r#"
#include <stdio.h>
int main() {
    char s[20] = "Hello";
    strcat(s, " World");
    printf("%s", s);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("Hello World")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_putchar() {
    let src = r#"
#include <stdio.h>
int main() {
    putchar('A');
    putchar('B');
    putchar('C');
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("ABC"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_const_var() {
    let src = r#"
#include <stdio.h>
int main() {
    const int MAX = 100;
    printf("%d", MAX);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("100")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_const_assign_error() {
    let src = r#"
int main() {
    const int x = 10;
    x = 20;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected compile error for assigning to const");
}

#[test]
fn test_e2e_const_inc_error() {
    let src = r#"
int main() {
    const int x = 10;
    x++;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected compile error for incrementing const");
}

// ============================================================================
// Float / Double
// ============================================================================

#[test]
fn test_e2e_float_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    float a = 3.5;
    float b = 2.0;
    float c = a + b;
    printf("%f", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.500000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_arithmetic() {
    let src = r#"
#include <stdio.h>
int main() {
    float a = 10.0;
    float b = 3.0;
    printf("%f", a - b);
    printf("%f", a * b);
    printf("%f", a / b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("7.000000")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("30.000000")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("3.333333")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_compare() {
    let src = r#"
#include <stdio.h>
int main() {
    float a = 5.0;
    float b = 3.0;
    printf("%d", a > b);
    printf("%d", a < b);
    printf("%d", a == b);
    printf("%d", a != b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("0")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_mixed_int() {
    let src = r#"
#include <stdio.h>
int main() {
    float a = 5.0;
    int b = 2;
    float c = a + b;
    printf("%f", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("7.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_cast() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;
    float b = (float)a;
    printf("%f", b);
    float c = 3.7;
    int d = (int)c;
    printf("%d", d);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_assign_int() {
    let src = r#"
#include <stdio.h>
int main() {
    float x = 5;
    printf("%f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_compound_assign() {
    let src = r#"
#include <stdio.h>
int main() {
    float x = 2.0;
    x += 3.0;
    printf("%f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_func_arg_implicit_cast() {
    let src = r#"
#include <stdio.h>
void foo(float x) {
    printf("%f", x);
}
int main() {
    foo(5);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_float_negative() {
    // 回归测试：PushConstF 符号扩展 bug（负 float 值被错误地符号扩展为 64 位）
    let src = r#"
#include <stdio.h>
int main() {
    float a = -1.5;
    float b = -2.0;
    printf("%.1f", a);
    printf("%.1f", b);
    printf("%.1f", a + b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("-1.5")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("-2.0")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("-3.5")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    double a = 3.5;
    double b = 2.0;
    double c = a + b;
    printf("%f", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.500000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_arr() {
    let src = r#"
#include <stdio.h>
int main() {
    double arr[3] = {1.1, 2.2, 3.3};
    printf("%f", arr[1]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("2.200000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_printf_precision() {
    let src = r#"
#include <stdio.h>
int main() {
    double x = 3.14159;
    printf("%.2f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3.14")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_func_arg_return() {
    let src = r#"
#include <stdio.h>
double add(double a, double b) {
    return a + b;
}
int main() {
    double r = add(1.5, 2.5);
    printf("%f", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("4.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_implicit_cast_from_int() {
    let src = r#"
#include <stdio.h>
int main() {
    double x = 7;
    printf("%f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("7.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_compare() {
    let src = r#"
#include <stdio.h>
int main() {
    double a = 5.0;
    double b = 3.0;
    printf("%d", a > b);
    printf("%d", a == b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("0")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_cast() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;
    double b = (double)a;
    printf("%f", b);
    double c = 3.7;
    int d = (int)c;
    printf("%d", d);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_compound_assign() {
    let src = r#"
#include <stdio.h>
int main() {
    double x = 2.0;
    x += 3.0;
    printf("%f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5.000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_precision_64bit() {
    let src = r#"
#include <stdio.h>
int main() {
    double x = 1.0000000001;
    printf("%.10f", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1.0000000001")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_scanf_lf() {
    let src = r#"
#include <stdio.h>
int main() {
    double x;
    scanf("%lf", &x);
    printf("%.6f", x);
    return 0;
}
"#;
    let result = compile_and_run_with_input(src, "3.1415926535");
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3.141593")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_double_scanf_lf_and_int() {
    let src = r#"
#include <stdio.h>
int main() {
    int n;
    double x;
    scanf("%d %lf", &n, &x);
    printf("%d ", n);
    printf("%.2f", x);
    return 0;
}
"#;
    let result = compile_and_run_with_input(src, "42 2.71828");
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
    assert!(outputs.iter().any(|l| l.contains("2.72")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_long_long_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    long long ll = 9223372036854775807LL;
    printf("%lld", ll);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(
        outputs.iter().any(|l| l.contains("9223372036854775807")),
        "Outputs: {:?}",
        outputs
    );
}

#[test]
fn test_e2e_long_long_arith() {
    let src = r#"
#include <stdio.h>
int main() {
    long long a = 3000000000LL;
    long long b = 2000000000LL;
    long long c = a + b;
    printf("%lld", c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5000000000")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_long_long_scanf() {
    let src = r#"
#include <stdio.h>
int main() {
    long long ll;
    scanf("%lld", &ll);
    printf("%lld", ll);
    return 0;
}
"#;
    let result = compile_and_run_with_input(src, "123456789012");
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("123456789012")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_union_basic() {
    let src = r#"
#include <stdio.h>
union U { int i; float f; };
int main() {
    union U u;
    u.i = 1;
    printf("%d", u.i);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_union_double_member() {
    let src = r#"
#include <stdio.h>
union U { int i; double d; };
int main() {
    union U u;
    u.d = 3.14;
    printf("%.2f", u.d);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3.14")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_union_sizeof() {
    let src = r#"
#include <stdio.h>
union U { int i; double d; };
int main() {
    printf("%d", (int)sizeof(union U));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("8")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_union_int_member() {
    let src = r#"
#include <stdio.h>
union U { int i; double d; };
int main() {
    union U u;
    u.i = 42;
    printf("%d", u.i);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_union_pointer() {
    let src = r#"
#include <stdio.h>
union U { int i; double d; };
int main() {
    union U u;
    union U *p = &u;
    p->i = 99;
    printf("%d", p->i);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("99")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_int_func_arg_implicit_cast_from_float() {
    let src = r#"
#include <stdio.h>
void bar(int x) {
    printf("%d", x);
}
int main() {
    bar(3.7);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_host_func_arg_implicit_cast() {
    let src = r#"
#include <stdio.h>
int main() {
    putchar(65.0);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("A"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_multi_array_var_decl() {
    let src = r#"
#include <stdio.h>
int main() {
    int pre[100], in[100];
    pre[0] = 1;
    pre[1] = 2;
    in[0] = 3;
    in[1] = 4;
    printf("%d %d %d %d", pre[0], pre[1], in[0], in[1]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("1 2 3 4"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_fprintf() {
    let src = r#"
#include <stdio.h>
int main() {
    fprintf(stdout, "hello %d", 42);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("hello 42"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_realloc() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int *)malloc(sizeof(int) * 2);
    p[0] = 10;
    p[1] = 20;
    p = (int *)realloc(p, sizeof(int) * 4);
    p[2] = 30;
    p[3] = 40;
    printf("%d %d %d %d", p[0], p[1], p[2], p[3]);
    free(p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("10 20 30 40"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_qsort() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(const void *a, const void *b) {
    int ia = *(int*)a;
    int ib = *(int*)b;
    return ia - ib;
}
int main() {
    int arr[] = {5, 2, 8, 1, 9};
    qsort(arr, 5, sizeof(int), cmp);
    printf("%d %d %d %d %d", arr[0], arr[1], arr[2], arr[3], arr[4]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("1 2 5 8 9"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_chinese_string() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("你好世界");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = outputs.join("");
    assert!(joined.contains("你好世界"), "Outputs: {:?}", outputs);
}

// ============================================================================
// Medium-difficulty random tests
// ============================================================================

#[test]
fn test_e2e_struct_array_bubble_sort() {
    let src = r#"
#include <stdio.h>
typedef struct {
    int id;
    int score;
} Student;

int main() {
    Student s[3];
    s[0].id = 3; s[0].score = 85;
    s[1].id = 1; s[1].score = 92;
    s[2].id = 2; s[2].score = 78;

    for (int i = 0; i < 2; i++) {
        for (int j = 0; j < 2 - i; j++) {
            if (s[j].score > s[j+1].score) {
                Student tmp = s[j];
                s[j] = s[j+1];
                s[j+1] = tmp;
            }
        }
    }

    for (int i = 0; i < 3; i++) {
        printf("%d %d\n", s[i].id, s[i].score);
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["2 78", "3 85", "1 92"]);
}

#[test]
fn test_e2e_linked_list() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
typedef struct Node {
    int data;
    struct Node *next;
} Node;

Node* create_list(int n) {
    Node *head = NULL;
    Node *tail = NULL;
    for (int i = 0; i < n; i++) {
        Node *p = (Node*)malloc(sizeof(Node));
        p->data = i + 1;
        p->next = NULL;
        if (!head) {
            head = tail = p;
        } else {
            tail->next = p;
            tail = p;
        }
    }
    return head;
}

void print_list(Node *head) {
    while (head) {
        printf("%d ", head->data);
        head = head->next;
    }
    printf("\n");
}

void free_list(Node *head) {
    while (head) {
        Node *tmp = head;
        head = head->next;
        free(tmp);
    }
}

int main() {
    Node *list = create_list(5);
    print_list(list);
    free_list(list);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1 2 3 4 5 "]);
}

#[test]
fn test_e2e_fibonacci_recursive() {
    let src = r#"
#include <stdio.h>
int fib(int n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    for (int i = 0; i <= 10; i++) {
        printf("%d ", fib(i));
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["0 1 1 2 3 5 8 13 21 34 55 "]);
}

#[test]
fn test_e2e_pointer_array() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 10, b = 20, c = 30;
    int *arr[3];
    arr[0] = &a;
    arr[1] = &b;
    arr[2] = &c;
    for (int i = 0; i < 3; i++) {
        printf("%d ", *arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["10 20 30 "]);
}

#[test]
fn test_e2e_strcat_strlen() {
    let src = r#"
#include <stdio.h>
#include <string.h>
int main() {
    char s1[30] = "Hello";
    char s2[10] = "World";
    strcat(s1, " ");
    strcat(s1, s2);
    printf("%s %d\n", s1, strlen(s1));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["Hello World 11"]);
}

#[test]
fn test_e2e_2d_array_func_arg() {
    let src = r#"
#include <stdio.h>
void print_mat(int m[][3], int rows) {
    for (int i = 0; i < rows; i++) {
        for (int j = 0; j < 3; j++) {
            printf("%d ", m[i][j]);
        }
        printf("\n");
    }
}
int main() {
    int mat[2][3] = {{1, 2, 3}, {4, 5, 6}};
    print_mat(mat, 2);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1 2 3 ", "4 5 6 "]);
}

#[test]
fn test_e2e_nested_struct() {
    let src = r#"
#include <stdio.h>
typedef struct {
    int x;
    int y;
} Point;

typedef struct {
    Point top_left;
    Point bottom_right;
} Rect;

int main() {
    Rect r;
    r.top_left.x = 0;
    r.top_left.y = 0;
    r.bottom_right.x = 10;
    r.bottom_right.y = 20;
    printf("%d %d %d %d\n", r.top_left.x, r.top_left.y, r.bottom_right.x, r.bottom_right.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["0 0 10 20"]);
}

#[test]
fn test_e2e_do_while_continue() {
    let src = r#"
#include <stdio.h>
int main() {
    int i = 0;
    int sum = 0;
    do {
        i++;
        if (i % 2 == 0) continue;
        sum += i;
    } while (i < 10);
    printf("%d\n", sum);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["25"]);
}

#[test]
fn test_e2e_nested_ternary() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5, b = 3, c = 7;
    int max = (a > b) ? ((a > c) ? a : c) : ((b > c) ? b : c);
    printf("%d\n", max);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["7"]);
}

#[test]
fn test_e2e_qsort_struct_array() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
typedef struct {
    int id;
    int score;
} Student;

int cmp(const void *a, const void *b) {
    Student *sa = (Student*)a;
    Student *sb = (Student*)b;
    return sa->score - sb->score;
}

int main() {
    Student s[4];
    s[0].id = 1; s[0].score = 85;
    s[1].id = 2; s[1].score = 92;
    s[2].id = 3; s[2].score = 78;
    s[3].id = 4; s[3].score = 88;
    qsort(s, 4, sizeof(Student), cmp);
    for (int i = 0; i < 4; i++) {
        printf("%d %d\n", s[i].id, s[i].score);
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["3 78", "1 85", "4 88", "2 92"]);
}

// ============================================================================
// Medium-difficulty random tests (batch 2)
// ============================================================================

#[test]
fn test_e2e_binary_search() {
    let src = r#"
#include <stdio.h>
int binary_search(int arr[], int n, int target) {
    int left = 0;
    int right = n - 1;
    while (left <= right) {
        int mid = left + (right - left) / 2;
        if (arr[mid] == target) return mid;
        if (arr[mid] < target) left = mid + 1;
        else right = mid - 1;
    }
    return -1;
}
int main() {
    int arr[7] = {1, 3, 5, 7, 9, 11, 13};
    printf("%d ", binary_search(arr, 7, 7));
    printf("%d ", binary_search(arr, 7, 1));
    printf("%d ", binary_search(arr, 7, 13));
    printf("%d\n", binary_search(arr, 7, 4));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["3 0 6 -1"]);
}

#[test]
fn test_e2e_string_reverse_inplace() {
    let src = r#"
#include <stdio.h>
void reverse(char s[]) {
    int len = 0;
    while (s[len] != 0) len++;
    int i = 0;
    int j = len - 1;
    while (i < j) {
        char tmp = s[i];
        s[i] = s[j];
        s[j] = tmp;
        i++;
        j--;
    }
}
int main() {
    char s1[10] = "hello";
    reverse(s1);
    printf("%s\n", s1);
    char s2[10] = "abcd";
    reverse(s2);
    printf("%s\n", s2);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["olleh", "dcba"]);
}

#[test]
fn test_e2e_array_stack() {
    let src = r#"
#include <stdio.h>
#define MAX 100
typedef struct {
    int data[MAX];
    int top;
} Stack;

void init(Stack *s) {
    s->top = -1;
}

int is_empty(Stack *s) {
    return s->top == -1;
}

void push(Stack *s, int x) {
    s->top++;
    s->data[s->top] = x;
}

int pop(Stack *s) {
    int x = s->data[s->top];
    s->top--;
    return x;
}

int peek(Stack *s) {
    return s->data[s->top];
}

int main() {
    Stack *s = (Stack*)malloc(sizeof(Stack));
    init(s);
    push(s, 10);
    push(s, 20);
    push(s, 30);
    printf("%d ", peek(s));
    printf("%d ", pop(s));
    printf("%d ", pop(s));
    printf("%d\n", is_empty(s));
    printf("%d ", pop(s));
    printf("%d\n", is_empty(s));
    free(s);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["30 30 20 0", "10 1"]);
}

#[test]
fn test_e2e_selection_sort() {
    let src = r#"
#include <stdio.h>
void selection_sort(int arr[], int n) {
    for (int i = 0; i < n - 1; i++) {
        int min_idx = i;
        for (int j = i + 1; j < n; j++) {
            if (arr[j] < arr[min_idx]) {
                min_idx = j;
            }
        }
        if (min_idx != i) {
            int tmp = arr[i];
            arr[i] = arr[min_idx];
            arr[min_idx] = tmp;
        }
    }
}
int main() {
    int arr[6] = {64, 25, 12, 22, 11, 90};
    selection_sort(arr, 6);
    for (int i = 0; i < 6; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["11 12 22 25 64 90 "]);
}

#[test]
fn test_e2e_decimal_to_binary() {
    let src = r#"
#include <stdio.h>
void print_binary(int n) {
    if (n == 0) {
        printf("0");
        return;
    }
    int bits[32];
    int i = 0;
    while (n > 0) {
        bits[i] = n & 1;
        n = n >> 1;
        i++;
    }
    for (int j = i - 1; j >= 0; j--) {
        printf("%d", bits[j]);
    }
}
int main() {
    print_binary(5);
    printf("\n");
    print_binary(13);
    printf("\n");
    print_binary(0);
    printf("\n");
    print_binary(255);
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["101", "1101", "0", "11111111"]);
}

// ============================================================================
// Medium-difficulty random tests (batch 3)
// ============================================================================

#[test]
fn test_e2e_hanoi_recursive() {
    let src = r#"
#include <stdio.h>
void hanoi(int n, char from, char to, char aux) {
    if (n == 1) {
        printf("%c -> %c\n", from, to);
        return;
    }
    hanoi(n - 1, from, aux, to);
    printf("%c -> %c\n", from, to);
    hanoi(n - 1, aux, to, from);
}
int main() {
    hanoi(3, 'A', 'C', 'B');
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["A -> C", "A -> B", "C -> B", "A -> C", "B -> A", "B -> C", "A -> C"]);
}

#[test]
fn test_e2e_pointer_sum_array() {
    let src = r#"
#include <stdio.h>
int sum_array(int *arr, int n) {
    int s = 0;
    for (int i = 0; i < n; i++) {
        s += *(arr + i);
    }
    return s;
}
int main() {
    int a[5] = {1, 2, 3, 4, 5};
    printf("%d\n", sum_array(a, 5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["15"]);
}

#[test]
fn test_e2e_vowel_count() {
    let src = r#"
#include <stdio.h>
int is_vowel(char c) {
    switch (c) {
        case 'a': return 1;
        case 'e': return 1;
        case 'i': return 1;
        case 'o': return 1;
        case 'u': return 1;
        case 'A': return 1;
        case 'E': return 1;
        case 'I': return 1;
        case 'O': return 1;
        case 'U': return 1;
        default: return 0;
    }
}
int main() {
    char s[] = "Hello World";
    int count = 0;
    for (int i = 0; s[i] != 0; i++) {
        if (is_vowel(s[i])) count++;
    }
    printf("%d\n", count);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["3"]);
}

#[test]
fn test_e2e_matrix_diagonal_sum() {
    let src = r#"
#include <stdio.h>
int main() {
    int mat[3][3] = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}};
    int sum = 0;
    for (int i = 0; i < 3; i++) {
        sum += mat[i][i];
    }
    printf("%d\n", sum);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["15"]);
}

#[test]
fn test_e2e_array_dedup() {
    let src = r#"
#include <stdio.h>
void selection_sort(int arr[], int n) {
    for (int i = 0; i < n - 1; i++) {
        int min_idx = i;
        for (int j = i + 1; j < n; j++) {
            if (arr[j] < arr[min_idx]) min_idx = j;
        }
        if (min_idx != i) {
            int tmp = arr[i];
            arr[i] = arr[min_idx];
            arr[min_idx] = tmp;
        }
    }
}
int remove_duplicates(int arr[], int n) {
    if (n == 0) return 0;
    selection_sort(arr, n);
    int j = 0;
    for (int i = 0; i < n; i++) {
        if (i == 0 || arr[i] != arr[i - 1]) {
            arr[j] = arr[i];
            j++;
        }
    }
    return j;
}
int main() {
    int arr[8] = {3, 1, 4, 1, 5, 9, 2, 6};
    int new_len = remove_duplicates(arr, 8);
    printf("%d\n", new_len);
    for (int i = 0; i < new_len; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["7", "1 2 3 4 5 6 9 "]);
}

// ============================================================================
// Medium-difficulty random tests (batch 4)
// ============================================================================

#[test]
fn test_e2e_palindrome_string() {
    let src = r#"
#include <stdio.h>
int is_palindrome(char s[]) {
    int len = 0;
    while (s[len] != 0) len++;
    int i = 0;
    int j = len - 1;
    while (i < j) {
        if (s[i] != s[j]) return 0;
        i++;
        j--;
    }
    return 1;
}
int main() {
    printf("%d\n", is_palindrome("radar"));
    printf("%d\n", is_palindrome("hello"));
    printf("%d\n", is_palindrome("a"));
    printf("%d\n", is_palindrome(""));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1", "0", "1", "1"]);
}

#[test]
fn test_e2e_matrix_add() {
    let src = r#"
#include <stdio.h>
int main() {
    int a[2][3] = {{1, 2, 3}, {4, 5, 6}};
    int b[2][3] = {{7, 8, 9}, {10, 11, 12}};
    int c[2][3];
    for (int i = 0; i < 2; i++) {
        for (int j = 0; j < 3; j++) {
            c[i][j] = a[i][j] + b[i][j];
            printf("%d ", c[i][j]);
        }
        printf("\n");
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["8 10 12 ", "14 16 18 "]);
}

#[test]
fn test_e2e_my_strlen() {
    let src = r#"
#include <stdio.h>
int my_strlen(char s[]) {
    int len = 0;
    while (s[len] != 0) len++;
    return len;
}
int main() {
    printf("%d\n", my_strlen(""));
    printf("%d\n", my_strlen("hello"));
    printf("%d\n", my_strlen("VitroVM"));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    // 输入字面量随项目更名由 "CideVM"（6 字符）变为 "VitroVM"（7 字符），期望值联动。
    assert_eq!(out, vec!["0", "5", "7"]);
}

#[test]
fn test_e2e_max_subarray_sum() {
    let src = r#"
#include <stdio.h>
int max_subarray_sum(int arr[], int n) {
    int max_so_far = arr[0];
    int curr_max = arr[0];
    for (int i = 1; i < n; i++) {
        curr_max = (curr_max + arr[i] > arr[i]) ? curr_max + arr[i] : arr[i];
        max_so_far = (max_so_far > curr_max) ? max_so_far : curr_max;
    }
    return max_so_far;
}
int main() {
    int a[8] = {-2, 1, -3, 4, -1, 2, 1, -5};
    printf("%d\n", max_subarray_sum(a, 8));
    int b[3] = {1, 2, 3};
    printf("%d\n", max_subarray_sum(b, 3));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["6", "6"]);
}

#[test]
fn test_e2e_simple_calc() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int mul(int a, int b) { return a * b; }
int divide(int a, int b) { return a / b; }
int main() {
    int a = 15;
    int b = 3;
    printf("%d\n", add(a, b));
    printf("%d\n", sub(a, b));
    printf("%d\n", mul(a, b));
    printf("%d\n", divide(a, b));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["18", "12", "45", "5"]);
}

// ============================================================================
// Medium-difficulty random tests (batch 5)
// ============================================================================

#[test]
fn test_e2e_insertion_sort() {
    let src = r#"
#include <stdio.h>
void insertion_sort(int arr[], int n) {
    for (int i = 1; i < n; i++) {
        int key = arr[i];
        int j = i - 1;
        while (j >= 0 && arr[j] > key) {
            arr[j + 1] = arr[j];
            j--;
        }
        arr[j + 1] = key;
    }
}
int main() {
    int arr[7] = {5, 2, 4, 6, 1, 3, 0};
    insertion_sort(arr, 7);
    for (int i = 0; i < 7; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["0 1 2 3 4 5 6 "]);
}

#[test]
fn test_e2e_my_strcpy() {
    let src = r#"
#include <stdio.h>
void my_strcpy(char dest[], char src[]) {
    int i = 0;
    while (src[i] != 0) {
        dest[i] = src[i];
        i++;
    }
    dest[i] = 0;
}
int main() {
    char s1[20];
    my_strcpy(s1, "Hello");
    printf("%s\n", s1);
    char s2[20];
    my_strcpy(s2, "VitroVM");
    printf("%s\n", s2);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["Hello", "VitroVM"]);
}

#[test]
fn test_e2e_array_reverse() {
    let src = r#"
#include <stdio.h>
void reverse(int arr[], int n) {
    int i = 0;
    int j = n - 1;
    while (i < j) {
        int tmp = arr[i];
        arr[i] = arr[j];
        arr[j] = tmp;
        i++;
        j--;
    }
}
int main() {
    int arr[6] = {1, 2, 3, 4, 5, 6};
    reverse(arr, 6);
    for (int i = 0; i < 6; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["6 5 4 3 2 1 "]);
}

#[test]
fn test_e2e_power_recursive() {
    let src = r#"
#include <stdio.h>
int power(int base, int exp) {
    if (exp == 0) return 1;
    if (exp == 1) return base;
    int half = power(base, exp / 2);
    if (exp % 2 == 0) {
        return half * half;
    }
    return base * half * half;
}
int main() {
    printf("%d\n", power(2, 0));
    printf("%d\n", power(2, 5));
    printf("%d\n", power(3, 4));
    printf("%d\n", power(5, 3));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1", "32", "81", "125"]);
}

#[test]
fn test_e2e_struct_rect_area() {
    let src = r#"
#include <stdio.h>
typedef struct {
    int width;
    int height;
} Rect;

int area(Rect r) {
    return r.width * r.height;
}

int main() {
    Rect a;
    a.width = 3;
    a.height = 4;
    Rect b;
    b.width = 5;
    b.height = 2;
    printf("%d\n", area(a));
    printf("%d\n", area(b));
    printf("%d\n", area(a) > area(b));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["12", "10", "1"]);
}

// ============================================================================
// Built-in template tests
// ============================================================================

#[test]
fn test_e2e_template_quick_sort() {
    let src = r#"
#include <stdio.h>
void quickSort(int arr[], int low, int high) {
    if (low < high) {
        int pivot = partition(arr, low, high);
        quickSort(arr, low, pivot - 1);
        quickSort(arr, pivot + 1, high);
    }
}

int partition(int arr[], int low, int high) {
    int pivot = arr[high];
    int i = low - 1;
    for (int j = low; j < high; j++) {
        if (arr[j] <= pivot) {
            i++;
            int temp = arr[i];
            arr[i] = arr[j];
            arr[j] = temp;
        }
    }
    int temp = arr[i + 1];
    arr[i + 1] = arr[high];
    arr[high] = temp;
    return i + 1;
}

int main() {
    int arr[7] = {3, 6, 8, 10, 1, 2, 1};
    quickSort(arr, 0, 6);
    for (int i = 0; i < 7; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1 1 2 3 6 8 10 "]);
}

#[test]
fn test_e2e_template_pointer_swap() {
    let src = r#"
#include <stdio.h>
void swap(int* a, int* b) {
    int temp = *a;
    *a = *b;
    *b = temp;
}

int main() {
    int x = 5, y = 10;
    swap(&x, &y);
    printf("%d %d\n", x, y);
    int arr[3] = {1, 2, 3};
    swap(&arr[0], &arr[2]);
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["10 5", "3 2 1"]);
}

// ============================================================================
// Medium-difficulty random tests (batch 6)
// ============================================================================

#[test]
fn test_e2e_template_bubble_sort_int() {
    let src = r#"
#include <stdio.h>
void bubbleSort(int arr[], int n) {
    for (int i = 0; i < n - 1; i++) {
        for (int j = 0; j < n - i - 1; j++) {
            if (arr[j] > arr[j + 1]) {
                int temp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = temp;
            }
        }
    }
}
int main() {
    int arr[6] = {5, 1, 4, 2, 8, 0};
    bubbleSort(arr, 6);
    for (int i = 0; i < 6; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["0 1 2 4 5 8 "]);
}

#[test]
fn test_e2e_my_strcmp() {
    let src = r#"
#include <stdio.h>
int my_strcmp(char a[], char b[]) {
    int i = 0;
    while (a[i] != 0 && b[i] != 0) {
        if (a[i] != b[i]) return a[i] - b[i];
        i++;
    }
    return a[i] - b[i];
}
int main() {
    printf("%d\n", my_strcmp("abc", "abc"));
    printf("%d\n", my_strcmp("abc", "abd"));
    printf("%d\n", my_strcmp("abd", "abc"));
    printf("%d\n", my_strcmp("ab", "abc"));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["0", "-1", "1", "-99"]);
}

#[test]
fn test_e2e_struct_array_avg() {
    let src = r#"
#include <stdio.h>
typedef struct {
    int score;
} Student;
int main() {
    Student s[4];
    s[0].score = 80;
    s[1].score = 90;
    s[2].score = 70;
    s[3].score = 100;
    int sum = 0;
    for (int i = 0; i < 4; i++) {
        sum += s[i].score;
    }
    float avg = (float)sum / 4;
    printf("%f\n", avg);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["85.000000"]);
}

#[test]
fn test_e2e_substring_find() {
    let src = r#"
#include <stdio.h>
int find_substring(char s[], char sub[]) {
    int i = 0;
    while (s[i] != 0) {
        int j = 0;
        while (sub[j] != 0 && s[i + j] == sub[j]) {
            j++;
        }
        if (sub[j] == 0) return i;
        i++;
    }
    return -1;
}
int main() {
    printf("%d\n", find_substring("hello world", "world"));
    printf("%d\n", find_substring("hello world", "lo"));
    printf("%d\n", find_substring("hello world", "xyz"));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["6", "3", "-1"]);
}

#[test]
fn test_e2e_array_rotate() {
    let src = r#"
#include <stdio.h>
void reverse(int arr[], int start, int end) {
    while (start < end) {
        int tmp = arr[start];
        arr[start] = arr[end];
        arr[end] = tmp;
        start++;
        end--;
    }
}
void rotate_right(int arr[], int n, int k) {
    k = k % n;
    reverse(arr, 0, n - 1);
    reverse(arr, 0, k - 1);
    reverse(arr, k, n - 1);
}
int main() {
    int arr[6] = {1, 2, 3, 4, 5, 6};
    rotate_right(arr, 6, 2);
    for (int i = 0; i < 6; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["5 6 1 2 3 4 "]);
}

// ============================================================================
// Medium-difficulty random tests (batch 7)
// ============================================================================

#[test]
fn test_e2e_find_mode() {
    let src = r#"
#include <stdio.h>
int find_mode(int arr[], int n) {
    int max_count = 0;
    int mode = arr[0];
    for (int i = 0; i < n; i++) {
        int count = 0;
        for (int j = 0; j < n; j++) {
            if (arr[j] == arr[i]) count++;
        }
        if (count > max_count) {
            max_count = count;
            mode = arr[i];
        }
    }
    return mode;
}
int main() {
    int a[8] = {1, 2, 3, 2, 4, 2, 5, 1};
    printf("%d\n", find_mode(a, 8));
    int b[5] = {7, 7, 8, 8, 8};
    printf("%d\n", find_mode(b, 5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["2", "8"]);
}

#[test]
fn test_e2e_array_queue() {
    let src = r#"
#include <stdio.h>
#define MAX 5
typedef struct {
    int data[MAX];
    int front;
    int rear;
    int count;
} Queue;

void init(Queue *q) {
    q->front = 0;
    q->rear = 0;
    q->count = 0;
}

int is_empty(Queue *q) {
    return q->count == 0;
}

int is_full(Queue *q) {
    return q->count == MAX;
}

void enqueue(Queue *q, int x) {
    q->data[q->rear] = x;
    q->rear = (q->rear + 1) % MAX;
    q->count++;
}

int dequeue(Queue *q) {
    int x = q->data[q->front];
    q->front = (q->front + 1) % MAX;
    q->count--;
    return x;
}

int main() {
    Queue *q = (Queue*)malloc(sizeof(Queue));
    init(q);
    enqueue(q, 10);
    enqueue(q, 20);
    enqueue(q, 30);
    printf("%d ", dequeue(q));
    printf("%d ", dequeue(q));
    enqueue(q, 40);
    enqueue(q, 50);
    enqueue(q, 60);
    printf("%d\n", is_full(q));
    while (!is_empty(q)) {
        printf("%d ", dequeue(q));
    }
    printf("\n");
    free(q);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["10 20 0", "30 40 50 60 "]);
}

#[test]
fn test_e2e_string_to_int() {
    let src = r#"
#include <stdio.h>
int my_atoi(char s[]) {
    int i = 0;
    int sign = 1;
    if (s[i] == '-') {
        sign = -1;
        i++;
    }
    int val = 0;
    while (s[i] != 0) {
        val = val * 10 + (s[i] - '0');
        i++;
    }
    return sign * val;
}
int main() {
    printf("%d\n", my_atoi("123"));
    printf("%d\n", my_atoi("-456"));
    printf("%d\n", my_atoi("0"));
    printf("%d\n", my_atoi("9876"));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["123", "-456", "0", "9876"]);
}

#[test]
fn test_e2e_gcd_lcm() {
    let src = r#"
#include <stdio.h>
int gcd(int a, int b) {
    while (b != 0) {
        int temp = b;
        b = a % b;
        a = temp;
    }
    return a;
}
int lcm(int a, int b) {
    return a * (b / gcd(a, b));
}
int main() {
    printf("%d\n", gcd(48, 18));
    printf("%d\n", gcd(7, 5));
    printf("%d\n", lcm(4, 6));
    printf("%d\n", lcm(21, 6));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["6", "1", "12", "42"]);
}

#[test]
fn test_e2e_bitmask_flags() {
    let src = r#"
#include <stdio.h>
#define FLAG_READ  1
#define FLAG_WRITE 2
#define FLAG_EXEC  4
int main() {
    int perm = FLAG_READ | FLAG_WRITE;
    printf("%d ", perm & FLAG_READ);
    printf("%d ", perm & FLAG_WRITE);
    printf("%d\n", perm & FLAG_EXEC);
    perm = perm | FLAG_EXEC;
    printf("%d ", perm & FLAG_EXEC);
    perm = perm & ~FLAG_WRITE;
    printf("%d ", perm & FLAG_WRITE);
    printf("%d\n", perm & FLAG_READ);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1 2 0", "4 0 1"]);
}

#[test]
fn test_e2e_struct_array_copy() {
    let src = r#"
#include <stdio.h>
typedef struct { int id; int score; } Student;
int main() {
    Student s[2];
    s[0].id = 1; s[0].score = 10;
    s[1].id = 2; s[1].score = 20;
    s[0] = s[1];
    printf("%d %d\n", s[0].id, s[0].score);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["2 20"]);
}

#[test]
fn test_e2e_struct_local_copy() {
    let src = r#"
#include <stdio.h>
typedef struct { int id; int score; } Student;
int main() {
    Student a;
    a.id = 1; a.score = 10;
    Student b;
    b = a;
    printf("%d %d\n", b.id, b.score);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out, vec!["1 10"]);
}

#[test]
fn test_e2e_printf_format_modifiers() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 42;
    float b = 3.14159;
    printf("Result: %6d\n", a);
    printf("Pi: %.2f\n", b);
    printf("Long: %ld\n", a);
    printf("Mixed: %6d %.2f %ld\n", a, b, a);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "Result:     42");
    assert_eq!(out[1], "Pi: 3.14");
    assert_eq!(out[2], "Long: 42");
    assert_eq!(out[3], "Mixed:     42 3.14 42");
}

#[test]
fn test_e2e_multi_array_var_decl_dims() {
    let src = r#"
#include <stdio.h>
int main() {
    int a[2], b[3];
    a[0] = 10; a[1] = 20;
    b[0] = 100; b[1] = 200; b[2] = 300;
    printf("%d %d %d %d %d\n", a[0], a[1], b[0], b[1], b[2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "10 20 100 200 300");
}

#[test]
fn test_e2e_realloc_new_block_copy() {
    // 2026-09-11 堆决议 §3：realloc 恒为新块拷贝，原「堆顶原地收缩」特例被移除
    // （原地收缩会把 heap_offset 回退进隔离区，破坏"隔离窗口内地址不复用"的保证）。
    // 原测试断言 old_addr == new_addr 即是在测分配器的复用行为本身——按决议 §4-3
    // 把该行为从契约中除名：改测"地址改变 + 原数据完整保留"（数据正确性不下沉）。
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(16);
    p[0] = 1; p[1] = 2; p[2] = 3; p[3] = 4;
    int old_addr = (int)p;
    p = (int*)realloc(p, 8);
    int new_addr = (int)p;
    printf("%d %d %d\n", old_addr == new_addr, p[0], p[1]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "0 1 2");
}

#[test]
fn test_e2e_function_pointer_basic() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int main() {
    int (*fp)(int, int) = add;
    printf("%d\n", fp(3, 4));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "7");
}

#[test]
fn test_e2e_function_pointer_as_arg() {
    let src = r#"
#include <stdio.h>
int apply(int (*op)(int), int x) { return op(x); }
int inc(int n) { return n + 1; }
int main() {
    printf("%d\n", apply(inc, 5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "6");
}

#[test]
fn test_e2e_function_pointer_reassign() {
    let src = r#"
#include <stdio.h>
int f1() { return 1; }
int f2() { return 2; }
int main() {
    int (*fp)() = f1;
    printf("%d\n", fp());
    fp = f2;
    printf("%d\n", fp());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "1");
    assert_eq!(out[1], "2");
}

// ============================================================================
// 函数指针高级测试（递归类型系统）
// ============================================================================

#[test]
fn test_e2e_function_pointer_array() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int main() {
    int (*fp[2])(int, int) = {add, sub};
    printf("%d %d\n", fp[0](3, 4), fp[1](7, 2));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "7 5");
}

#[test]
fn test_e2e_pointer_to_function_pointer() {
    let src = r#"
#include <stdio.h>
int greet(int x) { return x * 2; }
int main() {
    int (*fp)(int) = greet;
    int (**pp)(int) = &fp;
    printf("%d\n", (*pp)(5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "10");
}

#[test]
fn test_e2e_function_pointer_returning_pointer() {
    let src = r#"
#include <stdio.h>
int* make_arr() {
    static int arr[3] = {1, 2, 3};
    return arr;
}
int main() {
    int* (*fp)() = make_arr;
    int* p = fp();
    printf("%d %d %d\n", p[0], p[1], p[2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "1 2 3");
}

#[test]
fn test_e2e_sizeof_function_pointer_types() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("%d %d %d\n", sizeof(int (*)(int)), sizeof(int (**)(int)), sizeof(int *(*)(int)));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    // 所有函数指针/指向函数指针的指针都是 4 字节
    assert_eq!(out[0], "4 4 4");
}

#[test]
fn test_e2e_typedef_function_pointer_array() {
    let src = r#"
#include <stdio.h>
int mul(int a, int b) { return a * b; }
int divi(int a, int b) { return a / b; }
typedef int (*Op)(int, int);
int main() {
    Op ops[2] = {mul, divi};
    printf("%d %d\n", ops[0](3, 4), ops[1](8, 2));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "12 4");
}

#[test]
fn test_e2e_global_function_pointer_init() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int (*global_fp)(int, int) = add;
int main() {
    printf("%d\n", global_fp(3, 4));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "7");
}

#[test]
fn test_e2e_address_of_function() {
    let src = r#"
#include <stdio.h>
int double_val(int x) { return x * 2; }
int main() {
    int (*fp)(int) = &double_val;
    printf("%d\n", fp(5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "10");
}

#[test]
fn test_e2e_struct_member_function_pointer() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
struct Ops {
    int (*op)(int, int);
};
int main() {
    struct Ops ops;
    ops.op = add;
    printf("%d\n", ops.op(3, 4));
    ops.op = sub;
    printf("%d\n", ops.op(7, 2));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "7");
    assert_eq!(out[1], "5");
}

#[test]
fn test_e2e_function_pointer_double_arg() {
    let src = r#"
#include <stdio.h>
double apply_double(double (*op)(double), double x) { return op(x); }
double inc_double(double x) { return x + 1.5; }
int main() {
    double (*fp)(double) = inc_double;
    printf("%.1f\n", apply_double(fp, 2.5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "4.0");
}

#[test]
fn test_e2e_function_pointer_longlong_arg() {
    let src = r#"
#include <stdio.h>
long long apply_ll(long long (*op)(long long), long long x) { return op(x); }
long long inc_ll(long long x) { return x + 10000000000LL; }
int main() {
    long long (*fp)(long long) = inc_ll;
    printf("%lld\n", apply_ll(fp, 1LL));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    let out = outputs;
    assert_eq!(out[0], "10000000001");
}

#[test]
fn test_e2e_memory_leak_report() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(sizeof(int) * 4);
    p[0] = 1;
    printf("ok\n");
    return 0;
}
"#;
    let result = compile_and_run_detailed(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, _stdout, notes) = result.unwrap();
    assert_eq!(ret, 0);
    // E-P1-5：泄漏报告是引擎附注，读 note 通道（不再混在 stdout 里）。
    let joined = notes.join("\n");
    assert!(joined.contains("内存泄漏检测报告"), "应包含泄漏报告标题");
    assert!(joined.contains("未被 free"), "应提示未被 free");
}

#[test]
fn test_e2e_no_leak_when_freed() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(sizeof(int) * 4);
    p[0] = 1;
    printf("ok\n");
    free(p);
    return 0;
}
"#;
    let result = compile_and_run_detailed(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, _stdout, notes) = result.unwrap();
    assert_eq!(ret, 0);
    let joined = notes.join("\n");
    assert!(!joined.contains("内存泄漏检测报告"), "已 free 不应报告泄漏");
}

// ============================================================================
// Use-After-Free / Double-Free Detection
// ============================================================================

#[test]
fn test_e2e_use_after_free() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(sizeof(int));
    *p = 42;
    free(p);
    *p = 99;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected Use-After-Free trap, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("Use-After-Free") || err.contains("E3060") || err.contains("已释放"),
        "Error should mention Use-After-Free: {}",
        err
    );
}

#[test]
fn test_e2e_double_free() {
    let src = r#"
#include <stdlib.h>
int main() {
    int *p = (int*)malloc(4);
    free(p);
    free(p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected Double-Free trap, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("Double-Free") || err.contains("E3061") || err.contains("重复释放"),
        "Error should mention Double-Free: {}",
        err
    );
}

#[test]
fn test_e2e_use_after_free_alias() {
    let src = r#"
#include <stdlib.h>
int main() {
    int *a = (int*)malloc(4);
    int *b = a;
    free(a);
    *b = 1;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected Use-After-Free trap via alias, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(
        err.contains("Use-After-Free") || err.contains("E3060") || err.contains("已释放"),
        "Error should mention Use-After-Free: {}",
        err
    );
}

// ── Float/double 比较语义测试（E1 更替：epsilon 容差 → IEEE 精确）──
//
// E1（C23 锚定）将 double/float 比较从 epsilon 容差（1e-6）改为 IEEE 754 精确
// 语义：原容差使 `0.1 + 0.2 == 0.3` 判真，与 C 标准（Clang golden 实测为 0）
// 直接矛盾，而浮点精确性正是浮点教学的核心一课。IEEE 754 算术是确定性的，
// 与宿主编译器逐位一致，容差没有存在依据。以下断言与 Clang 行为逐项对齐。

#[test]
fn test_e2e_double_exact_equality() {
    let src = r#"
#include <stdio.h>
int main() {
    double a = 0.1;
    double b = 0.2;
    double c = a + b;
    printf("%d\n", c == 0.3);
    printf("%d\n", 0.5 == 0.5);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    // IEEE 精确：0.1+0.2 的位模式 != 0.3（Clang 实测一致）
    assert_eq!(output.join(""), "01", "0.1+0.2 == 0.3 为假（精确语义），0.5 == 0.5 为真");
}

#[test]
fn test_e2e_double_exact_inequality() {
    let src = r#"
#include <stdio.h>
int main() {
    double a = 0.1;
    double b = 0.2;
    double c = a + b;
    printf("%d\n", c != 0.3);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    assert_eq!(output.join(""), "1", "0.1 + 0.2 != 0.3 应为真（IEEE 精确语义）");
}

#[test]
fn test_e2e_double_exact_relational() {
    let src = r#"
#include <stdio.h>
int main() {
    double a = 0.1;
    double b = 0.2;
    double c = a + b;
    printf("%d", c <= 0.3);
    printf("%d", c >= 0.3);
    printf("%d", c > 0.3);
    printf("%d\n", c < 0.3);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    // c = 0.30000000000000004 > 0.3：<= 假、>= 真、> 真、< 假（与 Clang 一致）
    assert_eq!(output.join(""), "0110", "精确语义下 c 略大于 0.3");
}

#[test]
fn test_e2e_float_exact_equality() {
    let src = r#"
#include <stdio.h>
int main() {
    float x = 0.1234567;
    float y = 0.1234568;
    printf("%d\n", x == y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    // 两值相差约 6~7 个 f32 ULP，精确比较为假（与 Clang 一致）
    assert_eq!(output.join(""), "0", "相邻但不同的 float 精确比较为假");
}

#[test]
fn test_e2e_float_exact_inequality() {
    let src = r#"
#include <stdio.h>
int main() {
    float x = 0.1234567;
    float y = 0.1234568;
    printf("%d\n", x != y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    assert_eq!(output.join(""), "1", "相邻但不同的 float 精确不等为真");
}

#[test]
fn test_e2e_float_exact_relational() {
    let src = r#"
#include <stdio.h>
int main() {
    float x = 0.1234567;
    float y = 0.1234568;
    printf("%d", x <= y);
    printf("%d", x >= y);
    printf("%d", x > y);
    printf("%d\n", x < y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    assert_eq!(output.join(""), "1001", "x < y 精确成立");
}

#[test]
fn test_e2e_float_far_apart_still_different() {
    let src = r#"
#include <stdio.h>
int main() {
    float a = 1.0;
    float b = 2.0;
    printf("%d", a == b);
    printf("%d", a != b);
    printf("%d", a < b);
    printf("%d\n", a > b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    // diff = 1.0 > EPS_F32 => normal comparison
    assert_eq!(out.join(""), "0110", "Far apart floats should compare normally");
}

// ============================================================================
// 数据结构教材语法拓展测试（参数化宏、static局部变量、fgets/fputs）
// ============================================================================

#[test]
fn test_e2e_parametric_macro_max() {
    let src = r#"
#include <stdio.h>
#define MAX(a,b) ((a)>(b)?(a):(b))
int main() {
    int x = 5;
    int y = 10;
    printf("%d\n", MAX(x, y));
    printf("%d\n", MAX(3, 2));
    printf("%d\n", MAX(x + 1, y - 3));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "1037", "Parametric macro MAX should work");
}

#[test]
fn test_e2e_parametric_macro_swap() {
    let src = r#"
#include <stdio.h>
#define SWAP(t,a,b) { t temp=a; a=b; b=temp; }
int main() {
    int x = 1;
    int y = 2;
    SWAP(int, x, y)
    printf("%d %d\n", x, y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "2 1", "Parametric macro SWAP should work");
}

#[test]
fn test_e2e_parametric_macro_swap_semicolon() {
    let src = r#"
#include <stdio.h>
#define SWAP(t,a,b) { t temp=a; a=b; b=temp; }
int main() {
    int x = 1;
    int y = 2;
    if (x < y)
        SWAP(int, x, y);
    else
        x = 0;
    printf("%d %d\n", x, y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(
        out.join(""),
        "2 1",
        "Parametric macro SWAP with trailing semicolon in if/else should work"
    );
}

#[test]
fn test_e2e_parametric_macro_nested() {
    let src = r#"
#include <stdio.h>
#define MAX(a,b) ((a)>(b)?(a):(b))
#define MIN(a,b) ((a)<(b)?(a):(b))
int main() {
    int r = MAX(1, MIN(2, 3));
    printf("%d\n", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "2", "Nested parametric macros should work");
}

#[test]
fn test_e2e_static_local_var_counter() {
    let src = r#"
#include <stdio.h>
int count_calls() {
    static int count = 0;
    count++;
    return count;
}
int main() {
    printf("%d\n", count_calls());
    printf("%d\n", count_calls());
    printf("%d\n", count_calls());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "123", "Static local variable should persist across calls");
}

#[test]
fn test_e2e_static_local_var_init_only_once() {
    let src = r#"
#include <stdio.h>
int get_value() {
    static int v = 42;
    v = v + 1;
    return v;
}
int main() {
    printf("%d\n", get_value());
    printf("%d\n", get_value());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "4344", "Static local should initialize only once");
}

#[test]
fn test_e2e_static_local_var_array() {
    let src = r#"
#include <stdio.h>
int sum_arr() {
    static int arr[3] = {1, 2, 3};
    int s = 0;
    for (int i = 0; i < 3; i++) {
        s = s + arr[i];
        arr[i] = arr[i] + 1;
    }
    return s;
}
int main() {
    printf("%d\n", sum_arr());
    printf("%d\n", sum_arr());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "69", "Static local array should persist and mutate");
}

#[test]
fn test_e2e_fgets_fputs() {
    let src = r#"
#include <stdio.h>
int main() {
    FILE *fp = fopen("test.txt", "w");
    fputs("hello\n", fp);
    fputs("world\n", fp);
    fclose(fp);

    fp = fopen("test.txt", "r");
    char buf[20];
    fgets(buf, 20, fp);
    printf("%s", buf);
    fgets(buf, 20, fp);
    printf("%s", buf);
    fclose(fp);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert!(
        out.iter().any(|l| l.contains("hello")),
        "fgets should read first line: {:?}",
        out
    );
    assert!(
        out.iter().any(|l| l.contains("world")),
        "fgets should read second line: {:?}",
        out
    );
}

#[test]
fn test_e2e_fputs_stdout() {
    let src = r#"
#include <stdio.h>
int main() {
    fputs("hello ", stdout);
    fputs("world", stdout);
    fputs("\n", stdout);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "hello world", "fputs to stdout should produce output: {:?}", out);
}

#[test]
fn test_e2e_double_pointer_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 42;
    int *p = &x;
    int **pp = &p;
    printf("%d\n", **pp);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "42", "Double pointer dereference should print 42");
}

#[test]
fn test_e2e_double_pointer_struct() {
    let src = r#"
#include <stdio.h>
struct Node { int val; struct Node *next; };
void push_front(struct Node **head, int val) {
    struct Node *n = (struct Node*)malloc(sizeof(struct Node));
    n->val = val;
    n->next = *head;
    *head = n;
}
int main() {
    struct Node *head = 0;
    push_front(&head, 10);
    push_front(&head, 20);
    printf("%d\n", head->val);
    printf("%d\n", head->next->val);
    while (head) {
        struct Node *tmp = head;
        head = head->next;
        free(tmp);
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "2010", "struct Node** linked-list pattern should work");
}

#[test]
fn test_e2e_double_pointer_cast_and_index() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 1, b = 2;
    int *arr[2];
    arr[0] = &a;
    arr[1] = &b;
    int **pp = (int**)arr;
    printf("%d\n", *pp[0]);
    printf("%d\n", *pp[1]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "12", "Double pointer cast and index should work");
}

#[test]
fn test_e2e_double_pointer_arithmetic() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 10, y = 20;
    int *arr[2];
    arr[0] = &x;
    arr[1] = &y;
    int **pp = arr;
    printf("%d\n", **pp);
    pp = pp + 1;
    printf("%d\n", **pp);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "1020", "Double pointer arithmetic should step by 4 bytes");
}

#[test]
fn test_e2e_struct_return_by_value_basic() {
    let src = r#"
#include <stdio.h>
struct Point { int x; int y; };
struct Point make_point(int x, int y) {
    struct Point p;
    p.x = x;
    p.y = y;
    return p;
}
int main() {
    struct Point p = make_point(3, 4);
    printf("%d %d\n", p.x, p.y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "3 4", "Struct return by value should copy fields correctly");
}

#[test]
fn test_e2e_struct_return_by_value_member_access() {
    let src = r#"
#include <stdio.h>
struct Rect { int w; int h; };
struct Rect make_rect(int w, int h) {
    struct Rect r;
    r.w = w;
    r.h = h;
    return r;
}
int main() {
    printf("%d\n", make_rect(5, 6).w);
    printf("%d\n", make_rect(5, 6).h);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "56", "Direct member access on struct return should work");
}

#[test]
fn test_e2e_struct_return_by_value_as_arg() {
    let src = r#"
#include <stdio.h>
struct Vec { int x; int y; };
struct Vec make_vec(int x, int y) {
    struct Vec v;
    v.x = x;
    v.y = y;
    return v;
}
int area(struct Vec v) {
    return v.x * v.y;
}
int main() {
    printf("%d\n", area(make_vec(3, 4)));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "12", "Struct return used as function argument should work");
}

#[test]
fn test_e2e_vla_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 5;
    int a[n];
    a[0] = 10;
    a[1] = 20;
    a[4] = 50;
    printf("%d %d %d\n", a[0], a[1], a[4]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "10 20 50", "VLA basic access should work");
}

#[test]
fn test_e2e_vla_sizeof() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 5;
    int a[n];
    printf("%d\n", sizeof(a));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "20", "sizeof(VLA) should be n * sizeof(int) = 20");
}

#[test]
fn test_e2e_vla_in_loop() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 4;
    int a[n];
    for (int i = 0; i < n; i++) {
        a[i] = i * 10;
    }
    for (int i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "0 10 20 30 ", "VLA in loop should work");
}

#[test]
fn test_e2e_vla_pass_to_function() {
    let src = r#"
#include <stdio.h>
int sum(int a[], int n) {
    int s = 0;
    for (int i = 0; i < n; i++) {
        s += a[i];
    }
    return s;
}
int main() {
    int n = 5;
    int a[n];
    for (int i = 0; i < n; i++) {
        a[i] = i + 1;
    }
    printf("%d\n", sum(a, n));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "15", "VLA passed to function should decay to pointer");
}

#[test]
fn test_e2e_vla_multidim_first_vla() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 3;
    int a[n][3];
    for (int i = 0; i < n; i++) {
        for (int j = 0; j < 3; j++) {
            a[i][j] = i * 10 + j;
        }
    }
    printf("%d %d %d\n", a[0][0], a[1][2], a[2][1]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "0 12 21", "Multidim VLA with constant second dim should work");
}

#[test]
fn test_e2e_vla_subarray_sizeof() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 3;
    int a[n][3];
    printf("%d\n", sizeof(a[0]));
    return 0;
}
"#;
    let result = compile_and_run(src);
    println!("DEBUG subarray sizeof: {:?}", result);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "12", "sizeof(a[0]) for VLA subarray should be 12");
}

#[test]
fn test_e2e_vla_func_param() {
    let src = r#"
#include <stdio.h>
void fill(int n, int a[n]) {
    for (int i = 0; i < n; i++) {
        a[i] = i * 10;
    }
}
int main() {
    int n = 4;
    int a[n];
    fill(n, a);
    for (int i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "0 10 20 30 ", "VLA as function parameter should work");
}

#[test]
fn test_e2e_vla_multidim_all_vla() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 2;
    int m = 3;
    int a[n][m];
    for (int i = 0; i < n; i++) {
        for (int j = 0; j < m; j++) {
            a[i][j] = i * 10 + j;
        }
    }
    printf("%d %d %d %d\n", a[0][0], a[0][2], a[1][0], a[1][2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    println!("DEBUG all vla: {:?}", result);
    assert!(result.is_ok(), "Compile/run failed: {:?}", result);
    let (_, output) = result.unwrap();
    let out = output;
    assert_eq!(out.join(""), "0 2 10 12", "All-dim VLA should work");
}

#[test]
fn test_e2e_goto_forward_jump() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 0;
    goto end;
    x = 100;
end:
    printf("%d\n", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "0", "Forward goto should skip assignment");
}

#[test]
fn test_e2e_goto_backward_jump() {
    let src = r#"
#include <stdio.h>
int main() {
    int i = 0;
loop:
    if (i >= 3) goto end;
    printf("%d", i);
    i++;
    goto loop;
end:
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "012", "Backward goto should form loop");
}

#[test]
fn test_e2e_goto_undefined_label_error() {
    let src = r#"
#include <stdio.h>
int main() {
    goto nowhere;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Undefined goto label should produce compile error");
    let err = result.unwrap_err();
    assert!(
        err.contains("3071") || err.contains("未定义"),
        "Expected E3071 undefined label error, got: {}",
        err
    );
}

#[test]
fn test_e2e_conditional_ifdef_defined() {
    let src = r#"
#include <stdio.h>
#define MODE
#ifdef MODE
int get_val() { return 100; }
#else
int get_val() { return 200; }
#endif
int main() {
    printf("%d\n", get_val());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "100", "When MODE is defined, get_val() should return 100");
}

#[test]
fn test_e2e_conditional_ifdef_undefined() {
    let src = r#"
#include <stdio.h>
#ifdef MODE
int get_val() { return 100; }
#else
int get_val() { return 200; }
#endif
int main() {
    printf("%d\n", get_val());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "200", "When MODE is undefined, get_val() should return 200");
}

#[test]
fn test_e2e_conditional_header_guard() {
    let src = r#"
#include <stdio.h>
#ifndef MYHEADER_H
#define MYHEADER_H
int helper() { return 42; }
#endif
int main() {
    printf("%d\n", helper());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "42", "Header guard should allow helper() to be compiled");
}

#[test]
fn test_e2e_conditional_nested() {
    let src = r#"
#include <stdio.h>
#define OUTER
#ifdef OUTER
  #ifdef INNER
    int val = 1;
  #else
    int val = 2;
  #endif
#else
  int val = 3;
#endif
int main() {
    printf("%d\n", val);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "2", "When OUTER defined but INNER undefined, val should be 2");
}

#[test]
fn test_e2e_conditional_nested_both_defined() {
    let src = r#"
#include <stdio.h>
#define OUTER
#define INNER
#ifdef OUTER
  #ifdef INNER
    int val = 1;
  #else
    int val = 2;
  #endif
#else
  int val = 3;
#endif
int main() {
    printf("%d\n", val);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "1", "When both OUTER and INNER defined, val should be 1");
}

#[test]
fn test_e2e_conditional_ifndef() {
    let src = r#"
#include <stdio.h>
#ifndef RELEASE
int debug() { return 1; }
#else
int debug() { return 0; }
#endif
int main() {
    printf("%d\n", debug());
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "1", "When RELEASE is not defined, debug() should return 1");
}

#[test]
fn test_e2e_vla_sizeof_type() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 5;
    printf("%d\n", sizeof(int[n]));
    printf("%d\n", sizeof(int[n][3]));
    int m = 2;
    printf("%d\n", sizeof(int[n][m]));
    printf("%d\n", sizeof(double[n]));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(
        out.join(""),
        "20604040",
        "sizeof(VLA type) should compute runtime sizes correctly"
    );
}

#[test]
fn test_e2e_vla_sizeof_type_in_expr() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 5;
    int elem_count = sizeof(int[n]) / sizeof(int);
    printf("%d\n", elem_count);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    let out = outputs;
    assert_eq!(out.join(""), "5", "sizeof(int[n]) / sizeof(int) should equal n");
}

fn compile_and_run_raw_output(source: &str) -> Result<(i32, String), String> {
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

        // E-P1-5：纯程序 stdout（引擎附注走 note 通道），无需再截断后缀提示。
        let out_len = vitro_native::capi::vitro_get_program_output_length(session);
        let out_str = if out_len > 0 {
            let mut buf = vec![0u8; out_len as usize + 1];
            vitro_native::capi::vitro_get_program_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            String::from_utf8_lossy(&buf[..out_len as usize]).to_string()
        } else {
            String::new()
        };

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

        Ok((run_ret, out_str))
    }
}

#[test]
fn test_e2e_printf_no_auto_newline() {
    // 修复：printf 连续调用时不应自动添加换行符，行为需与 C 标准一致。
    let src = r#"
#include <stdio.h>
int main() {
    for (int i = 0; i < 3; i++) printf("%d ", i);
    return 0;
}
"#;
    let result = compile_and_run_raw_output(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, out) = result.unwrap();
    assert_eq!(out, "0 1 2 ", "printf calls should not automatically add newlines");
}

#[test]
fn test_e2e_engine_note_does_not_pollute_stdout() {
    // E-P1-5：程序自己打印"程序运行完成，返回值：N"与泄漏报告样式的文本时，必须原样
    // 保留在 stdout；引擎附注（完成提示、malloc(0) 教学警告）走 note 通道。
    // 旧实现用正则清洗 stdout，会把这些**真实输出**整段删掉（假阳性 output_gap）。
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    printf("程序运行完成，返回值：7\n");
    printf("===== 内存泄漏检测报告 =====\n");
    printf("==============================\n");
    void *p = malloc(0);
    (void)p;
    return 0;
}
"#;
    let (ret, stdout, notes) = compile_and_run_detailed(src).expect("compile+run");
    assert_eq!(ret, 0);
    let out = stdout.join("\n");
    let note = notes.join("\n");

    assert!(out.contains("程序运行完成，返回值：7"), "程序自身输出不得被清洗：{}", out);
    assert!(
        out.contains("===== 内存泄漏检测报告 ====="),
        "程序打印的报告样式文本不得被清洗：{}",
        out
    );
    assert_eq!(
        out.matches("==============================").count(),
        1,
        "30 等号分隔行必须保留：{}",
        out
    );

    assert!(note.contains("程序运行完成，返回值：0"), "引擎完成提示应在 note 通道：{}", note);
    assert!(note.contains("malloc(0)"), "malloc(0) 教学警告应在 note 通道：{}", note);
    assert!(!out.contains("malloc(0)"), "引擎附注不得混入 stdout：{}", out);
}

#[test]
fn test_e2e_main_args() {
    // 验证 main(int argc, char *argv[]) 能正确接收命令行参数。
    let src = r#"
#include <stdio.h>
int main(int argc, char *argv[]) {
    for (int i = 1; i < argc; i++) {
        printf("%s%s", argv[i], (i < argc - 1) ? " " : "");
    }
    printf("\n");
    return 0;
}
"#;
    let result = compile_and_run_with_argv(src, &["prog", "hello", "world"]);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l == "hello world"), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_main_no_args() {
    // 验证 main() 无参数时仍可正常运行（PushArgc/PushArgv 不应生成）。
    let src = r#"
#include <stdio.h>
int main() {
    printf("ok\n");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l == "ok"), "Outputs: {:?}", outputs);
}
