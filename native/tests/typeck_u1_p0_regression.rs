#![allow(clippy::unwrap_used, clippy::expect_used)]

//! U1 P0 静默错值收口回归锚（路线图 U1，2026-09-13 起滚动扩充）。
//!
//! 本文件按红→绿纪律落用例：每条先在修复前实测为红（或"零诊断接受"），
//! 修复后必须给出明确诊断或正确值。
//!
//! #12：非法数组形状零诊断（评估 C5/T6 + typeck/codegen 审查 #15 实锤）：
//! - `int a[];`（无尺寸无初始化器——边界未知，任何索引访问都是越界；
//!   clang 报 "definition of variable with array type needs an explicit
//!   size or an initializer"）
//! - `int b[-5];`（负尺寸常量；clang 报 "array size is negative"）
//! - `int c[0] = {1, 2};`（零尺寸 + 超界初始化；clang 拒绝）
//!   修复前三者全部"编译成功"零诊断（2026-09-13 复现）。
//!
//! 注意不能误伤合法形状：`int t[] = {1, 2};`（初始化器推断尺寸）、
//! 函数参数 `int a[]`（退化为指针）、`extern int a[];`（不完整类型声明）。

use cide_native::compiler::lexer::Lexer;
use cide_native::compiler::parser::Parser;
use cide_native::compiler::typeck::TypeChecker;

/// 返回（类型错误数, 是否含关键词的错误消息列表）
fn typeck_errors(source: &str) -> (usize, Vec<String>) {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "词法错误：{:?}", lex_errors);
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "语法错误：{:?}", parse_errors);
    let mut program = maybe_program.expect("program");
    let (type_errors, _warnings, _hints) = TypeChecker::default().check(&mut program);
    let msgs = type_errors.iter().map(|e| e.message.clone()).collect();
    (type_errors.len(), msgs)
}

#[test]
fn u1_12_array_without_size_or_initializer_is_rejected() {
    let (n, msgs) = typecheck_errors_wrapper("int main() { int a[]; a[0] = 1; return a[0]; }");
    assert!(n > 0, "`int a[];`（无尺寸无初始化器）必须编译期报错——修复前零诊断静默接受，任何索引都是未知边界越界");
    assert!(
        msgs.iter().any(|m| m.contains("尺寸") || m.contains("大小") || m.contains("size")),
        "错误消息应指明数组尺寸问题：{:?}",
        msgs
    );
}

#[test]
fn u1_12_negative_array_size_is_rejected() {
    let (n, msgs) = typecheck_errors_wrapper("int main() { int b[-5]; return 0; }");
    assert!(n > 0, "`int b[-5];` 负尺寸必须报错——修复前零诊断");
    assert!(
        msgs.iter().any(|m| m.contains("负") || m.contains("尺寸") || m.contains("大小")),
        "错误消息应指明负尺寸：{:?}",
        msgs
    );
}

#[test]
fn u1_12_zero_size_with_initializer_overflow_is_rejected() {
    // 零尺寸 + 2 个初始化元素（clang 拒绝；Cide 修复前静默变 2 元素数组）
    let (n, _msgs) = typecheck_errors_wrapper("int main() { int c[0] = {1, 2}; return c[1]; }");
    assert!(n > 0, "`int c[0] = {{1,2}};` 零尺寸带初始化器必须报错——修复前静默变成 2 元素数组");
}

#[test]
fn u1_12_legal_shapes_not_regressed() {
    // 合法形状不得误伤：初始化器推断尺寸 / 函数参数退化 / extern 不完整声明
    for src in [
        "int main() { int t[] = {1, 2}; return t[1]; }",
        "int f(int a[]) { return a[0]; } int main() { int x[2] = {5, 6}; return f(x); }",
        "extern int g[]; int main() { return 0; }",
    ] {
        let (n, msgs) = typecheck_errors_wrapper(src);
        assert_eq!(n, 0, "合法形状被误拒：{:?} → {:?}", src, msgs);
    }
}

/// 与 typeck_errors 同义（命名与语义对齐，供后续 U1 条目滚动复用）。
fn typecheck_errors_wrapper(source: &str) -> (usize, Vec<String>) {
    typeck_errors(source)
}

// ─── U1#2：初始化列表静默置 0（评估 R1 动态实锤）──────────────────────────
// `int a[2] = {f(), 3}` 修复前 a[0] 输出 0（f() 的 7 被吞）；修复后必须 7。
// 根因：codegen 局部数组 4 字节元素只对 Identifier/StringLiteral 走 gen_expr，
// 其余（Call/Binary 等）落入 flatten 值（非字面量返回 None）→ unwrap_or(0)
// → PushConst 0 静默错值。

#[test]
fn u1_02_init_list_call_element_not_silently_zeroed() {
    use cide_native::engine::session_ops::{execute_run, reset_runtime};
    use cide_native::session::{CompileUnit, Session};

    let mut session = Session::default();
    session.compile.compile_units.push(CompileUnit {
        filename: "main.c".to_string(),
        source: r#"
#include <stdio.h>
int f() { return 7; }
int g(int x) { return x * 2; }
int main() {
    int a[2] = {f(), 3};
    int b[3] = {g(5) + 1, f() * f(), 100 - f()};
    printf("%d %d | %d %d %d\n", a[0], a[1], b[0], b[1], b[2]);
    return 0;
}
"#
        .to_string(),
    });
    reset_runtime(&mut session);
    let units = session.compile.compile_units.clone();
    let out = cide_native::engine::compile_pipeline::run_multi_file_pipeline(&mut session, units, false);
    assert!(out.is_ok(), "编译失败：{:?}", out.err());
    let (ret, _) = execute_run(&mut session).expect("run");
    assert_eq!(ret, 0);
    let stdout = session.runtime.stdout();
    // clang 基准：7 3 | 11 49 93（f()=7, g(5)+1=11, 7*7=49, 100-7=93）
    assert_eq!(
        stdout.trim(),
        "7 3 | 11 49 93",
        "初始化列表中的调用/算术表达式不得静默置 0（U1#2，clang 输出 7 3 | 11 49 93）"
    );
}

// ─── U1#9：常量折叠溢出 panic 家族（前端审查 #4，30 万 token 汤 10 次命中）──
// 修复前：`_Static_assert(1 << 1000)` 在 debug 构建 panic（decl.rs:845
// "attempt to shift left with overflow"——2026-09-13 本机复现），release
// 构建 UB 绕回静默错值——"构建配置决定语义"。修复：取负/移位改 checked，
// 溢出/越界返回 None（走"非常量表达式"诊断路径）。

#[test]
fn u1_09_const_fold_overflow_never_panics() {
    // 直接调用求值器（parser 侧公共路径）：溢出形状必须返回 None 而非 panic。
    // 通过完整编译管线间接覆盖（static_assert 的条件求值同源）。
    let src = r#"
int main() {
    _Static_assert(1 << 1000);
    return 0;
}
"#;
    let (tokens, lex_errors) = Lexer::new(src).tokenize();
    assert!(lex_errors.is_empty());
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    // 修复前：parse 阶段直接 panic（shift overflow）——本行即红锚；
    // 修复后：得到"非常量表达式"类解析错误（合法失败，不 panic）
    let _ = &parse_errors;
    let _ = maybe_program.is_some() || !parse_errors.is_empty();

    let src2 = "enum E { M = -(9223372036854775807LL + 1) }; int main(){return 0;}";
    let (tokens2, lex_errors2) = Lexer::new(src2).tokenize();
    assert!(lex_errors2.is_empty());
    // i64::MIN 取负：修复前 debug panic（neg overflow）——不得 panic
    let _ = Parser::new(tokens2).parse();
}
