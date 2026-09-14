use std::collections::{HashMap, HashSet};

use crate::context::VmContext;
use crate::core::{CallFrame, VitroVM, FreedRegionInfo};
use vitro_runtime::{FreeBlock, MemoryRegionData, MemoryState, OutputChunk, RuntimeState, TraceEntryData, VisEventData};

/// VM 内存快照：全量或页级增量（页大小 4KB）。
#[derive(Clone)]
pub enum MemoryImage {
    /// 完整 1MB 内存拷贝。
    Full(Vec<u8>),
    /// 相对于某全量检查点的脏页集合。
    /// 页索引 0..255，每页 4096 字节。
    Delta { base_step: i32, pages: Vec<(u16, Vec<u8>)> },
}

impl MemoryImage {
    /// 获取内存总字节数（用于调试/统计）。
    pub fn byte_size(&self) -> usize {
        match self {
            MemoryImage::Full(v) => v.len(),
            MemoryImage::Delta { pages, .. } => pages.iter().map(|(_, p)| p.len()).sum(),
        }
    }

    /// 重建完整 1MB 内存，写入到提供的 buffer 中。
    /// 调用者应确保 `dst` 长度至少为 1MB，且已填充基础内存内容。
    pub fn apply_to(&self, dst: &mut [u8]) {
        match self {
            MemoryImage::Full(v) => {
                let len = v.len().min(dst.len());
                dst[..len].copy_from_slice(&v[..len]);
            }
            MemoryImage::Delta { pages, .. } => {
                for (page_idx, page_data) in pages {
                    let offset = (*page_idx as usize) * PAGE_SIZE;
                    if offset + page_data.len() <= dst.len() {
                        dst[offset..offset + page_data.len()].copy_from_slice(page_data);
                    }
                }
            }
        }
    }
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 256; // 1MB / 4KB

/// VM 全量/增量快照。
///
/// 注意：快照**不保存**编译期常量（bytecode、函数表、符号表等），
/// 因为这些可以从 `Session.compile` 重建。
/// 使用快照前，必须先调用 `setup_vm()` 确保 VM 已加载程序。
#[derive(Clone)]
pub struct VMSnapshot {
    // VM 核心运行时状态
    pub memory: MemoryImage,
    pub stack: Vec<u64>,
    pub call_stack: Vec<CallFrame>,
    pub ip: usize,
    pub mem_stack_top: u32,
    pub step_count: i32,
    pub current_line: i32,
    pub finished: bool,
    pub exit_code: i32,
    pub error: String,
    pub paused: bool,
    pub cancelled: bool,
    pub step_event_hit: bool,
    pub last_snapshot_step: i32,
    pub snapshot_vars: HashMap<String, u64>,
    pub qsort_depth: i32,
    pub vis_event_queue: Vec<VisEventData>,
    pub breakpoints: HashSet<i32>,
    pub global_count: usize,
    pub freed_logs: std::collections::BTreeMap<u32, FreedRegionInfo>,
    // Session 运行时/内存状态快照
    pub runtime: RuntimeSnapshot,
    pub memory_state: MemorySnapshot,
}

/// 运行时状态快照子集（不含输入/输出等大字段）。
#[derive(Clone)]
pub struct RuntimeSnapshot {
    /// E-P1-5：快照携带完整输出分段（含通道标记），时间旅行回退后仍能区分
    /// 程序 stdout 与引擎附注，不会退回"文本清洗"口径。
    pub output_chunks: Vec<OutputChunk>,
    pub trace: Vec<TraceEntryData>,
    pub current_line: i32,
    pub input_index: usize,
    pub input_char_offset: usize,
    pub waiting_input: bool,
    pub rand_seed: u32,
    pub vis_event_cache: Vec<VisEventData>,
    pub ungetc_char: Option<i32>,
}

/// 内存状态快照子集。
#[derive(Clone)]
pub struct MemorySnapshot {
    pub regions: Vec<MemoryRegionData>,
    pub free_list: Vec<FreeBlock>,
    /// 隔离区（FIFO，队首最老）——2026-09-11 堆决议引入，必须随快照往返，
    /// 否则时间旅行回退后隔离窗口丢失，UAF/Double-Free 检测出现假阴性。
    pub quarantine: std::collections::VecDeque<FreeBlock>,
    pub quarantine_bytes: i32,
    pub quarantine_budget: i32,
    /// 动态堆起点（R1）：时间旅行回退后统计口径仍须以本次运行的堆起点为基准。
    pub heap_base: u32,
    pub heap_offset: u32,
    pub alloc_counter: i32,
}

impl From<&RuntimeState> for RuntimeSnapshot {
    fn from(rt: &RuntimeState) -> Self {
        Self {
            output_chunks: rt.output_chunks.clone(),
            trace: rt.trace.clone(),
            current_line: rt.current_line,
            input_index: rt.input_index,
            input_char_offset: rt.input_char_offset,
            waiting_input: rt.waiting_input,
            rand_seed: rt.rand_seed,
            vis_event_cache: rt.vis_event_cache.clone(),
            ungetc_char: rt.ungetc_char,
        }
    }
}

impl From<&MemoryState> for MemorySnapshot {
    fn from(mem: &vitro_runtime::MemoryState) -> Self {
        Self {
            regions: mem.regions.clone(),
            free_list: mem.free_list.clone(),
            quarantine: mem.quarantine.clone(),
            quarantine_bytes: mem.quarantine_bytes,
            quarantine_budget: mem.quarantine_budget,
            heap_base: mem.heap_base,
            heap_offset: mem.heap_offset,
            alloc_counter: mem.alloc_counter,
        }
    }
}

/// 检查点管理器：定期保存 VM 快照，用于 Seek 恢复。
///
/// 支持两种快照模式：
/// - **全量快照**：每 `full_every` 个检查点保存一次完整 1MB 内存。
/// - **增量快照**：其余检查点仅保存自上一个全量检查点以来被修改的 4KB 页。
///
/// 增量快照可将单检查点内存从 ~1MB 降至典型 ~50-200KB（取决于程序行为），
/// 在 50 个检查点上限下整体内存占用从 50MB 降至约 5-10MB。
#[derive(Clone, Default)]
pub struct CheckpointManager {
    pub checkpoints: Vec<(i32, VMSnapshot)>,
    /// 固定间隔（步数），保底策略。
    pub interval: i32,
    /// 智能检查点：在控制流边界（函数调用/返回/循环/交换/内存操作）额外保存。
    pub smart_mode: bool,
    /// 最大检查点数量，防止长程序运行时内存无限增长。
    pub max_checkpoints: usize,
    /// 每 N 个检查点强制一个全量基准。
    pub full_every: usize,
}

impl CheckpointManager {
    pub fn new(interval: i32) -> Self {
        Self {
            checkpoints: Vec::new(),
            interval,
            smart_mode: true,
            max_checkpoints: 50,
            full_every: 5,
        }
    }

    /// 判断当前步是否需要保存检查点。
    ///
    /// 固定间隔保底 + 智能模式在控制流边界触发。
    pub fn should_checkpoint(&self, step: i32, semantic_label: &str) -> bool {
        // 固定间隔保底
        if step % self.interval == 0 {
            return true;
        }

        if !self.smart_mode {
            return false;
        }

        // 智能策略：函数调用、返回、循环边界、数组交换、内存操作
        let is_significant = semantic_label.starts_with("调用 ")
            || semantic_label == "返回"
            || semantic_label == "内存分配"
            || semantic_label == "释放内存"
            || semantic_label.contains("交换")
            || semantic_label.starts_with("循环");

        if !is_significant {
            return false;
        }

        // 避免检查点过于密集：距离上一个检查点至少 interval/4 步
        let min_gap = self.interval.max(4) / 4;
        if let Some((last_step, _)) = self.checkpoints.last() {
            if step - *last_step < min_gap {
                return false;
            }
        }

        true
    }

    /// 保存检查点。超过上限时自动移除最旧的检查点。
    ///
    /// 调用者负责在保存前通过 `should_checkpoint` 判断是否需要保存。
    pub fn save(&mut self, step: i32, vm: &mut VitroVM, ctx: &mut VmContext<'_>) {
        let is_full = self.checkpoints.is_empty() || self.checkpoints.len().is_multiple_of(self.full_every);

        let snap = if is_full {
            vm.clear_dirty_pages();
            vm.snapshot(ctx)
        } else {
            let base_step = self
                .checkpoints
                .iter()
                .rev()
                .find(|(_, s)| matches!(s.memory, MemoryImage::Full(_)))
                .map(|(s, _)| *s)
                .unwrap_or(0);
            vm.snapshot_incremental(ctx, base_step)
        };

        self.checkpoints.push((step, snap));

        // 移除最旧检查点；如果移除的是全量基准，需要把下一个全量之前的增量全删掉，
        // 否则增量会 dangling。简化处理：一直删到第一个是全量为止。
        // S3 A15（schema v0.1 签字回放）：step 0 锚点检查点**永不裁剪**——它是
        // 时间旅行的起点，裁掉后越窗 seek（目标 < 窗口起点）将永久失败
        //（实测 2050 步循环后 seek(5) 报"没有可用的检查点"）。
        while self.checkpoints.len() > self.max_checkpoints {
            let pinned = self.checkpoints[0].0 == 0;
            let remove_idx = if pinned { 1 } else { 0 };
            if remove_idx >= self.checkpoints.len() {
                break;
            }
            let removed_is_full = matches!(self.checkpoints[remove_idx].1.memory, MemoryImage::Full(_));
            self.checkpoints.remove(remove_idx);
            if !removed_is_full {
                // 如果删掉的是增量，继续删到下一个全量，保证链头是全量基准
                while self.checkpoints.len() > 1
                    && !matches!(self.checkpoints[0].1.memory, MemoryImage::Full(_))
                {
                    self.checkpoints.remove(0);
                }
            }
        }
    }

    /// 找到不超过 target 的最近检查点，并重建为可直接恢复的全量快照。
    pub fn nearest(&self, target: i32) -> Option<(i32, VMSnapshot)> {
        let idx = self.checkpoints.iter().rposition(|(s, _)| *s <= target)?;
        let (step, snap) = &self.checkpoints[idx];

        match &snap.memory {
            MemoryImage::Full(_) => Some((*step, snap.clone())),
            MemoryImage::Delta { base_step, .. } => {
                // 找到基础全量检查点
                let base_idx = self
                    .checkpoints
                    .iter()
                    .rposition(|(s, snap)| *s <= *base_step && matches!(snap.memory, MemoryImage::Full(_)))?;
                let base_snap = &self.checkpoints[base_idx].1;

                // 从 base 到 target 之间的所有增量应用到基础内存
                let mut full_memory = match &base_snap.memory {
                    MemoryImage::Full(m) => m.clone(),
                    _ => return None, // 不应该发生
                };

                for (_, intermediate) in &self.checkpoints[base_idx + 1..=idx] {
                    intermediate.memory.apply_to(&mut full_memory);
                }

                let mut reconstructed = snap.clone();
                reconstructed.memory = MemoryImage::Full(full_memory);
                Some((*step, reconstructed))
            }
        }
    }

    /// 清除所有检查点。
    pub fn clear(&mut self) {
        self.checkpoints.clear();
    }

    /// 获取当前保存的检查点数量。
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}
