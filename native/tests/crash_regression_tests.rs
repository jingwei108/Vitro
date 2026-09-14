//! 崩溃止血回归测试（2026-09-06 代码审查报告第一批修复）。
//!
//! 覆盖审查报告 P0 修复项，全部用例来自实测复现（release 版 vitro_cli 验证）：
//! - F-P0-1：深嵌套/长注释/多星号曾导致编译器栈溢出（SIGSEGV，无法被 catch_unwind 捕获）
//! - T-P0-8：自含 struct 曾导致 TypeChecker/compute_type_size 无限递归栈溢出
//! - V-P0-1/2：INT_MIN % -1、LLONG_MIN / -1 等曾直接 panic
//! - V-P0-5：qsort/bsearch 恶意 nmemb*size 曾 wrap 绕过边界检查导致 OOM
//! - E-P1-6：apply_fix 对含中文的行按字节切片曾 panic
//!
//! 防线哲学：测试不是为了标榜通过率，而是诚实地发现自己可能存在的问题。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};

/// 与 vitro_e2e.rs 相同的 C API 驱动方式（独立复制以避免跨测试文件依赖）。
fn compile_and_run(source: &str) -> Result<(i32, Vec<String>), String> {
    compile_and_run_with_filename(source, None, "main.c")
}

/// 带标准输入的运行（scanf/getchar 流式语义测试用）。
fn compile_and_run_with_input(source: &str, input: &str) -> Result<(i32, Vec<String>), String> {
    compile_and_run_with_filename(source, Some(input), "main.c")
}

fn compile_and_run_with_filename(
    source: &str,
    input: Option<&str>,
    filename: &str,
) -> Result<(i32, Vec<String>), String> {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        if session.is_null() {
            return Err("Failed to create session".to_string());
        }

        let fname = CString::new(filename).map_err(|e| e.to_string())?;
        let src = CString::new(source).map_err(|e| e.to_string())?;
        if let Some(input_str) = input {
            let normalized = input_str.replace("\r\n", "\n");
            (*session).runtime.input_lines = normalized.split_inclusive('\n').map(|l| l.to_string()).collect();
        }
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

// ===================== V-P0-1 / V-P0-2：算术溢出转教学 trap =====================

#[test]
fn test_int_min_mod_minus_one_traps_not_panics() {
    // Rust 的 % 在 i32::MIN % -1 时溢出 panic（debug 与 release 一致），必须先拦截。
    // 修复前：release 下 `attempt to calculate the remainder with overflow` panic。
    let src = "#include <stdio.h>\n#include <limits.h>\nint main(){ int a = INT_MIN; printf(\"%d\\n\", a % -1); return 0; }\n";
    let result = compile_and_run(src);
    match result {
        Err(msg) => assert!(msg.contains("取模溢出"), "应报告取模溢出教学诊断，实际: {}", msg),
        Ok(_) => panic!("INT_MIN % -1 应产生运行错误而非成功"),
    }
}

// ===================== V-P1-6：栈缓冲区溢出检测 =====================

#[test]
fn test_stack_buffer_overflow_strcpy_traps() {
    // 修复前：容量检查只覆盖堆 region，栈上 `char buf[4]` 被 strcpy 静默
    // 覆写相邻局部变量（教学 IDE 最需捕获的经典错误）。
    let src = "#include <string.h>\nint main(){ char buf[4]; strcpy(buf, \"hello world\"); return 0; }\n";
    match compile_and_run(src) {
        Err(msg) => assert!(
            msg.contains("E3070") && msg.contains("buf"),
            "strcpy 栈溢出应报告 E3070 并指出缓冲区名，实际: {}",
            msg
        ),
        Ok(_) => panic!("strcpy 写 11 字节进 char[4] 应触发 Buffer Overflow"),
    }
}

#[test]
fn test_stack_buffer_overflow_strcat_traps() {
    let src = "#include <string.h>\nint main(){ char buf[8] = \"abc\"; strcat(buf, \"1234567890\"); return 0; }\n";
    match compile_and_run(src) {
        Err(msg) => assert!(msg.contains("E3070"), "strcat 栈溢出应报告 E3070，实际: {}", msg),
        Ok(_) => panic!("strcat 溢出应触发 Buffer Overflow"),
    }
}

#[test]
fn test_legal_strcpy_strcat_unaffected() {
    // 反向回归：合法长度不得误伤
    let src = "#include <stdio.h>\n#include <string.h>\nint main(){ char buf[16]; strcpy(buf, \"hi\"); strcat(buf, \"!\"); printf(\"%s\\n\", buf); return 0; }\n";
    let result = compile_and_run(src);
    assert!(result.is_ok(), "合法 strcpy/strcat 不应误伤: {:?}", result.err());
}

// ===================== V-P1-12：无效 free 诊断 =====================

#[test]
fn test_free_interior_pointer_diagnosed() {
    // 修复前：free(p+1) 静默"成功"——学生以为释放成功且泄漏报告不出现该块
    let src = "#include <stdlib.h>\nint main(){ int* p = (int*)malloc(16); free(p + 1); return 0; }\n";
    match compile_and_run(src) {
        Err(msg) => assert!(
            msg.contains("无效 free") || msg.contains("E3027"),
            "free 块内部地址应给教学诊断，实际: {}",
            msg
        ),
        Ok(_) => panic!("free(p+1) 应触发无效 free 诊断"),
    }
}

#[test]
fn test_free_stack_address_diagnosed() {
    let src = "#include <stdlib.h>\nint main(){ int x = 5; free(&x); return 0; }\n";
    match compile_and_run(src) {
        Err(msg) => assert!(msg.contains("无效 free"), "free 栈地址应给教学诊断，实际: {}", msg),
        Ok(_) => panic!("free(&stack_var) 应触发无效 free 诊断"),
    }
}

#[test]
fn test_delete_nullptr_is_noop() {
    // V-P1-12 顺带暴露的存量缺陷：delete[] nullptr 未判空，ptr-4 wrap 为
    // 0xFFFFFFFC 后 free 触发误报。C++ 标准要求 delete nullptr 是 no-op。
    let src = "struct V { int* data; V() : data(0) {} ~V(){ delete[] data; } };\nint main(){ V v; return 0; }\n";
    let result = compile_and_run_with_filename(src, None, "main.cpp");
    assert!(result.is_ok(), "delete[] nullptr 应为安全 no-op: {:?}", result.err());
}

// ===================== V-P1-13：scanf 字符流语义 =====================

#[test]
fn test_scanf_streaming_within_line() {
    // 修复前：scanf 每次调用整行消费，"1 2\n3 4" 下第二次 scanf("%d") 读到 3
    let src = "#include <stdio.h>\nint main(){ int a, b; scanf(\"%d\", &a); scanf(\"%d\", &b); printf(\"%d %d\\n\", a, b); return 0; }\n";
    let result = compile_and_run_with_input(src, "1 2\n3 4\n");
    match result {
        Ok((_, outputs)) => assert!(
            outputs.iter().any(|l| l.contains("1 2")),
            "两次 scanf 应从同一行读出 1 和 2，实际: {:?}",
            outputs
        ),
        Err(e) => panic!("scanf 流式用例应正常运行: {}", e),
    }
}

// ===================== Issue A：scanf 格式串空白指令（教学阻断） =====================
// SharpTutor Issue A（2026-09-07）：`"%d %c %d"` 读 `3 + 4` 时 %c 捕获空格——
// 根因是 parse_scanf_specs 只提取 % 转换符、丢弃格式串空白指令（C11 7.21.6.2
// 要求空白指令匹配输入中任意数量（含零）的空白）。scanf/sscanf 共享解析，全族同病。
// 期望值全部来自 Clang（MSVC CRT）实测 Golden。

#[test]
fn test_scanf_whitespace_directive_before_c() {
    // Issue A 核心复现：`3 + 4` → a=3, op='+', b=4（Clang 实测输出 "3 + 4"）
    let src = "#include <stdio.h>\nint main(){ int a, b; char op; scanf(\"%d %c %d\", &a, &op, &b); printf(\"%d %c %d\\n\", a, op, b); return 0; }\n";
    let result = compile_and_run_with_input(src, "3 + 4\n");
    match result {
        Ok((_, outputs)) => assert!(
            outputs.iter().any(|l| l.trim() == "3 + 4"),
            "空白指令应吃掉空格让 %c 读到 '+'，实际: {:?}",
            outputs
        ),
        Err(e) => panic!("Issue A 用例应正常运行: {}", e),
    }
}

#[test]
fn test_scanf_c_without_directive_still_reads_space() {
    // 反向对照（Clang 实测 "%d%c" 读 "3 + 4" → op 为空格，码值 32）：
    // %c 不自动跳白是既有正确语义，修复不得破坏。
    let src = "#include <stdio.h>\nint main(){ int a; char op; scanf(\"%d%c\", &a, &op); printf(\"%d %d\\n\", a, (int)op); return 0; }\n";
    let result = compile_and_run_with_input(src, "3 + 4\n");
    match result {
        Ok((_, outputs)) => assert!(
            outputs.iter().any(|l| l.trim() == "3 32"),
            "无空白指令时 %c 应读到空格（码值 32），实际: {:?}",
            outputs
        ),
        Err(e) => panic!("反向对照用例应正常运行: {}", e),
    }
}

#[test]
fn test_scanf_multiple_whitespace_directives_equivalent() {
    // Clang 实测："%d  %c%d" 读 "  3    +4" → 3 [+] 4。
    // %d 自动跳前导空白；多个空白指令等价一个；空白指令后无空白也匹配零个。
    let src = "#include <stdio.h>\nint main(){ int a, b; char op; scanf(\"%d  %c%d\", &a, &op, &b); printf(\"%d %c %d\\n\", a, op, b); return 0; }\n";
    let result = compile_and_run_with_input(src, "  3    +4\n");
    match result {
        Ok((_, outputs)) => assert!(
            outputs.iter().any(|l| l.trim() == "3 + 4"),
            "多个空白指令应等价单个并吃掉全部空白，实际: {:?}",
            outputs
        ),
        Err(e) => panic!("多空白指令用例应正常运行: {}", e),
    }
}

#[test]
fn test_sscanf_whitespace_directive_same_semantics() {
    // sscanf 共享 parse_scanf_specs，同一修复受益（Clang 实测 "3 + 4"）
    let src = "#include <stdio.h>\nint main(){ int a, b; char op; sscanf(\"3 + 4\", \"%d %c %d\", &a, &op, &b); printf(\"%d %c %d\\n\", a, op, b); return 0; }\n";
    let result = compile_and_run(src);
    match result {
        Ok((_, outputs)) => assert!(
            outputs.iter().any(|l| l.trim() == "3 + 4"),
            "sscanf 空白指令语义应与 scanf 一致，实际: {:?}",
            outputs
        ),
        Err(e) => panic!("sscanf 空白指令用例应正常运行: {}", e),
    }
}

// ===================== Issue B：lambda 调用三缺陷（教学阻断） =====================
// SharpTutor Issue B（2026-09-07）三个根因：
// - B1：`[](int a,int b){return a+b;}(2,3)` 立即调用编译错——typeck 的
//   resolve_call_ptr 只处理 callee 是标识符的情形，Lambda 表达式节点的 callee
//   落到"非函数指针"兜底；
// - B2：lambda 变量槽位按闭包类字段大小分配（无捕获 = 0 字节），而槽里存的是
//   4 字节闭包对象地址，StoreLocal 直接冲出 1MB 线性内存（"StoreLocal: 地址越界"）；
//   同源问题另有两处：闭包对象本身也按 0 字节分配、实参位置按字段大小多压 word；
// - B3：调用非函数的兜底误用 E3045_CompoundAssignType，建议文本串成
//   "+= -= *= /= 等复合赋值要求操作数类型兼容"。
// 期望输出全部来自 Clang++（-std=c++17）实测 Golden。

/// C++ 用例需 .cpp 文件名以启用 C++ 前端。
fn compile_and_run_cpp(source: &str) -> Result<(i32, Vec<String>), String> {
    compile_and_run_with_filename(source, None, "main.cpp")
}

#[test]
fn test_lambda_immediate_invocation_b1() {
    // B1（Clang Golden "g1=5"）：立即调用 lambda 曾编译错
    let src = "#include <stdio.h>\nint main(){ int r = [](int a, int b) { return a + b; }(2, 3); printf(\"g1=%d\\n\", r); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("lambda 立即调用应可编译运行");
    assert!(outputs.iter().any(|l| l.trim() == "g1=5"), "期望 g1=5，实际: {:?}", outputs);
}

#[test]
fn test_lambda_immediate_invocation_as_argument_b2() {
    // B2（Clang Golden "g2=6 9"）：立即调用出现在 printf 与自定义函数的实参位置
    let src = "#include <stdio.h>\nint add(int x) { return x + 1; }\nint main(){ printf(\"g2=%d %d\\n\", [](int x) { return x + 1; }(5), add([](int x) { return x * 2; }(4))); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("lambda 实参位置应立即调用成功");
    assert!(outputs.iter().any(|l| l.trim() == "g2=6 9"), "期望 g2=6 9，实际: {:?}", outputs);
}

#[test]
fn test_lambda_var_declared_after_immediate_call_b2() {
    // B2 最小复现（Clang Golden "g3=5 101"）：先立即调用、后声明 lambda 变量时
    // 帧槽位越界——旧实现 main 帧只有 4 字节，StoreLocal offset=4 正好越过 1MB 内存
    let src = "#include <stdio.h>\nint main(){ int r2 = [](int a, int b) { return a + b; }(2, 3); auto f = [](int x) { return x + 100; }; printf(\"g3=%d %d\\n\", r2, f(1)); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("立即调用后声明 lambda 变量不应越界");
    assert!(outputs.iter().any(|l| l.trim() == "g3=5 101"), "期望 g3=5 101，实际: {:?}", outputs);
}

#[test]
fn test_lambda_capturing_immediate_invocation_b2() {
    // B2（Clang Golden "g4=15 117"）：有捕获闭包（单字段 / 双字段）作为实参，
    // 旧实现按字段大小算 word 数，双字段会多压 1 word 造成参数错位
    let src = "#include <stdio.h>\nint main(){ int base = 10; int b2 = 100; printf(\"g4=%d %d\\n\", [base](int x) { return x + base; }(5), [base, b2](int x) { return x + base + b2; }(7)); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("有捕获 lambda 立即调用不应参数错位");
    assert!(outputs.iter().any(|l| l.trim() == "g4=15 117"), "期望 g4=15 117，实际: {:?}", outputs);
}

#[test]
fn test_lambda_static_variable_slots_b2() {
    // B2 同源分支（Clang Golden "s=8 6"）：static lambda 变量走全局区分配，
    // 旧实现同样按闭包类字段大小（无捕获 = 0）占地，两个 static 闭包地址重叠，
    // printf 读到非法数据（实测输出乱码）
    let src = "#include <stdio.h>\nint main(){ static auto f = [](int x) { return x + 7; }; static auto g = [](int x) { return x * 3; }; printf(\"s=%d %d\\n\", f(1), g(2)); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("static lambda 变量不应地址重叠");
    assert!(outputs.iter().any(|l| l.trim() == "s=8 6"), "期望 s=8 6，实际: {:?}", outputs);
}

#[test]
fn test_lambda_variable_call_regression_b1() {
    // 反向对照：变量形式 lambda 调用是既有能力，本次修复不得破坏
    let src = "#include <stdio.h>\nint main(){ auto f = [](int x) { return x + 100; }; printf(\"g5=%d\\n\", f(1)); return 0; }\n";
    let (_, outputs) = compile_and_run_cpp(src).expect("lambda 变量调用应保持可用");
    assert!(outputs.iter().any(|l| l.trim() == "g5=101"), "期望 g5=101，实际: {:?}", outputs);
}

#[test]
fn test_call_non_function_reports_e3066_b3() {
    // B3：调用非函数应报专属错误码——Clang 语义 "called object type 'int' is not
    // a function or function pointer"；旧实现复用 E3045 并把建议串成复合赋值文案
    let src = "int main(){ int x = 10; x(); return 0; }\n";
    let err = compile_and_run_cpp(src).expect_err("调用非函数应当编译失败");
    assert!(err.contains("E3066"), "应报 E3066_CallNonFunction，实际诊断: {}", err);
    assert!(err.contains("调用目标不是函数"), "标题/建议应为调用语义，实际诊断: {}", err);
    assert!(!err.contains("E3045"), "不得再复用 E3045_CompoundAssignType，实际诊断: {}", err);
    assert!(!err.contains("复合赋值"), "不得再串出复合赋值的建议文本，实际诊断: {}", err);
}

#[test]
fn test_llong_min_div_mod_neg_one_traps_not_panics() {
    for (name, expr) in [("div", "/ -1"), ("mod", "% -1"), ("neg", "-a")] {
        let src = format!(
            "#include <stdio.h>\nint main(){{ long long a = -9223372036854775807LL; a = a - 1; printf(\"%lld\\n\", {}); return 0; }}\n",
            if expr == "-a" { "-a".to_string() } else { format!("a {}", expr) }
        );
        let _ = name;
        match compile_and_run(&src) {
            Err(msg) => assert!(msg.contains("溢出"), "long long {} 应报告溢出教学诊断，实际: {}", name, msg),
            Ok(_) => panic!("LLONG_MIN {} 应产生运行错误而非成功", name),
        }
    }
}

// ===================== F-P0-1：递归深度防护 =====================

#[test]
fn test_massive_line_comments_no_stack_overflow() {
    // 修复前：2 万行 `//c` 注释使 lexer next_token 递归 2 万层，栈溢出 SIGSEGV。
    let mut src = String::from("int main(){ return 0; }\n");
    for _ in 0..20000 {
        src.push_str("//c\n");
    }
    let result = compile_and_run(&src);
    assert!(result.is_ok(), "大量行注释不应影响编译: {:?}", result.err());
}

#[test]
fn test_deep_brace_nesting_reports_error_no_crash() {
    // 修复前：6 万层 `{{{{` 使 parse_block 递归栈溢出。
    let mut src = String::from("int main()");
    src.push_str(&"{".repeat(60000));
    src.push_str(&"}".repeat(60000));
    src.push_str(" return 0; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过深花括号嵌套应编译失败");
}

#[test]
fn test_deep_paren_nesting_reports_error_no_crash() {
    // 修复前：5 万层 `((((` 使 parse_primary 递归栈溢出。
    let mut src = String::from("int main(){ int x = ");
    src.push_str(&"(".repeat(50000));
    src.push('1');
    src.push_str(&")".repeat(50000));
    src.push_str("; return 0; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过深括号嵌套应编译失败");
}

#[test]
fn test_excessive_pointer_stars_report_error_no_crash() {
    // 修复前：10 万个 `*` 构造同深度嵌套 DeclaratorNode，后续遍历栈溢出。
    let mut src = String::from("int main(){ int ");
    src.push_str(&"*".repeat(100000));
    src.push_str(" p; return 0; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过深指针层级应编译失败");
}

#[test]
fn test_legal_deep_expression_still_compiles() {
    // 深度防护回归：40 层合法括号嵌套必须照常编译（上限 64 层远高于教学需求）。
    let mut src = String::from("int main(){ int x = ");
    src.push_str(&"(".repeat(40));
    src.push('1');
    src.push_str(&")".repeat(40));
    src.push_str("; return x; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_ok(), "40 层合法嵌套不应被深度防护误伤: {:?}", result.err());
}

// ===================== T-P0-8：自含 struct 环检测 =====================

#[test]
fn test_self_contained_struct_reports_e3072() {
    // 修复前：`struct S { struct S inner; }` 使 compute_type_size 无限递归栈溢出。
    // 学生写链表节点漏 `*` 即触发，是教学场景高频错误。
    let src = "struct S { int a; struct S inner; };\nint main(){ struct S s; s.a = 1; return 0; }\n";
    let result = compile_and_run(src);
    match result {
        Err(msg) => assert!(
            msg.contains("循环包含") || msg.contains("E3072"),
            "自含 struct 应报告 E3072 循环包含，实际: {}",
            msg
        ),
        Ok(_) => panic!("自含 struct 应编译失败"),
    }
}

#[test]
fn test_mutual_struct_containment_reports_e3072() {
    let src = "struct A { int x; struct B b; };\nstruct B { int y; struct A a; };\nint main(){ return 0; }\n";
    let result = compile_and_run(src);
    match result {
        Err(msg) => assert!(
            msg.contains("循环包含") || msg.contains("E3072"),
            "互相包含 struct 应报告 E3072 循环包含，实际: {}",
            msg
        ),
        Ok(_) => panic!("互相包含 struct 应编译失败"),
    }
}

#[test]
fn test_legal_linked_list_pointer_still_compiles() {
    // 环检测回归：指针成员不构成环，合法链表节点必须照常编译运行。
    let src = "struct Node { int val; struct Node* next; };\nint main(){ struct Node n; n.val = 1; n.next = 0; return n.val; }\n";
    let result = compile_and_run(src);
    assert!(result.is_ok(), "合法链表节点不应被环检测误伤: {:?}", result.err());
}

// ===================== V-P0-5：乘法溢出防护 =====================

#[test]
fn test_qsort_huge_params_rejected_no_oom() {
    // 修复前：qsort(base, 2^32, 2^32, cmp) 的 nmemb*size usize 乘法 wrap 为 0，
    // 绕过边界检查后 (0..2^32).collect() 分配 ~32GB 导致 OOM abort。
    let src = "#include <stdlib.h>\nint cmp(const void* a, const void* b){ return 0; }\nint main(){ qsort((void*)100, 4294967295u, 4294967295u, cmp); return 0; }\n";
    let result = compile_and_run(src);
    assert!(result.is_ok(), "恶意 qsort 参数应被静默拒绝而非 OOM: {:?}", result.err());
}

// ===================== E-P1-6：apply_fix 中文行安全 =====================

#[test]
fn test_apply_fix_chinese_line_no_panic() {
    use vitro_native::session::Diagnostic;

    let make_diag = |start_col: i32, end_col: i32, text: &str| Diagnostic {
        line: 1,
        column: 1,
        error_code: 3001,
        severity: 1,
        message: "test".to_string(),
        fix_suggestion: String::new(),
        fix_kind: 1, // ReplaceText
        replace_start_line: 1,
        replace_start_column: start_col,
        replace_end_line: 1,
        replace_end_column: end_col,
        replacement_text: text.to_string(),
        filename: "main.c".to_string(),
    };

    // 含中文的行：字节与字符语义混用时，旧实现按字节切片会 panic（非字符边界）。
    // "中文" 占 6 字节；列 3 在字节语义下落在 '文' 中间。
    let source = "int 中文变量 = 1;".to_string();

    // 字节列恰好合法（列 6：'文' 之后）
    let r = vitro_native::diagnostics::auto_fix::apply_fix(source.clone(), make_diag(6, 9, "X"));
    assert!(r.is_some(), "合法字节边界列应正常替换");

    // 字节列落在多字节字符中间（列 3）：旧实现 panic，新实现按字符语义回退
    let r = vitro_native::diagnostics::auto_fix::apply_fix(source.clone(), make_diag(3, 9, "X"));
    assert!(r.is_some(), "非字符边界列不应 panic，应按字符语义回退");

    // 字符语义列（列 4 = 第 5 个字符 '文' 之后的位置）也应工作
    let r = vitro_native::diagnostics::auto_fix::apply_fix(source.clone(), make_diag(4, 10, "X"));
    assert!(r.is_some(), "字符语义列应正常替换");
}

// ============ 堆语义专项（2026-09-11 决议：bump 分配 + 有界隔离）============
// 依据 堆有界隔离决议.md §6 验收清单。
// 期望值来源：本决议定义的引擎语义（非 Clang 对照——隔离区为 Vitro 教学特性；
// 与 Clang 的差异已在 C语言子集规范.md §2.9 记录）。

#[test]
fn test_heap_churn_beyond_quarantine_budget_no_wall() {
    // §6-2：超隔离预算（256KB）的 churn 循环 10 万次不撞墙；且驱逐后地址确实复用。
    // 反证：若无驱逐复用，100 字节 × 10485 次即耗尽 1MB。
    let src = "#include <stdio.h>\n#include <stdlib.h>\nint main(){ char *keep[3000]; for (int i = 0; i < 3000; i++) keep[i] = (char*)malloc(100); char *hi = keep[2999]; for (int i = 0; i < 3000; i++) free(keep[i]); char *x = (char*)malloc(100); printf(\"reuse=%d\\n\", x != 0 && x <= hi); for (int i = 0; i < 100000; i++) { char *p = (char*)malloc(100); if (!p) { printf(\"hit wall\\n\"); return 1; } p[0] = (char)i; free(p); } printf(\"churn ok\\n\"); free(x); return 0; }\n";
    let (ret, outputs) = compile_and_run(src).expect("churn 用例应正常完成");
    assert_eq!(ret, 0);
    assert!(
        outputs.iter().any(|l| l.trim() == "reuse=1"),
        "隔离区驱逐后必须复用旧地址，实际: {:?}",
        outputs
    );
    assert!(
        outputs.iter().any(|l| l.trim() == "churn ok"),
        "10 万次 churn 不得撞墙，实际: {:?}",
        outputs
    );
}

#[test]
fn test_heap_uaf_within_quarantine_window_detected() {
    // §6-3：free 后立即读必检出（隔离窗口内地址不复用是检测的物理基础）
    let src = "#include <stdio.h>\n#include <stdlib.h>\nint main(){ char *p = (char*)malloc(10); free(p); p[0] = 'X'; printf(\"not reached\\n\"); return 0; }\n";
    let err = compile_and_run(src).expect_err("隔离窗口内 UAF 必须触发 trap");
    assert!(err.contains("E3060"), "应报 Use-After-Free E3060，实际: {}", err);
}

#[test]
fn test_heap_double_free_within_quarantine_window_detected() {
    // §6-3：隔离窗口内双 free 必检出
    let src = "#include <stdio.h>\n#include <stdlib.h>\nint main(){ char *p = (char*)malloc(10); free(p); free(p); printf(\"not reached\\n\"); return 0; }\n";
    let err = compile_and_run(src).expect_err("隔离窗口内 Double-Free 必须触发 trap");
    assert!(err.contains("E3061"), "应报 Double-Free E3061，实际: {}", err);
}

#[test]
fn test_heap_1mb_wall_returns_null_with_teaching_hint() {
    // §6-3 三道墙之一：leak 路径 bump 单调推进至 1MB → malloc 返回 NULL + 教学提示。
    // 诚实记录：与决议 §5「教学 trap」措辞的差异——C 标准要求分配失败返回 NULL，
    // Clang 同样返回 NULL，故此处不 trap（对齐 Clang，避免教坏"不检查返回值"）。
    let src = "#include <stdio.h>\n#include <stdlib.h>\nint main(){ int n = 0; for (;;) { char *p = (char*)malloc(4096); if (!p) break; p[0] = 0; n++; } printf(\"wall=%d\\n\", n > 0); return 0; }\n";
    let (ret, outputs) = compile_and_run(src).expect("leak 用例应正常返回");
    assert_eq!(ret, 0);
    assert!(
        outputs.iter().any(|l| l.trim() == "wall=1"),
        "1MB 墙处必须得到 NULL，实际: {:?}",
        outputs
    );
    assert!(
        outputs.iter().any(|l| l.contains("内存耗尽")),
        "堆耗尽应给出教学提示，实际: {:?}",
        outputs
    );
}

#[test]
fn test_heap_realloc_preserves_data_across_move() {
    // §3：realloc 恒为新块拷贝，原数据（截断到新大小）必须完整保留
    let src = "#include <stdio.h>\n#include <stdlib.h>\nint main(){ char *p = (char*)malloc(8); for (int i = 0; i < 8; i++) p[i] = (char)('A' + i); char *q = (char*)realloc(p, 32); printf(\"moved=%d \", q != p); for (int i = 0; i < 8; i++) printf(\"%c\", q[i]); printf(\"\\n\"); free(q); return 0; }\n";
    let (ret, outputs) = compile_and_run(src).expect("realloc 用例应正常完成");
    assert_eq!(ret, 0);
    assert!(
        outputs.iter().any(|l| l.trim() == "moved=1 ABCDEFGH"),
        "realloc 应搬移新块并保留原数据，实际: {:?}",
        outputs
    );
}

#[test]
fn test_second_wall_max_steps_fuse() {
    // §6-3 三道墙之二：max_steps 保险丝（教学内容 = 可控地撞上限并拿到教学 trap，
    // 而不是无限等待）。会话级配置经 vitro_set_max_steps 注入。
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());
        let fname = CString::new("main.c").unwrap();
        let src = CString::new("int main(){ for(;;){} return 0; }\n").unwrap();
        vitro_native::capi::vitro_compile_unit(session, fname.as_ptr() as *const c_char, src.as_ptr() as *const c_char);
        assert_eq!(vitro_native::capi::vitro_compile_all(session), 0, "死循环程序应能编译");
        assert_eq!(
            vitro_native::capi::vitro_set_max_steps(session, 1000),
            0,
            "设置步数上限应成功"
        );

        let _ = vitro_native::capi::vitro_run(session);
        let err_ptr = vitro_native::capi::vitro_get_runtime_error(session);
        let err = if err_ptr.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string()
        };
        assert!(
            err.contains("步数超过限制"),
            "应报告步数超限的教学 trap，实际: {}",
            err
        );
        // 2026-09-11 补强：必须回显**配置的**上限（1000），而不是默认的 1000 万 ——
        // 此前 `setup_vm` 硬编码 `set_max_steps(10_000_000)` 会抹掉会话配置，
        // 而旧断言只查"消息含步数超限"，1000 万步同样满足，长期掩盖了该缺陷。
        assert!(
            err.contains("（1000 步）"),
            "trap 应回显配置的步数上限 1000，实际: {}",
            err
        );
        vitro_native::capi::vitro_session_destroy(session);
    }
}

#[test]
fn test_third_wall_region_table_bounded_when_step_fuse_trips_first() {
    // §5 三道墙之三：leak 路径（只分配不释放）上，若**步数保险丝先于 1MB 堆墙**触发，
    // region 表必须随步数同步封顶（有界）—— 否则"无限 malloc 不 free"在撞堆墙之前
    // 就先吃爆 host 内存（每条 region 记录数十字节）。
    //
    // 决议 §5 原文把该项记为"推导验证"，本用例把它落成可执行验收（2026-09-11）。
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        assert!(!session.is_null());
        let fname = CString::new("main.c").unwrap();
        // 只分配、不释放：bump 单调推进，会先撞 max_steps 再撞 1MB
        let src = CString::new(
            "#include <stdlib.h>\nint main(){ for(;;){ char *p = (char*)malloc(64); p[0] = 0; } return 0; }\n",
        )
        .unwrap();
        vitro_native::capi::vitro_compile_unit(
            session,
            fname.as_ptr() as *const c_char,
            src.as_ptr() as *const c_char,
        );
        assert_eq!(vitro_native::capi::vitro_compile_all(session), 0, "用例应能编译");
        const MAX_STEPS: i32 = 2000;
        assert_eq!(vitro_native::capi::vitro_set_max_steps(session, MAX_STEPS), 0);

        let _ = vitro_native::capi::vitro_run(session);

        let err_ptr = vitro_native::capi::vitro_get_runtime_error(session);
        let err = if err_ptr.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string()
        };
        assert!(
            err.contains("步数超过限制"),
            "步数保险丝应先于 1MB 堆墙触发，实际错误: {err}"
        );

        let regions = (*session).memory.regions.len();
        let heap_offset = (*session).memory.heap_offset;
        assert!(regions > 0, "应有已分配的 region 记录");
        // 每次 malloc 至少消耗 1 个 VM 步 ⇒ region 条数不可能超过步数上限
        assert!(
            regions <= MAX_STEPS as usize,
            "region 表应随步数封顶（有界），实际 {regions} 条 > 步数上限 {MAX_STEPS}"
        );
        assert!(
            heap_offset < vitro_native::session::MEM_SIZE,
            "本用例应先撞步数保险丝（heap_offset={heap_offset} 不应到达 MEM_SIZE={}）",
            vitro_native::session::MEM_SIZE
        );

        vitro_native::capi::vitro_session_destroy(session);
    }
}

// ============ 标准库 stub 头文件与 include 行号补偿（2026-09-11）============

#[test]
fn test_std_stub_headers_with_macros_compile_and_expand() {
    // 缺陷历史：include 内容的换行被替换为空格（为"避免行号偏移"），导致
    // `#define` 落到行中间而不再被识别为预处理指令 —— time.h / float.h /
    // errno.h / assert.h / stdarg.h 五个含宏的标准库 stub 整体编译失败且
    // 不给任何诊断（stdio.h 不含宏，故长期掩盖了该问题）。
    let src = "#include <stdio.h>\n#include <time.h>\n#include <float.h>\n#include <errno.h>\nint main(){ printf(\"%d %d %d\", CLOCKS_PER_SEC, FLT_RADIX, EINVAL); return 0; }\n";
    let (ret, outputs) = compile_and_run(src).expect("含宏的标准库 stub 必须可编译");
    assert_eq!(ret, 0);
    assert!(
        outputs.iter().any(|l| l.trim_start().starts_with("1000000 2 1")),
        "标准库宏必须可展开，实际: {:?}",
        outputs
    );
}

#[test]
fn test_std_stub_assert_macro_works() {
    // 函数式宏同样依赖"指令独占一行"
    let src = "#include <assert.h>\nint main(){ assert(1 == 1); return 0; }\n";
    let (ret, _) = compile_and_run(src).expect("assert.h 的函数式宏必须可编译");
    assert_eq!(ret, 0);
}

#[test]
fn test_include_does_not_shift_diagnostic_line_numbers() {
    // 修复改为"保留换行 + 行号补偿"后，诊断行号必须仍是原始行号
    // （错误写在第 4 行，include 展开的行数不得把它顶偏）。
    let src = "#include <stdio.h>\n#include <time.h>\nint main(){\n    int x = ;\n    return 0;\n}\n";
    let err = compile_and_run(src).expect_err("该程序应编译失败");
    assert!(
        err.contains("E2003") || err.contains("E2005"),
        "应为语法类诊断，实际: {}",
        err
    );
    assert!(
        err.contains("第4行"),
        "诊断行号必须保持原始第 4 行（不得被 include 展开顶偏），实际: {}",
        err
    );
}

// =============================================================================
// U1#8：递归深度防护补全（五通道）+ 后置 AST 深度预算（2026-09-13）
// 修复前全部实测栈溢出（release vitro_cli "has overflowed its stack"）：
// init_list / 赋值右结合链 / 一元链崩在 parser 递归；数组后缀链 / 左结合
// 加法链崩在 typeck 与 AST 递归 Drop（parser 层是循环不递归）。
// =============================================================================

#[test]
fn test_deep_init_list_reports_error_no_crash() {
    // 修复前：3000 层嵌套花括号初始化使 parse_init_list 递归栈溢出。
    let mut src = String::from("int main() { int x = ");
    src.push_str(&"{".repeat(3000));
    src.push('1');
    src.push_str(&"}".repeat(3000));
    src.push_str("; return x; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过深初始化列表嵌套应编译失败而非崩溃");
}

#[test]
fn test_deep_assign_chain_reports_error_no_crash() {
    // 修复前：3000 级右结合赋值链（a = b = b = ...）栈溢出——
    // 每层右侧递归 parse_assign，不经过 parse_primary 的深度防护。
    let mut src = String::from("int main() { int a, b; a ");
    src.push_str(&"= b ".repeat(3000));
    src.push_str("= 1; return a; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过长赋值链应编译失败而非崩溃");
}

#[test]
fn test_deep_unary_chain_reports_error_no_crash() {
    // 修复前：5000 个 `!` 使 parse_unary 递归栈溢出。
    let mut src = String::from("int main() { int x = 1; return ");
    src.push_str(&"!".repeat(5000));
    src.push_str("x; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过长一元运算符链应编译失败而非崩溃");
}

#[test]
fn test_array_suffix_chain_ast_budget_reports_error_no_crash() {
    // 修复前：5000 个 `[0]` 后缀使 AST 深度 5004，typeck / 递归 Drop 栈溢出。
    // parser 层是循环不递归——由后置 AST 深度预算（512）拦截。
    let mut src = String::from("int main() { int a[1][1]; return a[0]");
    src.push_str(&"[0]".repeat(5000));
    src.push_str("; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过长数组后缀链应被 AST 深度预算拒绝而非崩溃");
}

#[test]
fn test_left_assoc_chain_ast_budget_reports_error_no_crash() {
    // 修复前：5000 项左结合加法链（AST 深度 5003）崩在 typeck（外部审查实锤
    // "5000 项 a+a+... 崩，且崩在 typeck 非 codegen"）——后置 AST 深度预算拦截。
    let mut src = String::from("int main() { int a = 1; return a");
    src.push_str(&"+a".repeat(5000));
    src.push_str("; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_err(), "过长左结合链应被 AST 深度预算拒绝而非崩溃");
}

#[test]
fn test_normal_depth_expressions_not_affected_by_budget() {
    // 反向锚：合法的适度嵌套（几十层）不受五通道防护与 AST 预算误伤。
    let mut src = String::from("int main() { int a = 1; return ");
    src.push_str(&"(".repeat(30));
    src.push('a');
    src.push_str(&")".repeat(30));
    src.push_str(&" + a".repeat(100));
    src.push_str("; }\n");
    let result = compile_and_run(&src);
    assert!(result.is_ok(), "30 层括号 + 100 项加法的合法表达式不应被拒: {:?}", result.err());
}

/// U3#4/T1（2026-09-14）：模板实例化轮数上限的可触发性锚（保险丝义务）——
/// `template<class T> int f(T t){return f(&t);}` 每轮生成更深指针类型的实例
/// （f<int>→f<int*>→f<int**>→…），修复前无限实例化直至内存耗尽（外部审查
/// 3 秒栈溢出实锤；OOM 形态无法安全运行留痕，同 register_function 先例）。
/// 修复后 1024 轮上限触发 E1022 确定性诊断（有限时间内完成——本锚本身
/// 即"上限真实存在且可触发"的证明）。
#[test]
fn test_u3_template_instantiation_round_limit_triggered() {
    let src = "template <class T> int f(T t) { return f(&t); }\nint main() { return f(1); }\n";
    let (tokens, _) = vitro_native::compiler::lexer::Lexer::with_mode(src, true).tokenize();
    let (program, errs) = vitro_native::compiler::parser::Parser::with_mode(tokens, true).parse();
    assert!(errs.is_empty(), "parse: {errs:?}");
    let mut program = program.unwrap();
    let (type_errors, _, _) = vitro_native::compiler::typeck::TypeChecker::default().check(&mut program);
    assert!(
        type_errors.iter().any(|e| e.code == 1022),
        "自递归模板必须在轮数上限处确定性报错（E1022），实际：{type_errors:?}"
    );
}

/// U3#9 止血锚（2026-09-14）：两个类各含同名嵌套 struct（不同布局）时，
/// 展平命名下无法共存——修复前**静默覆盖**（后注册者胜出，先者的成员访问
/// 全部指向错误布局：A::Inner{x} 被 B::Inner{y} 覆盖后 a.i.x 报 E3042 假
/// 错误）；修复后显式 E3002 冲突诊断（显式拒绝优于静默错布局）。
/// 完整根治（嵌套名 mangled 化 Outer__Inner）登记下批。
#[test]
fn test_u3_nested_struct_conflict_diagnosed() {
    let src = "class A {\npublic:\n    struct Inner { int x; };\n    Inner i;\n};\nclass B {\npublic:\n    struct Inner { double y; };\n    Inner j;\n};\nint main() { return 0; }\n";
    let (tokens, _) = vitro_native::compiler::lexer::Lexer::with_mode(src, true).tokenize();
    let (program, errs) = vitro_native::compiler::parser::Parser::with_mode(tokens, true).parse();
    assert!(errs.is_empty(), "parse: {errs:?}");
    let mut program = program.unwrap();
    let (type_errors, _, _) = vitro_native::compiler::typeck::TypeChecker::default().check(&mut program);
    assert!(
        type_errors.iter().any(|e| e.code == 3002 && e.message.contains("Inner")),
        "同名嵌套 struct 冲突必须显式诊断（修复前静默覆盖 = 错误布局），实际：{type_errors:?}"
    );
}
