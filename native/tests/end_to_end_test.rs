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

        // Collect output
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

        // Check runtime error
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
fn test_e2e_hello_world() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("Hello, World!");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("Hello, World!")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_arithmetic_return() {
    let src = r#"
int main() {
    int a = 10;
    int b = 20;
    int c = a + b * 2;
    return c;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("50")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_if_else() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 5;
    if (x > 3) {
        printf("big");
    } else {
        printf("small");
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("big")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_for_loop_sum() {
    let src = r#"
#include <stdio.h>
int main() {
    int sum = 0;
    for (int i = 0; i < 5; i = i + 1) {
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
    assert!(outputs.iter().any(|l| l.contains("10")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_array_index() {
    let src = r#"
int main() {
    int arr[3];
    arr[0] = 1;
    arr[1] = 2;
    arr[2] = 3;
    return arr[1];
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("2")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_multidim_array() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[2][3];
    arr[1][2] = 42;
    printf("%d", arr[1][2]);
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
fn test_e2e_multidim_array_zero_init() {
    let src = r#"
#include <stdio.h>
int main() {
    int a[2][3] = {0};
    a[1][2] = 42;
    printf("%d %d %d\n", a[0][0], a[1][0], a[1][2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("0 0 42")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_multidim_array_flat_init() {
    let src = r#"
#include <stdio.h>
int main() {
    int a[2][3] = {1, 2, 3, 4, 5, 6};
    printf("%d %d %d %d %d %d\n", a[0][0], a[0][1], a[0][2], a[1][0], a[1][1], a[1][2]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1 2 3 4 5 6")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_struct_member() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
};
int main() {
    struct Point p;
    p.x = 3;
    p.y = 4;
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

#[test]
fn test_e2e_scanf_printf() {
    let src = r#"
#include <stdio.h>
int main() {
    int a;
    scanf("%d", &a);
    printf("got:%d", a);
    return 0;
}
"#;
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        let c_src = CString::new(src).unwrap();
        let compile_ret = vitro_native::capi::vitro_compile(session, c_src.as_ptr() as *const c_char);
        assert_eq!(compile_ret, 0);

        let input = CString::new("42").unwrap();
        vitro_native::capi::vitro_set_input(session, input.as_ptr() as *const c_char);

        let run_ret = vitro_native::capi::vitro_run(session);
        assert_eq!(run_ret, 0);

        let out_len = vitro_native::capi::vitro_get_output_length(session);
        let mut buf = vec![0u8; out_len as usize + 1];
        vitro_native::capi::vitro_get_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
        let out_str = String::from_utf8_lossy(&buf);
        assert!(out_str.contains("got:42"), "Output: {}", out_str);

        vitro_native::capi::vitro_session_destroy(session);
    }
}

#[test]
fn test_e2e_malloc_free() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int *p = malloc(sizeof(int));
    *p = 123;
    printf("%d", *p);
    free(p);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("123")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_switch_case() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 2;
    switch (x) {
        case 1: printf("one"); break;
        case 2: printf("two"); break;
        default: printf("other"); break;
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("two")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_define_macro() {
    let src = r#"
#include <stdio.h>
#define N 100
int main() {
    printf("%d", N);
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
fn test_e2e_pointer_deref() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 42;
    int *p = &x;
    printf("%d", *p);
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
fn test_e2e_compound_assign() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 10;
    a += 5;
    a -= 3;
    a *= 2;
    a /= 4;
    printf("%d", a);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    // (10 + 5 - 3) * 2 / 4 = 12 * 2 / 4 = 24 / 4 = 6
    assert!(outputs.iter().any(|l| l.contains('6')), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_array_bounds_trap() {
    let src = r#"
int main() {
    int arr[3];
    arr[0] = 1;
    arr[5] = 99;
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_err(), "Expected runtime bounds error, got: {:?}", result);
    let err = result.unwrap_err();
    assert!(err.contains("数组越界") || err.contains("bounds"), "Error: {}", err);
}

#[test]
fn test_e2e_while_loop() {
    let src = r#"
#include <stdio.h>
int main() {
    int i = 0;
    int sum = 0;
    while (i < 5) {
        sum = sum + i;
        i = i + 1;
    }
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
fn test_e2e_function_call() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) {
    return a + b;
}
int main() {
    int r = add(3, 4);
    printf("%d", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("7")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_recursive_factorial() {
    let src = r#"
#include <stdio.h>
int fact(int n) {
    if (n <= 1) return 1;
    return n * fact(n - 1);
}
int main() {
    printf("%d", fact(5));
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("120")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_interactive_scanf() {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());

        let src = CString::new(
            r#"
#include <stdio.h>
int main() {
    int a, b;
    scanf("%d %d", &a, &b);
    printf("sum = %d", a + b);
    return 0;
}
"#,
        )
        .unwrap();

        let compile_ret = vitro_native::capi::vitro_compile(session, src.as_ptr() as *const c_char);
        assert_eq!(compile_ret, 0);

        // 第一次运行：没有输入，应该等待输入
        let run_ret = vitro_native::capi::vitro_run(session);
        assert_eq!(run_ret, 2, "Expected waiting input (2), got {}", run_ret);

        let waiting = vitro_native::capi::vitro_is_waiting_input(session);
        assert_eq!(waiting, 1, "Expected waiting_input = 1");

        // 提供输入
        let line = CString::new("3 4").unwrap();
        let provide_ret = vitro_native::capi::vitro_provide_input_line(session, line.as_ptr() as *const c_char);
        assert_eq!(provide_ret, 0);

        // 再次运行：应该完成
        let run_ret2 = vitro_native::capi::vitro_run(session);
        assert_eq!(run_ret2, 0, "Expected success (0), got {}", run_ret2);

        let out_len = vitro_native::capi::vitro_get_output_length(session);
        assert!(out_len > 0);
        let mut buf = vec![0u8; out_len as usize + 1];
        vitro_native::capi::vitro_get_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
        let out_str = String::from_utf8_lossy(&buf);
        assert!(out_str.contains("sum = 7"), "Output: {}", out_str);

        vitro_native::capi::vitro_session_destroy(session);
    }
}

#[test]
fn test_e2e_interactive_getchar() {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());

        let src = CString::new(
            r#"
#include <stdio.h>
int main() {
    char c = getchar();
    printf("char = %c", c);
    return 0;
}
"#,
        )
        .unwrap();

        let compile_ret = vitro_native::capi::vitro_compile(session, src.as_ptr() as *const c_char);
        assert_eq!(compile_ret, 0);

        // 没有输入，应该等待
        let run_ret = vitro_native::capi::vitro_run(session);
        assert_eq!(run_ret, 2);

        // 提供输入
        let line = CString::new("X").unwrap();
        vitro_native::capi::vitro_provide_input_line(session, line.as_ptr() as *const c_char);

        let run_ret2 = vitro_native::capi::vitro_run(session);
        assert_eq!(run_ret2, 0);

        let out_len = vitro_native::capi::vitro_get_output_length(session);
        assert!(out_len > 0);
        let mut buf = vec![0u8; out_len as usize + 1];
        vitro_native::capi::vitro_get_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
        let out_str = String::from_utf8_lossy(&buf);
        assert!(out_str.contains("char = X"), "Output: {}", out_str);

        vitro_native::capi::vitro_session_destroy(session);
    }
}

// ========== VFS File I/O Tests ==========

#[test]
fn test_e2e_file_fopen() {
    let src = r#"
#include <stdio.h>
int main() {
    FILE* f = fopen("test.txt", "r");
    if (f) {
        printf("ok");
    } else {
        printf("fail");
    }
    fclose(f);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    assert!(outputs.iter().any(|l| l.contains("ok")), "outputs: {:?}", outputs);
}

#[test]
fn test_e2e_file_fread() {
    let src = r#"
#include <stdio.h>
int main() {
    FILE* f = fopen("test.txt", "r");
    char buf[20];
    int n = fread(buf, 1, 5, f);
    buf[5] = 0;
    printf("%s", buf);
    fclose(f);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    assert!(outputs.iter().any(|l| l.contains("hello")), "outputs: {:?}", outputs);
}

#[test]
fn test_e2e_file_fwrite() {
    let src = r#"
#include <stdio.h>
int main() {
    FILE* f = fopen("out.txt", "w");
    fwrite("hello", 1, 5, f);
    fclose(f);
    printf("ok");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    assert!(outputs.iter().any(|l| l.contains("ok")), "outputs: {:?}", outputs);
}

#[test]
fn test_e2e_file_not_found() {
    let src = r#"
#include <stdio.h>
int main() {
    FILE* f = fopen("noexist.txt", "r");
    if (f == NULL) {
        printf("null");
    } else {
        printf("fail");
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (_, outputs) = result.unwrap();
    assert!(outputs.iter().any(|l| l.contains("null")), "outputs: {:?}", outputs);
}

#[test]
fn test_e2e_anonymous_struct_var_decl_global() {
    let src = r#"
#include <stdio.h>
struct {
    int x;
    int y;
} p;
int main() {
    p.x = 3;
    p.y = 4;
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

#[test]
fn test_e2e_anonymous_struct_var_decl_local() {
    let src = r#"
#include <stdio.h>
int main() {
    struct {
        int a;
        char b;
    } s;
    s.a = 10;
    s.b = 5;
    printf("%d", s.a + s.b);
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
fn test_e2e_anonymous_struct_var_decl_with_init() {
    let src = r#"
#include <stdio.h>
int main() {
    struct {
        int x;
        int y;
    } p = { 7, 8 };
    printf("%d", p.x + p.y);
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
fn test_e2e_sizeof_string_literal() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("%d", sizeof("hello"));
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
fn test_e2e_global_char_array_string_init() {
    let src = r#"
#include <stdio.h>
char s[6] = "hello";
int main() {
    printf("%d", strlen(s));
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
fn test_e2e_for_loop_scope_isolation() {
    let src = r#"
#include <stdio.h>
int main() {
    int i = 100;
    for (int i = 0; i < 3; i++) {
        printf("%d ", i);
    }
    printf("%d", i);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("0 1 2 100")), "Outputs: {:?}", outputs);
}

#[test]
fn test_printf_one_arg_in_main() {
    let src = r#"
#include <stdio.h>
int main() {
    int r = 10;
    printf("%d", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("10")), "Outputs: {:?}", outputs);
}

// A1: Array parameter decay — sizeof(arr_param) should return pointer size (4)
#[test]
fn test_e2e_array_param_decay() {
    let src = r#"
#include <stdio.h>
void f(int a[5]) {
    printf("%d", sizeof(a));
}
int main() {
    int arr[5];
    f(arr);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("4")), "Outputs: {:?}", outputs);
}

// A1: Multidimensional array parameter decay — int a[3][3] decays to int (*a)[3]
#[test]
fn test_e2e_md_array_param_decay() {
    let src = r#"
#include <stdio.h>
void f(int a[3][3]) {
    printf("%d %d", sizeof(a), sizeof(a[0]));
}
int main() {
    int arr[3][3];
    f(arr);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    // sizeof(a) == sizeof(int*) == 4; sizeof(a[0]) == sizeof(int[3]) == 12
    assert!(outputs.iter().any(|l| l.contains("4 12")), "Outputs: {:?}", outputs);
}

// A3: Struct array nested initialization (local)
#[test]
fn test_e2e_struct_array_nested_init_local() {
    let src = r#"
#include <stdio.h>
struct S { int x; int y; };
int main() {
    struct S arr[] = {{1, 2}, {3, 4}};
    printf("%d %d %d %d", arr[0].x, arr[0].y, arr[1].x, arr[1].y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1 2 3 4")), "Outputs: {:?}", outputs);
}

// A3: Struct array nested initialization (global)
#[test]
fn test_e2e_struct_array_nested_init_global() {
    let src = r#"
#include <stdio.h>
struct S { int x; int y; };
struct S arr[] = {{5, 6}, {7, 8}};
int main() {
    printf("%d %d %d %d", arr[0].x, arr[0].y, arr[1].x, arr[1].y);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("5 6 7 8")), "Outputs: {:?}", outputs);
}

// A3: Nested struct initialization
#[test]
fn test_e2e_nested_struct_init() {
    let src = r#"
#include <stdio.h>
struct Inner { int a; int b; };
struct Outer { struct Inner i; int c; };
int main() {
    struct Outer o = {{9, 10}, 11};
    printf("%d %d %d", o.i.a, o.i.b, o.c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("9 10 11")), "Outputs: {:?}", outputs);
}

// A2: unsigned comparison wrap-around
#[test]
fn test_e2e_unsigned_comparison() {
    let src = r#"
#include <stdio.h>
int main() {
    unsigned int a = 0;
    a -= 1;
    if (a > 1) printf("yes");
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("yes")), "Outputs: {:?}", outputs);
}

// A2: unsigned logical right shift
#[test]
fn test_e2e_unsigned_lshr() {
    let src = r#"
#include <stdio.h>
int main() {
    unsigned int x = -1;
    x = x >> 1;
    printf("%u", x);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("2147483647")), "Outputs: {:?}", outputs);
}

// A2: unsigned division and modulo
#[test]
fn test_e2e_unsigned_div_mod() {
    let src = r#"
#include <stdio.h>
int main() {
    unsigned int a = 7;
    unsigned int b = 3;
    printf("%u %u", a / b, a % b);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("2 1")), "Outputs: {:?}", outputs);
}

// B3: extern declaration support
#[test]
fn test_e2e_extern_decl() {
    let src = r#"
#include <stdio.h>
extern int global_var;
int global_var = 42;
int main() {
    printf("%d", global_var);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("42")), "Outputs: {:?}", outputs);
}

// P0: comma operator
#[test]
fn test_e2e_comma_operator_basic() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = (1, 2, 3);
    printf("%d", a);
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
fn test_e2e_comma_operator_side_effects() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 0, y = 0;
    int r = (x = 1, y = 2, x + y);
    printf("%d %d %d", x, y, r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1 2 3")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_comma_operator_in_while() {
    let src = r#"
#include <stdio.h>
int main() {
    int a = 5;
    while (a--, a > 0) {
        printf("%d ", a);
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("4 3 2 1")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_comma_operator_in_for() {
    let src = r#"
#include <stdio.h>
int main() {
    int i, j;
    for (i = 0, j = 10; i < j; i++, j--) {
        printf("%d%d ", i, j);
    }
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("010 19 28 37 46 ")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_comma_operator_func_args_not_affected() {
    let src = r#"
#include <stdio.h>
int add(int a, int b) { return a + b; }
int main() {
    int r = add(1, 2);
    printf("%d", r);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("3")), "Outputs: {:?}", outputs);
}

// P0: Designated Initializer
#[test]
fn test_e2e_designated_struct_init() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
    int z;
};
int main() {
    struct Point p = {.y = 20, .x = 10};
    printf("%d %d %d", p.x, p.y, p.z);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("10 20 0")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_designated_array_init() {
    let src = r#"
#include <stdio.h>
int main() {
    int a[5] = {[0] = 1, [3] = 4};
    printf("%d %d %d %d %d", a[0], a[1], a[2], a[3], a[4]);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1 0 0 4 0")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_designated_struct_nested_field() {
    let src = r#"
#include <stdio.h>
struct S {
    int a;
    int b;
};
struct T {
    struct S s;
    int c;
};
int main() {
    struct T t = {.s = {1, 2}, .c = 3};
    printf("%d %d %d", t.s.a, t.s.b, t.c);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("1 2 3")), "Outputs: {:?}", outputs);
}

// P0: offsetof
#[test]
fn test_e2e_offsetof_struct() {
    let src = r#"
#include <stdio.h>
struct Point {
    int x;
    int y;
    int z;
};
int main() {
    int ox = offsetof(struct Point, x);
    int oy = offsetof(struct Point, y);
    int oz = offsetof(struct Point, z);
    printf("%d %d %d", ox, oy, oz);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("0 4 8")), "Outputs: {:?}", outputs);
}

#[test]
fn test_e2e_offsetof_union() {
    let src = r#"
#include <stdio.h>
union U {
    int a;
    char b;
    double c;
};
int main() {
    int oa = offsetof(union U, a);
    int ob = offsetof(union U, b);
    int oc = offsetof(union U, c);
    printf("%d %d %d", oa, ob, oc);
    return 0;
}
"#;
    let result = compile_and_run(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let (ret, outputs) = result.unwrap();
    assert_eq!(ret, 0);
    assert!(outputs.iter().any(|l| l.contains("0 0 0")), "Outputs: {:?}", outputs);
}
