//! StepPayload Schema v0.1 冻结测试（对应 `docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md`）。
//!
//! 目的：schema "定稿"必须有可执行的锚点 —— 字段名、枚举字面量、窗口常量一旦
//! 被无意改动，本测试立即失败，迫使改动方走版本化流程（字段只增不改语义）。
//!
//! 断言对象是 **capi 出口的 JSON**，不是 Rust 结构体：协议契约必须在出口成立。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::ffi::{c_char, CStr, CString};

use vitro_native::capi;
use vitro_native::session::Session;
use vitro_native::unified::contracts;
use vitro_native::unified::types::{PointerStatus, StepPayload};
use vitro_native::unified::vocabulary;

/// schema v0.1 顶层 14 字段（`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md` §1）。
const TOP_LEVEL_FIELDS: [&str; 14] = [
    "step_index",
    "code_line",
    "func_name",
    "semantic_label",
    "algorithm_step",
    "local_vars",
    "call_stack",
    "vis_events",
    "heatmap_line",
    "heatmap_count",
    "accessed_vars",
    "array_snapshots",
    "pointer_snapshots",
    "root_cause_hint",
];

unsafe fn take_string(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(p).to_string_lossy().to_string();
    capi::vitro_free_string(p);
    s
}

fn compile_session(source: &str) -> *mut Session {
    unsafe {
        let session = capi::vitro_session_create();
        assert!(!session.is_null(), "session 创建失败");
        let fname = CString::new("main.c").unwrap();
        let src = CString::new(source).unwrap();
        capi::vitro_compile_unit(session, fname.as_ptr(), src.as_ptr());
        let diag = take_string(capi::vitro_compile_json(session));
        let v: serde_json::Value = serde_json::from_str(&diag).unwrap();
        assert_eq!(v["ok"], true, "示例程序应编译成功: {}", diag);
        session
    }
}

fn step_until(session: *mut Session, steps: usize) -> Vec<serde_json::Value> {
    let mut collected = Vec::new();
    unsafe {
        // 统一模式必须显式初始化（vitro_step_begin 装载 VM + 建初始检查点）
        assert_eq!(
            capi::vitro_step_begin(session),
            0,
            "统一模式会话初始化失败（会话须已编译成功）"
        );
        for _ in 0..steps {
            let s = take_string(capi::vitro_step_next_json(session));
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            for p in v["payloads"].as_array().cloned().unwrap_or_default() {
                collected.push(p);
            }
            if v["finished"] == true || v["trapped"] == true {
                break;
            }
        }
    }
    collected
}

fn keys_of(value: &serde_json::Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("应为 JSON 对象")
        .keys()
        .cloned()
        .collect()
}

const SAMPLE: &str = r#"
#include <stdio.h>
int main() {
    int arr[3] = {1, 2, 3};
    int *p = arr;
    int s = 0;
    for (int i = 0; i < 3; i++) { s += arr[i]; }
    *p = 5;
    printf("%d", s);
    return s;
}
"#;

#[test]
fn test_step_payload_top_level_fields_frozen() {
    let session = compile_session(SAMPLE);
    let payloads = step_until(session, 60);
    unsafe { capi::vitro_session_destroy(session) };

    assert!(!payloads.is_empty(), "应至少收集到一个 payload");
    let expected: BTreeSet<String> = TOP_LEVEL_FIELDS.iter().map(|s| s.to_string()).collect();
    for p in &payloads {
        let actual = keys_of(p);
        assert_eq!(
            actual, expected,
            "StepPayload 顶层字段集合与 schema v0.1 不一致（新增字段须走版本化流程）：{}",
            p
        );
    }
}

#[test]
fn test_substructure_fields_frozen() {
    let session = compile_session(SAMPLE);
    let payloads = step_until(session, 60);
    unsafe { capi::vitro_session_destroy(session) };

    let mut saw_local = false;
    let mut saw_array = false;
    let mut saw_pointer = false;
    let mut saw_call_frame = false;
    let mut saw_accessed = false;

    for p in &payloads {
        for v in p["local_vars"].as_array().cloned().unwrap_or_default() {
            saw_local = true;
            assert_eq!(
                keys_of(&v),
                ["addr", "is_local", "name", "ty_name", "value"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
                "ApiVariableSnapshot 字段与 schema 不一致: {}",
                v
            );
        }
        for f in p["call_stack"].as_array().cloned().unwrap_or_default() {
            saw_call_frame = true;
            assert_eq!(
                keys_of(&f),
                ["func_name", "return_line"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
                "ApiFrameInfo 字段与 schema 不一致: {}",
                f
            );
        }
        for a in p["accessed_vars"].as_array().cloned().unwrap_or_default() {
            saw_accessed = true;
            assert_eq!(
                keys_of(&a),
                ["access_type", "name"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
                "AccessedVar 字段与 schema 不一致: {}",
                a
            );
            let ty = a["access_type"].as_str().unwrap_or("");
            assert!(
                ty == "Read" || ty == "Write",
                "access_type 只允许 Read/Write（大小写敏感），实际: {}",
                ty
            );
        }
        for arr in p["array_snapshots"].as_array().cloned().unwrap_or_default() {
            saw_array = true;
            assert_eq!(
                keys_of(&arr),
                ["element_ty", "elements", "name", "truncated"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
                "ArraySnapshot 字段与 schema 不一致: {}",
                arr
            );
        }
        for ptr in p["pointer_snapshots"].as_array().cloned().unwrap_or_default() {
            saw_pointer = true;
            assert_eq!(
                keys_of(&ptr),
                ["addr", "name", "status", "target_addr", "target_name", "ty_name"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
                "PointerSnapshot 字段与 schema 不一致: {}",
                ptr
            );
            let status = ptr["status"].as_str().unwrap_or("");
            assert!(
                ["Valid", "Freed", "Null", "Dangling"].contains(&status),
                "pointer status 必须落在四状态枚举内，实际: {}",
                status
            );
        }
    }

    assert!(saw_local, "样本程序应产生变量快照");
    assert!(saw_call_frame, "样本程序应产生调用栈");
    assert!(saw_accessed, "样本程序应产生 accessed_vars");
    assert!(saw_array, "样本程序应产生数组快照");
    assert!(saw_pointer, "样本程序应产生指针快照");
}

#[test]
fn test_pointer_status_enum_literals_frozen() {
    // 枚举字面量是协议的一部分：改名 = 破坏性变更
    for (status, literal) in [
        (PointerStatus::Valid, "Valid"),
        (PointerStatus::Freed, "Freed"),
        (PointerStatus::Null, "Null"),
        (PointerStatus::Dangling, "Dangling"),
    ] {
        let v = serde_json::to_value(status).unwrap();
        assert_eq!(v, serde_json::json!(literal), "PointerStatus 字面量被改动");
    }
}

#[test]
fn test_frame_cache_window_2000_frames_with_20pct_trim() {
    // 窗口语义：2000 帧上限、超限丢最早 20%（§4）
    let session = compile_session(
        "#include <stdio.h>\nint main(){ int s = 0; for (int i = 0; i < 900; i++) { s = s + i; } printf(\"%d\", s); return 0; }\n",
    );
    let steps = step_until(session, 20_000).len();
    assert!(steps > 2000, "样本应产生 >2000 步以触发窗口裁剪，实际 {}", steps);

    unsafe {
        let s = take_string(capi::vitro_get_step_payloads_json(session, 0, i32::MAX));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let window = v["payloads"].as_array().expect("payloads 必须是数组");
        let cache_start = v["cache_start_step"].as_i64().unwrap_or(-1);

        assert!(
            window.len() <= 2000,
            "frame_cache 窗口不得超过 2000 帧，实际 {}",
            window.len()
        );
        assert!(cache_start > 0, "窗口应已滑动，cache_start_step 实际 {}", cache_start);
        // 越窗查询被静默裁剪：请求 [0, MAX) 只返回窗口内子集
        let first_step = window[0]["step_index"].as_i64().unwrap_or(-1);
        assert_eq!(
            first_step, cache_start,
            "窗口首帧步号必须等于 cache_start_step"
        );
        capi::vitro_session_destroy(session);
    }
}

#[test]
fn test_step_payload_serializes_every_field() {
    // 类型链完整性：StepPayload 必须能整体序列化（不丢字段）
    let payload = StepPayload {
        step_index: 7,
        code_line: 3,
        func_name: "main".to_string(),
        semantic_label: "第 3 行".to_string(),
        algorithm_step: None,
        local_vars: Vec::new(),
        call_stack: Vec::new(),
        vis_events: Vec::new(),
        heatmap_line: 3,
        heatmap_count: 1,
        accessed_vars: Vec::new(),
        array_snapshots: Vec::new(),
        pointer_snapshots: Vec::new(),
        root_cause_hint: None,
    };
    let v = serde_json::to_value(&payload).unwrap();
    assert_eq!(keys_of(&v).len(), TOP_LEVEL_FIELDS.len());
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.2 轨道与词汇表防线（下游需求清单 B2）
//
// 这三组断言把 SharpTutor S4 §5 / S5 §1 的"文档共识"变成上游可执行测试位：
//   ① 预留位在 v0.1 阶段必须**缺省**（激活即触发 tripwire，强制走激活清单）；
//   ② `semantic_label` 词汇**闭合**（引擎产出的每个标签都必须登记在案）；
//   ③ "UNWINDING 不得合并单步"有可执行判据（CS3b 激活后由回放驱动复用）。

/// 递归收集 JSON 中所有对象（含嵌套）出现的键名。
fn all_keys_recursive(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                out.insert(k.clone());
                all_keys_recursive(v, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                all_keys_recursive(item, out);
            }
        }
        _ => {}
    }
}

/// B2-1：v0.1 阶段预留位字段**不得出现**在任何 payload（含嵌套子结构）中。
///
/// 这是 SharpTutor S5 §1 A2（"预留位缺省"）的上游机器化版本。断言失败 = 有人
/// 激活了 v0.2 字段却没走激活清单 —— 失败信息会把清单原样打印出来。
#[test]
fn test_v0_1_reserved_fields_absent() {
    let session = compile_session(SAMPLE);
    let payloads = step_until(session, 200);
    unsafe { capi::vitro_session_destroy(session) };
    assert!(!payloads.is_empty(), "应至少收集到一个 payload");

    let reserved: BTreeSet<&str> = contracts::RESERVED_FIELDS_V0_2.into_iter().collect();
    for p in &payloads {
        let mut keys = BTreeSet::new();
        all_keys_recursive(p, &mut keys);
        let hit: Vec<&String> = keys.iter().filter(|k| reserved.contains(k.as_str())).collect();
        assert!(
            hit.is_empty(),
            "v0.1 阶段出现了 v0.2 预留位字段 {:?}（payload step_index={}）。\n\
             若这是**有意激活**，请逐条走完 v0.2 激活清单后再改本测试：\n  {}",
            hit,
            p["step_index"],
            contracts::V0_2_ACTIVATION_CHECKLIST.join("\n  ")
        );
    }
}

/// B2-1（反向）：预留位字段名集合本身也是契约 —— 改一名 = 改协议，必须走版本化。
#[test]
fn test_reserved_field_names_frozen() {
    assert_eq!(
        contracts::RESERVED_FIELDS_V0_2.to_vec(),
        vec!["handler_depth", "unwinding", "unwind_frames_left", "current_exception"],
        "预留位字段名集合被改动 —— 这是协议变更，须同步 schema §7.x / S4 契约并重跑回放"
    );
    assert_eq!(contracts::SCHEMA_VERSION, "v0.1");
}

/// B2-3：`semantic_label` **词汇闭合** —— 引擎产出的每个非空 label 都必须能归类。
///
/// 新增标签而不登记词汇表 = 本测试失败（消费端 UI 直读词汇，未登记即静默失效）。
#[test]
fn test_semantic_label_vocabulary_closed() {
    let session = compile_session(VOCABULARY_SAMPLE);
    let payloads = step_until(session, 5_000);
    unsafe { capi::vitro_session_destroy(session) };
    assert!(!payloads.is_empty(), "词汇闭包样本应产生步数据");

    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for p in &payloads {
        let label = p["semantic_label"].as_str().unwrap_or("");
        if label.is_empty() {
            continue; // 空串 = 本步无标签（code_line == 0），不是词汇缺失
        }
        match vocabulary::classify(label) {
            Some(id) => {
                seen.insert(id);
            }
            None => panic!(
                "引擎产出了未登记的 semantic_label `{}`（step_index={}）。\n\
                 受控词汇表 = `crate::unified::vocabulary::SEMANTIC_LABEL_VOCABULARY`，\
                 新增词汇须同步 schema 附录 B 与 serve `semantic_labels`（词汇只增不改）。",
                label, p["step_index"]
            ),
        }
    }

    // 样本应覆盖 C 域主干词汇（否则"闭合"可能是空转而过的假绿）
    for required in ["loop", "swap", "call", "return", "heap_alloc", "heap_free", "recursive_call"] {
        assert!(
            seen.contains(required),
            "词汇闭包样本未覆盖 `{}`（实际覆盖 {:?}）—— 样本需要调整，否则防线有盲区",
            required,
            seen
        );
    }
}

/// B2-2：行为契约"UNWINDING 不得合并单步"的**可执行判据**（CS3b 激活后由回放复用）。
#[test]
fn test_unwinding_granularity_contract_frozen() {
    let contract = contracts::BEHAVIOR_CONTRACTS
        .iter()
        .find(|c| c.id == "unwinding_step_granularity")
        .expect("行为契约表必须登记 unwinding_step_granularity（CSharp前端引入计划 §6-B）");
    assert!(
        contract.statement.contains("不得合并单步"),
        "契约文本丢失核心约束：{}",
        contract.statement
    );
    assert_eq!(contract.status, "reserved", "CS3b 前该契约应为 reserved");
    assert_eq!(contract.batch, "CS3b");

    // 判据自检：S4 §5 A2 形态（栈深 3 → 逐帧 3/2/1）通过；一次弹两帧被拒。
    let ok = [
        contracts::UnwindSample { step_index: 10, unwinding: true, unwind_frames_left: 3 },
        contracts::UnwindSample { step_index: 11, unwinding: true, unwind_frames_left: 2 },
        contracts::UnwindSample { step_index: 12, unwinding: true, unwind_frames_left: 1 },
        contracts::UnwindSample { step_index: 13, unwinding: false, unwind_frames_left: 0 },
    ];
    assert!(contracts::check_unwinding_granularity(&ok).is_ok());

    let merged = [
        contracts::UnwindSample { step_index: 10, unwinding: true, unwind_frames_left: 3 },
        contracts::UnwindSample { step_index: 11, unwinding: true, unwind_frames_left: 1 },
    ];
    let err = contracts::check_unwinding_granularity(&merged).expect_err("合并单步必须被拒");
    assert!(err.contains("不得合并单步"), "错误信息须指向契约，实际：{}", err);
}

/// B2-1/B2-3：**文档↔代码单源校验** —— schema 文档必须登记代码里的预留位、台账与词汇。
///
/// 防止"文档写一套、代码另一套"（本仓库最贵的历史教训是 ty_name 的 Debug 泄漏）。
#[test]
fn test_schema_doc_v0_2_track_matches_code() {
    let doc = std::fs::read_to_string("../docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md")
        .or_else(|_| std::fs::read_to_string("docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md"))
        .expect("schema 文档缺失");

    for field in contracts::RESERVED_FIELDS_V0_2 {
        assert!(doc.contains(field), "schema 文档未登记预留位字段 `{}`", field);
    }
    for plan in contracts::V0_2_FIELD_LEDGER {
        assert!(
            doc.contains(plan.field),
            "schema §9 台账未登记 v0.2 字段 `{}`",
            plan.field
        );
    }
    for kind in vocabulary::SEMANTIC_LABEL_VOCABULARY {
        assert!(
            doc.contains(kind.id),
            "schema 附录 B 未登记词汇 `{}`（模板 {}）",
            kind.id,
            kind.template
        );
    }
    assert!(
        doc.contains("v0.1 已冻结"),
        "schema 文档须显式声明 v0.1 冻结状态（S1–S5 签字回放通过）"
    );
    assert!(
        doc.contains(contracts::SCHEMA_V0_1_FROZEN_AT),
        "schema 文档须记录冻结日期 {}",
        contracts::SCHEMA_V0_1_FROZEN_AT
    );
}

/// 词汇闭包样本：覆盖 C 域主干标签（循环/交换/调用/返回/堆/递归/兜底）。
///
/// 两个刻意的形态选择（否则防线有盲区）：
/// - 交换语句用 `temp` 命名**且函数内有循环变量 `i`**（`is_swap` 要求行内含 `temp`，
///   且既有口径下"交换"只在循环上下文成立）；
/// - 递归函数参数用 `x` —— 形参名若落在循环变量白名单（`i/j/k/m/n/…`）内，
///   该行会被判为"循环 {iter=…}"而不是"递归调用"。
const VOCABULARY_SAMPLE: &str = r#"
#include <stdio.h>
#include <stdlib.h>

int helper(int x) {
    int arr[3] = {1, 2, 3};
    int i = 0;
    int temp = arr[i];
    arr[i] = arr[i + 1];
    arr[i + 1] = temp;
    return x;
}

int fib(int x) {
    if (x < 2) return x;
    return fib(x - 1) + fib(x - 2);
}

int main() {
    int *p = (int *)malloc(16);
    int s = 0;
    for (int i = 0; i < 3; i = i + 1) { s = s + i; }
    p[0] = helper(1) + fib(4);
    printf("%d", s);
    free(p);
    return 0;
}
"#;
