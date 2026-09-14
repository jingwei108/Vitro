//! 统一模式 StepStreamBatch 解码器。

use std::collections::HashMap;

use super::{StepPayloadRef, StepStreamBatch, SymIdx};
use crate::unified::types::{
    AccessedVar, AlgorithmStepSnapshot, ApiFrameInfo, ApiVariableSnapshot, ArraySnapshot, PointerSnapshot, StepPayload,
};

/// 将 StepStreamBatch 解码为完整的 StepPayload 列表。
pub fn decode_batch(batch: &StepStreamBatch) -> Vec<StepPayload> {
    if batch.base_payloads.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let base = &batch.base_payloads[0];
    result.push(decode_step_payload_ref(base, &batch.symbol_table));

    let mut current_vars: HashMap<SymIdx, ApiVariableSnapshot> = HashMap::new();
    for v in &base.local_vars {
        current_vars.insert(
            v.name_idx,
            ApiVariableSnapshot {
                name: get_sym(&batch.symbol_table, v.name_idx),
                addr: v.addr,
                is_local: v.is_local,
                ty_name: get_sym(&batch.symbol_table, v.ty_name_idx),
                value: v.value.clone(),
            },
        );
    }

    // 维护变量出现顺序（base 顺序 + 后续新增追加到末尾）
    let mut var_order: Vec<SymIdx> = base.local_vars.iter().map(|v| v.name_idx).collect();

    // 维护其他快照的当前状态，用于应用差分
    let mut current_call_stack: Vec<ApiFrameInfo> = base
        .call_stack
        .iter()
        .map(|f| ApiFrameInfo {
            func_name: get_sym(&batch.symbol_table, f.func_name_idx),
            return_line: f.return_line,
        })
        .collect();
    let mut current_vis_events: Vec<crate::session::VisEvent> = base.vis_events.clone();
    let mut current_accessed_vars: Vec<AccessedVar> = base
        .accessed_vars
        .iter()
        .map(|a| AccessedVar {
            name: get_sym(&batch.symbol_table, a.name_idx),
            access_type: get_sym(&batch.symbol_table, a.access_type_idx),
        })
        .collect();
    let mut current_arrays: Vec<ArraySnapshot> = base
        .array_snapshots
        .iter()
        .map(|a| ArraySnapshot {
            name: get_sym(&batch.symbol_table, a.name_idx),
            element_ty: get_sym(&batch.symbol_table, a.element_ty_idx),
            elements: a.elements.clone(),
            truncated: a.truncated,
        })
        .collect();
    let mut current_pointers: Vec<PointerSnapshot> = base
        .pointer_snapshots
        .iter()
        .map(|p| PointerSnapshot {
            name: get_sym(&batch.symbol_table, p.name_idx),
            addr: p.addr,
            ty_name: get_sym(&batch.symbol_table, p.ty_name_idx),
            target_addr: p.target_addr,
            target_name: get_sym(&batch.symbol_table, p.target_name_idx),
            status: p.status,
        })
        .collect();

    for delta in &batch.deltas {
        // 应用 var_deltas
        for d in &delta.var_deltas {
            if let Some(var) = current_vars.get_mut(&d.name_idx) {
                var.value = d.value.clone();
            }
        }
        // 添加新变量
        for v in &delta.new_vars {
            current_vars.insert(
                v.name_idx,
                ApiVariableSnapshot {
                    name: get_sym(&batch.symbol_table, v.name_idx),
                    addr: v.addr,
                    is_local: v.is_local,
                    ty_name: get_sym(&batch.symbol_table, v.ty_name_idx),
                    value: v.value.clone(),
                },
            );
        }
        // 移除消失的变量
        for &idx in &delta.removed_var_name_indices {
            current_vars.remove(&idx);
        }

        // 更新 var_order：移除已消失的，追加新出现的
        var_order.retain(|idx| !delta.removed_var_name_indices.contains(idx));
        for v in &delta.new_vars {
            if !var_order.contains(&v.name_idx) {
                var_order.push(v.name_idx);
            }
        }

        // 按 var_order 构建 local_vars
        let local_vars: Vec<ApiVariableSnapshot> =
            var_order.iter().filter_map(|idx| current_vars.get(idx).cloned()).collect();

        // 应用 call_stack 差分
        if let Some(ref frames) = delta.call_stack {
            current_call_stack = frames
                .iter()
                .map(|f| ApiFrameInfo {
                    func_name: get_sym(&batch.symbol_table, f.func_name_idx),
                    return_line: f.return_line,
                })
                .collect();
        }

        // 应用 vis_events 差分
        if let Some(ref events) = delta.vis_events {
            current_vis_events = events.clone();
        }

        // 应用 accessed_vars 差分
        if let Some(ref vars) = delta.accessed_vars {
            current_accessed_vars = vars
                .iter()
                .map(|a| AccessedVar {
                    name: get_sym(&batch.symbol_table, a.name_idx),
                    access_type: get_sym(&batch.symbol_table, a.access_type_idx),
                })
                .collect();
        }

        // 应用 array_snapshots 差分
        if let Some(ref arrays) = delta.array_snapshots {
            for &idx in &delta.removed_array_name_indices {
                let name = get_sym(&batch.symbol_table, idx);
                current_arrays.retain(|a| a.name != name);
            }
            for a in arrays {
                let decoded = ArraySnapshot {
                    name: get_sym(&batch.symbol_table, a.name_idx),
                    element_ty: get_sym(&batch.symbol_table, a.element_ty_idx),
                    elements: a.elements.clone(),
                    truncated: a.truncated,
                };
                if let Some(pos) = current_arrays.iter().position(|x| x.name == decoded.name) {
                    current_arrays[pos] = decoded;
                } else {
                    current_arrays.push(decoded);
                }
            }
        }

        // 应用 pointer_snapshots 差分
        if let Some(ref pointers) = delta.pointer_snapshots {
            for &idx in &delta.removed_pointer_name_indices {
                let name = get_sym(&batch.symbol_table, idx);
                current_pointers.retain(|p| p.name != name);
            }
            for p in pointers {
                let decoded = PointerSnapshot {
                    name: get_sym(&batch.symbol_table, p.name_idx),
                    addr: p.addr,
                    ty_name: get_sym(&batch.symbol_table, p.ty_name_idx),
                    target_addr: p.target_addr,
                    target_name: get_sym(&batch.symbol_table, p.target_name_idx),
                    status: p.status,
                };
                if let Some(pos) = current_pointers.iter().position(|x| x.name == decoded.name) {
                    current_pointers[pos] = decoded;
                } else {
                    current_pointers.push(decoded);
                }
            }
        }

        result.push(StepPayload {
            step_index: delta.step_index,
            code_line: delta.code_line,
            func_name: batch.symbol_table[delta.func_name_idx as usize].clone(),
            semantic_label: batch.symbol_table[delta.semantic_label_idx as usize].clone(),
            algorithm_step: delta.algorithm_step.as_ref().map(|a| AlgorithmStepSnapshot {
                algorithm_name: batch.symbol_table[a.algorithm_name_idx as usize].clone(),
                display_name: batch.symbol_table[a.display_name_idx as usize].clone(),
                phase: batch.symbol_table[a.phase_idx as usize].clone(),
                description: batch.symbol_table[a.description_idx as usize].clone(),
            }),
            local_vars,
            call_stack: current_call_stack.clone(),
            vis_events: current_vis_events.clone(),
            heatmap_line: delta.heatmap_line,
            heatmap_count: delta.heatmap_count,
            accessed_vars: current_accessed_vars.clone(),
            array_snapshots: current_arrays.clone(),
            pointer_snapshots: current_pointers.clone(),
            root_cause_hint: delta.root_cause_hint.clone(),
        });
    }

    result
}

fn decode_step_payload_ref(base: &StepPayloadRef, sym: &[String]) -> StepPayload {
    StepPayload {
        step_index: base.step_index,
        code_line: base.code_line,
        func_name: get_sym(sym, base.func_name_idx),
        semantic_label: get_sym(sym, base.semantic_label_idx),
        algorithm_step: base.algorithm_step.as_ref().map(|a| AlgorithmStepSnapshot {
            algorithm_name: get_sym(sym, a.algorithm_name_idx),
            display_name: get_sym(sym, a.display_name_idx),
            phase: get_sym(sym, a.phase_idx),
            description: get_sym(sym, a.description_idx),
        }),
        local_vars: base
            .local_vars
            .iter()
            .map(|v| ApiVariableSnapshot {
                name: get_sym(sym, v.name_idx),
                addr: v.addr,
                is_local: v.is_local,
                ty_name: get_sym(sym, v.ty_name_idx),
                value: v.value.clone(),
            })
            .collect(),
        call_stack: base
            .call_stack
            .iter()
            .map(|f| ApiFrameInfo {
                func_name: get_sym(sym, f.func_name_idx),
                return_line: f.return_line,
            })
            .collect(),
        vis_events: base.vis_events.clone(),
        heatmap_line: base.heatmap_line,
        heatmap_count: base.heatmap_count,
        accessed_vars: base
            .accessed_vars
            .iter()
            .map(|a| AccessedVar {
                name: get_sym(sym, a.name_idx),
                access_type: get_sym(sym, a.access_type_idx),
            })
            .collect(),
        array_snapshots: base
            .array_snapshots
            .iter()
            .map(|a| ArraySnapshot {
                name: get_sym(sym, a.name_idx),
                element_ty: get_sym(sym, a.element_ty_idx),
                elements: a.elements.clone(),
                truncated: a.truncated,
            })
            .collect(),
        pointer_snapshots: base
            .pointer_snapshots
            .iter()
            .map(|p| PointerSnapshot {
                name: get_sym(sym, p.name_idx),
                addr: p.addr,
                ty_name: get_sym(sym, p.ty_name_idx),
                target_addr: p.target_addr,
                target_name: get_sym(sym, p.target_name_idx),
                status: p.status,
            })
            .collect(),
        root_cause_hint: base.root_cause_hint.clone(),
    }
}

#[inline]
fn get_sym(sym: &[String], idx: SymIdx) -> String {
    sym.get(idx as usize).cloned().unwrap_or_default()
}
