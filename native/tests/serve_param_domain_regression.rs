#![allow(clippy::unwrap_used, clippy::expect_used)]

//! W0-2 止血批次的红→绿回归锚（R-2026-09-01/02/03，裁定 §10）。
//!
//! 三个 panic 的最小复现（修复前实测 exit 101 / 会话死亡）：
//! - R-2026-09-01/02：`seek` 越过程序末尾（10 步程序 + `seek(50000)`/`seek(3000)`）
//!   → `engine.rs` `finish_replay_window` 的 `split_off(discard)`：理论 discard
//!   远大于缓存长度。修复 = `discard.min(len)` + 起点按实际推进量记账。
//! - R-2026-09-03：`payload.get` 负 `end` → `get_payloads` 的
//!   `((-1).min(cache_end) as usize)` 绕回 `usize::MAX` 后切片 panic。
//!   修复 = 先钳到 `[start_step, cache_end]` 再转 usize。
//!
//! 验收语义：**越界/负参 seek 返回结果帧（success 与否由引擎定），进程与会话存活，
//! 后续操作继续可用**——一条畸形请求/参数不得杀死会话。
//! 端到端防线：`scripts/core_asset_verdict/interaction_probe`（Go，J9 有牙，
//! 修复后 1800 请求零死亡全绿）；本文件把最小复现固化进 cargo test。

use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;

fn setup_10_step_program() -> Session {
    let mut session = Session::default();
    session.compile.compile_units.push(CompileUnit {
        filename: "main.c".to_string(),
        source: r#"
int main() {
    int s = 0;
    for (int i = 0; i < 3; i++) { s = s + i; }
    return s;
}
"#
        .to_string(),
    });
    let compiled = session_api::compile(&mut session);
    assert!(compiled["ok"].as_bool().unwrap_or(false), "编译失败：{compiled}");
    assert_eq!(session_api::step_begin(&mut session), 0);
    session
}

#[test]
fn seek_far_past_program_end_does_not_panic() {
    // R-2026-09-01：修复前 engine.rs finish_replay_window split_off panic
    let mut session = setup_10_step_program();
    let result = session_api::seek(&mut session, 50000);
    // 只断言"不 panic 且返回结果"；success 与否是引擎语义（此处程序已结束，
    // 无检查点可越窗重放时返回 success=false 是合法行为）
    let _ = result.expect("seek 应返回结果帧而非 panic");
    // 会话存活：后续 step.next 仍可用
    let step = session_api::step_next(&mut session);
    assert!(step.is_ok(), "seek 越界后 step.next 仍应可用：{:?}", step.err());
}

#[test]
fn seek_moderately_past_end_does_not_panic() {
    // R-2026-09-02：修复前 split index (1001) > len (10) panic
    let mut session = setup_10_step_program();
    let result = session_api::seek(&mut session, 3000);
    let _ = result.expect("seek 应返回结果帧而非 panic");
    assert!(session_api::step_next(&mut session).is_ok());
}

#[test]
fn seek_past_end_then_seek_back_still_works() {
    // 越界 seek 后再 seek 回合法步——修复后窗口记账必须自洽
    let mut session = setup_10_step_program();
    let _ = session_api::seek(&mut session, 50000).expect("不 panic");
    let back = session_api::seek(&mut session, 1);
    let _ = back.expect("回 seek 合法步不 panic");
}

#[test]
fn repeated_far_seeks_do_not_accumulate_bad_state() {
    // 多次远距 seek（对应探针 A 段随机序列形态）
    let mut session = setup_10_step_program();
    for target in [4999i32, 50000, 1000, 50000, 4999] {
        let r = session_api::seek(&mut session, target);
        assert!(r.is_ok(), "seek({target}) 不应 panic：{:?}", r.err());
    }
}

#[test]
fn get_payloads_negative_end_returns_empty() {
    // R-2026-09-03 单元级：负 end 直接钳制，返回空而非 panic
    use vitro_native::unified::engine::UnifiedEngine;
    use vitro_native::unified::types::StepPayload;

    let mut engine = UnifiedEngine::new();
    let dummy = |step: i32| StepPayload {
        step_index: step,
        code_line: step + 1,
        func_name: "main".to_string(),
        semantic_label: String::new(),
        algorithm_step: None,
        local_vars: Vec::new(),
        call_stack: Vec::new(),
        vis_events: Vec::new(),
        heatmap_line: step + 1,
        heatmap_count: 1,
        accessed_vars: Vec::new(),
        array_snapshots: Vec::new(),
        pointer_snapshots: Vec::new(),
        root_cause_hint: None,
    };
    engine.frame_cache = (50..=59).map(dummy).collect();
    engine.frame_cache_start_step = 50;

    // 修复前：(-1).min(cache_end) = -1 → as usize 绕回 usize::MAX → 切片 panic
    let out = engine.get_payloads(0, -1);
    assert!(out.is_empty(), "负 end 应返回空：{} 帧", out.len());

    let out = engine.get_payloads(-5, 3);
    assert!(out.is_empty(), "负 start 应返回空");

    // 合法窗口不受影响（同 unified_engine_window_test 的既有口径）
    let out = engine.get_payloads(52, 55);
    assert_eq!(out.len(), 3);
}

/// §6-9（v4 清单，2026-09-14）：程序结束后 `step.next` 不得重复投递末帧——
/// spec 附录 A 冻结不变量"每真实步恰投递一次"的终态延伸。
/// 修复前实测：结束后每次调用都重放末帧（binary 模板 90 步，call#92+ 持续
/// 返回 [s=89]），且重复帧同步污染 frame_cache（payload.get 窗口混入重放帧）。
/// 修复 = `UnifiedEngine::is_finished` 短路：终结后再调用返回空 payloads +
/// finished=true（末帧已在结束轮发布过）。
#[test]
fn test_step_next_after_finish_no_tail_replay() {
    let src = "int main() { int s = 0; for (int i = 0; i < 3; i++) { s += i; } return s; }";
    let mut session = Session::default();
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: src.to_string(),
    }];
    let r = session_api::compile(&mut session);
    assert_eq!(r.get("ok"), Some(&serde_json::json!(true)), "编译失败: {:?}", r.get("diagnostics"));
    let _ = session_api::run(&mut session);
    assert_eq!(session_api::step_begin(&mut session), 0);

    // 推进到终结：收集全部已发布帧的 step_index（含结束冲刷帧）
    let mut published: Vec<i64> = Vec::new();
    let mut finished = false;
    for _ in 0..200 {
        let v = session_api::step_next(&mut session).expect("step_next 不应失败");
        for p in v.get("payloads").and_then(|p| p.as_array()).unwrap_or(&vec![]) {
            if let Some(si) = p.get("step_index").and_then(|x| x.as_i64()) {
                published.push(si);
            }
        }
        if v.get("finished") == Some(&serde_json::json!(true)) {
            finished = true;
            break;
        }
    }
    assert!(finished, "200 步内应到达终结");
    let total = published.len();

    // 终结后再调用 5 次：payloads 必须为空、finished 保持 true——
    // 任何重放的末帧都违反"每真实步恰投递一次"
    for i in 0..5 {
        let v = session_api::step_next(&mut session).expect("终结后调用不应失败");
        let n = v.get("payloads").and_then(|p| p.as_array()).map(|a| a.len()).unwrap_or(0);
        assert_eq!(n, 0, "终结后第 {} 次调用不得重放帧（发布序列 {:?}）", i + 1, published);
        assert_eq!(v.get("finished"), Some(&serde_json::json!(true)), "finished 应保持");
    }
    // 序列本身严格递增（无重复投递）
    for w in published.windows(2) {
        assert_eq!(w[1], w[0] + 1, "发布序列必须严格递增: {:?}", published);
    }
    assert!(total >= 3, "程序应至少发布若干帧，实际 {}", total);
}
