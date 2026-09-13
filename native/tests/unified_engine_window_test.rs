#![allow(clippy::unwrap_used, clippy::expect_used)]

//! UnifiedEngine frame_cache 滑动窗口公共 API 测试。
//!
//! 这些测试不依赖真实 VM/Session，仅验证窗口化后的公共状态与方法行为。

use vitro_native::unified::engine::UnifiedEngine;
use vitro_native::unified::types::StepPayload;

fn dummy_payload(step: i32) -> StepPayload {
    StepPayload {
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
    }
}

#[test]
fn test_max_collected_step_with_window_start() {
    let mut engine = UnifiedEngine::new();
    // 手动模拟窗口起点为 100，缓存 5 帧（100..=104）
    engine.frame_cache_start_step = 100;
    engine.frame_cache = (100..=104).map(dummy_payload).collect();
    assert_eq!(engine.frame_cache_start_step(), 100);
    assert_eq!(engine.max_collected_step(), 104);
}

#[test]
fn test_get_payloads_returns_visible_range_only() {
    let mut engine = UnifiedEngine::new();
    engine.frame_cache_start_step = 50;
    engine.frame_cache = (50..=59).map(dummy_payload).collect();

    // 完全在窗口内
    let payloads = engine.get_payloads(52, 55);
    assert_eq!(payloads.len(), 3);
    assert_eq!(payloads[0].step_index, 52);
    assert_eq!(payloads[2].step_index, 54);

    // 部分在窗口外
    let payloads = engine.get_payloads(45, 53);
    assert_eq!(payloads.len(), 3);
    assert_eq!(payloads[0].step_index, 50);

    // 完全在窗口外（之前）
    let payloads = engine.get_payloads(10, 20);
    assert!(payloads.is_empty());

    // 完全在窗口外（之后）
    let payloads = engine.get_payloads(100, 110);
    assert!(payloads.is_empty());
}

#[test]
fn test_get_payloads_with_negative_start() {
    let mut engine = UnifiedEngine::new();
    engine.frame_cache_start_step = 5;
    engine.frame_cache = (5..=9).map(dummy_payload).collect();

    let payloads = engine.get_payloads(-10, 7);
    assert_eq!(payloads.len(), 2);
    assert_eq!(payloads[0].step_index, 5);
    assert_eq!(payloads[1].step_index, 6);
}

#[test]
fn test_reset_clears_window_start() {
    let mut engine = UnifiedEngine::new();
    engine.frame_cache_start_step = 200;
    engine.frame_cache = (200..=210).map(dummy_payload).collect();
    engine.reset();
    assert_eq!(engine.frame_cache_start_step(), 0);
    assert!(engine.frame_cache.is_empty());
    assert_eq!(engine.max_collected_step(), -1);
}
