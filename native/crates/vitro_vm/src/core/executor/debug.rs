use super::*;

/// vis_event_queue 环形上限（U2#8）：非 unified 出口（vitro_run/serve/cli run）
/// 全程不 drain，算法模板命中行在循环内每步 push 一条——上限后丢最旧，
/// 消除 10M 步数百 MB~GB 常驻（统一模式 collector 每步 take，不触上限）。
pub const VIS_EVENT_QUEUE_LIMIT: usize = 1024;

impl VitroVM {
    pub(crate) fn execute_debug(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) -> Option<StepResult> {
        match op {
            OpCode::StepEvent => {
                self.current_line = operand;
                if self.breakpoints.contains(&self.current_line) {
                    self.paused = true;
                }
                self.step_event_hit = true;
                for &(line, ty, ref ctx) in &self.vis_event_lines {
                    if line == operand {
                        self.vis_event_queue.push(VisEventData {
                            ty,
                            line: operand,
                            extra0: 0,
                            extra1: 0,
                            extra2: 0,
                            context: ctx.clone(),
                        });
                    }
                }
                let overflow = self.vis_event_queue.len().saturating_sub(VIS_EVENT_QUEUE_LIMIT);
                if overflow > 0 {
                    self.vis_event_queue.drain(..overflow);
                }
                if self.paused {
                    return Some(StepResult::Paused);
                }
                None
            }
            OpCode::TrapBounds => {
                let index = if let Some(&val) = self.stack.last() {
                    val as i64
                } else {
                    self.trap("TrapBounds: 值栈为空，无法获取索引", loc);
                    return Some(StepResult::Trap);
                };
                let mut name = "数组".to_string();
                let mut size = 0;
                if operand >= 0 {
                    let sym_idx = operand as usize;
                    if sym_idx < self.symbols.len() {
                        let sym = &self.symbols[sym_idx];
                        name = sym.name.clone();
                        size = sym.ty.array_size();
                    }
                } else {
                    size = -operand;
                }
                if index < 0 || index >= size as i64 {
                    let diag = if operand >= 0 {
                        format!(
                                        "🚫 数组越界：你访问了 {}[{}]，但数组 '{}' 只有 {} 个元素，有效索引是 0~{}。\n\n💡 原因：数组索引超出了合法范围。\n✅ 检查方法：确认索引变量值在 0 到 {} 之间。",
                                        name, index, name, size, size.saturating_sub(1), size.saturating_sub(1)
                                    )
                    } else {
                        format!(
                            "🚫 数组越界：索引 {} 超出了合法范围 0~{}。\n\n💡 原因：数组索引超出了合法范围。",
                            index,
                            size.saturating_sub(1)
                        )
                    };
                    self.trap(&diag, loc);
                }
                None
            }
            OpCode::TrapBoundsVla => {
                let bound = self.pop() as i64;
                let index = if let Some(&val) = self.stack.last() {
                    val as i64
                } else {
                    self.trap("TrapBoundsVla: 值栈为空，无法获取索引", loc);
                    return Some(StepResult::Trap);
                };
                let mut name = "数组".to_string();
                if operand >= 0 {
                    let sym_idx = operand as usize;
                    if sym_idx < self.symbols.len() {
                        name = self.symbols[sym_idx].name.clone();
                    }
                }
                if index < 0 || index >= bound {
                    let diag = format!(
                        "🚫 数组越界：你访问了 {}[{}]，但数组 '{}' 当前只有 {} 个元素，有效索引是 0~{}。\n\n💡 原因：变长数组(VLA)索引超出了运行时边界。\n✅ 检查方法：确认索引变量值在 0 到 {} 之间。",
                        name, index, name, bound, bound.saturating_sub(1), bound.saturating_sub(1)
                    );
                    self.trap(&diag, loc);
                }
                None
            }
            _ => None,
        }
    }

    // --- Single instruction dispatch (used by both step() and JIT generic fallback) ---
}
