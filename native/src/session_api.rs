//! 会话操作的语言中立层：`capi` / `vitro_cli serve` / wasm 共用同一套入口语义。
//!
//! 架构纪律（主计划 §2.2 纪律 2）：**三个出口只做薄包装**。此前 capi 第一批把
//! "运行 → JSON"的构造直接写在 C 导出函数内部；若 serve 再写一份，就会重蹈
//! "typeck 与 codegen 双轨语义"的覆辙。
//!
//! 分工：
//! - 本模块：业务语义（何时算 trap、`status` 枚举、诊断字段映射、窗口裁剪、
//!   断点集合写入）——**只此一份**；
//! - 出口：参数编解码 + 所有权（capi 的 rust-alloc C 字符串 / serve 的 NDJSON 行）。
//!
//! 返回 `serde_json::Value` 而非字符串：序列化时机与所有权归出口决定。

use crate::engine::compile_pipeline::{run_multi_file_pipeline, setup_vm};
use crate::engine::session_ops::{execute_run, inject_preset_files, reset_runtime_for_step};
use crate::session::Session;
use crate::unified::engine::UnifiedEngine;
use serde_json::{json, Value};

/// 错误 JSON 的统一形状（与成功帧同构：都有 `ok`，失败时带 `error`）。
pub fn error_json(message: impl Into<String>) -> Value {
    json!({ "ok": false, "error": message.into() })
}

/// 诊断 severity 数值 → schema 枚举字符串。
pub fn severity_name(severity: i32) -> &'static str {
    match severity {
        0 => "error",
        1 => "warning",
        2 => "hint",
        _ => "info",
    }
}

/// P3（2026-09-18）：severity 数值 → 显示前缀（E/W/H，info 级用 I）。
/// serve 帧 / CLI 的诊断码前缀**从 severity 单源派生**——与 `severity`
/// 字段同源故永不矛盾；静态码表（error_catalog）用
/// `vitro_shared::error_codes::code_prefix`（码段位白名单）。
/// 此前硬编码 `"E{}"`，W/H 级码被伪造为 E 前缀（`code=E3053` +
/// `severity=warning` 自相矛盾）。
pub fn severity_prefix(severity: i32) -> &'static str {
    match severity {
        0 => "E",
        1 => "W",
        2 => "H",
        _ => "I",
    }
}

/// 编译当前会话的编译单元，返回诊断 JSON。
///
/// `{"ok":bool,"diagnostics":[{code,error_code,severity,line,column,end_line,end_column,message,fix_suggestion,filename}]}`
pub fn compile(session: &mut Session) -> Value {
    let units = session.compile.compile_units.clone();
    if units.is_empty() {
        return error_json("尚未提供编译单元：请先调用 vitro_compile_unit");
    }
    let ok = run_multi_file_pipeline(session, units, false).is_ok();
    let diagnostics: Vec<Value> = session
        .compile
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "code": format!("{}{}", severity_prefix(d.severity), d.error_code),
                "error_code": d.error_code,
                "severity": severity_name(d.severity),
                "line": d.line,
                "column": d.column,
                // 精确跨度需动三处错误结构体；Phase 1 先给"起点 + 1"退化值
                "end_line": d.line,
                "end_column": d.column + 1,
                "message": d.message,
                "fix_suggestion": d.fix_suggestion,
                "filename": d.filename,
            })
        })
        .collect();
    json!({
        "ok": ok,
        "diagnostics": diagnostics,
        // E2 白箱教学层：宏展开链 + #if 分支选择原因（容量封顶，additive）
        "preprocessor_trace": session.compile.preprocessor_trace,
    })
}

/// P7（2026-09-19）：AST dump 出口——E1 B 级锚的 Rust 侧唯一出口。
/// 此前全仓 `dump_ast` 零命中、AST 27 处 serde 派生但无出口（typeck 与
/// parser 双报告独立确认）。**emitter 纪律**：Rust 侧 emitter =
/// `serde_json::to_value(ProgramNode)`（serde 派生，本侧唯一）——MoonBit
/// 侧必须实现显式 emitter 输出同构 JSON，禁 ToJson 直拼（总计划 §B 结构化
/// 锚：禁一侧 serde 一侧 ToJson）；两侧输出同经 Go canonicalizer
/// （scripts/canonicalize：键排序/转义统一/缩进固定）归一后逐字节比对。
/// session 不保留 AST（与 U1 intents 同因），此处重解析。
pub fn ast_dump(session: &mut Session, params: &Value) -> Value {
    let Some(source) = params.get("source").and_then(|v| v.as_str()) else {
        return error_json("ast.dump 需要 params.source");
    };
    let mut lexer = vitro_lexer::Lexer::new(source);
    let (tokens, lex_errors) = lexer.tokenize();
    if !lex_errors.is_empty() {
        // 诊断进 session（复用管线），AST 无从谈起。
        // S3（2026-09-19）防线维护：E1/E2 对拍出口同构——词法错误路径与
        // MoonBit 侧 dump_ast 输出同键（ok/lex_error_count/ast/parse_errors）
        let units = vec![crate::session::CompileUnit {
            filename: params.get("filename").and_then(|v| v.as_str()).unwrap_or("main.c").to_string(),
            source: source.to_string(),
        }];
        let _ = run_multi_file_pipeline(session, units, false);
        return json!({
            "ok": false,
            "lex_error_count": lex_errors.len(),
            "ast": null,
            "parse_errors": [],
            "stall_count": 0,
        });
    }
    let (program, parse_errors) = vitro_parser::Parser::new(tokens).parse();
    // S3（2026-09-19）：parse_errors 明细（E2 面：code/line/column 保序，
    // message 单列）——成功与失败路径都带（Rust 语义：错误非空时 parse
    // 仍可返回带占位的 AST，上层见 errors 丢弃）
    let pe_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "code": e.code,
                "line": e.line,
                "column": e.column,
                "message": e.message,
            })
        })
        .collect();
    match program {
        Some(p) => match serde_json::to_value(&p) {
            Ok(v) => json!({
                "ok": true,
                "parse_error_count": parse_errors.len(),
                "ast": v,
                "parse_errors": pe_json,
                "stall_count": 0,
            }),
            Err(e) => error_json(format!("AST 序列化失败: {e}")),
        },
        None => {
            let units = vec![crate::session::CompileUnit {
                filename: params.get("filename").and_then(|v| v.as_str()).unwrap_or("main.c").to_string(),
                source: source.to_string(),
            }];
            let _ = run_multi_file_pipeline(session, units, false);
            json!({
                "ok": false,
                "parse_error_count": parse_errors.len(),
                "ast": null,
                "parse_errors": pe_json,
                "stall_count": 0,
            })
        }
    }
}

/// P7（2026-09-19）：符号表 dump 出口——codegen 产物 `session.compile.symbols`
///（全局 + 局部符号的统一表：name/addr/is_local/ty/decl_line 等）。E1 锚的
/// 符号表面对拍用；emitter 纪律同 ast_dump。
pub fn symbols_dump(session: &mut Session, params: &Value) -> Value {
    let Some(source) = params.get("source").and_then(|v| v.as_str()) else {
        return error_json("symbols.dump 需要 params.source");
    };
    let units = vec![crate::session::CompileUnit {
        filename: params.get("filename").and_then(|v| v.as_str()).unwrap_or("main.c").to_string(),
        source: source.to_string(),
    }];
    let compiled = run_multi_file_pipeline(session, units, false).is_ok();
    let symbols: Vec<Value> = session
        .compile
        .symbols
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "addr": s.addr,
                "is_local": s.is_local,
                "decl_line": s.decl_line,
            })
        })
        .collect();
    json!({
        "ok": compiled,
        "symbols": symbols,
        "count": symbols.len(),
    })
}

/// U1（2026-09-19）：认知链最小导出。knowledge_graph / misconception /
/// learning_path / completion / intent / auto_fix 六个分析器此前外部生产
/// 调用全为 0（serve/capi/CLI 零出口）——"趁 Rust 版仍在做差分扫描"的
/// 退路对它们不存在，不补导出则 S8 片无等价性证据。data_flow 需要 CFG
/// 管线接线，本版未覆盖（诚实记录；S8 差分前补）。
///
/// 入参：`{"source": "…"`（必填，单文件编译单元）`, "records": [{ts, ok,
/// codes, trap}]`（编译历史，misconception 输入，可选）`, "completion":
/// {"line", "column", "prefix"}`（补全探测点，可选）`}`。
/// 返回：`{ok, diagnostics, misconceptions, learning_paths, knowledge_graph,
/// intents, completion, auto_fixes}`——各段结构即 S8 差分锚的字段面。
pub fn diagnostics_probe(session: &mut Session, params: &Value) -> Value {
    use crate::diagnostics::auto_fix;
    use crate::diagnostics::{knowledge_graph, learning_path, misconception_patterns};
    use crate::engine::completion;
    use crate::compiler::intent;

    let Some(source) = params.get("source").and_then(|v| v.as_str()) else {
        return error_json("diagnostics_probe 需要 params.source");
    };
    let units = vec![crate::session::CompileUnit {
        filename: params.get("filename").and_then(|v| v.as_str()).unwrap_or("main.c").to_string(),
        source: source.to_string(),
    }];
    let _ = run_multi_file_pipeline(session, units, false);
    let diags = session.compile.diagnostics.clone();

    // 诊断段：码串（severity 前缀按 P3 单源）+ 修复结构标志
    let diagnostics: Vec<Value> = diags
        .iter()
        .map(|d| {
            json!({
                "code": format!("{}{}", severity_prefix(d.severity), d.error_code),
                "severity": severity_name(d.severity),
                "line": d.line,
                "column": d.column,
                "fix_kind": d.fix_kind,
            })
        })
        .collect();

    // misconception / learning_path：外置编译历史驱动
    let records: Vec<misconception_patterns::CompileRecord> = params
        .get("records")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|r| misconception_patterns::CompileRecord {
                    timestamp_ms: r.get("ts").and_then(|v| v.as_i64()).unwrap_or(0),
                    success: r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
                    error_codes: r
                        .get("codes")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|c| c.as_i64().map(|x| x as i32)).collect())
                        .unwrap_or_default(),
                    trap_message: r.get("trap").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();
    let detected = misconception_patterns::detect_misconceptions(records);
    let misconceptions: Vec<Value> = detected
        .iter()
        .map(|m| {
            json!({
                "pattern_id": m.pattern_id,
                "pattern_name": m.pattern_name,
                "occurrence_count": m.occurrence_count,
                "confidence": m.confidence,
            })
        })
        .collect();
    let paths = learning_path::recommend_learning_paths(detected);
    let learning_paths: Vec<Value> = paths
        .iter()
        .map(|p| {
            json!({
                "target_misconception_id": p.target_misconception_id,
                "target_misconception_name": p.target_misconception_name,
                "estimated_time_minutes": p.estimated_time_minutes,
                "steps": p.steps.iter().map(|s| json!({
                    "step_type": s.step_type,
                    "title": s.title,
                    "target_id": s.target_id,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    // knowledge_graph：按本次诊断的错误码激活（去重升序），附全图规模
    let mut error_codes: Vec<i32> = diags
        .iter()
        .filter(|d| d.severity == 0)
        .map(|d| d.error_code)
        .collect();
    error_codes.sort_unstable();
    error_codes.dedup();
    let from_errors: Vec<Value> = error_codes
        .iter()
        .flat_map(|c| knowledge_graph::activate_from_error(*c))
        .map(|a| {
            json!({
                "concept_id": a.node.id,
                "title": a.node.title,
                "activated_by": a.activated_by,
                "neighbors": a.neighbors.iter().map(|n| json!({
                    "concept_id": n.node.id,
                    "relation": n.relation,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let knowledge_graph_out = json!({
        "from_errors": from_errors,
        "concepts_total": knowledge_graph::get_all_concept_nodes().len(),
        "edges_total": knowledge_graph::get_all_concept_edges().len(),
    });

    // intent：重解析拿 FuncDecl（session 不保留 AST），逐函数推断
    let intents: Vec<Value> = (|| {
        let mut lexer = vitro_lexer::Lexer::new(source);
        let (tokens, lex_errors) = lexer.tokenize();
        if !lex_errors.is_empty() {
            return vec![];
        }
        // 错误恢复：补全场景的源码常不完整——parser 恢复出 ProgramNode 即
        // 推断（parse_errors 非空但 program 为 Some 时仍可用，与 completion
        // 的 build_snapshot_from_source 同一恢复语义）
        let (program, _parse_errors) = vitro_parser::Parser::new(tokens).parse();
        let Some(program) = program else {
            return vec![];
        };
        program
            .funcs
            .iter()
            .flat_map(|f| {
                let fname = f.name.clone();
                intent::infer_intent(f).into_iter().map(move |s| (fname.clone(), s))
            })
            .map(|(fname, s)| {
                json!({
                    "func": fname,
                    "intent": s.intent.as_str(),
                    "score": s.score,
                    "reasons": s.reasons,
                })
            })
            .collect()
    })();

    // completion：探测点（0-based line/column + prefix）
    // 审阅处置（2026-09-19）：completion 块存在但 line/column 缺失或非法
    // 时 fail loud——此前静默跳过（unwrap_or_default 链），S8 对拍会掩盖
    // 调用侧错误；records 历史记录保持宽松（ts 缺省 0 是合法语义）
    let completion: Vec<Value> = match params.get("completion") {
        None => Vec::new(),
        Some(c) => {
            let (line, column) = match (
                c.get("line").and_then(|v| v.as_u64()),
                c.get("column").and_then(|v| v.as_u64()),
            ) {
                (Some(l), Some(col)) => (l as usize, col as usize),
                _ => {
                    return error_json(
                        "diagnostics_probe: params.completion 存在时 line/column 必须为非负整数",
                    )
                }
            };
            let prefix = c.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            completion::get_completion_candidates(session, source, line, column, prefix)
                .iter()
                .map(|c| {
                    json!({
                        "label": c.label,
                        "kind": c.kind.as_str(),
                        "insert_text": c.insert_text,
                    })
                })
                .collect()
        }
    };

    // auto_fix：对带结构化修复（fix_kind 1..=3）的诊断逐条应用
    let auto_fixes: Vec<Value> = diags
        .iter()
        .filter(|d| (1..=3).contains(&d.fix_kind))
        .filter_map(|d| {
            auto_fix::apply_fix(source.to_string(), d.clone()).map(|fixed| {
                json!({
                    "line": d.line,
                    "column": d.column,
                    "fix_kind": d.fix_kind,
                    "fixed_source": fixed,
                })
            })
        })
        .collect();

    json!({
        "ok": diags.iter().all(|d| d.severity != 0),
        "diagnostics": diagnostics,
        "misconceptions": misconceptions,
        "learning_paths": learning_paths,
        "knowledge_graph": knowledge_graph_out,
        "intents": intents,
        "completion": completion,
        "auto_fixes": auto_fixes,
    })
}

/// 全速运行并返回结果 JSON。
///
/// `{"ok":bool,"status":"finished|trap|waiting_input|not_compiled","return_value":n,"trap":"...","waiting_input":bool,"steps_executed":n}`
pub fn run(session: &mut Session) -> Value {
    // U1#1 二审 P0-A（防御性）：run 重置执行状态，旧程序的发布缓冲帧
    // 一并作废（主清理在 step_begin，此处兜底 compile→run→step.begin
    // 之外的路径组合）。
    session.unified_pending = None;
    if !session.compile.compiled {
        session.runtime.error = "程序尚未编译。请先编译代码。".to_string();
        return json!({
            "ok": false,
            "status": "not_compiled",
            "return_value": 0,
            "trap": session.runtime.error,
            "waiting_input": false,
            "steps_executed": 0,
        });
    }

    let (return_value, waiting_input) = match execute_run(session) {
        Ok((code, waiting)) => (code, waiting),
        Err(_) => (
            session.vm.as_ref().map(|vm| vm.exit_code()).unwrap_or(-1),
            false,
        ),
    };
    let steps_executed = session.vm.as_ref().map(|vm| vm.get_step_count()).unwrap_or(0);
    let trap = session.runtime.error.clone();
    let status = if !trap.is_empty() {
        "trap"
    } else if waiting_input {
        "waiting_input"
    } else {
        "finished"
    };
    json!({
        "ok": trap.is_empty(),
        "status": status,
        "return_value": return_value,
        "trap": trap,
        "waiting_input": waiting_input,
        "steps_executed": steps_executed,
    })
}

/// 增量喂入交互输入并继续运行，返回与 [`run`] 同构的结果 JSON。
///
/// **语义**（下游需求清单 A2）：`run` 返回 `waiting_input` 后，调用方可用本入口
/// 追加 stdin 文本（可含多行，按 [`vitro_runtime::RuntimeState::split_stdin`] 同口径切行）并让程序
/// 继续执行到 `finished` / `trapped` / 下一次 `waiting_input`。
///
/// 状态机：`waiting_input` --input.feed--> `running` --> `waiting_input | finished | trap`
/// （`feed` 在非等待态也可调用：文本追加到输入缓冲末尾，随后正常续跑。）
///
/// 与 capi `vitro_provide_input_line` 同源语义（薄包装），三出口共用本实现。
/// `text` 为空串时仅触发续跑（等价于"再推进一步"）。
pub fn input_feed(session: &mut Session, text: &str) -> Value {
    if !session.compile.compiled {
        return json!({
            "ok": false,
            "status": "not_compiled",
            "return_value": 0,
            "trap": "程序尚未编译。请先编译代码。",
            "waiting_input": false,
            "steps_executed": 0,
        });
    }
    if !text.is_empty() {
        // 追加（非覆盖）：与 set_stdin 的切行口径一致；游标不重置，接在已消费位置之后。
        // A1 遗留分支：`push_stdin_text` 同时清除 `stdin_eof` 粘滞位 —— 交互续跑本质是
        // "学生又键入了一行"，属预期的新内容到达（与批量模式的不可复活语义不同）。
        session.runtime.push_stdin_text(text);
    }
    // 关键顺序：**保留 `waiting_input=true`** 让 `execute_run` 走 resume 分支
    // （`is_resume = session.runtime.waiting_input`）——若在此提前清位，execute_run 会
    // 误判为新一次运行，`reset_runtime` + `setup_vm` 把程序从 main 重跑（实测产生
    // "第一个 scanf 读到新喂入文本"的错派发）。
    // 仅恢复 VM 暂停位：WaitingInput 时 host call 执行前 `ip -= 1` 且 VM 处于 paused，
    // 不 resume 则 vm.run 立即返回 paused、无法续跑。
    if let Some(ref mut vm) = session.vm {
        vm.resume();
    }
    run(session)
}

/// 错误码表机器可读导出（下游需求清单 B1）。
///
/// 返回 [`crate::diagnostics::error_catalog::export_json`] 的原始 JSON 文本；
/// 出口决定序列化时机与所有权（capi 为 rust-alloc 字符串，serve 为内联对象）。
pub fn error_catalog_json() -> String {
    crate::diagnostics::error_catalog::export_json()
}

/// `semantic_label` 受控词汇表导出（下游需求清单 B2-3 / SharpTutor S4 §6）。
///
/// 语义单源：[`crate::unified::vocabulary::SEMANTIC_LABEL_VOCABULARY`]——与 schema
/// 附录 B 同源，"词汇只增不改"。下游知识卡片按词汇驱动的缓存可随 vendor 更新同步。
pub fn semantic_labels() -> Value {
    crate::unified::vocabulary::vocabulary_json()
}

/// schema 版本轨道 + 行为契约导出（下游需求清单 B2-1/B2-2）。
///
/// 含预留位字段名集合、v0.2 激活清单与字段台账、行为契约表——把"文档共识"
/// 变成消费方可直读的机器可读清单（与冻结测试同源）。
pub fn contracts() -> Value {
    crate::unified::contracts::contracts_json()
}

/// 自 `cursor`（字节偏移）起的输出增量（**展示视图**：含引擎附注，兼容既有消费方）。
///
/// `{"delta":"...","cursor":<新游标>,"total":<总字节>,"stream":"display"}`。
///
/// 需要纯净程序 stdout（判分 / Shadow 比对）的消费方请用 [`output_delta_on`] 并传
/// `"stdout"`——E-P1-5 之前这里只有混装输出，消费方只能靠正则清洗。
pub fn output_delta(session: &Session, cursor: i32) -> Value {
    output_delta_on(session, cursor, "display")
}

/// 按输出通道取增量（E-P1-5）。
///
/// `view` 取值：`"display"`（默认，全部按序拼接，与旧行为一致）/ `"stdout"`（纯程序
/// stdout）/ `"stderr"`（程序 stderr）/ `"note"`（引擎附注：运行完成提示、泄漏报告、
/// 教学诊断）。未知取值退化为 `"display"`，保证不带该参数或传旧值的客户端行为不变。
///
/// 游标越界按末尾处理；落入多字节字符中间时前移到下一个字符边界，保证 `delta` 为合法 UTF-8。
pub fn output_delta_on(session: &Session, cursor: i32, view: &str) -> Value {
    let (stream, all) = match view.trim().to_ascii_lowercase().as_str() {
        "stdout" => ("stdout", session.runtime.stdout()),
        "stderr" => ("stderr", session.runtime.stderr()),
        "note" | "notes" => ("note", session.runtime.notes()),
        _ => ("display", session.runtime.display()),
    };
    let total = all.len();
    let mut start = if cursor < 0 { 0 } else { (cursor as usize).min(total) };
    while start < total && !all.is_char_boundary(start) {
        start += 1;
    }
    let delta = all.get(start..).unwrap_or("").to_string();
    json!({ "delta": delta, "cursor": total, "total": total, "stream": stream })
}

/// 初始化统一模式（时间旅行）会话。返回 0 成功；-2 表示尚未编译成功。
pub fn step_begin(session: &mut Session) -> i32 {
    if !session.compile.compiled {
        return -2;
    }
    // U1#1 二审 P0-A：清掉上一程序的发布缓冲帧。unified_pending 挂在
    // session 上跨 step_begin 存活——同会话二次运行（不调 session.reset）
    // 时，新程序首个 step.next 会先下发旧程序的滞留帧（对照实验实锤：
    // B 程序首帧 = A 的 step=4，携带 A 的 local_vars）。
    session.unified_pending = None;
    let mut engine = UnifiedEngine::new();
    engine.reset();
    let mut vm = session.vm.take().unwrap_or_default();
    reset_runtime_for_step(session);
    setup_vm(&mut vm, session);
    inject_preset_files(&mut vm, session);
    session.runtime.running = true;
    engine.checkpoints.save(0, &mut vm, &mut session.as_vm_context());
    session.vm = Some(vm);
    session.unified = Some(engine);
    0
}

/// 单步推进一次，返回该步的 `AutoStepResult` 形状 JSON（见 schema 文档 §6.1）。
pub fn step_next(session: &mut Session) -> Result<Value, String> {
    let Some(mut engine) = session.unified.take() else {
        return Err("统一模式会话未初始化：请先调用 vitro_step_begin".to_string());
    };
    let mut vm = session.vm.take().unwrap_or_default();
    let outcome = engine.run_batch(&mut vm, session, 1);
    session.vm = Some(vm);
    session.unified = Some(engine);
    let mut result = outcome?;
    // U1#1 P0-1：一帧发布缓冲（流式协议下行末判定需要未来信息——当前帧
    // 暂存，下一帧到来时回改上一帧后再发布）。当前帧与上一帧同 code_line
    // 时，上一帧是语句中间帧（赋值前数值：实测 binary 首帧"计算中点
    // mid=0"实际 mid=2），清其标注后发布。
    //
    // 修复记录：先前版本把 `session.unified_pending = ...` 写在
    // `if let Some(pending)` 块内，而该字段初始为 `None`（session.rs）——
    // 分支永不进入 ⇒ 字段永远是 None ⇒ **整段缓冲从未执行**（死锁），
    // 首帧旧值照样下发。赋值必须无条件执行（放在 if 之外）。
    //
    // R2（2026-09-14，下游 PR 审阅实锤回归）：首调（None 分支）曾发布
    // curr 的克隆后又把 curr 入缓冲——下一轮 pending 再度发布，同一真实步
    // 被投递两次（实测序列 0,0,1，违反 spec 附录 A "step_index 严格递增"
    // 冻结不变量）。修正为**首调只建立缓冲、返回空 payloads 序列**——
    // 这才是"滞后一帧"声明的完整语义（首调无帧可发；此后每次发布上一
    // 步的帧；程序结束冲刷）。消费方需容忍首调空数组（schema §6.1）。
    let flushed = result.payloads.is_empty() || result.finished;
    if flushed {
        // 程序结束/无新帧：冲刷缓存帧（与结束帧同行则它不是行末，清标注）
        if let Some(mut pending) = session.unified_pending.take() {
            if Some(pending.code_line) == result.payloads.last().map(|p| p.code_line) {
                pending.algorithm_step = None;
            }
            result.payloads.insert(0, pending);
        }
    } else {
        let curr = result.payloads.pop();
        match session.unified_pending.take() {
            Some(mut pending) => {
                if Some(pending.code_line) == curr.as_ref().map(|c| c.code_line) {
                    pending.algorithm_step = None;
                }
                // §7.3 append 语义：pending 前插发布位；curr 由 match 外
                // 统一移入缓冲（首版把 curr 发布出去并把缓冲置 None——
                // 中间帧带标注泄漏，帧级调试实锤后回正）。
                result.payloads.insert(0, pending);
            }
            None => {
                // 首调：curr 仅入缓冲、不发布（curr 已 pop，剩余本为空）。
                // 发布克隆的旧实现即 R2 重复投递的根源。
            }
        }
        session.unified_pending = curr;
    }
    serde_json::to_value(&result).map_err(|e| format!("JSON 序列化失败：{}", e))
}

/// 普通 VM 单步（非统一模式；vitro_cli step 子命令同款语义）。
///
/// 首次调用（`running == false`）先初始化步进环境并推进到第一个 step 事件；
/// 之后每次调用推进一条指令。R2：自 flutter_bridge 收口而来——原实现挂在全局
/// 单例上，语义本体在此（语言中立层），出口（CLI/capi）只做薄包装。
pub fn vm_step(session: &mut Session) -> crate::session::StepResult {
    use crate::session::StepStatus;
    use crate::vm::core::StepResult as VmStep;

    if !session.compile.compiled {
        return crate::session::StepResult {
            status: StepStatus::Trap,
            current_line: 0,
            output: String::new(),
            waiting_input: false,
        };
    }

    let mut vm = session.vm.take().unwrap_or_default();
    let result = if !session.runtime.running {
        reset_runtime_for_step(session);
        setup_vm(&mut vm, session);
        inject_preset_files(&mut vm, session);
        vm.pause();
        session.runtime.waiting_input = false;
        loop {
            match vm.step(&mut session.as_vm_context()) {
                VmStep::Ok => {
                    // 首次运行：遇到第一个 StepEvent 后暂停，避免无断点时持续执行到 max_steps
                    if vm.was_step_event_hit() {
                        session.runtime.current_line = vm.get_current_line();
                        break crate::session::StepResult {
                            status: StepStatus::Paused,
                            current_line: session.runtime.current_line,
                            output: session.runtime.output(),
                            waiting_input: false,
                        };
                    }
                }
                VmStep::Paused => {
                    session.runtime.current_line = vm.get_current_line();
                    break crate::session::StepResult {
                        status: StepStatus::Paused,
                        current_line: session.runtime.current_line,
                        output: session.runtime.output(),
                        waiting_input: false,
                    };
                }
                VmStep::WaitingInput => {
                    session.runtime.current_line = vm.get_current_line();
                    break crate::session::StepResult {
                        status: StepStatus::WaitingInput,
                        current_line: session.runtime.current_line,
                        output: session.runtime.output(),
                        waiting_input: true,
                    };
                }
                VmStep::Finished => {
                    session.runtime.running = false;
                    session.runtime.current_line = vm.get_current_line();
                    break crate::session::StepResult {
                        status: StepStatus::Finished,
                        current_line: session.runtime.current_line,
                        output: session.runtime.output(),
                        waiting_input: false,
                    };
                }
                VmStep::Trap => {
                    session.runtime.error = vm.get_error().to_string();
                    session.runtime.running = false;
                    session.runtime.current_line = vm.get_current_line();
                    break crate::session::StepResult {
                        status: StepStatus::Trap,
                        current_line: session.runtime.current_line,
                        output: session.runtime.output(),
                        waiting_input: false,
                    };
                }
            }
        }
    } else {
        match vm.step(&mut session.as_vm_context()) {
            VmStep::Ok | VmStep::Paused => {
                session.runtime.current_line = vm.get_current_line();
                step_out(session, StepStatus::Paused, false)
            }
            VmStep::WaitingInput => {
                session.runtime.current_line = vm.get_current_line();
                step_out(session, StepStatus::WaitingInput, true)
            }
            VmStep::Finished => {
                session.runtime.running = false;
                session.runtime.current_line = vm.get_current_line();
                step_out(session, StepStatus::Finished, false)
            }
            VmStep::Trap => {
                session.runtime.error = vm.get_error().to_string();
                session.runtime.running = false;
                session.runtime.current_line = vm.get_current_line();
                step_out(session, StepStatus::Trap, false)
            }
        }
    };
    session.vm = Some(vm);
    result
}

fn step_out(session: &Session, status: crate::session::StepStatus, waiting: bool) -> crate::session::StepResult {
    crate::session::StepResult {
        status,
        current_line: session.runtime.current_line,
        output: session.runtime.output(),
        waiting_input: waiting,
    }
}

/// 当前栈帧局部变量快照（vitro_cli step 的 `p` 命令；教学变量面板同源）。
pub fn variables(session: &Session) -> Vec<vitro_runtime::VariableSnapshotData> {
    match session.vm.as_ref() {
        Some(vm) => vm.get_variable_snapshot(),
        None => Vec::new(),
    }
}

/// 取 `[start, end)` 步区间的 StepPayload（裁剪到当前 frameCache 窗口内）。
pub fn payloads(session: &Session, start: i32, end: i32) -> Result<Value, String> {
    let Some(engine) = session.unified.as_ref() else {
        return Err("统一模式会话未初始化：请先调用 vitro_step_begin".to_string());
    };
    Ok(json!({
        "payloads": engine.get_payloads(start, end),
        "cache_start_step": engine.frame_cache_start_step,
        "max_collected_step": engine.max_collected_step(),
    }))
}

/// Seek 到指定步（窗口内直接命中；越窗走检查点恢复 + 正向重放）。
pub fn seek(session: &mut Session, target: i32) -> Result<Value, String> {
    let Some(mut engine) = session.unified.take() else {
        return Err("统一模式会话未初始化：请先调用 vitro_step_begin".to_string());
    };
    let mut vm = session.vm.take().unwrap_or_default();
    let result = engine.seek_to(target, &mut vm, session);
    session.vm = Some(vm);
    session.unified = Some(engine);
    serde_json::to_value(&result).map_err(|e| format!("JSON 序列化失败：{}", e))
}

/// 写入断点行号集合（会先清空旧断点）。非正行号忽略。
pub fn set_breakpoints(session: &mut Session, lines: &[i32]) -> i32 {
    if let Some(vm) = session.vm.as_mut() {
        vm.clear_breakpoints();
        for &line in lines {
            if line > 0 {
                vm.add_breakpoint(line);
            }
        }
        // S3 A2/A3 口径（下游 vitro-replay）：断点命中后的暂停态是粘性的，
        // **清空断点即恢复推进**——serve 出口明示的恢复手段（无独立 resume 方法）。
        // 注意暂停态有**两层**：VM 层 `vm.paused`（断点/step_event 置位）与
        // 统一模式引擎层 `unified.is_paused`（run_batch 见 Paused 置位），
        // 清断点必须同时恢复两层，否则引擎循环入口直接 break。
        // resume 对非暂停态是无操作，不影响活跃断点的设置。
        if lines.is_empty() {
            vm.resume();
            if let Some(engine) = session.unified.as_mut() {
                engine.resume();
            }
        }
    }
    0
}

/// 内存视图（堆决议 §3 的三色语义 + 下游需求清单 C2 的**三段式 `kind`**）。
///
/// `regions` 是统一的**内存地图**数组，每项带 `kind`：
/// - `"heap"`：`session.memory.regions` 原样（malloc/calloc/realloc/strdup/fopen/vfs），
///   携带 `alloc_line` / `alloc_by` / `is_freed`（三色堆图数据源）；
/// - `"global"`：VM 全局/静态符号合成（`name` = 变量名、`alloc_line` = 声明行、
///   `alloc_by` = `"static"`）；
/// - `"stack"`：活跃调用帧合成（`name` = 函数名、`alloc_line` = **进入该帧的调用行**、
///   `alloc_by` = `"call"`、`size` = 帧跨度 `original_stack_top - locals_base`）。
///
/// **为什么不把栈/全局写回 `session.memory.regions`**：该清单是堆统计的单源
/// （`total_allocated` / 碎片率 / 隔离区驱逐都以它为准），混入栈帧会让
/// "已分配堆内存"把栈算进去。故全局/栈区域**只在导出层合成**（C2 §"定型窗口内加最便宜"）。
///
/// ⚠️ 过渡形态：capi 第二批将把本查询定型为语言中立 schema（`kind` + `status` +
/// `alloc_line`）；此处 serve 出口先按同一形状暴露，第二批落地后对齐字段命名。
pub fn memory_regions(session: &Session) -> Value {
    let mut regions: Vec<(u32, Value)> = Vec::new();
    let heap_count = session.memory.regions.len();
    for r in &session.memory.regions {
        regions.push((r.addr, serde_json::to_value(r).unwrap_or(Value::Null)));
    }
    let (global_count, stack_count) = match session.vm.as_ref() {
        Some(vm) => {
            let globals = global_region_entries(vm);
            let stacks = stack_region_entries(vm);
            let n = (globals.len(), stacks.len());
            regions.extend(globals);
            regions.extend(stacks);
            n
        }
        None => (0, 0),
    };
    // 内存地图的自然顺序：地址升序（全局 → 堆 → 栈自高地址向下）
    regions.sort_by_key(|(addr, _)| *addr);

    json!({
        "regions": regions.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        // 分段计数（consumers 可用它判断三段式是否已生效，无需自行扫 kind）
        "region_counts": { "global": global_count, "stack": stack_count, "heap": heap_count },
        "free_list": session.memory.free_list,
        "quarantine": {
            "bytes": session.memory.quarantine_bytes,
            "budget": session.memory.quarantine_budget,
            "blocks": session.memory.quarantine.len(),
        },
        "heap_base": session.memory.heap_base,
        "heap_offset": session.memory.heap_offset,
        "alloc_counter": session.memory.alloc_counter,
    })
}

/// 全局/静态区域的导出条目（C2）：从 VM 符号表合成，`kind == "global"`。
///
/// `size` 口径：优先取**符号表槽位跨度**（下一个全局符号偏移 − 本符号偏移）——
/// 它与 codegen 的实际分配一致（含填充）；末位符号无后继可参照，退化为
/// `compute_type_size`（标量/指针/数组精确；struct/union/class 因 VM 侧无布局表
/// 返回 0，再退化为 1 个最小字节）。`alloc_line` 取声明行。
fn global_region_entries(vm: &crate::vm::core::VitroVM) -> Vec<(u32, Value)> {
    use vitro_runtime::GLOBAL_START;
    let mut globals: Vec<&vitro_runtime::Symbol> = vm.get_symbols().iter().filter(|s| !s.is_local).collect();
    globals.sort_by_key(|s| s.addr);
    // struct/union/class 布局表在 VM 侧不存在，空表即"只算标量与数组"的口径
    let no_fields: std::collections::HashMap<String, Vec<vitro_ast::StructField>> = std::collections::HashMap::new();
    let no_class_sizes: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    let mut out = Vec::with_capacity(globals.len());
    for (i, sym) in globals.iter().enumerate() {
        let addr = GLOBAL_START + sym.addr;
        let span = globals
            .get(i + 1)
            .map(|next| (GLOBAL_START + next.addr).saturating_sub(addr))
            .filter(|s| *s > 0);
        let size = match span {
            Some(s) => s as i32,
            None => match vitro_ast::compute_type_size(&sym.ty, &no_fields, &no_fields, &no_class_sizes) {
                0 => 1,
                n => n,
            },
        };
        out.push((
            addr,
            json!({
                "addr": addr,
                "size": size,
                "name": sym.name,
                "ty": vitro_runtime::type_display_name(&sym.ty),
                "is_heap": false,
                "is_freed": false,
                "alloc_line": sym.decl_line,
                "alloc_by": "static",
                "kind": "global",
            }),
        ));
    }
    out
}

/// 栈帧区域的导出条目（C2）：从活跃调用帧合成，`kind == "stack"`。
///
/// `name` = 函数名；`alloc_line` = **进入该帧的调用行**（`caller_line`；
/// `main` 的帧为 0 —— 它不是被调用出来的）；`size` = 帧跨度
/// （`original_stack_top - locals_base`，与 VM 的 `mem_stack_top -= frame_size` 同源）。
fn stack_region_entries(vm: &crate::vm::core::VitroVM) -> Vec<(u32, Value)> {
    vm.get_call_stack()
        .iter()
        .map(|f| {
            let addr = f.locals_base;
            (
                addr,
                json!({
                    "addr": addr,
                    "size": f.original_stack_top.saturating_sub(f.locals_base) as i32,
                    "name": f.func_name,
                    "ty": "frame",
                    "is_heap": false,
                    "is_freed": false,
                    "alloc_line": f.caller_line,
                    "alloc_by": "call",
                    "kind": "stack",
                }),
            )
        })
        .collect()
}

/// 会话级配置读取（判分/回放场景需要确认两边配置一致）。
///
/// 2026-09-11 补齐：此前只能 `config.set` 而读不回 `max_steps` / `call_depth_limit`，
/// 消费方无法确认保险丝真的生效（实测暴露过"设置成功但程序跑到默认上限"的静默失败）。
/// VM 尚未创建时两项返回 `null`（表示"尚未落到 VM 上"，与会话字段类配置区分）。
pub fn config(session: &Session) -> Value {
    let (max_steps, call_depth_limit) = match session.vm.as_ref() {
        Some(vm) => (json!(vm.max_steps()), json!(vm.call_depth_limit())),
        None => (Value::Null, Value::Null),
    };
    json!({
        "deterministic": session.runtime.deterministic,
        "quarantine_budget": session.memory.quarantine_budget,
        "input_mode_batch": matches!(session.runtime.input_mode, crate::session::InputMode::Batch),
        "compiled": session.compile.compiled,
        "max_steps": max_steps,
        "call_depth_limit": call_depth_limit,
    })
}

/// `session.reset` 语义单源：清空编译/运行状态，**保留会话级配置**
/// （隔离预算、判分确定性、argv），与引擎 `reset_runtime` 的"配置保留、运行
/// 清空"语义一致。serve 与后续出口共用本入口（R3 自 vitro_cli 收口）。
pub fn reset_session_preserving_config(session: &mut Session) {
    let quarantine_budget = session.memory.quarantine_budget;
    let deterministic = session.runtime.deterministic;
    let argc = session.runtime.argc;
    let argv = std::mem::take(&mut session.runtime.argv);
    *session = Session::default();
    session.memory.quarantine_budget = quarantine_budget;
    session.runtime.deterministic = deterministic;
    session.runtime.argc = argc;
    session.runtime.argv = argv;
}

/// E2：引擎能力清单（机器可读真实能力；"版本宏当能力探测"的三层配套之一）。
///
/// 口径（C23 锚定决议）：`__STDC_VERSION__=202311L` 是**名义锚点**，真实能力
/// 以本清单为准；内存模型常量从 `vitro_runtime` 单源引用，禁止在此复刻数值。
pub fn capabilities() -> Value {
    use crate::unified::contracts::{BEHAVIOR_CONTRACTS, RESERVED_FIELDS_V0_2, SCHEMA_V0_1_FROZEN_AT, SCHEMA_VERSION, V0_2_FIELD_LEDGER};
    use crate::unified::vocabulary::SEMANTIC_LABEL_VOCABULARY;
    use vitro_runtime::{GLOBAL_REGION_LIMIT, GLOBAL_START, HEAP_START, MEM_SIZE, NULL_TRAP_SIZE};
    json!({
        "engine": "vitro",
        "abi_version": crate::capi::VITRO_ABI_VERSION,
        // 引擎版本串（含构建期 git 短哈希）：消费方据此自检"产物是否当前提交构建"
        "engine_version": crate::capi::engine_version_string(),
        // 协议轨道（B2）：v0.1 冻结状态 + 预留位 + v0.2 台账，消费方据此做版本协商
        "schema": {
            "version": SCHEMA_VERSION,
            "frozen_at": SCHEMA_V0_1_FROZEN_AT,
            "reserved_fields_v0_2": RESERVED_FIELDS_V0_2,
            "v0_2_field_ledger": V0_2_FIELD_LEDGER,
        },
        // 行为契约（B2-2）：不得被性能优化破坏的可观测行为
        "behavior_contracts": BEHAVIOR_CONTRACTS,
        // 词汇表条数（完整表见 serve `semantic_labels`）——便于消费方探测词汇扩充
        "semantic_label_kinds": SEMANTIC_LABEL_VOCABULARY.len(),
        "languages": {
            "c": {
                "anchor": "ISO C23 (ISO/IEC 9899:2024) 教学子集",
                "stdc_version_macro_nominal": "202311L",
                "spec": "C-语言子集/C语言子集规范.md",
                "predefined_macros": {
                    "__STDC_VERSION__": "202311L",
                    "__VITRO_SUBSET__": "1",
                },
                "preprocessor": {
                    "object_macros": true,
                    "function_macros": true,
                    "stringize": true,
                    "token_paste": true,
                    "paste_result_must_be_single_token": true,
                    "conditionals": ["#if", "#ifdef", "#ifndef", "#elif", "#else", "#endif"],
                    "defined": true,
                    "has_include": true,
                    "include_once": true,
                    "include_cycle_detection": true,
                    "expand_depth_fuse": 64,
                    "teaching_layer": [
                        "macro_shadowing_warning",
                        "macro_arg_side_effect_warning",
                        "expansion_trace",
                        "branch_reason",
                    ],
                    "dropped": [
                        "自引用宏 trick（展开栈查重直接停止）",
                        "## 动态拼标识符的元编程（结果必须为单个合法 token）",
                        "X-macro 高级用法",
                        "宏拼接 include 路径",
                    ],
                },
            },
            "cpp": {
                "anchor": "C++ 教学子集（Phase 31+，Stage 0~6）",
                "spec": "C-语言子集/C++子集规范.md",
            },
        },
        "memory_model": {
            "mem_size": MEM_SIZE,
            "null_trap_size": NULL_TRAP_SIZE,
            "global_start": GLOBAL_START,
            "global_region_limit": GLOBAL_REGION_LIMIT,
            "heap_start_default": HEAP_START,
            "heap_start": "动态：max(HEAP_START, align4(global_data_end))",
        },
    })
}
