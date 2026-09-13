#![allow(clippy::unwrap_used, clippy::expect_used)]

//! JIT 路径等价性防线（J10，裁定 §14.9/§14.11.4，2026-09-13）。
//!
//! 背景：R-2026-09-13 暴露"JIT fast path 在录制期间穿透内层循环 → 静默错值"，
//! 且既有防线形状互补地全部漏掉（教学用例 <100 次 / 排序类带分支 Abort /
//! 单层循环无穿透）。修复（录制期禁用 fast path）已转绿，但"JIT 生效区间
//! 的正确性必须有独立覆盖"是长期义务——不得由"解释器正确"推断"JIT 正确"。
//!
//! 本防线三锚定，任何一路错都会红：
//!   1. parity 锚：同一程序 JIT 开启与禁用各跑一次，stdout + 返回码必须一致；
//!   2. 期望值锚：结果必须等于手算值（不依赖引擎任何一方自证）；
//!   3. 生效性锚（J9）：JIT 分支断言 steps_accelerated > 0（保证测的确实是
//!      "JIT 生效区间"，否则等价性断言空转）；禁用分支断言 == 0。
//!
//! 真禁用开关 `set_jit_enabled(false)` 是结构性禁用（fast path 不命中、
//! 热点检测与录制均不触发），与 `jit_traces_mut().clear()`（清表后 ip_hits
//! 仍会累积重新录制）不同——后者会让"解释分支"混入 JIT 步，对照失效。

use cide_native::engine::compile_pipeline::run_multi_file_pipeline;
use cide_native::engine::session_ops::{execute_run, reset_runtime};
use cide_native::session::{CompileUnit, Session};
use std::time::Instant;

struct RunOutcome {
    ret: i32,
    stdout: String,
    traces_compiled: u32,
    steps_accelerated: u64,
}

fn compile(source: &str) -> Session {
    let mut session = Session::default();
    session.compile.compile_units.push(CompileUnit {
        filename: "main.c".to_string(),
        source: source.to_string(),
    });
    reset_runtime(&mut session);
    let units = session.compile.compile_units.clone();
    run_multi_file_pipeline(&mut session, units, false).expect("compile");
    session
}

fn run(source: &str, jit: bool) -> RunOutcome {
    let mut session = compile(source);
    if let Some(vm) = session.vm.as_mut() {
        if !jit {
            vm.set_jit_enabled(false);
        }
    }
    let (ret, _) = execute_run(&mut session).expect("run");
    let (traces_compiled, steps_accelerated) = session
        .vm
        .as_ref()
        .map(|vm| (vm.jit_stats().traces_compiled, vm.jit_stats().steps_accelerated))
        .unwrap_or((0, 0));
    RunOutcome {
        ret,
        stdout: session.runtime.stdout(),
        traces_compiled,
        steps_accelerated,
    }
}

/// 三锚断言：parity + 手算期望值 + JIT 生效性。
///
/// `expect_accel`：该形状循环回边是否超过 JIT_THRESHOLD(100)。
/// true  = JIT 分支必须真的在加速（JIT 生效区间，J10 核心）；
/// false = 两分支都必须零加速步（锚定"阈值下不触发录制"语义——
///         短循环形状的意义是"开关存在不改变语义"，不是等价性空转）。
fn assert_parity(source: &str, expected_stdout: &str, expected_ret: i32, expect_accel: bool) {
    let jit = run(source, true);
    let interp = run(source, false);

    // 生效性锚（J9）：expect_accel 时等价性断言必须真的测在 JIT 生效区间上
    if expect_accel {
        assert!(
            jit.steps_accelerated > 0,
            "自检失败：JIT 分支未产生加速步（traces={}, accel={}）——等价性断言在测空气",
            jit.traces_compiled,
            jit.steps_accelerated
        );
    } else {
        assert_eq!(
            (jit.traces_compiled, jit.steps_accelerated),
            (0, 0),
            "自检失败：短循环（<阈值）不应触发 JIT——阈值语义被破坏"
        );
    }
    assert_eq!(
        (interp.traces_compiled, interp.steps_accelerated),
        (0, 0),
        "自检失败：禁用开关未生效，解释分支混入 JIT 步"
    );

    // parity 锚
    assert_eq!(jit.ret, interp.ret, "JIT 与解释路径返回码不一致：JIT 路径语义分叉");
    assert_eq!(
        jit.stdout, interp.stdout,
        "JIT 与解释路径 stdout 不一致：JIT 路径语义分叉\n  JIT:    {:?}\n  interp: {:?}",
        jit.stdout, interp.stdout
    );

    // 期望值锚（手算，独立于引擎）
    assert_eq!(jit.stdout, expected_stdout, "JIT 路径输出与手算期望不符（静默错值）");
    assert_eq!(jit.ret, expected_ret, "JIT 路径返回码与期望不符");
}

/// 嵌套形状不需要 JIT 生效锚的变体：外层 trace 必然 Abort（遇内层回边），
/// 只有内层被 JIT 化——仍要求 steps_accelerated > 0（内层在加速）。
#[test]
fn jit_parity_nested_counting_200x200() {
    // 教科书穿透形状（R-2026-09-13 原案）：inner=40000 i=200 j=200
    let source = r#"
#include <stdio.h>
int main() {
    int i, j, inner = 0;
    for (i = 0; i < 200; i++) {
        for (j = 0; j < 200; j++) {
            inner = inner + 1;
        }
    }
    printf("inner=%d i=%d j=%d\n", inner, i, j);
    return 0;
}
"#;
    assert_parity(source, "inner=40000 i=200 j=200\n", 0, true);
}

#[test]
fn jit_parity_single_hot_loop_arith_bitwise() {
    // 单层热循环（bulk 模板主战场）：sum=500^2=250000, XOR(i&3) over 500 = 0
    let source = r#"
#include <stdio.h>
int main() {
    int sum = 0, x = 0, i;
    for (i = 0; i < 500; i++) {
        sum = sum + (i * 2 + 1);
        x = x ^ (i & 3);
    }
    printf("sum=%d x=%d i=%d\n", sum, x, i);
    return 0;
}
"#;
    assert_parity(source, "sum=250000 x=0 i=500\n", 0, true);
}

#[test]
fn jit_parity_array_write_read_500() {
    // 数组读写进 trace（LoadMem/StoreMem 模板语义）：
    // sum(i^2, i=0..499) = 499*500*999/6 = 41,541,750
    let source = r#"
#include <stdio.h>
int main() {
    int a[500];
    int i;
    for (i = 0; i < 500; i++) {
        a[i] = i * i;
    }
    long long sum = 0;
    for (i = 0; i < 500; i++) {
        sum = sum + a[i];
    }
    printf("sum=%lld\n", sum);
    return 0;
}
"#;
    assert_parity(source, "sum=41541750\n", 0, true);
}

#[test]
fn jit_parity_long_long_accumulate_100k() {
    // 64 位槽累加（LoadLocalQ/StoreLocalQ 模板语义）：
    // sum(3i+7, i=0..99999) = 3*99999*100000/2 + 700000 = 15,000,550,000（超 int）
    let source = r#"
#include <stdio.h>
int main() {
    long long sum = 0;
    int i;
    for (i = 0; i < 100000; i++) {
        sum = sum + 3LL * i + 7;
    }
    printf("sum=%lld\n", sum);
    return 0;
}
"#;
    assert_parity(source, "sum=15000550000\n", 0, true);
}

#[test]
fn jit_parity_short_loop_swap_fib46_below_threshold() {
    // 循环携带依赖 + 变量交换，但仅 44 次（3..=46）< JIT_THRESHOLD(100)——
    // JIT 预期不触发。本用例锚定两件事：① 阈值下不录制（accel==0）；
    // ② 开关存在不改变语义（parity 照常断言）。fib(46)=1836311903 是
    // int 范围内最大 fib 值（fib(47) 溢出 int），终值可手算锚定。
    let source = r#"
#include <stdio.h>
int main() {
    int a = 1, b = 1, t = 0, i;
    for (i = 3; i <= 46; i++) {
        t = a + b;
        a = b;
        b = t;
    }
    printf("fib46=%d\n", b);
    return 0;
}
"#;
    assert_parity(source, "fib46=1836311903\n", 0, false);
}

#[test]
fn jit_parity_two_var_carried_300() {
    // JIT 生效区间内的双变量循环携带状态：a = 2i，b = sum(2k, k=1..i)。
    // 300 轮后：a=600，b = 2*(1+...+300) = 300*301 = 90300。
    let source = r#"
#include <stdio.h>
int main() {
    int a = 0, b = 0, i;
    for (i = 0; i < 300; i++) {
        a = a + 2;
        b = b + a;
    }
    printf("a=%d b=%d i=%d\n", a, b, i);
    return 0;
}
"#;
    assert_parity(source, "a=600 b=90300 i=300\n", 0, true);
}

#[test]
fn jit_parity_return_value_channel() {
    // 返回码通道：JIT 路径的 return 表达式求值（trace 退出后取栈顶）
    let source = r#"
int main() {
    int sum = 0, i;
    for (i = 0; i < 300; i++) {
        sum = sum + 7;
    }
    return sum % 256;
}
"#;
    // 300*7=2100, 2100 % 256 = 2100 - 8*256 = 2100-2048 = 52
    let jit = run(source, true);
    let interp = run(source, false);
    assert!(jit.steps_accelerated > 0, "JIT 分支未生效");
    assert_eq!(
        (interp.traces_compiled, interp.steps_accelerated),
        (0, 0),
        "禁用开关未生效"
    );
    assert_eq!(jit.ret, 52, "JIT 路径返回码错值：期望 52");
    assert_eq!(interp.ret, 52);
    assert_eq!(jit.ret, interp.ret);
}

/// 附带输出当前加速比参考数据（非门禁，仅 CI 日志可见性）。
#[test]
fn jit_speedup_reference() {
    let source = r#"
#include <stdio.h>
int main() {
    int sum = 0, i;
    for (i = 0; i < 200000; i++) {
        sum = sum + 1;
    }
    printf("%d\n", sum);
    return 0;
}
"#;
    let start = Instant::now();
    let jit = run(source, true);
    let jit_time = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let interp = run(source, false);
    let interp_time = start.elapsed().as_secs_f64();
    assert_eq!(jit.stdout, "200000\n");
    assert_eq!(interp.stdout, jit.stdout);
    println!(
        "[J10-REF] single_loop_200k: JIT={:.4}s (accel={} steps), interp={:.4}s, speedup={:.2}x",
        jit_time,
        jit.steps_accelerated,
        interp_time,
        interp_time / jit_time.max(0.0001)
    );
}
