#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::diagnostics::error_codes::ErrorCode;
use vitro_native::engine::compile_pipeline::run_multi_file_pipeline;
use vitro_native::session::{CompileUnit, Session};

fn make_session() -> Session {
    Session::default()
}

#[test]
fn test_multi_file_basic() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
#include <stdio.h>
int add(int a, int b);

int main() {
    int result = add(3, 4);
    printf("%d", result);
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
int add(int a, int b) {
    return a + b;
}
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(result.is_ok(), "多文件基本编译失败: {:?}", session.compile.errors);
}

#[test]
fn test_multi_file_static_func_isolation() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
#include <stdio.h>

int main() {
    int result = secret_add(3, 4);
    printf("%d", result);
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
static int secret_add(int a, int b) {
    return a + b;
}
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(result.is_err(), "static 函数跨文件调用应该编译失败");

    let has_static_error = session
        .compile
        .diagnostics
        .iter()
        .any(|d| d.error_code == ErrorCode::E3058_StaticFuncAccess as i32);
    assert!(has_static_error, "应报告 E3058 static 函数访问错误");
}

#[test]
fn test_same_file_static_func_ok() {
    let mut session = make_session();
    let units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: r#"
#include <stdio.h>

static int helper(int x) {
    return x * 2;
}

int main() {
    int result = helper(5);
    printf("%d", result);
    return 0;
}
"#
        .to_string(),
    }];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(result.is_ok(), "同一文件内 static 函数调用应成功: {:?}", session.compile.errors);
}

#[test]
fn test_multi_file_static_func_same_name() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
#include <stdio.h>

static int helper(int x) {
    return x + 1;
}

int main() {
    int a = helper(5);
    printf("%d", a);
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
static int helper(int x) {
    return x * 2;
}

int use_helper(int x) {
    return helper(x);
}
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(
        result.is_ok(),
        "同名 static 函数在不同文件应互不干扰: {:?}",
        session.compile.errors
    );
}

#[test]
fn test_diagnostic_filename() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
int main() {
    int x = unknown_var;
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(result.is_err());

    let diag = session.compile.diagnostics.first().unwrap();
    assert_eq!(diag.filename, "main.c", "诊断应指向 main.c");
}

#[test]
fn test_multi_file_static_global_isolation() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
#include <stdio.h>

extern int secret_val;

int main() {
    printf("%d", secret_val);
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
static int secret_val = 42;
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(result.is_err(), "static 全局变量跨文件访问应该编译失败");

    let has_static_error = session
        .compile
        .diagnostics
        .iter()
        .any(|d| d.error_code == ErrorCode::E3059_StaticGlobalAccess as i32);
    assert!(has_static_error, "应报告 E3059 static 全局变量访问错误");
}

#[test]
fn test_same_file_static_global_ok() {
    let mut session = make_session();
    let units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: r#"
#include <stdio.h>

static int counter = 0;

int next() {
    counter++;
    return counter;
}

int main() {
    printf("%d", next());
    return 0;
}
"#
        .to_string(),
    }];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(
        result.is_ok(),
        "同一文件内 static 全局变量访问应成功: {:?}",
        session.compile.errors
    );
}

#[test]
fn test_multi_file_static_global_same_name() {
    let mut session = make_session();
    let units = vec![
        CompileUnit {
            filename: "main.c".to_string(),
            source: r#"
#include <stdio.h>

static int val = 10;

int get_main_val() {
    return val;
}

int main() {
    printf("%d", get_main_val());
    return 0;
}
"#
            .to_string(),
        },
        CompileUnit {
            filename: "utils.c".to_string(),
            source: r#"
static int val = 20;

int get_utils_val() {
    return val;
}
"#
            .to_string(),
        },
    ];

    let result = run_multi_file_pipeline(&mut session, units, false);
    assert!(
        result.is_ok(),
        "同名 static 全局变量在不同文件应互不干扰: {:?}",
        session.compile.errors
    );
}
