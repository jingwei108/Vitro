use std::collections::HashMap;

use crate::session::VisEvent;
use crate::unified::root_cause::RootCauseHint;
use crate::unified::types::{PointerStatus, StepPayload};

mod decode;
mod diff;
mod encode;

/// 符号表索引。
pub type SymIdx = i32;

/// 使用符号表的变量快照（不变数据 dedup）。
#[derive(Debug, Clone)]
pub struct ApiVarSnapshotRef {
    pub name_idx: SymIdx,
    pub addr: u32,
    pub is_local: bool,
    pub ty_name_idx: SymIdx,
    pub value: String,
}

/// 差分变量：仅包含值发生变化的变量。
#[derive(Debug, Clone)]
pub struct VarDelta {
    pub name_idx: SymIdx,
    pub value: String,
}

/// 使用符号表的指针快照。
#[derive(Debug, Clone)]
pub struct PointerSnapshotRef {
    pub name_idx: SymIdx,
    pub addr: u32,
    pub ty_name_idx: SymIdx,
    pub target_addr: u32,
    pub target_name_idx: SymIdx,
    pub status: PointerStatus,
}

/// 使用符号表的数组快照。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArraySnapshotRef {
    pub name_idx: SymIdx,
    pub element_ty_idx: SymIdx,
    pub elements: Vec<String>,
    /// U2#10：截断标记（旧流缺省 false，serde default 向后兼容）。
    #[serde(default)]
    pub truncated: bool,
}

/// 使用符号表的访问变量。
#[derive(Debug, Clone)]
pub struct AccessedVarRef {
    pub name_idx: SymIdx,
    pub access_type_idx: SymIdx,
}

/// 使用符号表的调用帧。
#[derive(Debug, Clone)]
pub struct ApiFrameInfoRef {
    pub func_name_idx: SymIdx,
    pub return_line: i32,
}

/// 使用符号表的算法步骤。
#[derive(Debug, Clone)]
pub struct AlgorithmStepSnapshotRef {
    pub algorithm_name_idx: SymIdx,
    pub display_name_idx: SymIdx,
    pub phase_idx: SymIdx,
    pub description_idx: SymIdx,
}

/// 使用符号表的 StepPayload（基准快照）。
#[derive(Debug, Clone)]
pub struct StepPayloadRef {
    pub step_index: i32,
    pub code_line: i32,
    pub func_name_idx: SymIdx,
    pub semantic_label_idx: SymIdx,
    pub algorithm_step: Option<AlgorithmStepSnapshotRef>,
    pub local_vars: Vec<ApiVarSnapshotRef>,
    pub call_stack: Vec<ApiFrameInfoRef>,
    pub vis_events: Vec<VisEvent>,
    pub heatmap_line: i32,
    pub heatmap_count: u64,
    pub accessed_vars: Vec<AccessedVarRef>,
    pub array_snapshots: Vec<ArraySnapshotRef>,
    pub pointer_snapshots: Vec<PointerSnapshotRef>,
    pub root_cause_hint: Option<RootCauseHint>,
}

/// 差分 StepPayload（基于前一个 StepPayloadRef 或 StepPayloadDelta）。
#[derive(Debug, Clone)]
pub struct StepPayloadDelta {
    pub step_index: i32,
    pub code_line: i32,
    pub func_name_idx: SymIdx,
    pub semantic_label_idx: SymIdx,
    pub algorithm_step: Option<AlgorithmStepSnapshotRef>,
    /// 值发生变化的变量。
    pub var_deltas: Vec<VarDelta>,
    /// 新出现的变量。
    pub new_vars: Vec<ApiVarSnapshotRef>,
    /// 消失的变量名索引。
    pub removed_var_name_indices: Vec<SymIdx>,
    /// `None` 表示调用栈无变化；`Some(vec)` 为完整新调用栈。
    pub call_stack: Option<Vec<ApiFrameInfoRef>>,
    /// `None` 表示当前步无可视化事件；`Some(vec)` 为当前步完整事件列表。
    pub vis_events: Option<Vec<VisEvent>>,
    pub heatmap_line: i32,
    pub heatmap_count: u64,
    /// `None` 表示访问变量集合无变化；`Some(vec)` 为完整新集合。
    pub accessed_vars: Option<Vec<AccessedVarRef>>,
    /// `None` 表示数组快照集合无变化；`Some(vec)` 为新增/替换的数组快照。
    pub array_snapshots: Option<Vec<ArraySnapshotRef>>,
    /// 已删除的数组变量名索引（相对于上一步）。
    pub removed_array_name_indices: Vec<SymIdx>,
    /// `None` 表示指针快照集合无变化；`Some(vec)` 为新增/替换的指针快照。
    pub pointer_snapshots: Option<Vec<PointerSnapshotRef>>,
    /// 已删除的指针变量名索引（相对于上一步）。
    pub removed_pointer_name_indices: Vec<SymIdx>,
    pub root_cause_hint: Option<RootCauseHint>,
}

/// Stream 批量传输单元。
///
/// 编码规则：
/// - `base_payloads` 包含每 batch 的第 1 个完整快照（step 0）。
/// - `deltas` 包含后续步的差分数据（基于前一步的局部变量状态）。
/// - `symbol_table` 全局去重字符串池。
#[derive(Debug, Clone)]
pub struct StepStreamBatch {
    pub symbol_table: Vec<String>,
    pub base_payloads: Vec<StepPayloadRef>,
    pub deltas: Vec<StepPayloadDelta>,
    pub finished: bool,
    pub trapped: bool,
    pub waiting_input: bool,
    pub paused: bool,
    pub current_line: i32,
    pub trap_message: Option<String>,
    /// 当前 frame_cache 窗口起始步号，供前端同步窗口起点。
    pub cache_start_step: i32,
}

struct SymbolTable {
    symbols: Vec<String>,
    index: HashMap<String, SymIdx>,
}

impl SymbolTable {
    fn new() -> Self {
        let mut s = Self {
            symbols: Vec::new(),
            index: HashMap::new(),
        };
        // 索引 0 预留为空字符串
        s.insert(String::new());
        s
    }

    fn insert(&mut self, s: String) -> SymIdx {
        if let Some(&idx) = self.index.get(&s) {
            return idx;
        }
        let idx = self.symbols.len() as SymIdx;
        self.symbols.push(s.clone());
        self.index.insert(s, idx);
        idx
    }

    fn into_vec(self) -> Vec<String> {
        self.symbols
    }
}

/// 将一组 StepPayload 编码为优化的 StepStreamBatch。
pub fn encode_payloads(payloads: &[StepPayload], cache_start_step: i32) -> StepStreamBatch {
    if payloads.is_empty() {
        return StepStreamBatch {
            symbol_table: vec![String::new()],
            base_payloads: Vec::new(),
            deltas: Vec::new(),
            finished: false,
            trapped: false,
            waiting_input: false,
            paused: false,
            current_line: 0,
            trap_message: None,
            cache_start_step,
        };
    }

    let mut sym = SymbolTable::new();

    // 编码第 0 个 payload 为 base
    let base = encode::encode_step_payload(&payloads[0], &mut sym);
    let mut deltas = Vec::new();

    let mut prev_vars: HashMap<SymIdx, String> = HashMap::new();
    for v in &base.local_vars {
        prev_vars.insert(v.name_idx, v.value.clone());
    }

    // 对后续 payload 做差分编码
    let mut current_full_ref = base.clone();
    for i in 1..payloads.len() {
        let delta = encode::encode_step_delta(&payloads[i - 1], &payloads[i], &mut sym, &prev_vars);

        // 增量更新 prev_vars：应用当前 delta 到 current_full_ref
        for d in &delta.var_deltas {
            if let Some(v) = current_full_ref.local_vars.iter_mut().find(|v| v.name_idx == d.name_idx) {
                v.value = d.value.clone();
            }
        }
        for v in &delta.new_vars {
            current_full_ref.local_vars.push(v.clone());
        }
        current_full_ref
            .local_vars
            .retain(|v| !delta.removed_var_name_indices.contains(&v.name_idx));

        prev_vars.clear();
        for v in &current_full_ref.local_vars {
            prev_vars.insert(v.name_idx, v.value.clone());
        }

        deltas.push(delta);
    }

    StepStreamBatch {
        symbol_table: sym.into_vec(),
        base_payloads: vec![base],
        deltas,
        finished: false,
        trapped: false,
        waiting_input: false,
        paused: false,
        current_line: payloads.last().map(|p| p.code_line).unwrap_or(0),
        trap_message: None,
        cache_start_step,
    }
}

/// 将 StepStreamBatch 解码为完整的 StepPayload 列表。
pub fn decode_batch(batch: &StepStreamBatch) -> Vec<StepPayload> {
    decode::decode_batch(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified::types::{ApiFrameInfo, ApiVariableSnapshot, ArraySnapshot, PointerSnapshot};

    fn make_payload(step: i32, vars: Vec<ApiVariableSnapshot>) -> StepPayload {
        StepPayload {
            step_index: step,
            code_line: step + 1,
            func_name: "main".to_string(),
            semantic_label: format!("step {}", step),
            algorithm_step: None,
            local_vars: vars,
            call_stack: vec![ApiFrameInfo {
                func_name: "main".to_string(),
                return_line: 0,
            }],
            vis_events: Vec::new(),
            heatmap_line: step + 1,
            heatmap_count: 1,
            accessed_vars: Vec::new(),
            array_snapshots: Vec::new(),
            pointer_snapshots: Vec::new(),
            root_cause_hint: None,
        }
    }

    fn var(name: &str, ty: &str, value: &str) -> ApiVariableSnapshot {
        ApiVariableSnapshot {
            name: name.to_string(),
            addr: 0,
            is_local: true,
            ty_name: ty.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn test_empty_payloads() {
        let batch = encode_payloads(&[], 0);
        assert!(batch.base_payloads.is_empty());
        assert!(batch.deltas.is_empty());
        assert_eq!(batch.symbol_table.len(), 1); // 空字符串
    }

    #[test]
    fn test_single_payload_roundtrip() {
        let payloads = vec![make_payload(0, vec![var("i", "int", "0"), var("n", "int", "5")])];
        let batch = encode_payloads(&payloads, 0);
        let decoded = decode_batch(&batch);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].step_index, 0);
        assert_eq!(decoded[0].local_vars.len(), 2);
        assert_eq!(decoded[0].local_vars[0].name, "i");
        assert_eq!(decoded[0].local_vars[0].value, "0");
    }

    #[test]
    fn test_delta_encoding_roundtrip() {
        let payloads = vec![
            make_payload(0, vec![var("i", "int", "0"), var("n", "int", "5")]),
            make_payload(1, vec![var("i", "int", "1"), var("n", "int", "5")]),
            make_payload(2, vec![var("i", "int", "2"), var("n", "int", "5")]),
        ];
        let batch = encode_payloads(&payloads, 0);
        assert_eq!(batch.base_payloads.len(), 1);
        assert_eq!(batch.deltas.len(), 2);

        // 验证差分：只有 i 变化，n 不变
        assert_eq!(batch.deltas[0].var_deltas.len(), 1);
        assert_eq!(batch.deltas[0].var_deltas[0].value, "1");
        assert!(batch.deltas[0].new_vars.is_empty());
        assert!(batch.deltas[0].removed_var_name_indices.is_empty());

        let decoded = decode_batch(&batch);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[1].local_vars[0].value, "1");
        assert_eq!(decoded[2].local_vars[0].value, "2");
        assert_eq!(decoded[2].local_vars[1].value, "5"); // n 未变
    }

    #[test]
    fn test_variable_add_remove_roundtrip() {
        let payloads = vec![
            make_payload(0, vec![var("i", "int", "0")]),
            make_payload(1, vec![var("i", "int", "1"), var("j", "int", "10")]),
            make_payload(2, vec![var("j", "int", "11")]),
        ];
        let batch = encode_payloads(&payloads, 0);
        assert_eq!(batch.deltas[0].new_vars.len(), 1); // j 新增
        assert_eq!(batch.deltas[1].removed_var_name_indices.len(), 1); // i 移除
        assert_eq!(batch.deltas[1].var_deltas.len(), 1); // j 变化

        let decoded = decode_batch(&batch);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].local_vars.len(), 1);
        assert_eq!(decoded[1].local_vars.len(), 2);
        assert_eq!(decoded[2].local_vars.len(), 1);
        assert_eq!(decoded[2].local_vars[0].name, "j");
        assert_eq!(decoded[2].local_vars[0].value, "11");
    }

    #[test]
    fn test_symbol_table_dedup() {
        let payloads = vec![
            make_payload(0, vec![var("i", "int", "0")]),
            make_payload(1, vec![var("i", "int", "1")]),
        ];
        let batch = encode_payloads(&payloads, 0);
        // func_name "main", semantic_label "step 0"/"step 1", name "i", ty "int", value "0"/"1"
        // 只有 "main" 和 "i" 和 "int" 是重复的，应该被去重
        let sym_set: std::collections::HashSet<&String> = batch.symbol_table.iter().collect();
        assert_eq!(sym_set.len(), batch.symbol_table.len()); // 无重复
    }

    #[test]
    fn test_call_stack_delta() {
        let p0 = make_payload(0, vec![var("i", "int", "0")]);
        let mut p1 = make_payload(1, vec![var("i", "int", "1")]);
        let mut p2 = make_payload(2, vec![var("i", "int", "2")]);

        // p0/p1 调用栈相同，p2 进入 foo
        p1.call_stack = p0.call_stack.clone();
        p2.call_stack = vec![
            ApiFrameInfo {
                func_name: "main".to_string(),
                return_line: 3,
            },
            ApiFrameInfo {
                func_name: "foo".to_string(),
                return_line: 0,
            },
        ];

        let batch = encode_payloads(&[p0, p1, p2], 0);
        assert!(batch.deltas[0].call_stack.is_none()); // p1 调用栈未变
        assert!(batch.deltas[1].call_stack.is_some()); // p2 调用栈变化
        assert_eq!(batch.deltas[1].call_stack.as_ref().unwrap().len(), 2);

        let decoded = decode_batch(&batch);
        assert_eq!(decoded[2].call_stack.len(), 2);
        assert_eq!(decoded[2].call_stack[1].func_name, "foo");
    }

    #[test]
    fn test_array_and_pointer_delta() {
        let mut p0 = make_payload(0, vec![var("i", "int", "0")]);
        p0.array_snapshots = vec![ArraySnapshot {
            name: "a".to_string(),
            element_ty: "int".to_string(),
            elements: vec!["1".to_string(), "2".to_string()],
            truncated: false,
        }];
        p0.pointer_snapshots = vec![PointerSnapshot {
            name: "p".to_string(),
            addr: 100,
            ty_name: "int*".to_string(),
            target_addr: 200,
            target_name: "x".to_string(),
            status: PointerStatus::Valid,
        }];

        let mut p1 = p0.clone();
        // p1 数组变化（元素值改变）
        p1.array_snapshots[0].elements = vec!["3".to_string(), "2".to_string()];
        // p1 指针不变

        let mut p2 = p1.clone();
        // p2 删除数组 a，新增数组 b；指针 p 目标改变
        p2.array_snapshots = vec![ArraySnapshot {
            name: "b".to_string(),
            element_ty: "int".to_string(),
            elements: vec!["7".to_string()],
            truncated: false,
        }];
        p2.pointer_snapshots[0].target_addr = 300;

        let batch = encode_payloads(&[p0, p1, p2], 0);

        // p1：数组变化，指针未变
        assert!(batch.deltas[0].array_snapshots.is_some());
        assert!(batch.deltas[0].pointer_snapshots.is_none());

        // p2：数组替换 + 删除 a，指针变化
        assert!(batch.deltas[1].array_snapshots.is_some());
        assert!(!batch.deltas[1].removed_array_name_indices.is_empty());
        assert!(batch.deltas[1].pointer_snapshots.is_some());

        let decoded = decode_batch(&batch);
        assert_eq!(decoded[2].array_snapshots.len(), 1);
        assert_eq!(decoded[2].array_snapshots[0].name, "b");
        assert_eq!(decoded[2].pointer_snapshots[0].target_addr, 300);
    }

    #[test]
    fn test_accessed_vars_and_vis_events_delta() {
        let mut p0 = make_payload(0, vec![var("i", "int", "0")]);
        p0.accessed_vars = vec![crate::unified::types::AccessedVar {
            name: "i".to_string(),
            access_type: "Read".to_string(),
        }];
        p0.vis_events = vec![crate::session::VisEvent {
            ty: 1,
            line: 10,
            extra0: 0,
            extra1: 0,
            extra2: 0,
            context: "swap".to_string(),
        }];

        let mut p1 = p0.clone();
        p1.accessed_vars[0].access_type = "Write".to_string();
        // vis_events 不变

        let mut p2 = p1.clone();
        p2.vis_events.clear();
        p2.accessed_vars = p1.accessed_vars.clone();

        let batch = encode_payloads(&[p0, p1, p2], 0);
        assert!(batch.deltas[0].accessed_vars.is_some());
        assert!(batch.deltas[0].vis_events.is_none()); // 未变
        assert!(batch.deltas[1].vis_events.is_some()); // 变为空列表
        assert_eq!(batch.deltas[1].vis_events.as_ref().unwrap().len(), 0);

        let decoded = decode_batch(&batch);
        assert_eq!(decoded[1].accessed_vars[0].access_type, "Write");
        assert_eq!(decoded[2].vis_events.len(), 0);
    }
}
