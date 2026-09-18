//! Vitro CLI — 直接运行 Rust 后端编译器/VM 的命令行调试工具
//!
//! 用法:
//!   cargo run --bin vitro_cli -- compile <file.c>
//!   cargo run --bin vitro_cli -- run    <file.c> [-i input.txt]
//!   cargo run --bin vitro_cli -- step   <file.c> [-i input.txt]
//!   cargo run --bin vitro_cli -- unified <file.c> [-i input.txt]

use std::env;
use std::fs;
use std::io::{self, BufRead, Read, Write};

use vitro_native::session::{CompileUnit, InputMode, Session};
use vitro_native::session_api;

fn print_usage() {
    eprintln!("Vitro CLI — C 语言教学 IDE 后端调试工具");
    eprintln!();
    eprintln!("用法:");
    eprintln!("  vitro_cli compile <file.c>           编译并显示诊断信息");
    eprintln!("  vitro_cli run    <file.c> [-i <in>] [-- <arg>...]  编译并全速运行（-- 后参数传给 main）");
    eprintln!("  vitro_cli step   <file.c> [-i <in>]  交互式单步调试");
    eprintln!("  vitro_cli unified <file.c> [-i <in>] [--max-steps <n>] 统一模式（时间旅行）执行并摘要");
    eprintln!("  vitro_cli export <file1.c> [file2.c ...] -o <out.json> [--builtin-libc]  预编译为字节码产物");
    eprintln!("  vitro_cli serve                      JSON-lines 会话模式（stdin 读请求 / stdout 写响应）");
    eprintln!();
    eprintln!("特殊文件名:");
    eprintln!("  -          从标准输入读取源代码（如 echo '...' | vitro_cli run -）");
    eprintln!();
    eprintln!("选项:");
    eprintln!("  -i <file>   从文件读取标准输入（多行输入）");
    eprintln!("  -o <file>   指定输出文件（仅 export 命令需要）");
    eprintln!("  --builtin-libc  库模式导出（export 命令）：不混入已有 Bytecode Libc 符号");
    eprintln!();
    eprintln!("serve 会话模式（每行一个 JSON 请求，响应与请求 id 关联）：");
    eprintln!("  {{\"id\":1,\"method\":\"compile\",\"params\":{{\"source\":\"int main(){{return 0;}}\"}}}}");
    eprintln!("  {{\"id\":2,\"method\":\"run\"}} / output.delta / step.begin / step.next / payload.get");
    eprintln!("  {{\"id\":3,\"method\":\"seek\",\"params\":{{\"step\":10}}}} / breakpoints.set / memory.regions / input.feed");
    eprintln!("  {{\"id\":4,\"method\":\"session.reset\"}} / config.get / config.set / shutdown");
    eprintln!("  {{\"id\":5,\"method\":\"capabilities\"}} / error_catalog / semantic_labels / contracts");
    eprintln!("  会话拓扑：单 serve 进程 = 单活跃会话（session.create 为清空重建，无并发句柄）");
}

fn read_source(path: &str) -> String {
    if path == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
            eprintln!("错误: 无法读取标准输入: {}", e);
            std::process::exit(1);
        });
        buf
    } else {
        fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("错误: 无法读取文件 '{}': {}", path, e);
            std::process::exit(1);
        })
    }
}

fn read_input_file(path: &str) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("错误: 无法读取输入文件 '{}': {}", path, e);
        std::process::exit(1);
    });
    // 保留换行（与 capi / FRB / serve 同一口径）：`getchar()` 需读到 '\n'
    vitro_native::session::RuntimeState::split_stdin(&text)
}

/// 编译单个源文件到会话（R2：本地 Session 直驱，无全局单例）。
/// 输出格式与 flutter_bridge 时代逐字节一致。
fn compile_file(session: &mut Session, path: &str, source: &str) -> bool {
    let filename = if path == "-" {
        "main.c".to_string()
    } else {
        path.to_string()
    };
    session.compile.compile_units = vec![CompileUnit {
        filename,
        source: source.to_string(),
    }];
    let units = session.compile.compile_units.clone();
    let ok = vitro_native::engine::compile_pipeline::run_multi_file_pipeline(session, units, false).is_ok();

    let diagnostics = &session.compile.diagnostics;
    if !diagnostics.is_empty() {
        println!("=== 诊断信息 ===");
        for d in diagnostics {
            let severity = match d.severity {
                0 => "错误",
                1 => "警告",
                2 => "提示",
                _ => "信息",
            };
            println!("[{}] {}:{}  {} ({}{})", severity, d.line, d.column, d.message, session_api::severity_prefix(d.severity), d.error_code);
            if !d.fix_suggestion.is_empty() {
                println!("    建议: {}", d.fix_suggestion);
            }
        }
    }

    if !ok {
        eprintln!("\n编译失败。");
        false
    } else {
        println!("\n编译成功。");
        if !session.compile.algorithm_matches.is_empty() {
            println!("检测到算法:");
            for m in &session.compile.algorithm_matches {
                println!("  • {} (置信度: {}%)", m.display_name, m.confidence);
            }
        }
        true
    }
}

/// `compile` 子命令：编译失败必须以**非零退出码**结束。
///
/// 此前丢弃了 `compile_file` 的返回值，`vitro_cli compile bad.c` 会带着诊断信息
/// 退出 0 —— CI 脚本 / headless 消费方（SharpTutor）据此判断"编译通过"，是静默失败。
fn cmd_compile(path: &str) {
    let source = read_source(path);
    let mut session = Session::default();
    if !compile_file(&mut session, path, &source) {
        std::process::exit(1);
    }
}

fn cmd_run(path: &str, input_lines: Vec<String>, argv: Vec<String>) {
    let source = read_source(path);
    let mut session = Session::default();
    if !compile_file(&mut session, path, &source) {
        std::process::exit(1);
    }

    // 注入输入（保留换行口径已在 read_input_file 中保证；waiting_input 复位与
    // 旧 provide_input_line 一致）
    for line in input_lines {
        session.runtime.input_lines.push(line);
    }
    session.runtime.waiting_input = false;
    // CLI run 是 headless 批处理路径（stdin 一次性由 -i 给足）：输入耗尽即 EOF，
    // 不进入交互等待。否则 `while (scanf(...) != EOF)` 这类 C 第一课习语会永久挂起
    // （见下游需求清单 A1）。
    session.runtime.input_mode = InputMode::Batch;

    session.runtime.argc = argv.len() as i32;
    session.runtime.argv = argv;

    use vitro_native::engine::session_ops::execute_run;
    let result = if !session.compile.compiled {
        Err("程序尚未编译。请先编译代码。".to_string())
    } else {
        execute_run(&mut session)
    };
    println!("\n=== 运行输出 ===");
    println!("{}", session.runtime.output());
    match result {
        Ok((_, waiting)) => {
            if waiting {
                println!("[程序等待输入，但输入已耗尽]");
            }
        }
        Err(e) => {
            eprintln!("运行错误: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_step(path: &str, input_lines: Vec<String>) {
    let source = read_source(path);
    let mut session = Session::default();
    if !compile_file(&mut session, path, &source) {
        std::process::exit(1);
    }

    // 注入输入
    for line in input_lines {
        session.runtime.input_lines.push(line);
    }
    session.runtime.waiting_input = false;

    println!("=== 交互式单步调试 ===");
    println!("命令: [Enter]=下一步, p=打印变量, o=打印输出, q=退出, r=运行到结束");
    println!();

    let mut step_count = 0;
    loop {
        let line = session.runtime.current_line;
        let source_line = source.lines().nth((line.saturating_sub(1)) as usize).unwrap_or("").trim();

        print!("步 {:4} | 行 {:3}: {}  > ", step_count, line, source_line);
        if io::stdout().flush().is_err() {
            break;
        }

        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            break;
        }
        let cmd = buf.trim();

        match cmd {
            "q" | "quit" => {
                println!("退出调试。");
                break;
            }
            "p" | "print" => {
                let vars = session_api::variables(&session);
                if vars.is_empty() {
                    println!("  (无局部变量)");
                } else {
                    for v in &vars {
                        println!("  {}: {:?} = {}", v.name, v.ty, v.value);
                    }
                }
                continue;
            }
            "o" | "output" => {
                let out = session.runtime.output();
                if out.is_empty() {
                    println!("  (无输出)");
                } else {
                    println!("{}", out);
                }
                continue;
            }
            "r" | "run" => {
                use vitro_native::engine::session_ops::execute_run;
                let result = execute_run(&mut session);
                println!("\n=== 最终输出 ===");
                println!("{}", session.runtime.output());
                if let Err(err) = result {
                    eprintln!("运行错误: {}", err);
                }
                break;
            }
            "" => {
                // 下一步
            }
            _ => {
                println!("未知命令: {}", cmd);
                continue;
            }
        }

        let result = session_api::vm_step(&mut session);
        step_count += 1;

        use vitro_native::session::StepStatus;
        match result.status {
            StepStatus::Paused => {}
            StepStatus::WaitingInput => {
                println!("  [等待输入...]");
            }
            StepStatus::Finished => {
                println!("\n程序执行完毕。");
                println!("\n=== 最终输出 ===");
                println!("{}", session.runtime.output());
                break;
            }
            StepStatus::Trap => {
                eprintln!("\n运行错误 (trap)。");
                println!("\n=== 当前输出 ===");
                println!("{}", session.runtime.output());
                break;
            }
        }
    }
}

fn cmd_export(source_paths: &[String], output_path: &str, is_builtin_libc: bool) {
    use vitro_native::engine::compile_pipeline::run_multi_file_pipeline;
    use vitro_native::session::{CompileUnit, Session};

    let mut units = Vec::new();
    for path in source_paths {
        let source = read_source(path);
        units.push(CompileUnit { filename: path.clone(), source });
    }

    // BytecodeGen 需要 main 函数，添加一个空 stub，导出后过滤掉
    units.push(CompileUnit {
        filename: "__export_main_stub.c".to_string(),
        source: "int main() { return 0; }".to_string(),
    });

    let mut session = Session::default();
    if let Err(e) = run_multi_file_pipeline(&mut session, units, is_builtin_libc) {
        eprintln!("编译失败: {}", e);
        let diags: Vec<String> = session
            .compile
            .diagnostics
            .iter()
            .map(|d| format!("{}:{}: {} ({}{})", d.filename, d.line, d.message, session_api::severity_prefix(d.severity), d.error_code))
            .collect();
        if !diags.is_empty() {
            eprintln!("诊断:\n{}", diags.join("\n"));
        }
        std::process::exit(1);
    }

    #[derive(serde::Serialize)]
    struct BytecodeLibcExport {
        version: u32,
        code_len: usize,
        code: Vec<vitro_runtime::instruction::Instruction>,
        func_table: std::collections::HashMap<String, vitro_native::session::FuncMeta>,
        func_index: std::collections::HashMap<String, i32>,
        globals_init_32: Vec<(u32, i32)>,
        globals_init_64: Vec<(u32, u64)>,
        string_data: Vec<(u32, String)>,
        f64_constants: Vec<f64>,
        i64_constants: Vec<i64>,
        globals_size: u32,
    }

    // 过滤掉 main stub 相关的产物
    let mut code = session.compile.bytecode.clone();
    let mut func_table = session.compile.func_table.clone();
    let mut func_index = session.compile.func_index.clone();
    func_table.remove("main");
    func_index.remove("main");

    // --builtin-libc 模式：移除 BytecodeGen 预注册的旧 Bytecode Libc 函数。
    // 这些函数只在 func_index 中有条目（用于用户代码调用固定索引），
    // 但没有 func_table 条目（不是当前源码实际定义的）。
    if is_builtin_libc {
        use vitro_runtime::bytecode_libc_index::BYTECODE_LIBC_ALL_FUNCS;
        let old_names: Vec<String> = func_index
            .keys()
            .filter(|name| BYTECODE_LIBC_ALL_FUNCS.contains(&name.as_str()) && !func_table.contains_key(name.as_str()))
            .cloned()
            .collect();
        for name in old_names {
            func_index.remove(&name);
        }
    }

    // 移除 BytecodeGen 生成的入口 wrapper（Jump + Call main + Ret）
    // code[0] 是 Jump 到 wrapper_ip，wrapper_ip 位置是 Call main 和 Ret
    let wrapper_ip = if !code.is_empty() && code[0].op == vitro_runtime::opcode::OpCode::Jump {
        code[0].operand as usize
    } else {
        code.len()
    };
    // 将入口 Jump 替换为 Nop（Bytecode Libc 作为库，不需要入口 Jump）
    if !code.is_empty() {
        code[0] = vitro_runtime::instruction::Instruction::new(
            vitro_runtime::opcode::OpCode::Nop,
            0,
            vitro_runtime::instruction::SourceLoc::default(),
        );
    }
    // 截断掉 wrapper 部分（Call main + Ret）
    code.truncate(wrapper_ip);

    // 计算全局变量使用的最大偏移
    let globals_size = session
        .compile
        .globals_init
        .iter()
        .map(|(offset, _)| *offset)
        .chain(session.compile.globals_init_64.iter().map(|(offset, _)| *offset))
        .max()
        .unwrap_or(0);

    let export = BytecodeLibcExport {
        version: 1,
        code_len: code.len(),
        code,
        func_table,
        func_index,
        globals_init_32: session.compile.globals_init.clone(),
        globals_init_64: session.compile.globals_init_64.clone(),
        string_data: session.compile.string_data.clone(),
        f64_constants: session.compile.f64_constants.clone(),
        i64_constants: session.compile.i64_constants.clone(),
        globals_size: globals_size + 4, // 预留一点余量
    };

    let json = serde_json::to_string_pretty(&export).unwrap_or_else(|e| {
        eprintln!("序列化失败: {}", e);
        std::process::exit(1);
    });

    fs::write(output_path, json).unwrap_or_else(|e| {
        eprintln!("写入输出文件失败 '{}': {}", output_path, e);
        std::process::exit(1);
    });

    println!("预编译完成: {}", output_path);
    println!("  代码长度: {} 条指令", export.code_len);
    println!("  函数数量: {}", export.func_index.len());
    println!("  全局变量大小: {} bytes", export.globals_size);
}

fn cmd_unified(path: &str, input_lines: Vec<String>, max_steps: i32) {
    let source = read_source(path);

    // 使用底层 API 进行统一模式执行
    use vitro_native::engine::compile_pipeline::{run_multi_file_pipeline, setup_vm};
    use vitro_native::engine::session_ops::{inject_preset_files, reset_runtime_for_step};
    use vitro_native::session::{CompileUnit, Session};
    use vitro_native::unified::engine::UnifiedEngine;
    use vitro_native::vm::core::VitroVM;

    let mut session = Session::default();
    session.compile.compile_units.push(CompileUnit {
        filename: path.to_string(),
        source: source.clone(),
    });

    let units = session.compile.compile_units.clone();
    if run_multi_file_pipeline(&mut session, units, false).is_err() {
        eprintln!("编译失败。");
        std::process::exit(1);
    }

    let mut engine = UnifiedEngine::with_max_steps(max_steps);
    engine.reset();

    let mut vm = VitroVM::default();
    reset_runtime_for_step(&mut session);
    setup_vm(&mut vm, &session);
    inject_preset_files(&mut vm, &mut session);
    session.runtime.running = true;

    // 保存初始检查点
    engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());
    session.vm = Some(vm);

    // 注入输入 (通过 flutter_bridge 的全局 session 不行，因为这里用的是本地 session)
    // 我们直接把输入放到 session.runtime.input_lines 中
    session.runtime.input_lines = input_lines;
    session.runtime.input_index = 0;

    println!("=== 统一模式执行（时间旅行引擎）===");

    let mut total_steps = 0;
    let mut trapped = false;
    let mut trap_msg = None;

    loop {
        let mut vm = session.vm.take().unwrap_or_default();
        let result = engine.run_batch(&mut vm, &mut session, 100);
        session.vm = Some(vm);

        match result {
            Ok(batch) => {
                total_steps += batch.payloads.len() as i32;
                if batch.finished {
                    break;
                }
                if batch.trapped {
                    trapped = true;
                    trap_msg = batch.trap_message;
                    break;
                }
                if batch.waiting_input && session.runtime.input_lines.is_empty() {
                    println!("[程序等待输入，但输入已耗尽]");
                    break;
                }
            }
            Err(e) => {
                trapped = true;
                trap_msg = Some(e);
                break;
            }
        }

        if total_steps % 500 == 0 {
            print!("\r  已执行 {} 步...", total_steps);
            let _ = io::stdout().flush();
        }
    }

    println!("\r  共执行 {} 步", total_steps);

    // 输出摘要
    println!("\n=== 执行摘要 ===");
    println!("总步数: {}", total_steps);
    if trapped {
        println!("状态: 异常终止");
        if let Some(msg) = trap_msg {
            println!("错误: {}", msg);
        }
    } else {
        println!("状态: 正常结束");
    }

    println!("\n=== 最终输出 ===");
    println!("{}", session.runtime.output());

    // 打印最后几步的变量
    if !engine.frame_cache.is_empty() {
        let last = &engine.frame_cache[engine.frame_cache.len().saturating_sub(1)];
        println!("\n=== 最后一步变量 (行 {}) ===", last.code_line);
        for v in &last.local_vars {
            println!("  {}: {} = {}", v.name, v.ty_name, v.value);
        }
    }
}

// ─── serve：JSON-lines 会话模式（Phase 1 出口 3）──────────────────────────────
//
// 设计要点（主计划 §3.3）：
// - **NDJSON**：每行一个请求 / 一个响应，天然流式、可 `jq`、任意语言可消费；
// - 请求可选 `id`，响应原样回填（异步竞态对账）；
// - **错误帧与成功帧同构**：都有 `id` / `ok`，二选一携带 `result` 或 `error`；
// - `session.reset` 供长寿命进程复用（避免高频重启进程）；
// - 与 capi **共用 `session_api` 入口语义**（纪律 §2.2-2：三出口只做薄包装）。

fn serve_ok(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "id": id, "ok": true, "result": result })
}

fn serve_err(id: serde_json::Value, kind: &str, message: impl Into<String>) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "ok": false,
        "error": { "kind": kind, "message": message.into() }
    })
}

/// 会话拓扑语义（下游需求清单 D2）：**单 serve 进程 = 单活跃会话**。
///
/// 方法表里的 `session.create` / `session.reset` / `session.destroy` 都不带会话句柄
/// 参数，因为进程内只有一个 `Session`；三者都是"清空同一个实例后重建"，
/// `reset` 额外保留会话级配置（隔离预算/判分确定性/argv）。
/// 需要并发逻辑会话（如"长寿命诊断进程 + 瞬态运行进程"）时，请起多个 serve 进程
/// ——这是当前唯一受支持的并发形态。
fn serve_session_semantics() -> serde_json::Value {
    serde_json::json!({
        "model": "single-active-session",
        "active_sessions": 1,
        "concurrent_sessions": false,
        "handle_parameter": false,
        "create_semantics": "clear-and-rebuild（清空重建同一实例）",
        "reset_semantics": "清空编译/运行状态，保留会话级配置（隔离预算/deterministic/argv）",
        "destroy_semantics": "清空重建（进程存活；如需回收进程请用 shutdown）",
        "concurrency_recommendation": "并发场景起多个 serve 进程",
    })
}

/// 会话级配置写入（与 capi 的 `vitro_set_max_steps` / `vitro_set_deterministic` /
/// `vitro_set_quarantine_budget` 同一批 Session 字段，语义一致）。
fn serve_apply_config(session: &mut Session, params: &serde_json::Value) -> serde_json::Value {
    if let Some(v) = params.get("quarantine_budget").and_then(|v| v.as_i64()) {
        session.memory.quarantine_budget = v.clamp(0, 1024 * 1024) as i32;
    }
    if let Some(v) = params.get("deterministic").and_then(|v| v.as_bool()) {
        session.runtime.deterministic = v;
    }
    if let Some(v) = params.get("max_steps").and_then(|v| v.as_i64()) {
        // 走 Session 的会话级入口：会话尚未编译（vm == None）时也生效 ——
        // 此前 `if let Some(vm)` 写法会在 compile 之前静默丢弃该配置（见 Session::set_max_steps）
        session.set_max_steps(v.max(1).min(i32::MAX as i64) as i32);
    }
    if let Some(v) = params.get("call_depth_limit").and_then(|v| v.as_i64()) {
        session.set_call_depth_limit(v.max(1) as usize);
    }
    session_api::config(session)
}

fn serve_handle(session: &mut Session, line: &str) -> (serde_json::Value, bool) {
    let req: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return (
                serve_err(serde_json::Value::Null, "protocol", format!("非法 JSON 请求：{}", e)),
                false,
            )
        }
    };
    let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let Some(method) = req.get("method").and_then(|m| m.as_str()) else {
        return (serve_err(id, "protocol", "请求缺少 method 字段"), false);
    };
    let params = req.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));

    match method {
        "ping" => (
            serve_ok(
                id,
                serde_json::json!({ "pong": true, "abi": vitro_native::capi::VITRO_ABI_VERSION }),
            ),
            false,
        ),
        // E2：机器可读能力清单（"版本宏当能力探测"三层配套之一）
        "capabilities" => (serve_ok(id, session_api::capabilities()), false),
        // U1（2026-09-19）：认知链最小导出——六个零出口分析器的结构化 JSON
        //（misconception/learning_path/knowledge_graph/completion/intent/auto_fix；
        // data_flow 待 CFG 管线接线）。S8 差分锚的字段面以本方法为单源。
        "diagnostics_probe" => (serve_ok(id, session_api::diagnostics_probe(session, &params)), false),
        // 错误码表机器可读导出（下游需求清单 B1）：静态元数据，无状态。
        "error_catalog" => {
            let raw = session_api::error_catalog_json();
            let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({ "catalog": [] }));
            (serve_ok(id, parsed), false)
        }
        // `semantic_label` 受控词汇表（下游需求清单 B2-3）：静态元数据，无状态。
        "semantic_labels" => (serve_ok(id, session_api::semantic_labels()), false),
        // schema 轨道 + 行为契约（下游需求清单 B2-1/B2-2）：静态元数据，无状态。
        "contracts" => (serve_ok(id, session_api::contracts()), false),
        // ── 会话生命周期（下游需求清单 D2：**单 serve 进程 = 单活跃会话**）──────
        // 本进程内仅有一个 `Session` 实例，`create`/`destroy` 都是"清空重建同一实例"，
        // 不携带并发句柄参数、也不支持并发逻辑会话；响应显式回带 session 语义字段，
        // 消费方不必靠文档猜。并发拓扑（长寿命诊断进程 + 瞬态运行进程）请起两个 serve 进程。
        "session.create" => {
            *session = Session::default();
            (
                serve_ok(
                    id,
                    serde_json::json!({
                        "created": true,
                        "session": serve_session_semantics(),
                        "config": session_api::config(session),
                    }),
                ),
                false,
            )
        }
        "session.reset" => {
            // R3：reset 语义单源 session_api（serve/capi 共用）
            session_api::reset_session_preserving_config(session);
            (
                serve_ok(
                    id,
                    serde_json::json!({
                        "reset": true,
                        "session": serve_session_semantics(),
                        "config": session_api::config(session),
                    }),
                ),
                false,
            )
        }
        "session.destroy" => {
            *session = Session::default();
            (
                serve_ok(
                    id,
                    serde_json::json!({
                        "destroyed": true,
                        "session": serve_session_semantics(),
                    }),
                ),
                false,
            )
        }
        "shutdown" => (serve_ok(id, serde_json::json!({ "shutdown": true })), true),
        "config.get" => (serve_ok(id, session_api::config(session)), false),
        "config.set" => {
            let cfg = serve_apply_config(session, &params);
            (serve_ok(id, cfg), false)
        }
        "compile" => {
            // 覆盖式语义：params.files 即当前完整编译单元集合（非追加），
            // 便于长寿命会话反复替换被测程序而无需重启进程。
            let mut units: Vec<CompileUnit> = Vec::new();
            if let Some(files) = params.get("files").and_then(|v| v.as_array()) {
                for f in files {
                    units.push(CompileUnit {
                        filename: f
                            .get("filename")
                            .and_then(|v| v.as_str())
                            .unwrap_or("main.c")
                            .to_string(),
                        source: f.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    });
                }
            } else if let Some(source) = params.get("source").and_then(|v| v.as_str()) {
                units.push(CompileUnit {
                    filename: params
                        .get("filename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("main.c")
                        .to_string(),
                    source: source.to_string(),
                });
            }
            if units.is_empty() {
                return (
                    serve_err(id, "protocol", "compile 需要 params.files 或 params.source"),
                    false,
                );
            }
            session.compile.compile_units = units;
            session.compile.compiled = false;
            session.unified = None;
            let diagnostics = session_api::compile(session);
            (serve_ok(id, diagnostics), false)
        }
        "run" => {
            if let Some(input) = params.get("input").and_then(|v| v.as_str()) {
                // 保留换行（与 capi / FRB / CLI -i 同一口径）
                session.runtime.set_stdin(input);
            }
            if let Some(argv) = params.get("argv").and_then(|v| v.as_array()) {
                let args: Vec<String> = argv
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                session.runtime.argc = args.len() as i32;
                session.runtime.argv = args;
            }
            if let Some(batch) = params.get("batch_input").and_then(|v| v.as_bool()) {
                session.runtime.input_mode = if batch { InputMode::Batch } else { InputMode::Interactive };
            }
            serve_apply_config(session, &params);
            (serve_ok(id, session_api::run(session)), false)
        }
        // 交互式增量喂入（下游需求清单 A2）：run 返回 waiting_input 后追加 stdin 并续跑。
        // 参数：{ text: string }；省略 text 等价于"续推进一步"。
        "input.feed" => {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            (serve_ok(id, session_api::input_feed(session, text)), false)
        }
        "output.delta" => {
            let cursor = params.get("cursor").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            // E-P1-5：可选 `stream` 选择输出通道（display / stdout / stderr / note）。
            // 缺省 display 与旧行为逐字节一致。
            let stream = params.get("stream").and_then(|v| v.as_str()).unwrap_or("display");
            (serve_ok(id, session_api::output_delta_on(session, cursor, stream)), false)
        }
        "step.begin" => match session_api::step_begin(session) {
            0 => (
                serve_ok(id, serde_json::json!({ "ready": true, "max_collected_step": -1 })),
                false,
            ),
            code => (
                serve_err(
                    id,
                    "state",
                    format!("统一模式初始化失败（返回 {}）：会话需先编译成功", code),
                ),
                false,
            ),
        },
        "step.next" => match session_api::step_next(session) {
            Ok(v) => (serve_ok(id, v), false),
            Err(e) => (serve_err(id, "state", e), false),
        },
        "payload.get" => {
            let start = params.get("start").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let end = params.get("end").and_then(|v| v.as_i64()).unwrap_or(i32::MAX as i64) as i32;
            match session_api::payloads(session, start, end) {
                Ok(v) => (serve_ok(id, v), false),
                Err(e) => (serve_err(id, "state", e), false),
            }
        }
        "seek" => {
            let Some(step) = params.get("step").and_then(|v| v.as_i64()) else {
                return (serve_err(id, "protocol", "seek 需要 params.step"), false);
            };
            match session_api::seek(session, step as i32) {
                Ok(v) => (serve_ok(id, v), false),
                Err(e) => (serve_err(id, "state", e), false),
            }
        }
        "breakpoints.set" => {
            let lines: Vec<i32> = params
                .get("lines")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_i64()).map(|x| x as i32).collect())
                .unwrap_or_default();
            session_api::set_breakpoints(session, &lines);
            (serve_ok(id, serde_json::json!({ "lines": lines })), false)
        }
        "memory.regions" => (serve_ok(id, session_api::memory_regions(session)), false),
        other => (serve_err(id, "protocol", format!("未知方法：{}", other)), false),
    }
}

fn cmd_serve() {
    let stdin = io::stdin();
    let mut session = Session::default();
    let mut out = io::stdout();

    eprintln!("vitro_cli serve：JSON-lines 会话模式（EOF 或 shutdown 退出）");
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("stdin 读取错误: {}", e);
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        // R-2026-09-04：一条畸形请求不得杀死整个会话进程（W0-2 止血项）。
        // panic 时保守重建会话并返回错误帧——panic 可能留下半破坏状态
        // （如 unified 引擎 take-后-panic，U2 #12 深水区），静默复用半状态
        // 是更深的缺陷；显式"会话已重置"让消费方拿到确定信号。
        let (response, shutdown) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            serve_handle(&mut session, &line)
        })) {
            Ok(r) => r,
            Err(payload) => {
                let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "未知 panic".to_string()
                };
                session = Session::default();
                (
                    serve_err(
                        serde_json::Value::Null,
                        "internal",
                        format!("内部错误（会话已重置）：{}", msg),
                    ),
                    false,
                )
            }
        };
        let text = match serde_json::to_string(&response) {
            Ok(t) => t,
            Err(e) => format!(
                "{{\"id\":null,\"ok\":false,\"error\":{{\"kind\":\"internal\",\"message\":\"序列化失败：{}\"}}}}",
                e
            ),
        };
        if writeln!(out, "{}", text).is_err() {
            break;
        }
        let _ = out.flush();
        if shutdown {
            break;
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let cmd = &args[1];

    // serve 不需要文件参数：会话内容全部经 stdin 的 JSON 请求提供
    if cmd == "serve" {
        cmd_serve();
        return;
    }

    if args.len() < 3 {
        print_usage();
        std::process::exit(1);
    }

    let file_path = &args[2];

    // 解析 -i 选项与 -- 后的命令行参数
    let mut input_lines = Vec::new();
    let mut argv = vec![file_path.clone()];
    let mut i = 3;
    let mut passthrough = false;
    while i < args.len() {
        if passthrough {
            argv.push(args[i].clone());
            i += 1;
        } else if args[i] == "-i" && i + 1 < args.len() {
            input_lines = read_input_file(&args[i + 1]);
            i += 2;
        } else if args[i] == "--" {
            passthrough = true;
            i += 1;
        } else {
            // 未识别的位置参数视为传给 main 的 argv
            argv.push(args[i].clone());
            i += 1;
        }
    }

    match cmd.as_str() {
        "compile" => cmd_compile(file_path),
        "run" => cmd_run(file_path, input_lines, argv),
        "step" => cmd_step(file_path, input_lines),
        "unified" => {
            let mut max_steps = 100_000;
            let mut i = 3;
            while i < args.len() {
                if args[i] == "--max-steps" && i + 1 < args.len() {
                    max_steps = args[i + 1].parse().unwrap_or_else(|_| {
                        eprintln!("错误: --max-steps 需要有效的整数");
                        std::process::exit(1);
                    });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            cmd_unified(file_path, input_lines, max_steps)
        }
        "export" => {
            // export 命令需要至少一个源文件和 -o 选项
            let mut output_path = String::new();
            let mut is_builtin_libc = false;
            let mut source_paths = vec![file_path.clone()];
            let mut i = 3;
            while i < args.len() {
                if args[i] == "-o" && i + 1 < args.len() {
                    output_path = args[i + 1].clone();
                    i += 2;
                } else if args[i] == "--builtin-libc" {
                    is_builtin_libc = true;
                    i += 1;
                } else {
                    source_paths.push(args[i].clone());
                    i += 1;
                }
            }
            if output_path.is_empty() {
                eprintln!("错误: export 命令需要 -o <输出文件> 选项");
                print_usage();
                std::process::exit(1);
            }
            cmd_export(&source_paths, &output_path, is_builtin_libc);
        }
        _ => {
            eprintln!("未知命令: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
