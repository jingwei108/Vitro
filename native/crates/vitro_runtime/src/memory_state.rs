use std::collections::{HashMap, VecDeque};

/// 总内存空间（1MB）。
pub const MEM_SIZE: u32 = 1024 * 1024;
/// NULL 陷阱区大小。
pub const NULL_TRAP_SIZE: u32 = 0x1000;
/// 全局区起始地址。
pub const GLOBAL_START: u32 = 0x1000;
/// 堆区默认起始地址（全局数据未越过时的堆起点下限）。
pub const HEAP_START: u32 = 0x5000;
/// 全局数据区（全局变量/静态变量/vtable/字符串字面量）的编译期硬上限，绝对地址。
///
/// R1 ② 判据单源化：取代 codegen `literal.rs` 的 `MEM_SIZE / 16` 与 VM `setup_argv`
/// 的 `HEAP_START` 两套魔数——所有"全局区还能不能长"的判断统一以本常量为界。
/// 取值与旧字符串字面量判据（`MEM_SIZE / 16` = 64KB）保持一致，不收紧存量行为。
pub const GLOBAL_REGION_LIMIT: u32 = MEM_SIZE / 16;
/// 栈区起始地址（从高地址向低地址增长）。
pub const STACK_START: u32 = MEM_SIZE;
/// 快照间隔步数。
pub const SNAPSHOT_INTERVAL: i32 = 100_000;
/// 最大调用栈深度。
pub const MAX_STACK_DEPTH: usize = 10_000;
/// 隔离区默认字节预算：堆上限的 1/4 = 256KB。
///
/// 依据 [`堆有界隔离决议.md`] §1：采用 ASAN 原版的**有界隔离**
/// —— 隔离窗口保证 UAF/Double-Free 必被检出，超预算时 FIFO 驱逐最老块复用，
/// 保证合法 churn（分配-释放循环）不误伤。预算可按会话调整。
///
/// [`堆有界隔离决议.md`]: ../../../../docs/current/06-出口与协议/堆有界隔离决议.md
pub const DEFAULT_QUARANTINE_BUDGET: i32 = (MEM_SIZE / 4) as i32;

/// 4 字节向上对齐。
pub fn align4(x: u32) -> u32 {
    (x + 3) & !3
}

/// argv 区域占用字节数（指针数组 + 字符串数据，4 字节对齐）。
///
/// R1 ③：argv 自 `GLOBAL_REGION_LIMIT` 向下分配，本函数是占用的唯一计算口径，
/// `setup_argv` 的编址与 `compute_heap_base` 的堆起点都从它推导，杜绝两处各算一套。
pub fn argv_region_footprint(argc: i32, argv: &[String]) -> u32 {
    if argc <= 0 {
        return 0;
    }
    // 指针数组按 argc 开槽（未填满的槽为零，与 setup_argv 的写入布局一致），
    // 字符串数据只按实际存在的 min(argc, argv.len()) 条计数。
    let count = (argc as usize).min(argv.len());
    let strings: u64 = argv.iter().take(count).map(|s| s.len() as u64 + 1).sum();
    align4((argc as u64 * 4 + strings) as u32)
}

/// R1 ① 动态堆起点：`max(HEAP_START, align4(global_data_end))`；存在 argv 时
/// 还必须越过 argv 顶界（`GLOBAL_REGION_LIMIT`），否则堆 bump 会覆盖参数区。
///
/// `global_data_end` 为全局数据区末端的**绝对地址**（codegen 导出，
/// 含 Bytecode Libc 预留段），未编译时传 0 即退化为静态 `HEAP_START`。
pub fn compute_heap_base(global_data_end: u32, argc: i32, argv: &[String]) -> u32 {
    let mut base = HEAP_START.max(align4(global_data_end));
    if argc > 0 && !argv.is_empty() {
        base = base.max(GLOBAL_REGION_LIMIT);
    }
    base
}

/// 内存区域基础数据：VM 内部使用；`vitro_native` 会定义带 `#[frb]` 的同名包装。
///
/// `kind` 为**三段式内存地图**的段标识（`global` / `stack` / `heap`）。
/// 本结构只承载堆区域（`kind == "heap"`）；栈/全局区域由导出层
/// （`session_api::memory_regions`）从 VM 符号表与调用帧**合成**，不写回本清单
/// ——否则 `total_allocated` 之类的堆统计会把栈帧算成"已分配堆内存"。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryRegionData {
    pub addr: u32,
    pub size: i32,
    pub name: String,
    pub ty: String,
    pub is_heap: bool,
    pub is_freed: bool,
    /// 分配时的源码行号（教学用途）
    pub alloc_line: i32,
    /// 分配方式，如 "malloc" / "realloc" / "fopen" / "static" / "call"
    pub alloc_by: String,
    /// 段标识：`global` / `stack` / `heap`（下游需求清单 C2 三段式定型字段）。
    /// `serde(default)` = `heap`：旧快照/旧调用方不带该字段时按堆区域解析，向后兼容。
    #[serde(default = "default_region_kind")]
    pub kind: String,
}

/// `kind` 缺省值：本结构只装堆区域，故缺省即 `heap`。
fn default_region_kind() -> String {
    "heap".to_string()
}

/// 内存碎片基础数据。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryFragmentData {
    pub addr: u32,
    pub size: i32,
}

/// 堆统计信息基础数据。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeapStatsData {
    /// 本次运行的堆起点（R1：动态堆起点，原为常量 HEAP_START）
    #[serde(default)]
    pub heap_base: i32,
    /// 总堆空间（heap_offset - heap_base），字节
    pub total_heap: i32,
    /// 已分配且未释放的堆内存，字节
    pub allocated: i32,
    /// 外部碎片（free_list 中所有块之和），字节
    pub fragmented: i32,
    /// 碎片率（0~100）
    pub fragmentation_rate: i32,
}

/// 空闲内存块。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FreeBlock {
    pub addr: u32,
    pub size: i32,
}

/// 内存状态：跟踪堆分配、隔离区、可复用空闲块与堆顶偏移。
///
/// 堆模型（2026-09-11 决议）：**bump 分配 + 有界隔离**
/// - `malloc`：先按需驱逐隔离区，再 first-fit 复用驱逐块，否则 bump 推进 `heap_offset`；
/// - `free`：块进 `quarantine`（FIFO，地址暂不复用），超预算时驱逐最老块到 `free_list`；
/// - 因此 `free_list` 只承载"隔离期满已归还"的块，`heap_offset` 单调不减。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryState {
    pub regions: Vec<MemoryRegionData>,
    /// **addr → `regions` 下标索引**（U2#2，2026-09-14）：`regions` 只增不删
    /// （条目复用改字段、`clear` 整体重置），下标稳定——free / malloc 复用 /
    /// realloc 的按 addr 线性扫描（G6 实测稳态 ~16387 条 × 每次操作）经此索引
    /// 变 O(1)。**不变量：`region_index` 键集与 `regions` 的 addr 集一致、addr
    /// 唯一**；写入必须走 [`Self::push_region`]，快照恢复/整体重装后必须调
    /// [`Self::rebuild_region_index`]。不参与序列化（可从 `regions` 重建）。
    #[serde(skip)]
    pub region_index: std::collections::HashMap<u32, usize>,
    /// 可复用空闲块（来源：隔离区驱逐归还）。malloc 在此做 first-fit。
    pub free_list: Vec<FreeBlock>,
    /// 隔离区：已 free 但地址暂不复用的块，FIFO（队首最老）。
    #[serde(default)]
    pub quarantine: VecDeque<FreeBlock>,
    /// 隔离区当前占用字节数（与 `quarantine` 同步维护，避免每次求和）。
    #[serde(default)]
    pub quarantine_bytes: i32,
    /// 隔离区字节预算，超预算触发 FIFO 驱逐。
    #[serde(default = "default_quarantine_budget")]
    pub quarantine_budget: i32,
    /// 动态堆起点（R1 ①）：由全局数据上界与 argv 占用决定，运行重置时经
    /// `set_heap_base` 设置；统计口径（total_heap 等）统一以它为基准。
    #[serde(default = "default_heap_base")]
    pub heap_base: u32,
    pub heap_offset: u32,
    pub alloc_counter: i32,
}

fn default_quarantine_budget() -> i32 {
    DEFAULT_QUARANTINE_BUDGET
}

fn default_heap_base() -> u32 {
    HEAP_START
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
            region_index: std::collections::HashMap::new(),
            free_list: Vec::new(),
            quarantine: VecDeque::new(),
            quarantine_bytes: 0,
            quarantine_budget: DEFAULT_QUARANTINE_BUDGET,
            heap_base: HEAP_START,
            heap_offset: HEAP_START,
            alloc_counter: 0,
        }
    }
}

impl MemoryState {
    /// 设置本次运行的堆起点（R1 ①）：`heap_base` 与 `heap_offset` 必须同步重置，
    /// 运行入口（`reset_runtime`）统一经此落位，禁止各自直接写字段造成口径分裂。
    pub fn set_heap_base(&mut self, base: u32) {
        self.heap_base = base;
        self.heap_offset = base;
    }

    // ── U2#2 索引操作（addr → regions 下标，2026-09-14）────────────────────
    // 所有 regions 的写入路径必须经由这两个方法（或整体重装后 rebuild），
    // 保证 `region_index` 与 `regions` 的不变量。

    /// 追加一条 region 并登记索引。**调用方必须保证该 addr 尚无条目**
    /// （分配路径的新地址满足；复用路径应走 [`Self::find_region_mut`] 改字段）。
    #[inline]
    pub fn push_region(&mut self, data: MemoryRegionData) {
        self.region_index.insert(data.addr, self.regions.len());
        self.regions.push(data);
    }

    /// 按 addr 精确取条目（O(1)）。**只找 addr，不看 `is_freed`**——
    /// 调用方的语义判定（复用要求 is_freed、free 要求 !is_freed）在自己侧做。
    #[inline]
    pub fn find_region_mut(&mut self, addr: u32) -> Option<&mut MemoryRegionData> {
        let idx = *self.region_index.get(&addr)?;
        self.regions.get_mut(idx)
    }

    /// 从 `regions` 全量重建索引（快照恢复 / 反序列化 / 整体 `clear` 后调用）。
    /// O(N)，N 为稳态条目数（~16k），远小于 seek 的 O(距离) 重放成本。
    pub fn rebuild_region_index(&mut self) {
        self.region_index.clear();
        self.region_index.reserve(self.regions.len());
        for (i, r) in self.regions.iter().enumerate() {
            self.region_index.insert(r.addr, i);
        }
    }

    /// 索引一致性校验（测试/调试用）：键集与 addr 集、下标指向、addr 唯一。
    pub fn verify_region_index(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::with_capacity(self.regions.len());
        for (i, r) in self.regions.iter().enumerate() {
            if !seen.insert(r.addr) {
                return Err(format!("regions 内 addr 0x{:X} 重复（下标 {}）", r.addr, i));
            }
            match self.region_index.get(&r.addr) {
                Some(&j) if j == i => {}
                other => {
                    return Err(format!(
                        "索引失配：regions[{}] addr 0x{:X} 的索引指向 {:?}",
                        i, r.addr, other
                    ))
                }
            }
        }
        if self.region_index.len() != self.regions.len() {
            return Err(format!(
                "索引大小 {} != regions 大小 {}（存在悬空键）",
                self.region_index.len(),
                self.regions.len()
            ));
        }
        Ok(())
    }

    /// 分配 `aligned_size` 字节（决议 §1/§3 的 bump + 有界隔离）。
    ///
    /// 顺序：
    /// 1. 隔离区超预算 → FIFO 驱逐最老块归还 `free_list`；
    /// 2. `free_list` first-fit 复用（来源见 1）；
    /// 3. 否则 bump 推进 `heap_offset` —— leak 路径由此单调推进直至撞内存墙。
    ///
    /// 成功返回地址，超出 `mem_limit` 返回 None。
    pub fn allocate_raw(&mut self, aligned_size: u32, mem_limit: u32) -> Option<u32> {
        if aligned_size == 0 {
            return Some(0);
        }
        self.evict_quarantine();
        if let Some(addr) = self.take_from_free_list(aligned_size) {
            return Some(addr);
        }
        let addr = self.heap_offset;
        let new_offset = addr as u64 + aligned_size as u64;
        if new_offset > mem_limit as u64 || new_offset > u32::MAX as u64 {
            return None;
        }
        self.heap_offset = new_offset as u32;
        Some(addr)
    }

    /// 从 `free_list` 做 first-fit 取块；块大于请求时切分，余量留在表中。
    fn take_from_free_list(&mut self, aligned_size: u32) -> Option<u32> {
        let idx = self
            .free_list
            .iter()
            .position(|b| (b.size as u32) >= aligned_size)?;
        let addr = self.free_list[idx].addr;
        if (self.free_list[idx].size as u32) > aligned_size {
            self.free_list[idx].addr += aligned_size;
            self.free_list[idx].size -= aligned_size as i32;
        } else {
            self.free_list.remove(idx);
        }
        Some(addr)
    }

    /// 块进入 FIFO 隔离区 —— free 路径的唯一出口，地址在隔离期内不复用。
    pub fn release_to_quarantine(&mut self, block: FreeBlock) {
        self.quarantine_bytes += block.size;
        self.quarantine.push_back(block);
    }

    /// 隔离区超预算时按 FIFO 驱逐最老块至 `free_list`，直至回到预算内。
    ///
    /// 这是 churn 与 leak 的分水岭：合法循环的隔离占用稳定在预算内（最老块持续
    /// 被驱逐复用，堆顶不推进，无限可跑）；只分配不释放的 leak 块根本不进隔离区，
    /// 由 bump 持续推进直至撞 1MB 墙（教学信号）。
    fn evict_quarantine(&mut self) {
        if self.quarantine_bytes <= self.quarantine_budget {
            return;
        }
        while self.quarantine_bytes > self.quarantine_budget {
            let Some(block) = self.quarantine.pop_front() else {
                break;
            };
            self.quarantine_bytes -= block.size;
            self.free_list.push(block);
        }
        // 合并相邻块，提升后续 first-fit 命中率（仅驱逐路径的代价）
        self.merge_free_list();
    }

    /// 合并 free_list 中地址相邻的空闲块。
    pub fn merge_free_list(&mut self) {
        self.free_list.sort_by_key(|b| b.addr);
        let mut merged: Vec<FreeBlock> = Vec::new();
        for block in self.free_list.drain(..) {
            if let Some(last) = merged.last_mut() {
                if (last.addr as u64) + (last.size as u64) == (block.addr as u64) {
                    last.size += block.size;
                } else {
                    merged.push(block);
                }
            } else {
                merged.push(block);
            }
        }
        self.free_list = merged;
    }

    /// 释放 `addr` 对应的已分配堆区域（若存在且未释放）。
    ///
    /// 块**进入隔离区**（地址在隔离期内不复用），不直接归还 `free_list` ——
    /// 这是 UAF / Double-Free 检测窗口的物理基础（决议 §1/§3）。
    /// 成功释放返回 `true`，找不到对应区域或已释放返回 `false`。
    pub fn free_region(&mut self, addr: u32) -> bool {
        // U2#2：addr 索引 O(1) 定位（旧线性扫描——vfs 释放路径同享索引收益）
        let block = self.find_region_mut(addr).filter(|r| !r.is_freed).map(|r| {
            r.is_freed = true;
            FreeBlock {
                addr,
                size: (((r.size as u32) + 3) & !3) as i32,
            }
        });
        match block {
            Some(b) => {
                self.release_to_quarantine(b);
                true
            }
            None => false,
        }
    }
}

/// 从 regions 中按地址查找指定内存块（供测试与诊断使用）。
pub fn find_region_by_addr(regions: &[MemoryRegionData], addr: u32) -> Option<&MemoryRegionData> {
    regions.iter().find(|r| r.addr == addr)
}

/// 计算当前已分配但未释放的堆内存总量。
pub fn total_allocated(regions: &[MemoryRegionData]) -> i32 {
    regions.iter().filter(|r| !r.is_freed).map(|r| r.size).sum()
}

/// 估算外部碎片大小（简化版：free_list 总和）。
pub fn total_fragmented(free_list: &[FreeBlock]) -> i32 {
    free_list.iter().map(|b| b.size).sum()
}

/// 计算碎片率（0~100）。`heap_base` 为本次运行的动态堆起点（R1）。
pub fn fragmentation_rate(free_list: &[FreeBlock], heap_offset: u32, heap_base: u32) -> i32 {
    let heap_total = heap_offset.saturating_sub(heap_base);
    if heap_total == 0 {
        return 0;
    }
    let fragmented = total_fragmented(free_list) as u64;
    let rate = (fragmented * 100) / (heap_total as u64);
    rate.min(100) as i32
}

/// 构建教学用的 `HeapStatsData` 快照。
pub fn build_heap_stats(
    regions: &[MemoryRegionData],
    free_list: &[FreeBlock],
    heap_offset: u32,
    heap_base: u32,
) -> HeapStatsData {
    let total_heap = heap_offset.saturating_sub(heap_base) as i32;
    let allocated = total_allocated(regions);
    let fragmented = total_fragmented(free_list);
    let rate = fragmentation_rate(free_list, heap_offset, heap_base);
    HeapStatsData {
        heap_base: heap_base as i32,
        total_heap,
        allocated,
        fragmented,
        fragmentation_rate: rate,
    }
}

/// 保留 `MemoryRegionData` 作为键的兼容性辅助：按地址分组。
pub fn group_regions_by_name(regions: &[MemoryRegionData]) -> HashMap<String, Vec<&MemoryRegionData>> {
    let mut map: HashMap<String, Vec<&MemoryRegionData>> = HashMap::new();
    for r in regions {
        map.entry(r.name.clone()).or_default().push(r);
    }
    map
}
