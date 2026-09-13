#![allow(clippy::unwrap_used, clippy::expect_used)]

//! C++ lambda 的两项能力缺口回归（2026-09-11 PR 清单条目 1 / 条目 2）。
//!
//! 1. **返回类型推断**：`__call` 签名此前在 `resolve_lambda`（typeck）与 Pass 4 生成的
//!    `FuncDecl` 中**都硬编码 `Type::int()`**，非 int 返回的 lambda 在调用点被当作 int
//!    （`printf("%.2f", d(1.5))` 触发 E3062 格式不匹配）。
//! 2. **文件作用域 lambda 变量**：全局 `auto gf = [](int x){ return x + 7; };` 此前报
//!    E3004（`declare_var` 登记的是替换前的 `auto`），随后调用点报 E3066。
//!
//! 对照输出均由 Clang++（`-std=c++14`）实测确认。

use cide_native::session::{CompileUnit, Session};
use cide_native::session_api;

/// 编译并运行一段 C++ 源码，返回 (返回值, 程序 stdout)。
fn run_cpp(source: &str) -> (i32, String) {
    let mut session = Session::default();
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.cpp".to_string(),
        source: source.to_string(),
    }];
    let compiled = session_api::compile(&mut session);
    assert!(
        compiled["ok"].as_bool().unwrap_or(false),
        "编译失败：{compiled}"
    );
    let result = session_api::run(&mut session);
    assert_eq!(result["status"], "finished", "程序应正常结束：{result}");
    (
        result["return_value"].as_i64().unwrap_or(-1) as i32,
        session.runtime.stdout(),
    )
}

#[test]
fn test_lambda_return_type_is_inferred() {
    // 条目 1：double 返回的 lambda 不得被当作 int（Clang++ 输出 d=3.00 / i=110）
    let src = r#"#include <stdio.h>
int main() {
    auto d = [](double x) { return x * 2.0; };
    printf("d=%.2f\n", d(1.5));
    auto i = [](int x) { return x + 100; };
    printf("i=%d\n", i(10));
    return 0;
}
"#;
    let (ret, out) = run_cpp(src);
    assert_eq!(ret, 0);
    // U0#1④：精确行断言（contains("d=3.00") 对错值 "d=3.001" 同样为真——
    // printf 已限定 %.2f，期望行就是 "d=3.00"，整行相等是可用最强断言）
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert!(lines.contains(&"d=3.00"), "double 返回的 lambda 应为 3.00，实际：{out:?}");
    assert!(lines.contains(&"i=110"), "int 返回的 lambda 应仍为 110，实际：{out:?}");
}

#[test]
fn test_file_scope_lambda_variable_is_callable() {
    // 条目 2：文件作用域 lambda 变量（Clang++ 输出 gg=8 6）
    let src = r#"#include <stdio.h>
auto gf = [](int x) { return x + 7; };
int main() {
    printf("gg=%d %d\n", gf(1), gf(-1));
    return 0;
}
"#;
    let (ret, out) = run_cpp(src);
    assert_eq!(ret, 0);
    assert!(out.contains("gg=8 6"), "文件作用域 lambda 应可调用，实际：{out:?}");
}

#[test]
fn test_local_lambda_with_capture_still_works() {
    // 既有能力的回归护栏：带捕获的局部 lambda（返回类型推断改动不得破坏它）
    let src = r#"#include <stdio.h>
int main() {
    int base = 5;
    auto add = [base](int x) { return x + base; };
    printf("r=%d\n", add(3));
    return 0;
}
"#;
    let (ret, out) = run_cpp(src);
    assert_eq!(ret, 0);
    assert!(out.contains("r=8"), "带捕获的局部 lambda 应仍工作，实际：{out:?}");
}
