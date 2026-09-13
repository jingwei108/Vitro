#![allow(clippy::unwrap_used, clippy::expect_used)]

//! VM JIT 加速比基准（方法学校正版，2026-09-13，裁定 §14.8）。
//!
//! 校正前的两处缺陷：
//! ① `jit_traces_mut().clear()` 只清表不禁用——`ip_hits` 继续累积，回边达阈值后
//!    重新录制，"纯解释"轮实为"前 ~100 次迭代解释 + 之后 JIT"的混合；
//! ② 两分支入口不同（JIT 分支 `execute_run` vs 解释分支裸 `vm.run`），
//!    双变量同时变化，加速比既不能证明 JIT 快也不能证明慢。
//!
//! 校正后：
//! ① 真禁用开关 `set_jit_enabled(false)`（结构性禁用：fast path 不命中、热点检测
//!    与 trace 录制均不触发）；
//! ② 两分支统一入口 `execute_run`，唯一差异是 JIT 开关；
//! ③ 每分支 best-of-N 轮压低单次计时噪声；
//! ④ fail loud 自检（J9：对照前提失效时拒绝给出数字）——解释分支断言
//!    `traces_compiled == 0 && steps_accelerated == 0`；声明 `expect_jit` 的形状
//!    断言 JIT 分支确实产生了加速步。

use vitro_native::engine::compile_pipeline::run_multi_file_pipeline;
use vitro_native::engine::session_ops::{execute_run, reset_runtime};
use vitro_native::session::{CompileUnit, Session};
use std::time::Instant;

const ROUNDS: usize = 5;

struct BenchResult {
    jit_time: f64,
    interp_time: f64,
    ret: i32,
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

fn timed_run(session: &mut Session) -> (f64, i32) {
    // 嵌套形状修复后内层真正执行（修复前外层 trace 穿透内层、步数被跳过），
    // 1k×1k 会撞默认 10M 步上限——基准显式放宽到 50M，语义与 vitro_cli --max-steps 一致。
    if let Some(vm) = session.vm.as_mut() {
        vm.set_max_steps(50_000_000);
    }
    let start = Instant::now();
    let (ret, _) = execute_run(session).expect("run");
    (start.elapsed().as_secs_f64(), ret)
}

/// `expect_jit`：该程序形状是否应产生 JIT 加速步（供 fail-loud 自检）。
/// 递归无循环回边，录制遇 `Call` 即 Abort，加速步为 0 是预期行为。
fn bench_source(source: &str, expect_jit: bool) -> BenchResult {
    // --- JIT 分支（默认开启）---
    let mut best_jit = f64::INFINITY;
    let mut ret = 0;
    let mut jit_result = BenchResult {
        jit_time: 0.0,
        interp_time: 0.0,
        ret: 0,
        traces_compiled: 0,
        steps_accelerated: 0,
    };
    for _ in 0..ROUNDS {
        let mut session = compile(source);
        let (t, r) = timed_run(&mut session);
        best_jit = best_jit.min(t);
        ret = r;
        if let Some(vm) = session.vm.as_ref() {
            jit_result.traces_compiled = vm.jit_stats().traces_compiled;
            jit_result.steps_accelerated = vm.jit_stats().steps_accelerated;
        }
    }
    jit_result.jit_time = best_jit;
    jit_result.ret = ret;
    if expect_jit {
        assert!(
            jit_result.traces_compiled > 0 && jit_result.steps_accelerated > 0,
            "自检失败：JIT 分支未产生任何加速步（traces={}, accel={}）——基准测的是解释 vs 解释，数字无效",
            jit_result.traces_compiled,
            jit_result.steps_accelerated
        );
    }

    // --- 纯解释分支：真禁用开关 + 同一入口 ---
    let mut best_interp = f64::INFINITY;
    let mut ret2 = 0;
    for _ in 0..ROUNDS {
        let mut session = compile(source);
        if let Some(vm) = session.vm.as_mut() {
            vm.set_jit_enabled(false);
        }
        let (t, r) = timed_run(&mut session);
        best_interp = best_interp.min(t);
        ret2 = r;
        if let Some(vm) = session.vm.as_ref() {
            assert_eq!(
                (vm.jit_stats().traces_compiled, vm.jit_stats().steps_accelerated),
                (0, 0),
                "自检失败：禁用开关未生效，解释分支混入了 JIT 步——对照前提失效"
            );
        }
    }
    jit_result.interp_time = best_interp;
    assert_eq!(ret, ret2, "JIT and interpreter should produce same return code");
    jit_result
}

fn report(name: &str, r: &BenchResult) {
    println!(
        "[BENCH] {}: JIT={:.4}s (traces={}, accel={} steps), interp={:.4}s, speedup={:.2}x, ret={}",
        name,
        r.jit_time,
        r.traces_compiled,
        r.steps_accelerated,
        r.interp_time,
        r.interp_time / r.jit_time.max(0.0001),
        r.ret
    );
}

#[test]
fn bench_nested_loop_1k() {
    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 1000; i++) {
        for (int j = 0; j < 1000; j++) {
            sum = sum + 1;
        }
    }
    return sum;
}
"#;
    let r = bench_source(source, true);
    report("nested_loop_1k", &r);
    // 期望值锚：返回码即计算结果（1k*1k=1,000,000）——
    // 旧版只断言 ret==0，程序错值（JIT 穿透期 sum=101000）时照样绿
    assert_eq!(r.ret, 1_000_000);
}

#[test]
fn bench_factorial_recursive_10() {
    let source = r#"
int fact(int n) {
    if (n <= 1) return 1;
    return n * fact(n - 1);
}
int main() {
    return fact(10);
}
"#;
    // 递归无循环回边：JIT 不应产生加速步（录制遇 Call 即 Abort）——本形状测开关开销
    let r = bench_source(source, false);
    report("factorial_recursive_10", &r);
    assert_eq!(r.ret, 3_628_800);
}

#[test]
fn bench_array_sum_200k() {
    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 200000; i++) {
        sum = sum + 1;
    }
    return sum;
}
"#;
    let r = bench_source(source, true);
    report("array_sum_200k", &r);
    assert_eq!(r.ret, 200_000);
}
