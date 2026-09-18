//! U1 认知链 probe 用例（2026-09-19）：serve `diagnostics_probe` 的内容级
//! 断言锚，语料与断言外置 `tests/cognitive_probe_cases.json`（人审数据，
//! 不审代码——仓库脚本形态约定）。
//!
//! 背景：knowledge_graph / misconception / learning_path / completion /
//! intent / auto_fix 六分析器此前外部生产调用全为 0——"趁 Rust 版仍在做
//! 差分扫描"的退路对它们不存在。本锚保证 probe 出口不再退化回零出口；
//! S8 片建立 MoonBit 双实现对拍时复用同一 JSON（差分锚的字段面单源）。
//! data_flow 未覆盖（需 CFG 管线接线，诚实记录，S8 前补）。

use serde_json::Value;

fn probe(params: &Value) -> Value {
    let mut session = vitro_native::session::Session::default();
    vitro_native::session_api::diagnostics_probe(&mut session, params)
}

#[test]
fn test_cognitive_probe_cases() {
    let raw = std::fs::read_to_string("tests/cognitive_probe_cases.json").unwrap_or_else(|e| panic!("读用例 JSON 失败: {e}"));
    let doc: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("用例 JSON 解析失败: {e}"));
    let cases = doc["cases"].as_array().unwrap_or_else(|| panic!("cases 缺失或非数组"));
    assert!(!cases.is_empty(), "用例集不得为空");

    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed>");
        let expect = &case["expect"];
        let out = probe(&case["params"]);

        let mut errs: Vec<String> = Vec::new();

        if let Some(want) = expect["ok"].as_bool() {
            let got = out["ok"].as_bool().unwrap_or(false);
            if got != want {
                errs.push(format!("ok: 期望 {want}，实际 {got}"));
            }
        }
        if let Some(min) = expect["misconceptions_min"].as_i64() {
            let n = out["misconceptions"].as_array().map(|a| a.len()).unwrap_or(0);
            if (n as i64) < min {
                errs.push(format!("misconceptions: 期望 >= {min}，实际 {n}"));
            }
        }
        if let Some(id) = expect["misconception_ids_contain"].as_str() {
            let hit = out["misconceptions"].as_array().is_some_and(|a| {
                a.iter().any(|m| m["pattern_id"].as_str() == Some(id))
            });
            if !hit {
                errs.push(format!("misconceptions: 未含 pattern_id={id}"));
            }
        }
        if let Some(min) = expect["learning_paths_min"].as_i64() {
            let n = out["learning_paths"].as_array().map(|a| a.len()).unwrap_or(0);
            if (n as i64) < min {
                errs.push(format!("learning_paths: 期望 >= {min}，实际 {n}"));
            }
        }
        if let Some(code) = expect["diagnostics_codes_contain"].as_str() {
            let hit = out["diagnostics"].as_array().is_some_and(|a| {
                a.iter().any(|d| d["code"].as_str() == Some(code))
            });
            if !hit {
                errs.push(format!("diagnostics: 未含 code={code}"));
            }
        }
        if let Some(cid) = expect["kg_from_errors_concepts_contain"].as_str() {
            let hit = out["knowledge_graph"]["from_errors"].as_array().is_some_and(|a| {
                a.iter().any(|c| c["concept_id"].as_str() == Some(cid))
            });
            if !hit {
                errs.push(format!("knowledge_graph.from_errors: 未含 concept_id={cid}"));
            }
        }
        if let Some(min) = expect["kg_concepts_total_min"].as_i64() {
            let n = out["knowledge_graph"]["concepts_total"].as_i64().unwrap_or(0);
            if n < min {
                errs.push(format!("knowledge_graph.concepts_total: 期望 >= {min}，实际 {n}"));
            }
        }
        if let Some(label) = expect["completion_labels_contain"].as_str() {
            let hit = out["completion"].as_array().is_some_and(|a| {
                a.iter().any(|c| c["label"].as_str() == Some(label))
            });
            if !hit {
                errs.push(format!("completion: 未含 label={label}"));
            }
        }
        if let Some(min) = expect["intents_min"].as_i64() {
            let n = out["intents"].as_array().map(|a| a.len()).unwrap_or(0);
            if (n as i64) < min {
                errs.push(format!("intents: 期望 >= {min}，实际 {n}"));
            }
        }
        if let Some(f) = expect["intents_func_contain"].as_str() {
            let hit = out["intents"].as_array().is_some_and(|a| {
                a.iter().any(|i| i["func"].as_str() == Some(f))
            });
            if !hit {
                errs.push(format!("intents: 未含 func={f}"));
            }
        }
        if let Some(min) = expect["auto_fixes_min"].as_i64() {
            let n = out["auto_fixes"].as_array().map(|a| a.len()).unwrap_or(0);
            if (n as i64) < min {
                errs.push(format!("auto_fixes: 期望 >= {min}，实际 {n}"));
            }
        }
        if let Some(sub) = expect["auto_fix_fixed_contains"].as_str() {
            let hit = out["auto_fixes"].as_array().is_some_and(|a| {
                a.iter().any(|f| {
                    f["fixed_source"].as_str().is_some_and(|s| s.contains(sub))
                })
            });
            if !hit {
                errs.push(format!("auto_fixes: fixed_source 未含 {sub:?}"));
            }
        }

        if !errs.is_empty() {
            failures.push(format!("[{}] {}", name, errs.join("；")));
        }
    }
    assert!(
        failures.is_empty(),
        "认知链 probe 用例失败 {} 条：\n{}",
        failures.len(),
        failures.join("\n")
    );
}
