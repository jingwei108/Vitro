#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Bytecode Libc 自举一致性测试（Phase B）
//!
//! 核心逻辑：把 Layer C（Bytecode Libc）的 C 源码同时交给：
//! 1. Clang：编译成原生可执行文件，输出作为唯一 golden；
//! 2. Vitro：编译成字节码，在 VM 中运行，输出与 golden 对比。
//!
//! 测试哲学：
//! - ALL_IN：所有 Bytecode Libc 的 C 源码必须参与验证。
//! - GOLDEN_FROM_CLANG：golden 只能来自 Clang，不能来自 Vitro 自己。
//! - NO_CODE_DISTORTION：Bytecode Libc 的 C 源码不得为了通过 Vitro 编译器而改写。

use vitro_native::engine::compile_pipeline::run_multi_file_pipeline;
use vitro_native::engine::session_ops::execute_run;
use vitro_native::session::{CompileUnit, Session};
use std::process::Command;

const BASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/bytecode_libc_consistency");
const RUNTIME_LIBC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/runtime_libc");

/// 从 Vitro session 中提取纯净的 stdout。
///
/// E-P1-5：引擎附注（"程序运行完成，返回值：N"、内存泄漏报告）已走 note 通道，
/// 这里直接读结构化 stdout 投影——**不再需要文本正则清洗**（旧实现会在程序自己
/// 打印同类文本时误删真实输出）。
fn extract_vitro_stdout(session: &Session) -> String {
    session.runtime.stdout()
}

/// 运行一个一致性测试用例。
///
/// * `driver_name` — 驱动文件名（如 "test_isdigit.c"）
/// * `lib_sources` — 库源码文件名列表（如 &["src/ctype.c"]
///
/// 注意：Vitro 路径已自动加载预编译的 Bytecode Libc，无需显式传入库源码；
///       Clang 路径仍需链接 runtime_libc C 源码以生成 golden。
fn run_consistency_case(driver_name: &str, lib_sources: &[&str]) {
    let driver_path = format!("{}/drivers/{}", BASE_DIR, driver_name);
    let driver_src =
        std::fs::read_to_string(&driver_path).unwrap_or_else(|e| panic!("无法读取驱动文件 {}: {}", driver_path, e));

    // ── 1. Clang 路径：生成 golden ──────────────────────────────────────
    let exe_name = format!("{}_{}_clang.exe", driver_name.replace('.', "_"), std::process::id());
    let exe_path = std::path::Path::new(BASE_DIR).join(&exe_name);
    let mut clang_args: Vec<String> = vec![
        "-std=c99".to_string(),
        "-O0".to_string(),
        "-fno-builtin".to_string(), // 禁用内建函数，避免与 Bytecode Libc 重名冲突
        "-Wall".to_string(),
        "-Werror".to_string(),
        driver_path.clone(),
    ];
    for lib in lib_sources {
        clang_args.push(format!("{}/{}", RUNTIME_LIBC_DIR, lib));
    }
    // 允许重复符号：Bytecode Libc 函数名可能与 CRT 冲突（如 atoi）
    if cfg!(windows) {
        clang_args.push("-Xlinker".to_string());
        clang_args.push("/FORCE:MULTIPLE".to_string());
    } else {
        clang_args.push("-Wl,--allow-multiple-definition".to_string());
    }
    clang_args.push("-o".to_string());
    clang_args.push(exe_path.to_string_lossy().to_string());

    let clang_out = Command::new("clang")
        .args(&clang_args)
        .output()
        .expect("无法执行 clang。请确保 clang 已安装并在 PATH 中。");

    if !clang_out.status.success() {
        let stderr = String::from_utf8_lossy(&clang_out.stderr);
        panic!("Clang 编译失败:\n{}", stderr);
    }

    let run_out = Command::new(&exe_path)
        .output()
        .unwrap_or_else(|e| panic!("无法运行 Clang 编译出的可执行文件 {}: {}", exe_path.display(), e));

    if !run_out.status.success() {
        panic!("Clang 编译的程序运行失败: {}", String::from_utf8_lossy(&run_out.stderr));
    }

    let golden_raw = String::from_utf8_lossy(&run_out.stdout);
    let golden = normalize_output(&golden_raw);

    // 清理临时可执行文件
    let _ = std::fs::remove_file(&exe_path);

    // ── 2. Vitro 路径：编译并运行 ────────────────────────────────────────
    // 对于已切换到 Bytecode 产品路径的函数（ctype + abs），setup_vm 会自动加载
    // 预编译的 Bytecode Libc；对于仍走 Host 路径的函数，需显式传入 runtime_libc 源码。
    let mut units: Vec<CompileUnit> = vec![CompileUnit {
        filename: driver_name.to_string(),
        source: driver_src,
    }];
    for lib in lib_sources {
        let lib_path = format!("{}/{}", RUNTIME_LIBC_DIR, lib);
        let lib_src =
            std::fs::read_to_string(&lib_path).unwrap_or_else(|e| panic!("无法读取库文件 {}: {}", lib_path, e));
        units.push(CompileUnit {
            filename: lib.to_string(),
            source: lib_src,
        });
    }

    let mut session = Session::default();
    let compile_result = run_multi_file_pipeline(&mut session, units, false);
    if let Err(e) = compile_result {
        let diags: Vec<String> = session
            .compile
            .diagnostics
            .iter()
            .map(|d| format!("{}:{}: {} (E{})", d.filename, d.line, d.message, d.error_code))
            .collect();
        panic!("Vitro 编译失败: {}\n诊断:\n{}", e, diags.join("\n"));
    }

    let run_result = execute_run(&mut session);
    if let Err(e) = run_result {
        panic!("Vitro 运行失败: {}", e);
    }

    let vitro_output_raw = extract_vitro_stdout(&session);
    let vitro_output = normalize_output(&vitro_output_raw);

    // ── 3. 对比 ──────────────────────────────────────────────────────────
    assert_eq!(
        vitro_output, golden,
        "Vitro 输出与 Clang golden 不一致!\n\n--- Vitro ---\n{:?}\n--- Clang ---\n{:?}",
        vitro_output, golden
    );
}

/// 规范化输出字符串，消除 \r\n vs \n 的差异，并去除尾部空白。
fn normalize_output(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end().to_string()
}

#[test]
fn test_bc_isdigit() {
    // ctype 函数已走 Bytecode Libc 产品路径，无需显式传入源码
    run_consistency_case("test_isdigit.c", &[]);
}

#[test]
fn test_bc_abs() {
    // abs 已走 Bytecode Libc 产品路径
    run_consistency_case("test_abs.c", &[]);
}

#[test]
fn test_bc_tolower() {
    run_consistency_case("test_tolower.c", &[]);
}

#[test]
fn test_bc_strlen() {
    // strlen 已走 Bytecode Libc 产品路径
    run_consistency_case("test_strlen.c", &[]);
}

#[test]
fn test_bc_strcmp() {
    // strcmp 已走 Bytecode Libc 产品路径
    run_consistency_case("test_strcmp.c", &[]);
}

#[test]
fn test_bc_strcpy() {
    run_consistency_case("test_strcpy.c", &["src/string.c"]);
}

#[test]
fn test_bc_strcat() {
    run_consistency_case("test_strcat.c", &["src/string.c"]);
}

#[test]
fn test_bc_strncpy() {
    run_consistency_case("test_strncpy.c", &["src/string.c"]);
}

#[test]
fn test_bc_memcpy() {
    run_consistency_case("test_memcpy.c", &["src/string.c"]);
}

#[test]
fn test_bc_memmove() {
    run_consistency_case("test_memmove.c", &["src/string.c"]);
}

#[test]
fn test_bc_ctype_extra() {
    run_consistency_case("test_ctype_extra.c", &[]);
}

#[test]
fn test_bc_rand() {
    run_consistency_case("test_rand.c", &["src/stdlib.c"]);
}
