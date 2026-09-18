#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};
use std::path::Path;

use vitro_native::session::InputMode;

#[allow(dead_code)]
fn compile_and_run(source: &str, input: Option<&str>, input_mode: InputMode) -> Result<(i32, Vec<String>), String> {
    compile_and_run_with_filename(source, input, input_mode, "main.c")
}

#[allow(dead_code)]
fn compile_and_run_cpp(source: &str, input: Option<&str>, input_mode: InputMode) -> Result<(i32, Vec<String>), String> {
    compile_and_run_with_filename(source, input, input_mode, "main.cpp")
}

fn compile_and_run_with_filename(
    source: &str,
    input: Option<&str>,
    input_mode: InputMode,
    filename: &str,
) -> Result<(i32, Vec<String>), String> {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        if session.is_null() {
            return Err("Failed to create session".to_string());
        }

        (*session).runtime.input_mode = input_mode;

        // 设置输入数据（如果提供）
        if let Some(input_str) = input {
            // 保留换行符以模拟真实 stdin 字节流（getchar 需要读到 \n）
            let normalized = input_str.replace("\r\n", "\n");
            (*session).runtime.input_lines = normalized.split_inclusive('\n').map(|l| l.to_string()).collect();
        }

        let fname = CString::new(filename).map_err(|e| e.to_string())?;
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

        // E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道），不再做文本清洗。
        // 逐行 trim + 丢空行的归一化与 load_golden 的规则对称（两侧口径一致）。
        let mut outputs = Vec::new();
        let out_len = vitro_native::capi::vitro_get_program_output_length(session);
        if out_len > 0 {
            let mut buf = vec![0u8; out_len as usize + 1];
            vitro_native::capi::vitro_get_program_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            let out_str = String::from_utf8_lossy(&buf[..out_len as usize]);
            for line in out_str.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    outputs.push(trimmed.to_string());
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

// E-P1-5：`filter_vitro_diagnostics`（行级状态机清洗"程序运行完成"与泄漏报告）已删除。
//
// 旧的清洗规则有三处后果性差异（与 Python 驱动的全局正则不一致）：行内截断 vs 整段删除、
// 哨兵 `==30` vs `>=30` 个等号、丢弃空行 vs 仅 strip 首尾。根因是引擎把程序 stdout 与
// 引擎附注写进同一条字节流，现已按通道分离（stdout / stderr / note），
// 本防线直接读 `vitro_get_program_output*`，口径与 Python 驱动完全一致。

fn load_cases(dir: &Path) -> Vec<(String, String, Option<String>)> {
    load_cases_with_ext(dir, "c")
}

fn load_cpp_cases(dir: &Path) -> Vec<(String, String, Option<String>)> {
    load_cases_with_ext(dir, "cpp")
}

fn load_cases_with_ext(dir: &Path, ext: &str) -> Vec<(String, String, Option<String>)> {
    let mut cases = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some(ext) {
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                let input_path = path.with_extension("in");
                let input = if input_path.exists() {
                    Some(std::fs::read_to_string(&input_path).unwrap_or_default())
                } else {
                    None
                };
                cases.push((name, source, input));
            }
        }
    }
    cases.sort_by(|a, b| a.0.cmp(&b.0));
    cases
}

fn load_golden(case_name: &str, subdir: &str) -> Option<Vec<String>> {
    let path = Path::new("tests/cases_golden").join(subdir).join(format!("{}.out", case_name));
    let content = std::fs::read_to_string(&path).ok()?;
    let lines: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Some(lines)
}

fn run_case(
    name: &str,
    source: &str,
    input: Option<&str>,
    golden_subdir: &str,
    input_mode: InputMode,
) -> Result<(), String> {
    run_case_with_compiler(name, source, input, golden_subdir, input_mode, false)
}

fn run_cpp_case(
    name: &str,
    source: &str,
    input: Option<&str>,
    golden_subdir: &str,
    input_mode: InputMode,
) -> Result<(), String> {
    run_case_with_compiler(name, source, input, golden_subdir, input_mode, true)
}

fn run_case_with_compiler(
    name: &str,
    source: &str,
    input: Option<&str>,
    golden_subdir: &str,
    input_mode: InputMode,
    is_cpp: bool,
) -> Result<(), String> {
    // 使用真实源文件路径作为编译单元文件名，使 #include "..." 能基于同一目录解析自定义头文件。
    // 模板生成用例 golden_subdir 为空，保持默认 main.c/main.cpp 行为。
    let filename = if golden_subdir.is_empty() {
        if is_cpp {
            "main.cpp".to_string()
        } else {
            "main.c".to_string()
        }
    } else {
        format!("tests/cases/{}/{}.{}", golden_subdir, name, if is_cpp { "cpp" } else { "c" })
    };
    let result = compile_and_run_with_filename(source, input, input_mode, &filename);
    match result {
        Ok((ret, outputs)) => {
            if ret != 0 {
                return Err(format!("Exit code {} != 0", ret));
            }
            // E-P1-5：输出已是纯程序 stdout（引擎附注走 note 通道），无需清洗。
            let filtered = outputs;
            if let Some(golden) = load_golden(name, golden_subdir) {
                if filtered != golden {
                    return Err(format!(
                        "Output mismatch.\nExpected ({} lines): {:?}\nActual ({} lines): {:?}",
                        golden.len(),
                        golden,
                        filtered.len(),
                        filtered
                    ));
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[test]
fn test_vitro_e2e_baseline() {
    // E2：故意双侧编译失败的用例（Shadow 判 match：环检测 vs Clang 无限嵌套
    // 包含错误），e2e 的"必须可运行"契约不适用。
    // U1#11 新增三例（H-1/H-3）：include 找不到 / <> 引自定义头——Vitro 修复
    // 后报 E1021 编译失败，Clang 同样 fatal error（stdout 均空，Shadow 判 match）。
    // P1 新增 j1：声明符链式后缀 1300 层——Vitro 报 E1006（MAX_DECLARATOR_SUFFIX
    // 止血，修复前 release 栈溢出零诊断崩溃），Clang 同样编译失败
    // （bracket nesting depth 限制）→ Shadow 判 match。
    const KNOWN_BASELINE_COMPILE_FAILURES: &[&str] = &[
        "e2_include_cycle",
        "e3_static_assert_fail",
        "e2_include_not_found_quote",
        "e2_include_not_found_angle",
        "e2_angle_local_header",
        "j1_declarator_depth",
    ];

    let cases = load_cases(Path::new("tests/cases/baseline"));
    let known_compile_fail: std::collections::HashSet<&str> =
        KNOWN_BASELINE_COMPILE_FAILURES.iter().copied().collect();
    let mut failures = Vec::new();
    for (name, source, input) in &cases {
        if known_compile_fail.contains(name.as_str()) {
            continue;
        }
        // 带 `.in` 的用例 = "预设完整输入"，按批量语义执行（与 Shadow 防线口径统一）。
        // 交互模式下输入耗尽会挂起 `waiting_input`（学生等着键入），而 E2E 没有"再喂
        // 一行"的通道：`scanf_eof_loop` 这类**故意读到流末**的用例会以 run_ret=2 假失败。
        // 无 `.in` 的用例保持交互模式，覆盖交互路径。
        let mode = if input.is_some() {
            InputMode::Batch
        } else {
            InputMode::Interactive
        };
        if let Err(e) = run_case(name, source, input.as_deref(), "baseline", mode) {
            failures.push(format!("{}: {}", name, e));
        }
    }
    if !failures.is_empty() {
        panic!(
            "Baseline e2e failures ({} of {}):\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n")
        );
    }
}

/// Known template failures documented in E2E_FAILURES.md.
/// These are NOT silently ignored — they are monitored by
/// `test_vitro_e2e_template_known_failures` below.
// infixEvaluation_default 已修复 (2026-09-06 第三批 codegen 修复：自增/自减
// 作为数组索引的代码生成缺陷，如 opStack[++opTop]；输出与 Clang golden 一致)
// spfa_default 已修复（2026-09-13 U0#1③：模板队列溢出，容量改 MAXV*MAXV）——
// 双向对账咬合后从本表移除，同步 E2E_FAILURES.md 历史已修复表。
const KNOWN_TEMPLATE_FAILURES: &[&str] = &["bTree_default"];

/// Known K&R failures documented in KR_FAILURES.md.
/// Monitored by `test_vitro_e2e_knr_known_failures` below.
const KNOWN_KR_FAILURES: &[&str] = &[
    // 阶段 2: K&R 第 3-4 章（残留）
    // kr_4_3/kr_4_4/kr_4_5/kr_4_6 已修复 (作用域隔离 + printf %g 格式支持 + math.h)
    // kr_4_9 已修复 (用户函数 qsort 可遮蔽内置函数)
    // 阶段 3: K&R 第 5-6 章
    // kr_5_1 已修复 (添加 ungetc Host Func)
    // kr_5_2 已修复 (添加 ungetc Host Func)
    // kr_5_8 已修复 (Parser 支持函数指针类型转换；Golden 按 Clang+stdlib.h 重新生成)
    // kr_5_9 已修复 (TypeChecker 支持 (*fp)(args) + BytecodeGen 函数指针解引用不加载内存)
    // kr_5_10 已修复 (VM 支持 main(int argc, char *argv[]) + CLI 传递参数)
    // kr_5_11 已修复 (char*[] 指针数组 elem_type_size / 初始化路径)
    // kr_5_13 已修复 (getchar Batch 模式多行输入)
    // kr_5_14 已修复 (Parser 支持函数指针类型转换/抽象声明符)
    // kr_6_1 已修复 (全局变量区与字符串字面量区重叠，字符串地址延迟分配)
    // kr_6_2 已修复 (Parser 指针无名参数 + ungetc)
    // kr_6_3 已修复 (Parser 指针无名参数 + ungetc)
    // kr_6_4 已修复 (Parser 指针无名参数 + ungetc)
    // kr_6_5 已修复 (添加 strdup Host Func)
    // kr_6_6 已修复 (添加 strdup Host Func)
];

/// Known LeetCode failures documented in LEETCODE_FAILURES.md.
/// Monitored by `test_vitro_e2e_leetcode_known_failures` below.
const KNOWN_LEETCODE_FAILURES: &[&str] = &[
    // 阶段 4~5 逐步填充
];

/// Known C++ failures documented in CPP_FAILURES.md.
/// Monitored by `test_vitro_e2e_cpp_known_failures` below.
const KNOWN_CPP_FAILURES: &[&str] = &[
    // M6 推进过程中逐步填充
];

#[test]
fn test_vitro_e2e_template_generated() {
    let cases = load_cases(Path::new("tests/cases_template_generated"));
    let known: std::collections::HashSet<&str> = KNOWN_TEMPLATE_FAILURES.iter().copied().collect();

    let mut failures = Vec::new();
    for (name, source, input) in &cases {
        if known.contains(name.as_str()) {
            continue;
        }
        if let Err(e) = run_case(name, source, input.as_deref(), "", InputMode::Interactive) {
            failures.push(format!("{}: {}", name, e));
        }
    }
    if !failures.is_empty() {
        panic!(
            "Template generated e2e failures ({} of {} non-known):\n{}\n\n\
             These are NEW failures not yet in KNOWN_TEMPLATE_FAILURES. \
             Please investigate, record in E2E_FAILURES.md, and update the list.",
            failures.len(),
            cases.len() - known.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_knr() {
    let cases = load_cases(Path::new("tests/cases/knr"));
    let known: std::collections::HashSet<&str> = KNOWN_KR_FAILURES.iter().copied().collect();

    let mut failures = Vec::new();
    for (name, source, input) in &cases {
        if known.contains(name.as_str()) {
            continue;
        }
        // K&R getchar 用例在 Batch 模式下运行（输入耗尽返回 EOF）
        let mode = if source.contains("getchar()") {
            InputMode::Batch
        } else {
            InputMode::Interactive
        };
        if let Err(e) = run_case(name, source, input.as_deref(), "knr", mode) {
            failures.push(format!("{}: {}", name, e));
        }
    }
    if !failures.is_empty() {
        panic!(
            "K&R e2e failures ({} of {} non-known):\n{}\n\n\
             These are NEW failures not yet in KNOWN_KR_FAILURES. \
             Please investigate, record in KR_FAILURES.md, and update the list.",
            failures.len(),
            cases.len() - known.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_leetcode() {
    let cases = load_cases(Path::new("tests/cases/leetcode"));
    let known: std::collections::HashSet<&str> = KNOWN_LEETCODE_FAILURES.iter().copied().collect();

    let mut failures = Vec::new();
    for (name, source, input) in &cases {
        if known.contains(name.as_str()) {
            continue;
        }
        if let Err(e) = run_case(name, source, input.as_deref(), "leetcode", InputMode::Interactive) {
            failures.push(format!("{}: {}", name, e));
        }
    }
    if !failures.is_empty() {
        panic!(
            "LeetCode e2e failures ({} of {} non-known):\n{}\n\n\
             These are NEW failures not yet in KNOWN_LEETCODE_FAILURES. \
             Please investigate, record in LEETCODE_FAILURES.md, and update the list.",
            failures.len(),
            cases.len() - known.len(),
            failures.join("\n")
        );
    }
}

/// Monitor known failures: if any of them starts passing, this test fails
/// to remind us to update the documentation and remove it from the list.
#[test]
fn test_vitro_e2e_template_known_failures() {
    let mut passed_unexpectedly = Vec::new();
    for name in KNOWN_TEMPLATE_FAILURES {
        let path = Path::new("tests/cases_template_generated").join(format!("{}.c", name));
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let input_path = path.with_extension("in");
        let input = if input_path.exists() {
            Some(std::fs::read_to_string(&input_path).unwrap_or_default())
        } else {
            None
        };
        if run_case(name, &source, input.as_deref(), "", InputMode::Interactive).is_ok() {
            passed_unexpectedly.push(name.to_string());
        }
    }
    if !passed_unexpectedly.is_empty() {
        panic!(
            "Known failures unexpectedly PASSED ({}). \
             Please update E2E_FAILURES.md and KNOWN_TEMPLATE_FAILURES in vitro_e2e.rs:\n{}",
            passed_unexpectedly.len(),
            passed_unexpectedly.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_knr_known_failures() {
    let mut passed_unexpectedly = Vec::new();
    for name in KNOWN_KR_FAILURES {
        let path = Path::new("tests/cases/knr").join(format!("{}.c", name));
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let input_path = path.with_extension("in");
        let input = if input_path.exists() {
            Some(std::fs::read_to_string(&input_path).unwrap_or_default())
        } else {
            None
        };
        let mode = if source.contains("getchar()") {
            InputMode::Batch
        } else {
            InputMode::Interactive
        };
        if run_case(name, &source, input.as_deref(), "knr", mode).is_ok() {
            passed_unexpectedly.push(name.to_string());
        }
    }
    if !passed_unexpectedly.is_empty() {
        panic!(
            "Known K&R failures unexpectedly PASSED ({}). \
             Please update KR_FAILURES.md and KNOWN_KR_FAILURES in vitro_e2e.rs:\n{}",
            passed_unexpectedly.len(),
            passed_unexpectedly.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_leetcode_known_failures() {
    let mut passed_unexpectedly = Vec::new();
    for name in KNOWN_LEETCODE_FAILURES {
        let path = Path::new("tests/cases/leetcode").join(format!("{}.c", name));
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let input_path = path.with_extension("in");
        let input = if input_path.exists() {
            Some(std::fs::read_to_string(&input_path).unwrap_or_default())
        } else {
            None
        };
        if run_case(name, &source, input.as_deref(), "leetcode", InputMode::Interactive).is_ok() {
            passed_unexpectedly.push(name.to_string());
        }
    }
    if !passed_unexpectedly.is_empty() {
        panic!(
            "Known LeetCode failures unexpectedly PASSED ({}). \
             Please update LEETCODE_FAILURES.md and KNOWN_LEETCODE_FAILURES in vitro_e2e.rs:\n{}",
            passed_unexpectedly.len(),
            passed_unexpectedly.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_cpp() {
    let cases = load_cpp_cases(Path::new("tests/cases/cpp"));
    let known: std::collections::HashSet<&str> = KNOWN_CPP_FAILURES.iter().copied().collect();

    let mut failures = Vec::new();
    for (name, source, input) in &cases {
        if known.contains(name.as_str()) {
            continue;
        }
        let mode = if source.contains("getchar()") {
            InputMode::Batch
        } else {
            InputMode::Interactive
        };
        if let Err(e) = run_cpp_case(name, source, input.as_deref(), "cpp", mode) {
            failures.push(format!("{}: {}", name, e));
        }
    }
    if !failures.is_empty() {
        panic!(
            "C++ e2e failures ({} of {} non-known):\n{}\n\n\
             These are NEW failures not yet in KNOWN_CPP_FAILURES. \
             Please investigate, record in CPP_FAILURES.md, and update the list.",
            failures.len(),
            cases.len() - known.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_cpp_known_failures() {
    let mut passed_unexpectedly = Vec::new();
    for name in KNOWN_CPP_FAILURES {
        let path = Path::new("tests/cases/cpp").join(format!("{}.cpp", name));
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let input_path = path.with_extension("in");
        let input = if input_path.exists() {
            Some(std::fs::read_to_string(&input_path).unwrap_or_default())
        } else {
            None
        };
        let mode = if source.contains("getchar()") {
            InputMode::Batch
        } else {
            InputMode::Interactive
        };
        if run_cpp_case(name, &source, input.as_deref(), "cpp", mode).is_ok() {
            passed_unexpectedly.push(name.to_string());
        }
    }
    if !passed_unexpectedly.is_empty() {
        panic!(
            "Known C++ failures unexpectedly PASSED ({}). \
             Please update CPP_FAILURES.md and KNOWN_CPP_FAILURES in vitro_e2e.rs:\n{}",
            passed_unexpectedly.len(),
            passed_unexpectedly.join("\n")
        );
    }
}

#[test]
fn test_vitro_e2e_generate_report() {
    let baseline_cases = load_cases(Path::new("tests/cases/baseline"));
    let template_cases = load_cases(Path::new("tests/cases_template_generated"));
    let knr_cases = load_cases(Path::new("tests/cases/knr"));
    let leetcode_cases = load_cases(Path::new("tests/cases/leetcode"));
    let cpp_cases = load_cpp_cases(Path::new("tests/cases/cpp"));

    let known_template: std::collections::HashSet<&str> = KNOWN_TEMPLATE_FAILURES.iter().copied().collect();
    let known_kr: std::collections::HashSet<&str> = KNOWN_KR_FAILURES.iter().copied().collect();
    let known_leetcode: std::collections::HashSet<&str> = KNOWN_LEETCODE_FAILURES.iter().copied().collect();
    let known_cpp: std::collections::HashSet<&str> = KNOWN_CPP_FAILURES.iter().copied().collect();

    let mut report = String::new();
    report.push_str("# Vitro E2E 测试报告\n\n");
    let now = std::time::SystemTime::now();
    let dt = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let ts = format!("{}.{:03}Z", dt.as_secs(), dt.subsec_millis());
    report.push_str(&format!("生成时间 (Unix): {}\n\n", ts));

    report.push_str("## 摘要\n\n");
    report.push_str("| 类别 | 总数 | 通过 | 已知失败 |\n");
    report.push_str("|------|------|------|----------|\n");
    report.push_str(&format!(
        "| Baseline | {} | {} | 0 |\n",
        baseline_cases.len(),
        baseline_cases.len()
    ));
    report.push_str(&format!(
        "| Template Generated | {} | {} | {} |\n",
        template_cases.len(),
        template_cases.len() - known_template.len(),
        known_template.len()
    ));
    report.push_str(&format!(
        "| K&R | {} | {} | {} |\n",
        knr_cases.len(),
        knr_cases.len().saturating_sub(known_kr.len()),
        known_kr.len()
    ));
    report.push_str(&format!(
        "| LeetCode | {} | {} | {} |\n",
        leetcode_cases.len(),
        leetcode_cases.len().saturating_sub(known_leetcode.len()),
        known_leetcode.len()
    ));
    report.push_str(&format!(
        "| C++ | {} | {} | {} |\n",
        cpp_cases.len(),
        cpp_cases.len().saturating_sub(known_cpp.len()),
        known_cpp.len()
    ));
    report.push('\n');

    if !KNOWN_TEMPLATE_FAILURES.is_empty() {
        report.push_str("## 已知失败详情（Template Generated）\n\n");
        report.push_str("| 用例 | 根因文件 |\n");
        report.push_str("|------|----------|\n");
        for name in KNOWN_TEMPLATE_FAILURES {
            report.push_str(&format!("| {} | E2E_FAILURES.md |\n", name));
        }
        report.push('\n');
    }

    if !KNOWN_KR_FAILURES.is_empty() {
        report.push_str("## 已知失败详情（K&R）\n\n");
        report.push_str("| 用例 | 根因文件 |\n");
        report.push_str("|------|----------|\n");
        for name in KNOWN_KR_FAILURES {
            report.push_str(&format!("| {} | KR_FAILURES.md |\n", name));
        }
        report.push('\n');
    }

    if !KNOWN_LEETCODE_FAILURES.is_empty() {
        report.push_str("## 已知失败详情（LeetCode）\n\n");
        report.push_str("| 用例 | 根因文件 |\n");
        report.push_str("|------|----------|\n");
        for name in KNOWN_LEETCODE_FAILURES {
            report.push_str(&format!("| {} | LEETCODE_FAILURES.md |\n", name));
        }
        report.push('\n');
    }

    if !KNOWN_CPP_FAILURES.is_empty() {
        report.push_str("## 已知失败详情（C++）\n\n");
        report.push_str("| 用例 | 根因文件 |\n");
        report.push_str("|------|----------|\n");
        for name in KNOWN_CPP_FAILURES {
            report.push_str(&format!("| {} | CPP_FAILURES.md |\n", name));
        }
        report.push('\n');
    }

    report.push_str("## Golden 生成失败\n\n");
    report.push_str("详见 `tests/cases_golden/GOLDEN_FAILURES.md`\n\n");

    let report_path = Path::new("tests/TEST_REPORT.md");
    std::fs::write(report_path, report).expect("write report");
}
