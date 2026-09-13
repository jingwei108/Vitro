use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// R2：原 flutter_bridge::compile_and_run（全局单例）收口为本地 Session 直驱——
/// 基准对象不变：编译管线 + 统一模式执行环境初始化。
fn compile_and_run(source: &str) {
    use vitro_native::engine::compile_pipeline::{run_multi_file_pipeline, setup_vm};
    use vitro_native::engine::session_ops::{inject_preset_files, reset_runtime_for_step};
    use vitro_native::session::{CompileUnit, Session};
    use vitro_native::unified::engine::UnifiedEngine;
    use vitro_native::vm::core::VitroVM;

    let mut session = Session::default();
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: black_box(source.to_string()),
    }];
    let units = session.compile.compile_units.clone();
    if run_multi_file_pipeline(&mut session, units, false).is_err() {
        return;
    }

    let mut engine = UnifiedEngine::new();
    engine.reset();
    let mut vm = VitroVM::default();
    reset_runtime_for_step(&mut session);
    setup_vm(&mut vm, &session);
    inject_preset_files(&mut vm, &mut session);
    session.runtime.running = true;
    engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());
    session.vm = Some(vm);
}

fn benchmark_bubble_sort(c: &mut Criterion) {
    let source = r#"
int main() {
    int arr[100];
    for (int i = 0; i < 100; i++) arr[i] = 100 - i;
    for (int i = 0; i < 99; i++) {
        for (int j = 0; j < 99 - i; j++) {
            if (arr[j] > arr[j + 1]) {
                int t = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = t;
            }
        }
    }
    return 0;
}
"#;
    c.bench_function("bubble_sort_100", |b| b.iter(|| compile_and_run(source)));
}

fn benchmark_factorial(c: &mut Criterion) {
    let source = r#"
int fact(int n) {
    if (n <= 1) return 1;
    return n * fact(n - 1);
}
int main() {
    return fact(20);
}
"#;
    c.bench_function("factorial_recursive_20", |b| b.iter(|| compile_and_run(source)));
}

criterion_group!(benches, benchmark_bubble_sort, benchmark_factorial);
criterion_main!(benches);
