use std::collections::{HashMap, HashSet};

use crate::context::VmContext;
use crate::core::{CallFrame, FreedRegionInfo, VitroVM};
use vitro_runtime::{FreeBlock, MemoryRegionData, MemoryState, OutputLog, RuntimeState, TraceEntryData, VisEventData};

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
    /// U2#3：结构换为有界 OutputLog（快照往返携带预算/丢弃计数，恢复后继续有界）。
    pub output: std::sync::Arc<OutputLog>,
    pub trace: std::sync::Arc<Vec<TraceEntryData>>,
    pub current_line: i32,
    pub input_index: usize,
    pub input_char_offset: usize,
    /// U2#11 后半：EOF 粘滞位与 heatmap 必须随快照往返——否则回到过去后
    /// 保留"未来"的值（EOF 已置位 → 重新执行时读取直接 EOF；行计数是
    /// 未来累计），回放行为与当时不一致（时间旅行确定性破坏）。
    pub stdin_eof: bool,
    pub heatmap: std::sync::Arc<vitro_runtime::ExecutionHeatmap>,
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
            output: rt.output.clone(),
            trace: rt.trace.clone(),
            current_line: rt.current_line,
            input_index: rt.input_index,
            input_char_offset: rt.input_char_offset,
            stdin_eof: rt.stdin_eof,
            heatmap: rt.heatmap.clone(),
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

        self.evict_over_limit();
    }

    /// 超上限时移除最旧检查点（U2#5：从 save 内抽出以便直接单测）。
    ///
    /// 不变量：**链中每个 Delta 的 `base_step` 必须解析到链内存在的 Full**。
    /// - step 0 锚点检查点永不裁剪（S3 A15：时间旅行起点，裁掉后越窗 seek
    ///   永久失败——实测 2050 步循环后 seek(5) 报"没有可用的检查点"）；
    /// - **删 Full 时必须级联删其后的 Delta 到下一个 Full**（U2#5 修复）：
    ///   悬空 Delta 的 base 指向被删的 Full，`nearest` 会把它叠到**更早**的
    ///   Full 上——脏页错叠 = **seek 静默错内存**（无报错的最坏调试器缺陷）。
    ///   原实现仅在删 Delta 时从链头级联，且 pinned 场景（[0] 恒 Full）删
    ///   [1] 的 Full 时悬空 Delta 留存到数量达标退出；
    /// - 删 Delta 无需级联：后继 Delta 与它同 base（基于最近 Full）。
    fn evict_over_limit(&mut self) {
        while self.checkpoints.len() > self.max_checkpoints {
            let pinned = self.checkpoints[0].0 == 0;
            let remove_idx = if pinned { 1 } else { 0 };
            if remove_idx >= self.checkpoints.len() {
                break;
            }
            let removed_is_full = matches!(self.checkpoints[remove_idx].1.memory, MemoryImage::Full(_));
            self.checkpoints.remove(remove_idx);
            if removed_is_full {
                // 删 Full：其后 Delta 全悬空（base 指向被删快照）——级联删到
                // 下一个 Full。U2#5 修复：原实现此分支无级联，pinned 场景删
                // [1] 的 Full 后悬空 Delta 留存（链头 [0] 恒 Full 掩盖了它）。
                let next_full = self.checkpoints[remove_idx..]
                    .iter()
                    .position(|(_, s)| matches!(s.memory, MemoryImage::Full(_)))
                    .map(|i| i + remove_idx)
                    .unwrap_or(self.checkpoints.len());
                self.checkpoints.drain(remove_idx..next_full);
            } else if remove_idx == 0 {
                // 删链头 Delta（防御——正常链头应恒为 Full）：删到 Full，
                // 维持原实现的链头不变量。
                while !self.checkpoints.is_empty() && !matches!(self.checkpoints[0].1.memory, MemoryImage::Full(_)) {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// 最小可测快照（仅 memory 有意义；其余字段零值——本测试只打淘汰/重建路径）。
    fn fake_snap(memory: MemoryImage) -> VMSnapshot {
        VMSnapshot {
            memory,
            stack: Vec::new(),
            call_stack: Vec::new(),
            ip: 0,
            mem_stack_top: 0,
            step_count: 0,
            current_line: 0,
            finished: false,
            exit_code: 0,
            error: String::new(),
            paused: false,
            cancelled: false,
            step_event_hit: false,
            last_snapshot_step: 0,
            snapshot_vars: std::collections::HashMap::new(),
            qsort_depth: 0,
            vis_event_queue: Vec::new(),
            breakpoints: std::collections::HashSet::new(),
            global_count: 0,
            freed_logs: std::collections::BTreeMap::new(),
            runtime: crate::snapshot::RuntimeSnapshot::from(&vitro_runtime::RuntimeState::default()),
            memory_state: crate::snapshot::MemorySnapshot::from(&vitro_runtime::MemoryState::default()),
        }
    }

    /// 链不变量：每个 Delta 的 base_step 都能解析到链内一个**步数不超过它**
    /// 的 Full（且该 Full 真正是它写作时的基准——用 base 命中即可证不悬空）。
    fn assert_no_dangling(mgr: &CheckpointManager) {
        for (i, (step, snap)) in mgr.checkpoints.iter().enumerate() {
            if let MemoryImage::Delta { base_step, .. } = &snap.memory {
                assert!(
                    mgr.checkpoints[..i]
                        .iter()
                        .any(|(s, b)| *s == *base_step && matches!(b.memory, MemoryImage::Full(_))),
                    "检查点[{}]（step={}）悬空：base_step={} 不在链内（seek 到此将叠错内存）",
                    i,
                    step,
                    base_step
                );
            }
        }
    }

    /// U2#5 红锚（修复前红）：pinned 场景（step 0 锚点）删除 [1] 的 Full 时，
    /// 其后基于该 Full 的 Delta 未级联删除——悬空留存，nearest 会把它们叠到
    /// step 0 的 Full 上（脏页错叠 = seek 静默错内存）。
    #[test]
    fn evict_full_cascades_dangling_deltas() {
        let mut mgr = CheckpointManager::new(20);
        mgr.max_checkpoints = 4;
        // 链：F@0(pinned), F@10, D@20(base 10), F@30, D@40(base 30)
        mgr.checkpoints = vec![
            (0, fake_snap(MemoryImage::Full(vec![0; 8]))),
            (10, fake_snap(MemoryImage::Full(vec![1; 8]))),
            (
                20,
                fake_snap(MemoryImage::Delta {
                    base_step: 10,
                    pages: vec![(0, vec![2; 8])],
                }),
            ),
            (30, fake_snap(MemoryImage::Full(vec![3; 8]))),
            (
                40,
                fake_snap(MemoryImage::Delta {
                    base_step: 30,
                    pages: vec![(0, vec![4; 8])],
                }),
            ),
        ];
        mgr.evict_over_limit();
        // 修复前：删 [1]（Full@10）→ D@20 悬空留存（链 [0]=F, [1]=D@20, [2]=F@30...）
        assert_no_dangling(&mgr);
    }

    /// 行为级锚：悬空链上 nearest(20) 的内存重建——修复前 D@20 被叠到 F@0
    /// 上（结果 vec![0,..] 脏页覆盖为 [2]），修复后 D@20 已被级联删除，
    /// nearest(20) 落到 F@0（vec![0;8]）——两者可区分（修复前输出 [2;8]）。
    #[test]
    fn nearest_on_evicted_chain_reconstructs_correctly() {
        let mut mgr = CheckpointManager::new(20);
        mgr.max_checkpoints = 4;
        mgr.checkpoints = vec![
            (0, fake_snap(MemoryImage::Full(vec![0; 8]))),
            (10, fake_snap(MemoryImage::Full(vec![1; 8]))),
            (
                20,
                fake_snap(MemoryImage::Delta {
                    base_step: 10,
                    pages: vec![(0, vec![2; 8])],
                }),
            ),
            (30, fake_snap(MemoryImage::Full(vec![3; 8]))),
            (
                40,
                fake_snap(MemoryImage::Delta {
                    base_step: 30,
                    pages: vec![(0, vec![4; 8])],
                }),
            ),
        ];
        mgr.evict_over_limit();
        let (step, snap) = mgr.nearest(20).expect("nearest 应有解");
        let mem = match snap.memory {
            MemoryImage::Full(m) => m,
            _ => panic!("nearest 应重建为 Full"),
        };
        // 修复后链 = [F@0, F@30, D@40]（D@20 已级联删）→ nearest(20) = F@0
        assert_eq!(step, 0, "D@20 被级联删除后应落到 F@0");
        assert_eq!(mem, vec![0u8; 8], "内存应为 F@0 基准（悬空叠错时为 [2;8]）");
    }

    /// 既有语义保持：删 Delta 无过度级联（同 base 的后继 Delta 不受影响）
    /// 与 pinned 锚点永不裁剪。
    #[test]
    fn evict_delta_keeps_siblings_and_pin() {
        let mut mgr = CheckpointManager::new(20);
        mgr.max_checkpoints = 3;
        mgr.checkpoints = vec![
            (0, fake_snap(MemoryImage::Full(vec![0; 8]))),
            (
                10,
                fake_snap(MemoryImage::Delta {
                    base_step: 0,
                    pages: vec![(0, vec![1; 8])],
                }),
            ),
            (
                20,
                fake_snap(MemoryImage::Delta {
                    base_step: 0,
                    pages: vec![(0, vec![2; 8])],
                }),
            ),
            (30, fake_snap(MemoryImage::Full(vec![3; 8]))),
        ];
        mgr.evict_over_limit();
        assert_eq!(mgr.checkpoints[0].0, 0, "step 0 锚点永不裁剪");
        assert_no_dangling(&mgr);
        // 同 base 的兄弟 Delta：删 [1]（Delta）不应级联删 [2]（同 base 0）
        assert!(mgr.checkpoints.iter().any(|(s, _)| *s == 20), "同 base 兄弟 Delta 不应被级联");
    }
}
