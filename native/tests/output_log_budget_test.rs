#![allow(clippy::unwrap_used, clippy::expect_used)]

//! U2#3（2026-09-14）：output_chunks 有界化——环形缓冲 + O(1) 长度。
//!
//! 评估 R11：输出分段只增不减，失控 putchar（10M 步上限）可累积数百 MB~GB。
//! 修复 = `OutputLog`（预算内保真、超预算丢最旧 + 截断附注）+
//! 逐通道/总长度 O(1) 缓存（废除全量拼接取长度）。

use vitro_native::engine::compile_pipeline::{run_compile_pipeline, setup_vm};
use vitro_native::session::Session;
use vitro_runtime::{InputMode, OutputLog};

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

/// 红锚：超预算输出必须有界（丢最旧），且尾部保真 + 截断可见。
/// 修复前 output_chunks 是裸 Vec（无界累积），本断言红。
#[test]
fn test_u23_output_log_bounded_keeps_tail() {
    // 10 万次 putchar('A'..) → 100KB 输出；预算压到 16KB
    let source = r#"
int main() {
    for (int i = 0; i < 100000; i++) {
        putchar('A' + (i % 26));
    }
    putchar('!');
    putchar('\n');
    return 0;
}
"#;
    let mut session = make_session(source);
    session.runtime.input_mode = InputMode::Batch;
    std::sync::Arc::make_mut(&mut session.runtime.output).set_budget(16 * 1024);
    let mut vm = vitro_native::vm::core::VitroVM::new();
    setup_vm(&mut vm, &session);
    loop {
        match vm.step(&mut session.as_vm_context()) {
            vitro_native::vm::core::StepResult::Finished => break,
            vitro_native::vm::core::StepResult::Trap => panic!("trap: {}", vm.get_error()),
            _ => {}
        }
    }

    let program = session.runtime.output.len_of(vitro_runtime::OutputKind::Stdout)
        + session.runtime.output.len_of(vitro_runtime::OutputKind::Stderr);
    assert!(
        program <= 16 * 1024,
        "程序输出总量必须有界：{} > 预算 16KB（修复前无界累积 100KB+；引擎附注不占预算）",
        program
    );
    // 尾部保真：最后的 "!\\n" 必须保留在 stdout 尾部
    let stdout = session.runtime.stdout();
    assert!(
        stdout.ends_with("!\n"),
        "环形丢弃只丢最旧，尾部必须保真：{:?}",
        &stdout[stdout.len().saturating_sub(8)..]
    );
    // 截断可见：引擎附注里应有截断提示
    assert!(
        session.runtime.notes().contains("截断"),
        "截断必须有引擎附注可见，notes={:?}",
        session.runtime.notes()
    );
    // 被丢弃的字节有记账
    assert!(session.runtime.output.dropped_bytes() > 0, "丢弃字节计数应 > 0");
}

/// O(1) 长度缓存一致性：各通道 len() 必须与 join 结果逐字节相等
///（合并写入 / 混合通道 / 环形裁剪 / 清空各状态点）。
#[test]
fn test_u23_output_log_len_cache_consistency() {
    let mut log = OutputLog::default();
    log.set_budget(256);

    log.push_stdout("abc");
    log.push_stdout("defg"); // 小段合并
    log.push_stderr("err1\n");
    log.push_note("note1");
    log.push_note("note2");

    assert_eq!(log.len_of(vitro_runtime::OutputKind::Stdout), 7);
    assert_eq!(log.len_of(vitro_runtime::OutputKind::Stderr), 5);
    assert_eq!(log.len_of(vitro_runtime::OutputKind::Note), 12);
    assert_eq!(log.total_bytes(), 7 + 5 + 12);

    // 裁剪触发：灌 1KB stdout
    log.push_stdout("X".repeat(1024));
    assert!(log.dropped_bytes() > 0, "应发生环形丢弃");
    // 截断注记允许一次性小溢出（~200B），故容差 256 而非严格 budget
    assert!(log.total_bytes() <= 256 + 256, "裁剪后总量应有界：{}", log.total_bytes());
    // 一致性：total == join(display) 长度
    let joined = log.join_all();
    assert_eq!(log.total_bytes(), joined.len(), "O(1) 总长必须与拼接结果一致");
    assert_eq!(
        log.len_of(vitro_runtime::OutputKind::Stdout),
        log.join(vitro_runtime::OutputKind::Stdout).len()
    );

    // clear 复位
    log.clear();
    assert_eq!(log.total_bytes(), 0);
    assert_eq!(log.len_of(vitro_runtime::OutputKind::Stdout), 0);
    assert_eq!(log.dropped_bytes(), 0);
}

/// push_stdout 的 O(1) 长度在程序运行后与 stdout() 一致（capi 通道）。
#[test]
fn test_u23_runtime_len_api_matches_join() {
    let source = r#"
#include <stdio.h>
int main() {
    printf("hello ");
    printf("world\n");
    fprintf(stderr, "warn\n");
    return 0;
}
"#;
    let mut session = make_session(source);
    session.runtime.input_mode = InputMode::Batch;
    let mut vm = vitro_native::vm::core::VitroVM::new();
    setup_vm(&mut vm, &session);
    loop {
        match vm.step(&mut session.as_vm_context()) {
            vitro_native::vm::core::StepResult::Finished => break,
            vitro_native::vm::core::StepResult::Trap => panic!("trap: {}", vm.get_error()),
            _ => {}
        }
    }
    assert_eq!(
        session.runtime.output.len_of(vitro_runtime::OutputKind::Stdout),
        session.runtime.stdout().len(),
        "stdout O(1) 长度必须与 join 一致"
    );
    assert_eq!(
        session.runtime.output.len_of(vitro_runtime::OutputKind::Stderr),
        session.runtime.stderr().len()
    );
    assert_eq!(session.runtime.output.total_bytes(), session.runtime.display().len());
}
