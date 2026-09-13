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
fn test_jit_trace_bulk_accelerates_loop() {
    // 循环体足够简单，能完全被 JIT 模板覆盖；
    // 循环 1000 次，前 JIT_THRESHOLD 次逐步执行后触发 trace 录制，
    // 之后应通过 execute_trace_bulk 批量执行多轮迭代。
    let source = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 1000; i++) {
        sum += i;
    }
    return sum;
}
"#;
    let mut session = make_session(source);
    let mut vm = setup_vm_for_session(&mut session);

    let exit_code = vm.run(&mut session.as_vm_context());

    // 1+2+...+999 = 499500，取低 8/16/32 位（返回 int）
    assert_eq!(exit_code, 499500);

    let stats = vm.jit_stats();
    println!("JIT stats: {:?}", stats);
    assert!(stats.traces_compiled > 0, "至少应编译一条 trace，stats={:?}", stats);
    // 加速步数应远大于单次 trace 长度（否则说明 bulk 只执行了一轮）
    assert!(stats.steps_accelerated > 100, "JIT 应加速多轮循环迭代，stats={:?}", stats);
}
