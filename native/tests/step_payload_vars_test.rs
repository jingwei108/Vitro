#![allow(clippy::unwrap_used, clippy::expect_used)]

//! StepPayload 变量快照的可见性与类型名回归（2026-09-11 PR 清单 P2-7）。
//!
//! 覆盖三项曾经的问题：
//! 1. **同名变量**：两个 `for` 各声明一个 `i` 时，同一 payload 只应出现一个 `i`，
//!    且必须是"当前执行点已进入作用域"的那个（按声明行切换）；
//! 2. **跨函数隔离**：`helper` 的局部变量不得出现在 `main` 的 payload（反之亦然）——
//!    此前符号表不过滤函数归属，会用错误的 `locals_base` 读出垃圾值；
//! 3. **数组/类型名**：数组的"值"是元素摘要（而非首元素或地址），`ty_name` 是
//!    C 风格可读名（`int[5]` / `int*`），不再泄漏 Rust Debug 的内部枚举结构。
//!
//! 另覆盖算法标注一致性（同一 PR 清单的 P0-1 / P0-2 / P0-3）：
//! 描述不得引用越界下标、同一步的 `semantic_label` 与 `algorithm_step.description`
//! 必须一致、冒泡"第 k 趟"文案必须等于"第 k 大"。

use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;

/// 逐 step 收集 payload（与 `vitro_cli serve` 的 `step.next` 同一入口语义）。
fn collect_payloads(units: Vec<(&str, &str)>) -> Vec<serde_json::Value> {
    let mut session = Session::default();
    session.compile.compile_units = units
        .into_iter()
        .map(|(filename, source)| CompileUnit {
            filename: filename.to_string(),
            source: source.to_string(),
        })
        .collect();

    let compiled = session_api::compile(&mut session);
    assert!(
        compiled["ok"].as_bool().unwrap_or(false),
        "编译失败：{compiled}"
    );
    assert_eq!(session_api::step_begin(&mut session), 0, "step_begin 应成功");

    let mut payloads = Vec::new();
    for _ in 0..20_000 {
        let step = session_api::step_next(&mut session).expect("step_next 不应失败");
        if let Some(arr) = step["payloads"].as_array() {
            payloads.extend(arr.iter().cloned());
        }
        if step["finished"].as_bool().unwrap_or(false) || step["trapped"].as_bool().unwrap_or(false) {
            break;
        }
    }
    payloads
}

fn vars_of(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    payload["local_vars"].as_array().cloned().unwrap_or_default()
}

#[test]
fn test_local_vars_no_duplicate_names_and_scope_switch() {
    let src = r#"#include <stdio.h>
int main() {
    int s = 0;
    for (int i = 0; i < 2; i++) {
        s += i;
    }
    for (int i = 10; i < 12; i++) {
        s += i;
    }
    printf("%d\n", s);
    return 0;
}
"#;
    let payloads = collect_payloads(vec![("main.c", src)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    // 1) 同一 payload 内不得出现同名变量
    for p in &payloads {
        let names: Vec<String> = vars_of(p)
            .iter()
            .map(|v| v["name"].as_str().unwrap_or("").to_string())
            .collect();
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(names.len(), uniq.len(), "同一 payload 内出现同名变量：{names:?}");
    }

    // 2) 可见的 i 应按声明行在两个作用域之间切换（addr 恰好两个，且切换有序）
    let mut addrs_in_order: Vec<u32> = Vec::new();
    let mut addr_at_second_loop: Option<u32> = None;
    for p in &payloads {
        for v in vars_of(p) {
            if v["name"] != "i" {
                continue;
            }
            let addr = v["addr"].as_u64().unwrap_or(0) as u32;
            let val: i64 = v["value"].as_str().unwrap_or("0").parse().unwrap_or(-1);
            if addrs_in_order.last() != Some(&addr) {
                addrs_in_order.push(addr);
            }
            if val >= 10 {
                addr_at_second_loop = Some(addr);
            }
        }
    }
    assert_eq!(
        addrs_in_order.len(),
        2,
        "两个 for 各自声明的 i 应各自在其作用域内可见（未按声明行过滤时会退化为 1 个地址）：{addrs_in_order:?}"
    );
    assert_eq!(
        addr_at_second_loop,
        addrs_in_order.last().copied(),
        "第二个循环应看到后声明的那个 i"
    );
}

#[test]
fn test_local_vars_scoped_to_current_function() {
    let helper = r#"int helper(int x) {
    int hn = 0;
    int i = 0;
    for (i = 0; i < 3; i++) {
        x = x + hn;
    }
    return x;
}
"#;
    let main = r#"#include <stdio.h>
int helper(int x);
int main() {
    int mv = helper(1);
    printf("%d\n", mv);
    return 0;
}
"#;
    let payloads = collect_payloads(vec![("helper.c", helper), ("main.c", main)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut saw_main = false;
    let mut saw_helper = false;
    for p in &payloads {
        let func = p["func_name"].as_str().unwrap_or("");
        let vars = vars_of(p);
        let names: Vec<&str> = vars.iter().filter_map(|v| v["name"].as_str()).collect();
        match func {
            "main" => {
                saw_main = true;
                assert!(
                    !names.contains(&"hn"),
                    "helper 的局部变量混入 main 的 payload：{names:?}"
                );
            }
            "helper" => {
                saw_helper = true;
                assert!(
                    !names.contains(&"mv"),
                    "main 的局部变量混入 helper 的 payload：{names:?}"
                );
            }
            _ => {}
        }
    }
    assert!(saw_main && saw_helper, "未覆盖 main / helper 两侧（main={saw_main} helper={saw_helper}）");
}

#[test]
fn test_local_vars_array_summary_and_readable_type_name() {
    let src = r#"#include <stdio.h>
int main() {
    int a[5] = {5, 3, 1, 4, 2};
    int x = 7;
    printf("%d %d\n", a[0], x);
    return 0;
}
"#;
    let payloads = collect_payloads(vec![("main.c", src)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut saw_array = false;
    let mut saw_summary = false;
    let mut saw_initialized_elements = false;
    let mut saw_int = false;
    for p in &payloads {
        for v in vars_of(p) {
            let name = v["name"].as_str().unwrap_or("");
            let ty = v["ty_name"].as_str().unwrap_or("");
            let val = v["value"].as_str().unwrap_or("");

            // ty_name 必须是 C 风格可读名（不得是 Rust Debug 的枚举结构）
            assert!(!ty.contains("is_unsigned"), "ty_name 泄漏 Rust Debug 形式：{ty:?}");
            assert!(!ty.contains("is_const"), "ty_name 泄漏 Rust Debug 形式：{ty:?}");
            assert!(!ty.contains('{'), "ty_name 泄漏 Rust Debug 形式：{ty:?}");

            if name == "a" {
                saw_array = true;
                assert_eq!(ty, "int[5]", "数组类型名应为 C 风格 int[5]，实际 {ty:?}");
                if val.starts_with('{') {
                    saw_summary = true;
                    // 初始化前的步骤摘要是 `{0, 0, 0, 0, 0}`，初始化后才含真实元素
                    if val.contains('5') && val.contains('2') {
                        saw_initialized_elements = true;
                    }
                }
            }
            if name == "x" && val == "7" {
                saw_int = true;
                assert_eq!(ty, "int", "标量类型名应为 int，实际 {ty:?}");
            }
        }
    }
    assert!(saw_array, "未在 payload 中看到数组 a");
    assert!(saw_summary, "数组的 value 应为元素摘要（形如 {{5, 3, 1, 4, 2}}）");
    assert!(
        saw_initialized_elements,
        "应至少有一个步骤显示已初始化的元素摘要（含 5 与 2）"
    );
    assert!(saw_int, "未观察到 x == 7 的步骤");
}

// ============================================================================
// 算法标注一致性（2026-09-11 PR 清单 P0-2 / P0-3）
// ============================================================================

/// 解析文本中所有 `arr[N]` 的下标。
fn parse_arr_indices(text: &str) -> Vec<i32> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("arr[") {
        let after = &rest[pos + 4..];
        let Some(close) = after.find(']') else { break };
        if let Ok(n) = after[..close].trim().parse::<i32>() {
            out.push(n);
        }
        rest = &after[close..];
    }
    out
}

const BUBBLE_SRC: &str = r#"#include <stdio.h>

void bubble_sort(int arr[], int n) {
    int i, j, temp;
    for (i = 0; i < n - 1; i++) {
        for (j = 0; j < n - 1 - i; j++) {
            if (arr[j] > arr[j + 1]) {
                temp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = temp;
            }
        }
    }
}

int main() {
    int a[5] = {5, 3, 1, 4, 2};
    bubble_sort(a, 5);
    for (int k = 0; k < 5; k++) printf("%d ", a[k]);
    return 0;
}
"#;

#[test]
fn test_algorithm_step_never_references_out_of_range_index() {
    // P0-2：内层 `j` 停在退出值上时不得再产出 "比较/交换 arr[j] 与 arr[j+1]"。
    // 修复前实测 5 元素数组会产生 24 步含 `arr[5]`（数组上界是 4）的描述。
    let payloads = collect_payloads(vec![("main.c", BUBBLE_SRC)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut saw_compare = false;
    let mut saw_swap = false;
    for p in &payloads {
        let desc = p["algorithm_step"]["description"].as_str().unwrap_or("");
        for idx in parse_arr_indices(desc) {
            assert!(idx < 5, "算法描述引用了越界索引（数组长 5）：{desc:?}");
        }
        if desc.starts_with("比较 arr[") {
            saw_compare = true;
        }
        if desc.starts_with("交换 arr[") {
            saw_swap = true;
        }
    }
    assert!(saw_compare && saw_swap, "未覆盖比较/交换两类描述（compare={saw_compare} swap={saw_swap}）");
}

#[test]
fn test_swap_label_agrees_with_algorithm_step_in_same_payload() {
    // P0-3：同一 payload 内 `semantic_label` 与 `algorithm_step.description` 的交换标注
    // 必须指向同一对下标。修复前 label 取 `loop_vars.first()`（常是规模量 `n`），
    // 会与 desc 互相矛盾（`交换 arr[5]↔arr[6]` vs `交换 arr[0]↔arr[1]`）。
    let payloads = collect_payloads(vec![("main.c", BUBBLE_SRC)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut compared = 0;
    for p in &payloads {
        let label = p["semantic_label"].as_str().unwrap_or("");
        let desc = p["algorithm_step"]["description"].as_str().unwrap_or("");
        if !label.starts_with("交换 arr[") || !desc.starts_with("交换 arr[") {
            continue;
        }
        let li = parse_arr_indices(label);
        let di = parse_arr_indices(desc);
        assert_eq!(li, di, "同一 payload 内两套交换标注不一致：label={label:?} desc={desc:?}");
        compared += 1;
    }
    assert!(compared > 0, "未观察到同时含交换标注的 payload");
}

#[test]
fn test_bubble_pass_description_matches_rank() {
    // P0-1：第 pass 趟确定的是「第 pass 大」的元素（升序冒泡每趟把未排序区间的最大值
    // 冒到区间末尾）。修复前用的是 `n - i`，与排名恰好相反（n=5 时第 1 趟显示第 5 大）。
    let payloads = collect_payloads(vec![("main.c", BUBBLE_SRC)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut seen = 0;
    for p in &payloads {
        let desc = p["algorithm_step"]["description"].as_str().unwrap_or("");
        let Some(rest) = desc.strip_prefix("第 ") else { continue };
        let Some((pass_part, tail)) = rest.split_once(" 趟：将第 ") else { continue };
        let Some((kth_part, _)) = tail.split_once(' ') else { continue };
        let pass: i32 = pass_part.trim().parse().unwrap_or(-1);
        let kth: i32 = kth_part.trim().parse().unwrap_or(-1);
        assert_eq!(pass, kth, "趟数与排名不一致：{desc:?}（应满足「第 k 趟 = 第 k 大」）");
        seen += 1;
    }
    assert!(seen > 0, "未观察到冒泡趟数描述");
}

// ============================================================================
// 多文件会话的行号归属（2026-09-11 PR 清单 P0-4）
// ============================================================================

const MULTI_HELPER: &str = r#"int helper(int x) {
    int arr[3] = {1, 2, 3};
    int j = 0;
    int temp = 0;
    for (j = 0; j < 3; j++) {
        temp = arr[j];
        arr[j] = x;
    }
    return temp;
}
"#;

const MULTI_MAIN: &str = r#"#include <stdio.h>
int helper(int x);
int main() {
    int v = 0;
    v = helper(7);
    printf("%d\n", v);
    return 0;
}
"#;

#[test]
fn test_multifile_semantic_label_uses_correct_file() {
    // P0-4：多文件编译把各单元合并为一份源码，`code_line` 是**全局行号**；此前固定用
    // `compile_units.first()` 的文件内行号去查 —— 必然错配，会拿 helper.c 的行去解释
    // main.c 的执行点，产出"看似合理但完全无关"的描述。
    // 本用例中只有 helper.c 含 `temp = arr[j];`（产出"交换"标签），
    // 因此 main 的 payload 一旦出现"交换"，就说明行号串了文件。
    let payloads = collect_payloads(vec![("helper.c", MULTI_HELPER), ("main.c", MULTI_MAIN)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut helper_swap = false;
    for p in &payloads {
        let label = p["semantic_label"].as_str().unwrap_or("");
        let func = p["func_name"].as_str().unwrap_or("");
        if func == "helper" && label.contains("交换") {
            helper_swap = true;
        }
        if func == "main" {
            assert!(
                !label.contains("交换"),
                "main 的语义标签串用了 helper.c 的源码行：label={label:?} line={}",
                p["code_line"]
            );
        }
    }
    assert!(helper_swap, "helper 侧未产出交换标签，本用例失去判别力");
}

#[test]
fn test_function_definition_line_is_not_reported_as_recursion() {
    // 函数定义行 `int helper(int x) {` 含 `helper(`，但把它标成"递归调用 helper"
    // 是直接呈现给学生的错误描述（真实递归调用发生在函数体内）。
    let payloads = collect_payloads(vec![("helper.c", MULTI_HELPER), ("main.c", MULTI_MAIN)]);
    assert!(!payloads.is_empty(), "未收集到 payload");

    let mut saw_helper = false;
    for p in &payloads {
        let label = p["semantic_label"].as_str().unwrap_or("");
        let func = p["func_name"].as_str().unwrap_or("");
        if func == "helper" {
            saw_helper = true;
            assert!(
                label != "递归调用 helper",
                "函数定义行被误标为递归调用（line {}）",
                p["code_line"]
            );
        }
    }
    assert!(saw_helper, "未观察到 helper 的 payload");
}
