//! 统一模式 StepPayload 差分编码器。

use std::collections::HashMap;

use super::{
    AccessedVarRef, AlgorithmStepSnapshotRef, ApiFrameInfoRef, ApiVarSnapshotRef, ArraySnapshotRef, PointerSnapshotRef,
    StepPayloadDelta, StepPayloadRef, SymIdx, SymbolTable, VarDelta,
};
use crate::unified::stream::diff;
use crate::unified::types::{ApiVariableSnapshot, StepPayload};

pub fn encode_step_payload(payload: &StepPayload, sym: &mut SymbolTable) -> StepPayloadRef {
    StepPayloadRef {
        step_index: payload.step_index,
        code_line: payload.code_line,
        func_name_idx: sym.insert(payload.func_name.clone()),
        semantic_label_idx: sym.insert(payload.semantic_label.clone()),
        algorithm_step: payload.algorithm_step.as_ref().map(|a| AlgorithmStepSnapshotRef {
            algorithm_name_idx: sym.insert(a.algorithm_name.clone()),
            display_name_idx: sym.insert(a.display_name.clone()),
            phase_idx: sym.insert(a.phase.clone()),
            description_idx: sym.insert(a.description.clone()),
        }),
        local_vars: payload
            .local_vars
            .iter()
            .map(|v| ApiVarSnapshotRef {
                name_idx: sym.insert(v.name.clone()),
                addr: v.addr,
                is_local: v.is_local,
                ty_name_idx: sym.insert(v.ty_name.clone()),
                value: v.value.clone(),
            })
            .collect(),
        call_stack: payload
            .call_stack
            .iter()
            .map(|f| ApiFrameInfoRef {
                func_name_idx: sym.insert(f.func_name.clone()),
                return_line: f.return_line,
            })
            .collect(),
        vis_events: payload.vis_events.clone(),
        heatmap_line: payload.heatmap_line,
        heatmap_count: payload.heatmap_count,
        accessed_vars: payload
            .accessed_vars
            .iter()
            .map(|a| AccessedVarRef {
                name_idx: sym.insert(a.name.clone()),
                access_type_idx: sym.insert(a.access_type.clone()),
            })
            .collect(),
        array_snapshots: payload
            .array_snapshots
            .iter()
            .map(|a| ArraySnapshotRef {
                name_idx: sym.insert(a.name.clone()),
                element_ty_idx: sym.insert(a.element_ty.clone()),
                elements: a.elements.clone(),
                truncated: a.truncated,
            })
            .collect(),
        pointer_snapshots: payload
            .pointer_snapshots
            .iter()
            .map(|p| PointerSnapshotRef {
                name_idx: sym.insert(p.name.clone()),
                addr: p.addr,
                ty_name_idx: sym.insert(p.ty_name.clone()),
                target_addr: p.target_addr,
                target_name_idx: sym.insert(p.target_name.clone()),
                status: p.status,
            })
            .collect(),
        root_cause_hint: payload.root_cause_hint.clone(),
    }
}

pub fn encode_step_delta(
    prev: &StepPayload,
    curr: &StepPayload,
    sym: &mut SymbolTable,
    prev_vars: &HashMap<SymIdx, String>,
) -> StepPayloadDelta {
    let curr_vars: HashMap<String, &ApiVariableSnapshot> =
        curr.local_vars.iter().map(|v| (v.name.clone(), v)).collect();

    let mut var_deltas = Vec::new();
    let mut new_vars = Vec::new();
    let mut removed_var_name_indices = Vec::new();

    for v in &curr.local_vars {
        let name_idx = sym.insert(v.name.clone());
        if let Some(prev_val) = prev_vars.get(&name_idx) {
            if prev_val != &v.value {
                var_deltas.push(VarDelta {
                    name_idx,
                    value: v.value.clone(),
                });
            }
        } else {
            new_vars.push(ApiVarSnapshotRef {
                name_idx,
                addr: v.addr,
                is_local: v.is_local,
                ty_name_idx: sym.insert(v.ty_name.clone()),
                value: v.value.clone(),
            });
        }
    }

    for v in &prev.local_vars {
        let name_idx = sym.insert(v.name.clone());
        if !curr_vars.contains_key(&v.name) {
            removed_var_name_indices.push(name_idx);
        }
    }

    // call_stack 差分
    let call_stack = if diff::frame_lists_equal(&prev.call_stack, &curr.call_stack) {
        None
    } else {
        Some(
            curr.call_stack
                .iter()
                .map(|f| ApiFrameInfoRef {
                    func_name_idx: sym.insert(f.func_name.clone()),
                    return_line: f.return_line,
                })
                .collect(),
        )
    };

    // vis_events 差分
    let vis_events = if diff::vis_events_equal(&prev.vis_events, &curr.vis_events) {
        None
    } else {
        Some(curr.vis_events.clone())
    };

    // accessed_vars 差分
    let accessed_vars = if diff::accessed_vars_equal(&prev.accessed_vars, &curr.accessed_vars) {
        None
    } else {
        Some(
            curr.accessed_vars
                .iter()
                .map(|a| AccessedVarRef {
                    name_idx: sym.insert(a.name.clone()),
                    access_type_idx: sym.insert(a.access_type.clone()),
                })
                .collect(),
        )
    };

    // array_snapshots 差分：按数组名索引追踪新增/替换/删除
    let (array_snapshots, removed_array_name_indices) =
        diff_array_snapshots(&prev.array_snapshots, &curr.array_snapshots, sym);

    // pointer_snapshots 差分：按指针名索引追踪新增/替换/删除
    let (pointer_snapshots, removed_pointer_name_indices) =
        diff_pointer_snapshots(&prev.pointer_snapshots, &curr.pointer_snapshots, sym);

    StepPayloadDelta {
        step_index: curr.step_index,
        code_line: curr.code_line,
        func_name_idx: sym.insert(curr.func_name.clone()),
        semantic_label_idx: sym.insert(curr.semantic_label.clone()),
        algorithm_step: curr.algorithm_step.as_ref().map(|a| AlgorithmStepSnapshotRef {
            algorithm_name_idx: sym.insert(a.algorithm_name.clone()),
            display_name_idx: sym.insert(a.display_name.clone()),
            phase_idx: sym.insert(a.phase.clone()),
            description_idx: sym.insert(a.description.clone()),
        }),
        var_deltas,
        new_vars,
        removed_var_name_indices,
        call_stack,
        vis_events,
        heatmap_line: curr.heatmap_line,
        heatmap_count: curr.heatmap_count,
        accessed_vars,
        array_snapshots,
        removed_array_name_indices,
        pointer_snapshots,
        removed_pointer_name_indices,
        root_cause_hint: curr.root_cause_hint.clone(),
    }
}

fn diff_array_snapshots(
    prev: &[crate::unified::types::ArraySnapshot],
    curr: &[crate::unified::types::ArraySnapshot],
    sym: &mut SymbolTable,
) -> (Option<Vec<ArraySnapshotRef>>, Vec<SymIdx>) {
    if prev.len() == curr.len() && prev.iter().zip(curr.iter()).all(|(a, b)| diff::arrays_equal(a, b)) {
        return (None, Vec::new());
    }

    let curr_names: std::collections::HashSet<String> = curr.iter().map(|a| a.name.clone()).collect();
    let mut removed = Vec::new();
    for a in prev {
        if !curr_names.contains(&a.name) {
            removed.push(sym.insert(a.name.clone()));
        }
    }

    let snapshots = curr
        .iter()
        .map(|a| ArraySnapshotRef {
            name_idx: sym.insert(a.name.clone()),
            element_ty_idx: sym.insert(a.element_ty.clone()),
            elements: a.elements.clone(),
            truncated: a.truncated,
        })
        .collect();

    (Some(snapshots), removed)
}

fn diff_pointer_snapshots(
    prev: &[crate::unified::types::PointerSnapshot],
    curr: &[crate::unified::types::PointerSnapshot],
    sym: &mut SymbolTable,
) -> (Option<Vec<PointerSnapshotRef>>, Vec<SymIdx>) {
    if prev.len() == curr.len() && prev.iter().zip(curr.iter()).all(|(a, b)| diff::pointers_equal(a, b)) {
        return (None, Vec::new());
    }

    let curr_names: std::collections::HashSet<String> = curr.iter().map(|p| p.name.clone()).collect();
    let mut removed = Vec::new();
    for p in prev {
        if !curr_names.contains(&p.name) {
            removed.push(sym.insert(p.name.clone()));
        }
    }

    let snapshots = curr
        .iter()
        .map(|p| PointerSnapshotRef {
            name_idx: sym.insert(p.name.clone()),
            addr: p.addr,
            ty_name_idx: sym.insert(p.ty_name.clone()),
            target_addr: p.target_addr,
            target_name_idx: sym.insert(p.target_name.clone()),
            status: p.status,
        })
        .collect();

    (Some(snapshots), removed)
}
