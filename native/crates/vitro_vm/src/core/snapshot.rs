//! VM 快照：全量快照、增量快照与从快照恢复。

use super::state::VitroVM;
use crate::context::VmContext;

impl VitroVM {
    /// 创建全量快照。
    ///
    /// 调用者应确保 `session.compile` 中的编译产物与当前 VM 加载的程序一致。
    pub fn snapshot(&self, session: &VmContext<'_>) -> crate::snapshot::VMSnapshot {
        crate::snapshot::VMSnapshot {
            memory: crate::snapshot::MemoryImage::Full(self.memory.clone()),
            stack: self.stack.clone(),
            call_stack: self.call_stack.clone(),
            ip: self.ip,
            mem_stack_top: self.mem_stack_top,
            step_count: self.step_count,
            current_line: self.current_line,
            finished: self.finished,
            exit_code: self.exit_code,
            error: self.error.clone(),
            paused: self.paused,
            cancelled: self.cancelled,
            step_event_hit: self.step_event_hit,
            last_snapshot_step: self.last_snapshot_step,
            snapshot_vars: self.snapshot_vars.clone(),
            qsort_depth: self.qsort_depth,
            vis_event_queue: self.vis_event_queue.clone(),
            breakpoints: self.breakpoints.clone(),
            global_count: self.global_count,
            freed_logs: self.freed_logs.clone(),
            runtime: crate::snapshot::RuntimeSnapshot::from(&*session.runtime),
            memory_state: crate::snapshot::MemorySnapshot::from(&*session.memory),
        }
    }

    /// 基于脏页生成增量快照。
    pub fn snapshot_incremental(&self, session: &VmContext<'_>, base_step: i32) -> crate::snapshot::VMSnapshot {
        let mut pages = Vec::new();
        for word in 0..4 {
            let mut bitmap = self.dirty_pages[word];
            while bitmap != 0 {
                let bit = bitmap.trailing_zeros() as usize;
                bitmap &= !(1u64 << bit);
                let page_idx = word * 64 + bit;
                if page_idx >= crate::snapshot::PAGE_COUNT {
                    continue;
                }
                let offset = page_idx * crate::snapshot::PAGE_SIZE;
                let page_data = self.memory[offset..offset + crate::snapshot::PAGE_SIZE].to_vec();
                pages.push((page_idx as u16, page_data));
            }
        }
        crate::snapshot::VMSnapshot {
            memory: crate::snapshot::MemoryImage::Delta { base_step, pages },
            stack: self.stack.clone(),
            call_stack: self.call_stack.clone(),
            ip: self.ip,
            mem_stack_top: self.mem_stack_top,
            step_count: self.step_count,
            current_line: self.current_line,
            finished: self.finished,
            exit_code: self.exit_code,
            error: self.error.clone(),
            paused: self.paused,
            cancelled: self.cancelled,
            step_event_hit: self.step_event_hit,
            last_snapshot_step: self.last_snapshot_step,
            snapshot_vars: self.snapshot_vars.clone(),
            qsort_depth: self.qsort_depth,
            vis_event_queue: self.vis_event_queue.clone(),
            breakpoints: self.breakpoints.clone(),
            global_count: self.global_count,
            freed_logs: self.freed_logs.clone(),
            runtime: crate::snapshot::RuntimeSnapshot::from(&*session.runtime),
            memory_state: crate::snapshot::MemorySnapshot::from(&*session.memory),
        }
    }

    /// 将当前 VM 状态写入已有的 `VMSnapshot`，复用其内存 buffer。
    ///
    /// 当 `target.memory` 为 `MemoryImage::Full` 且长度匹配时，仅执行 `copy_from_slice`，
    /// 避免每步分配新的 1MB Vec。若类型或长度不匹配，则回退到 `clone()`。
    pub fn snapshot_into(&self, session: &VmContext<'_>, target: &mut crate::snapshot::VMSnapshot) {
        match &mut target.memory {
            crate::snapshot::MemoryImage::Full(buf) if buf.len() == self.memory.len() => {
                buf.copy_from_slice(&self.memory);
            }
            _ => {
                target.memory = crate::snapshot::MemoryImage::Full(self.memory.clone());
            }
        }
        target.stack = self.stack.clone();
        target.call_stack = self.call_stack.clone();
        target.ip = self.ip;
        target.mem_stack_top = self.mem_stack_top;
        target.step_count = self.step_count;
        target.current_line = self.current_line;
        target.finished = self.finished;
        target.exit_code = self.exit_code;
        target.error = self.error.clone();
        target.paused = self.paused;
        target.cancelled = self.cancelled;
        target.step_event_hit = self.step_event_hit;
        target.last_snapshot_step = self.last_snapshot_step;
        target.snapshot_vars = self.snapshot_vars.clone();
        target.qsort_depth = self.qsort_depth;
        target.vis_event_queue = self.vis_event_queue.clone();
        target.breakpoints = self.breakpoints.clone();
        target.global_count = self.global_count;
        target.freed_logs = self.freed_logs.clone();
        target.runtime = crate::snapshot::RuntimeSnapshot::from(&*session.runtime);
        target.memory_state = crate::snapshot::MemorySnapshot::from(&*session.memory);
    }

    /// 从快照恢复 VM 和 Session 运行时状态。
    ///
    /// 恢复前必须先调用 `setup_vm()` 加载编译产物，否则 `code`、`func_table` 等为空，
    /// 恢复后的 VM 将无法继续执行。
    pub fn restore(&mut self, snap: &crate::snapshot::VMSnapshot, session: &mut VmContext<'_>) {
        // VM 内存（1MB）
        snap.memory.apply_to(&mut self.memory);

        // VM 栈与调用帧
        self.stack = snap.stack.clone();
        self.call_stack = snap.call_stack.clone();

        // VM 执行指针与计数器
        self.ip = snap.ip;
        self.mem_stack_top = snap.mem_stack_top;
        self.step_count = snap.step_count;
        self.current_line = snap.current_line;

        // VM 状态标志
        self.finished = snap.finished;
        self.exit_code = snap.exit_code;
        self.error = snap.error.clone();
        self.paused = snap.paused;
        self.cancelled = snap.cancelled;
        self.step_event_hit = snap.step_event_hit;
        self.last_snapshot_step = snap.last_snapshot_step;
        self.snapshot_vars = snap.snapshot_vars.clone();
        self.qsort_depth = snap.qsort_depth;

        // 可视化与调试
        self.vis_event_queue = snap.vis_event_queue.clone();
        self.breakpoints = snap.breakpoints.clone();
        self.global_count = snap.global_count;

        // Session 运行时状态
        session.runtime.output_chunks = snap.runtime.output_chunks.clone();
        session.runtime.trace = snap.runtime.trace.clone();
        session.runtime.current_line = snap.runtime.current_line;
        session.runtime.input_index = snap.runtime.input_index;
        session.runtime.input_char_offset = snap.runtime.input_char_offset;
        session.runtime.waiting_input = snap.runtime.waiting_input;
        session.runtime.rand_seed = snap.runtime.rand_seed;
        session.runtime.vis_event_cache = snap.runtime.vis_event_cache.clone();
        session.runtime.ungetc_char = snap.runtime.ungetc_char;

        // Session 内存管理状态：逐字段恢复（MemorySnapshot 与 MemoryState 是不同类型）。
        // 隔离区三件套必须一起恢复（2026-09-11 堆决议），否则回退后 UAF 检测出现假阴性。
        session.memory.regions = snap.memory_state.regions.clone();
        session.memory.free_list = snap.memory_state.free_list.clone();
        session.memory.quarantine = snap.memory_state.quarantine.clone();
        session.memory.quarantine_bytes = snap.memory_state.quarantine_bytes;
        session.memory.quarantine_budget = snap.memory_state.quarantine_budget;
        session.memory.heap_base = snap.memory_state.heap_base;
        session.memory.heap_offset = snap.memory_state.heap_offset;
        session.memory.alloc_counter = snap.memory_state.alloc_counter;

        self.freed_logs = snap.freed_logs.clone();
    }
}
