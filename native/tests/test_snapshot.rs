#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::engine::compile_pipeline::{run_compile_pipeline, setup_vm};
use vitro_native::session::Session;
use vitro_native::vm::core::VitroVM;

fn make_session(source: &str) -> Session {
    let mut session = Session::default();
    session.compile.compile_units.push(vitro_native::session::CompileUnit {
        filename: "main.c".to_string(),
        source: source.to_string(),
    });
    let mut full_source = source.to_string();
    if !full_source.ends_with('\n') {
        full_source.push('\n');
    }
    run_compile_pipeline(&mut session, &full_source).expect("compile failed");
    session
}

fn setup_vm_for_session(session: &mut Session) -> VitroVM {
    let mut vm = VitroVM::new();
    setup_vm(&mut vm, session);
    vm
}

#[test]
fn test_snapshot_roundtrip() {
    let source = r#"
#include <stdio.h>
int main() {
    int a = 1;
    int b = 2;
    int c = a + b;
    printf("%d", c);
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行若干步
    for _ in 0..5 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    let step_count_before = vm.get_executed_steps();
    let stack_before = vm.get_stack().to_vec();
    let call_stack_before = vm.get_call_stack().to_vec();
    let current_line_before = vm.get_current_line();

    // 创建快照
    let snap = vm.snapshot(&session.as_vm_context());

    // 继续执行若干步，改变状态
    for _ in 0..10 {
        let _ = vm.step(&mut session.as_vm_context());
    }
    assert_ne!(vm.get_executed_steps(), step_count_before);

    // 从快照恢复
    vm.restore(&snap, &mut session.as_vm_context());

    // 验证状态完全一致
    assert_eq!(vm.get_executed_steps(), step_count_before);
    assert_eq!(vm.get_stack(), stack_before.as_slice());
    assert_eq!(vm.get_call_stack(), call_stack_before.as_slice());
    assert_eq!(vm.get_current_line(), current_line_before);
}

#[test]
fn test_snapshot_memory_preserved() {
    let source = r#"
int main() {
    int arr[3] = {10, 20, 30};
    arr[1] = 99;
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行到 main 内部
    for _ in 0..20 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    let snap = vm.snapshot(&session.as_vm_context());

    // 继续执行，修改内存
    for _ in 0..10 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    // 恢复
    vm.restore(&snap, &mut session.as_vm_context());

    // 读取内存验证
    let _mem = vm.memory_ref();
    // 全局区起始 0x1000，arr 应该在全局区（因为是局部变量但放在栈上……
    // 这个测试主要验证 memory 的 copy_from_slice 工作正常，
    // 具体地址取决于 bytecode gen，这里只比较整段内存
    let mem_full = match &snap.memory {
        vitro_native::vm::snapshot::MemoryImage::Full(v) => v.as_slice(),
        _ => panic!("expected full snapshot"),
    };
    assert_eq!(vm.memory_ref(), mem_full);
}

#[test]
fn test_heatmap_collection() {
    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 5; i++) {
        sum = sum + i;
    }
    return sum;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行完整程序
    loop {
        match vm.step(&mut session.as_vm_context()) {
            vitro_native::vm::core::StepResult::Finished => break,
            vitro_native::vm::core::StepResult::Trap => panic!("trap: {}", vm.get_error()),
            _ => {}
        }
    }

    let heatmap = &session.runtime.heatmap;
    let max_count = heatmap.max_count();
    assert!(max_count > 0, "heatmap should have recorded executions");

    // for 循环体应该被多次执行
    let loop_line = 4; // sum = sum + i;
    let loop_count = heatmap.line_counts.get(&loop_line).copied().unwrap_or(0);
    assert!(
        loop_count >= 5,
        "loop body line {} should execute at least 5 times, got {}",
        loop_line,
        loop_count
    );
}

#[test]
fn test_snapshot_restore_safe() {
    // 回归测试：restore() 使用安全拷贝，不应因内存大小不匹配而 panic
    let source = r#"
int main() {
    int x = 1;
    return x;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行若干步后创建快照
    for _ in 0..5 {
        let _ = vm.step(&mut session.as_vm_context());
    }
    let snap = vm.snapshot(&session.as_vm_context());

    // 继续执行改变状态
    for _ in 0..5 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    // 恢复不应 panic
    vm.restore(&snap, &mut session.as_vm_context());

    // 恢复后 VM 应能继续执行而不 trap
    let result = vm.step(&mut session.as_vm_context());
    assert!(
        !matches!(result, vitro_native::vm::core::StepResult::Trap),
        "VM should continue after restore, got trap: {}",
        vm.get_error()
    );
}

#[test]
fn test_snapshot_continue_execution_equals_direct_run() {
    // 核心一致性测试：snapshot → restore → 继续执行 的结果必须与 直接执行 完全一致
    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 10; i++) {
        sum = sum + i;
    }
    return sum;
}
"#;
    // 路径 A：直接执行到结束
    let mut session_a = make_session(source);
    let mut vm_a = setup_vm_for_session(&mut session_a);
    loop {
        match vm_a.step(&mut session_a.as_vm_context()) {
            vitro_native::vm::core::StepResult::Finished => break,
            vitro_native::vm::core::StepResult::Trap => panic!("path A trap: {}", vm_a.get_error()),
            _ => {}
        }
    }
    let final_steps_a = vm_a.get_executed_steps();
    let final_line_a = vm_a.get_current_line();
    let final_stack_a = vm_a.get_stack().to_vec();

    // 路径 B：执行 15 步 → snapshot → restore → 继续执行到结束
    let mut session_b = make_session(source);
    let mut vm_b = setup_vm_for_session(&mut session_b);
    for _ in 0..15 {
        let _ = vm_b.step(&mut session_b.as_vm_context());
    }
    let snap = vm_b.snapshot(&session_b.as_vm_context());

    // 修改状态（模拟执行）
    for _ in 0..5 {
        let _ = vm_b.step(&mut session_b.as_vm_context());
    }

    // 从快照恢复
    vm_b.restore(&snap, &mut session_b.as_vm_context());

    // 继续执行到结束
    loop {
        match vm_b.step(&mut session_b.as_vm_context()) {
            vitro_native::vm::core::StepResult::Finished => break,
            vitro_native::vm::core::StepResult::Trap => panic!("path B trap: {}", vm_b.get_error()),
            _ => {}
        }
    }

    assert_eq!(vm_b.get_executed_steps(), final_steps_a, "step count mismatch");
    assert_eq!(vm_b.get_current_line(), final_line_a, "final line mismatch");
    assert_eq!(vm_b.get_stack(), final_stack_a.as_slice(), "final stack mismatch");
}

#[test]
fn test_f64_constants_cleared_on_recompile() {
    // 回归测试：复编译时 f64_constants 必须被清空
    let source1 = r#"
#include <stdio.h>
int main() {
    double a = 3.14159265358979;
    printf("%.14f", a);
    return 0;
}
"#;
    let mut session = make_session(source1);
    let f64_count_after_first = session.compile.f64_constants.len();
    assert!(f64_count_after_first > 0, "First compile should produce f64 constants");

    // 重新编译不含 double 的代码
    let source2 = r#"
int main() {
    return 42;
}
"#;
    let mut full_source = source2.to_string();
    if !full_source.ends_with('\n') {
        full_source.push('\n');
    }
    run_compile_pipeline(&mut session, &full_source).expect("second compile failed");

    // 验证 f64_constants 被清空
    assert!(
        session.compile.f64_constants.is_empty(),
        "f64_constants should be cleared after recompile, but still has {} entries",
        session.compile.f64_constants.len()
    );
}

#[test]
fn test_incremental_snapshot_size() {
    let source = r#"
int main() {
    int arr[100] = {0};
    arr[0] = 1;
    arr[50] = 2;
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行到 main 内部，产生一些内存写入
    for _ in 0..30 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    // 全量快照
    let full = vm.snapshot(&session.as_vm_context());
    let full_size = full.memory.byte_size();
    assert_eq!(full_size, 1024 * 1024, "Full snapshot should be 1MB");

    // 增量快照（假设 base_step=0）
    let inc = vm.snapshot_incremental(&session.as_vm_context(), 0);
    let inc_size = inc.memory.byte_size();

    // 增量应该远小于 1MB（通常只写入了栈上的 arr 和少量局部变量）
    assert!(
        inc_size < 100 * 1024,
        "Incremental snapshot should be much smaller than 1MB, got {} bytes",
        inc_size
    );

    // 验证增量可以正确恢复
    // 先继续执行改变内存
    for _ in 0..10 {
        let _ = vm.step(&mut session.as_vm_context());
    }
    let mem_before_restore = vm.memory_ref().to_vec();

    // 恢复增量快照（此时 vm.memory 应该被增量覆盖）
    // 注意：增量快照假设当前 memory 已经是 base 状态，这里直接用全量做 base 测试
    vm.restore(&full, &mut session.as_vm_context()); // 先恢复到全量基准
    vm.restore(&inc, &mut session.as_vm_context()); // 再应用增量

    let mem_after_restore = vm.memory_ref().to_vec();
    assert_eq!(
        mem_before_restore, mem_after_restore,
        "Restore from incremental should match original state"
    );
}

#[test]
fn test_checkpoint_manager_incremental_chain() {
    use vitro_vm::snapshot::CheckpointManager;

    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 5; i++) {
        sum += i;
    }
    return sum;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    let mut cp = CheckpointManager::new(10);
    cp.full_every = 3; // 每 3 个检查点一个全量，便于测试

    // 模拟执行并保存检查点
    for step in 0..35 {
        if step % 5 == 0 {
            cp.save(step, &mut vm, &mut session.as_vm_context());
        }
        let _ = vm.step(&mut session.as_vm_context());
    }

    // 应该保存了 7 个检查点（0,5,10,15,20,25,30）
    assert!(!cp.is_empty());

    // 验证 nearest 能正确重建增量快照
    let (step, reconstructed) = cp.nearest(28).expect("should find nearest checkpoint");
    assert_eq!(step, 25);

    // 重建后的快照必须是 Full
    match &reconstructed.memory {
        vitro_native::vm::snapshot::MemoryImage::Full(_) => {}
        _ => panic!("reconstructed snapshot should be Full"),
    }

    // 验证恢复后 VM 能继续执行
    vm.restore(&reconstructed, &mut session.as_vm_context());
    let mut steps_after = 0;
    for _ in step..35 {
        if matches!(
            vm.step(&mut session.as_vm_context()),
            vitro_native::vm::core::StepResult::Finished
        ) {
            break;
        }
        steps_after += 1;
    }
    assert!(
        steps_after > 0,
        "VM should be able to continue after restore from reconstructed checkpoint"
    );
}

#[test]
fn test_smart_checkpoint_triggers() {
    use vitro_vm::snapshot::CheckpointManager;

    let mut cp = CheckpointManager::new(20);
    cp.smart_mode = true;

    // 固定间隔保底
    assert!(cp.should_checkpoint(0, ""));
    assert!(cp.should_checkpoint(20, ""));
    assert!(!cp.should_checkpoint(5, ""));

    // 模拟保存步 0 的检查点，使后续智能判断能感知到上一个检查点位置
    cp.checkpoints.push((
        0,
        vitro_vm::snapshot::VMSnapshot {
            memory: vitro_vm::snapshot::MemoryImage::Full(vec![0; 1024 * 1024]),
            stack: Vec::new(),
            call_stack: Vec::new(),
            ip: 0,
            mem_stack_top: 0,
            step_count: 0,
            current_line: 0,
            finished: false,
            exit_code: 0,
            error: String::new(),
            paused: false,
            cancelled: false,
            step_event_hit: false,
            last_snapshot_step: 0,
            snapshot_vars: std::collections::HashMap::new(),
            qsort_depth: 0,
            vis_event_queue: Vec::new(),
            breakpoints: std::collections::HashSet::new(),
            global_count: 0,
            freed_logs: std::collections::BTreeMap::new(),
            runtime: vitro_vm::snapshot::RuntimeSnapshot {
                output_chunks: Vec::new(),
                trace: Vec::new(),
                current_line: 0,
                input_index: 0,
                input_char_offset: 0,
                waiting_input: false,
                rand_seed: 0,
                vis_event_cache: Vec::new(),
                ungetc_char: None,
            },
            memory_state: vitro_vm::snapshot::MemorySnapshot {
                regions: Vec::new(),
                free_list: Vec::new(),
                quarantine: std::collections::VecDeque::new(),
                quarantine_bytes: 0,
                quarantine_budget: vitro_runtime::memory_state::DEFAULT_QUARANTINE_BUDGET,
                heap_base: vitro_runtime::HEAP_START,
                heap_offset: 0,
                alloc_counter: 0,
            },
        },
    ));

    // 智能触发：函数调用
    assert!(cp.should_checkpoint(21, "调用 printf"));
    cp.checkpoints.push((21, cp.checkpoints[0].1.clone()));

    // 密集保护：距离上一个检查点太近时不触发
    assert!(!cp.should_checkpoint(22, "调用 scanf")); // 只离 21 差 1 步

    // 智能触发：返回
    assert!(cp.should_checkpoint(30, "返回"));

    // 智能触发：内存分配
    assert!(cp.should_checkpoint(35, "内存分配"));

    // 智能触发：交换
    assert!(cp.should_checkpoint(40, "交换 arr[0]↔arr[1]"));
}

#[test]
fn test_snapshot_into_equivalence_and_reuse() {
    let source = r#"
#include <stdio.h>
int main() {
    int arr[10] = {0};
    arr[3] = 42;
    printf("%d", arr[3]);
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    // 执行若干步，产生栈/内存状态
    for _ in 0..20 {
        let _ = vm.step(&mut session.as_vm_context());
    }

    // 全量快照作为 ground truth
    let full_snap = vm.snapshot(&session.as_vm_context());

    // 用 snapshot_into 复写到另一个 VMSnapshot
    let mut reused_snap = vm.snapshot(&session.as_vm_context());
    // 先破坏 reused_snap 的内存，确保 copy 真正发生
    if let vitro_native::vm::snapshot::MemoryImage::Full(buf) = &mut reused_snap.memory {
        buf.fill(0xAA);
    }
    vm.snapshot_into(&session.as_vm_context(), &mut reused_snap);

    // 两者必须等价
    assert_eq!(reused_snap.step_count, full_snap.step_count);
    assert_eq!(reused_snap.ip, full_snap.ip);
    assert_eq!(reused_snap.stack, full_snap.stack);
    assert_eq!(reused_snap.call_stack, full_snap.call_stack);
    assert_eq!(
        reused_snap.memory.byte_size(),
        full_snap.memory.byte_size(),
        "snapshot_into memory size should match full snapshot"
    );

    // 分别恢复到两个独立 VM 并验证状态一致
    let mut session_a = make_session(source);
    let mut vm_a = setup_vm_for_session(&mut session_a);
    vm_a.restore(&full_snap, &mut session_a.as_vm_context());

    let mut session_b = make_session(source);
    let mut vm_b = setup_vm_for_session(&mut session_b);
    vm_b.restore(&reused_snap, &mut session_b.as_vm_context());

    assert_eq!(vm_a.get_executed_steps(), vm_b.get_executed_steps());
    assert_eq!(vm_a.get_stack(), vm_b.get_stack());
    assert_eq!(vm_a.get_call_stack(), vm_b.get_call_stack());
    assert_eq!(vm_a.memory_ref(), vm_b.memory_ref());
    assert_eq!(session_a.runtime.output_chunks, session_b.runtime.output_chunks);
}

/// U2#5（2026-09-14）：>50 检查点触发淘汰后，回退 seek 的语义正确性——
/// 悬空 Delta 在旧实现下会让 nearest 把脏页叠到更早的 Full 上（**静默旧值**，
/// 无报错的最坏调试器缺陷）。与"不淘汰参考实现"（max_checkpoints 巨大）对照：
/// 同步 seek 后变量值必须一致。
#[test]
fn test_u25_seek_after_eviction_matches_reference() {
    let source = r#"
#include <stdio.h>
int main() {
    int x = 0;
    int sum = 0;
    for (int i = 0; i < 400; i++) {
        x = x + 3;
        sum = sum + x;
    }
    printf("%d %d\n", x, sum);
    return 0;
}
"#;
    // run_batch 跑满（智能检查点在循环边界高频触发，>50 → 淘汰发生），
    // 记录每步的 (step, x, sum)；随后回退 seek 到若干步并与记录对照。
    use vitro_native::unified::engine::UnifiedEngine;

    let collect = |max_checkpoints: usize| -> Vec<(i32, i64, i64)> {
        let mut session = make_session(source);
        let mut engine = UnifiedEngine::with_max_steps(1_000_000);
        engine.checkpoints.max_checkpoints = max_checkpoints;
        engine.reset();
        let mut vm = setup_vm_for_session(&mut session);
        session.runtime.running = true;
        engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());
        let mut trace = Vec::new();
        loop {
            let r = engine.run_batch(&mut vm, &mut session, 1).expect("run_batch");
            // 每步从 frame_cache 尾帧读 x/sum
            if let Some(p) = r.payloads.last() {
                let x = p
                    .local_vars
                    .iter()
                    .find(|v| v.name == "x")
                    .and_then(|v| v.value.parse::<i64>().ok());
                let sum = p
                    .local_vars
                    .iter()
                    .find(|v| v.name == "sum")
                    .and_then(|v| v.value.parse::<i64>().ok());
                if let (Some(x), Some(sum)) = (x, sum) {
                    trace.push((p.step_index, x, sum));
                }
            }
            let fin = r.finished;
            drop(r);
            if fin {
                break;
            }
        }
        trace
    };

    let evicted = collect(50); // 生产配置：淘汰触发
    let reference = collect(10_000); // 参考实现：不淘汰

    assert!(evicted.len() > 600, "应收集足够步（实际 {}）", evicted.len());
    assert!(!reference.is_empty());
    // 两侧终值一致（程序语义相同）
    let (.., ex, esum) = evicted.last().unwrap();
    let (.., rx, rsum) = reference.last().unwrap();
    assert_eq!((ex, esum), (rx, rsum), "终值必须一致（x={}, sum={} vs {}, {}）", ex, esum, rx, rsum);

    // 中段抽点对照（淘汰窗口内外）
    for probe in [evicted.len() / 4, evicted.len() / 2, evicted.len() * 3 / 4] {
        let e = evicted[probe];
        let r = reference[probe];
        assert_eq!((e.1, e.2), (r.1, r.2), "步 {} 的 (x, sum) 淘汰侧 ({}, {}) != 参考侧 ({}, {})", e.0, e.1, e.2, r.1, r.2);
    }
}

/// U2#1（2026-09-14）：远距 seek 的重放循环内帧缓存必须滚动截断——
/// 修复前重放从不 trim（run_batch 每批后有、seek 循环没有），重放期间
/// 帧缓存无界累积 = 裁定实测的 seek 峰值 1.2~2.3KB/步（63.6GB 事故的
/// 瞬时形态）。结构断言：远距 seek 后窗口 ≤ frame_cache_window_size + 1。
#[test]
fn test_u21_seek_replay_window_bounded() {
    use vitro_native::unified::engine::UnifiedEngine;

    let source = r#"
#include <stdio.h>
int main() {
    int x = 0;
    for (int i = 0; i < 3000; i++) {
        x = x + 1;
    }
    printf("%d\n", x);
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut engine = UnifiedEngine::with_max_steps(1_000_000);
    engine.reset();
    let mut vm = setup_vm_for_session(&mut session);
    session.runtime.running = true;
    engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());

    // 无界形态 = 检查点稀疏的长重放：step_begin 后只 save(0)，正向跑少量步
    //（窗口小、无中间检查点）→ 越窗远距 seek → 从 checkpoint 0 重放全长。
    for _ in 0..10 {
        let r = engine.run_batch(&mut vm, &mut session, 1).expect("run_batch");
        if r.finished {
            break;
        }
    }
    let r = engine.seek_to(2990, &mut vm, &mut session);
    assert!(r.success, "seek 应成功：{:?}", r.error);
    let window = engine.frame_cache.len();
    assert!(
        window <= engine.frame_cache_window() + 1,
        "远距 seek 重放期间帧窗口必须滚动有界：len={} > window={}（修复前无界累积 = 重放距离全长，即裁定实测的 seek 峰值 1.2~2.3KB/步）",
        window,
        engine.frame_cache_window()
    );
    // 语义：重放完成后窗口尾 = 目标步（trim 只截头部）
    let tail = engine.frame_cache_start_step + engine.frame_cache.len() as i32 - 1;
    assert_eq!(tail, 2990, "窗口尾帧应为目标步（起点 {}，len {}）", engine.frame_cache_start_step, engine.frame_cache.len());
    // 语义：目标帧可取回（get_payloads 公开口径）
    let visible = engine.get_payloads(2989, 2990);
    assert_eq!(visible.len(), 1, "目标步必须可索引（半开区间；窗口起点 {}，len {}）", engine.frame_cache_start_step, engine.frame_cache.len());
}

/// U2#11 前半：seek 重放步数受引擎 max_steps 预算约束——修复前循环内
/// 只有 is_cancelled（对比 run_batch 有步数判断），本断言红。
#[test]
fn test_u21_seek_replay_respects_max_steps() {
    use vitro_native::unified::engine::UnifiedEngine;

    let source = r#"
#include <stdio.h>
int main() {
    int x = 0;
    for (int i = 0; i < 3000; i++) { x = x + 1; }
    printf("%d\n", x);
    return 0;
}
"#;
    let mut session = make_session(source);
    let mut engine = UnifiedEngine::with_max_steps(100); // 预算远小于程序长度
    engine.reset();
    let mut vm = setup_vm_for_session(&mut session);
    session.runtime.running = true;
    engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());

    let r = engine.seek_to(2990, &mut vm, &mut session);
    assert!(!r.success, "超预算的远距重放必须失败");
    assert!(r.error.unwrap().contains("重放步数超过限制"), "错误应说明预算");
}
