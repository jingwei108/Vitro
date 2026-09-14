//! 防线 6① golden 回归（U1#1 收官，2026-09-14）：算法标注首现序列固化比对。
//!
//! **口径**（与 `tmp/annot_extract.py` 及三审流程一致）：`templates/*/source.c`
//! → compile → run → step.begin → step.next ×4000 → 收集 `(phase, desc)` 首次
//! 出现（含 code_line 与源码行文本）。R2 后 step.next 首调为空帧（一帧发布
//! 缓冲）——首现收集幂等，不受影响；程序结束后 run_batch 重放末帧同样幂等。
//!
//! **golden 定位（诚实边界）**：本 golden 是**三审修复链落地后的行为基线快照**
//!（2026-09-14 全量重提取：37 模板有标注 / 317 条首现 / 45 零标注），**不是
//! 语义人审认证**——人审勾选（`算法标注golden人审清单.md`）在此基线上继续，
//! 勾选结论更新本 golden 时须附红→绿锚。防的是**漂移**：任何判据/管道改动
//! 使首现序列、挂载行或零标注集合变化，本测试即红；确属预期改进时更新
//! golden 并在提交信息注明依据。
//!
//! **双向断言**：有标注模板集与 golden 键集严格相等（新复亮 / 新转零都红，
//! 均需人审裁定后更新 golden）。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;

const STEP_BUDGET: usize = 4000;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
struct FirstOccurrence {
    /// 该条标注归属的算法（如 `bst_insert` / `bst_search`）——三审 P0-2 裁定 (b)：
    /// golden 收录算法归属，跨函数混流（bstSearch 建树段讲插入）与算法标签漂移
    /// 由此可检。JSON 帧本就携带（`AlgorithmStepSnapshot`），v3 固化时丢失，v4 补上。
    #[serde(default)]
    algorithm: String,
    /// 算法教学名（如"冒泡排序"）。
    #[serde(default)]
    display_name: String,
    phase: String,
    desc: String,
    code_line: i64,
    #[serde(default)]
    src: String,
}

fn templates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../templates")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n).collect();
        format!("{}…", cut)
    }
}

/// 单模板提取首现序列（serve 同款管线：compile → run → step.begin → step.next×N）。
fn extract_first_occurrences(source: &str) -> Vec<FirstOccurrence> {
    let mut session = Session::default();
    // 对齐 serve 的 compile 方法（vitro_cli.rs "compile" 分支）：先设
    // compile_units 再走 session_api::compile——后者内含算法检测，标注推断
    // 依赖其结果；直调 run_multi_file_pipeline 会绕过检测（首跑全零标注实证）。
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: source.to_string(),
    }];
    let compile_result = session_api::compile(&mut session);
    assert!(
        compile_result.get("ok") == Some(&serde_json::json!(true)),
        "模板编译失败: {:?}",
        compile_result.get("diagnostics")
    );

    let _ = session_api::run(&mut session);
    assert_eq!(session_api::step_begin(&mut session), 0, "step_begin 失败");

    let lines: Vec<&str> = source.lines().collect();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut first: Vec<FirstOccurrence> = Vec::new();
    for _ in 0..STEP_BUDGET {
        let Ok(value) = session_api::step_next(&mut session) else { break };
        let Some(payloads) = value.get("payloads").and_then(|p| p.as_array()) else { continue };
        for pl in payloads {
            let Some(step) = pl.get("algorithm_step").filter(|a| !a.is_null()) else { continue };
            let algorithm = step.get("algorithm_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let display_name = step.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let phase = step.get("phase").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let desc = step.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // 去重口径钉死为 (phase, desc)（v4 清单 §0 口径 1：条数 = 317）；
            // algorithm/display_name 是首现帧的属性——同文案后到的其他算法不再单列。
            if phase.is_empty() || !seen.insert((phase.clone(), desc.clone())) {
                continue;
            }
            let code_line = pl.get("code_line").and_then(|v| v.as_i64()).unwrap_or(-1);
            let src = if code_line >= 1 && (code_line as usize) <= lines.len() {
                lines[(code_line - 1) as usize].trim().to_string()
            } else {
                String::new()
            };
            first.push(FirstOccurrence { algorithm, display_name, phase, desc, code_line, src });
        }
    }
    first
}

/// golden 全量比对：37 模板首现序列逐条相等 + 有标注模板集与 golden 键集
/// 严格相等（双向防漂移）。失败信息带首个分歧点，便于定位判据/管道改动。
#[test]
fn test_algorithm_annotation_golden_v3() {
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/algorithm_annotations_v3.json");
    let golden_text = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("读取 golden 失败 {}: {}", golden_path.display(), e));
    let golden: std::collections::BTreeMap<String, Vec<FirstOccurrence>> =
        serde_json::from_str(&golden_text).expect("golden JSON 解析失败");

    let dir = templates_dir();
    let mut tpl_names: Vec<String> = std::fs::read_dir(&dir)
        .expect("templates 目录不存在")
        .flatten()
        .filter(|e| e.path().join("source.c").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    tpl_names.sort();

    let mut annotated: Vec<String> = Vec::new();
    let mut mismatches: Vec<String> = Vec::new();
    for name in &tpl_names {
        let source = std::fs::read_to_string(dir.join(name).join("source.c")).unwrap();
        let actual = extract_first_occurrences(&source);
        if actual.is_empty() {
            continue;
        }
        annotated.push(name.clone());
        match golden.get(name) {
            None => mismatches.push(format!("模板 {} 新复亮（golden 无条目，需人审裁定后收录）", name)),
            Some(expected) => {
                if expected.len() != actual.len() {
                    mismatches.push(format!(
                        "{}: 首现条数 {} != golden {}",
                        name,
                        actual.len(),
                        expected.len()
                    ));
                }
                for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
                    if e != a {
                        mismatches.push(format!(
                            "{}[{}]: phase/行 golden=({}, {}) 实际=({}, {})；desc golden={} 实际={}",
                            name,
                            i,
                            e.phase,
                            e.code_line,
                            a.phase,
                            a.code_line,
                            truncate(&e.desc, 24),
                            truncate(&a.desc, 24)
                        ));
                    }
                }
            }
        }
    }

    let golden_keys: HashSet<&String> = golden.keys().collect();
    let annotated_set: HashSet<&String> = annotated.iter().collect();
    for k in golden_keys.difference(&annotated_set) {
        mismatches.push(format!("模板 {} 在 golden 有条目但当前零标注（判据收紧？需裁定）", k));
    }

    assert!(
        mismatches.is_empty(),
        "算法标注 golden 漂移（{} 处，首 {} 处）：\n{}",
        mismatches.len(),
        mismatches.len().min(8),
        mismatches.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}
