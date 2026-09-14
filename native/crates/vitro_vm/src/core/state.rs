//! VM 状态管理：VitroVM 结构体、生命周期、快照、调用帧与符号管理。

use crate::context::VmContext;
use crate::instruction::{Instruction, SourceLoc};
use crate::jit_trace::{JitStats, TraceRecorder};
use vitro_runtime::VisEventData;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// R1 ②：内存布局常量单源化——`vitro_runtime` 为唯一真相，本文件仅再导出。
// （此前与 `vitro_runtime::memory_state` 同值双写，属"同一概念多个真相来源"。）
pub use vitro_runtime::{
    GLOBAL_START, HEAP_START, MAX_STACK_DEPTH, MEM_SIZE, NULL_TRAP_SIZE, SNAPSHOT_INTERVAL, STACK_START,
};

pub use vitro_runtime::{FuncMeta, Symbol as VMSymbol};

#[derive(Debug, Clone)]
pub struct FreedRegionInfo {
    pub addr: u32,
    pub size: u32,
    pub alloc_line: i32,
    pub freed_line: i32,
    /// Step index when the memory was allocated (for unified-mode timeline).
    pub alloc_step: i32,
    /// Step index when the memory was freed (for unified-mode timeline).
    pub freed_step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame {
    pub return_ip: usize,
    pub locals_base: u32,
    pub local_count: i32,
    pub func_name: String,
    pub original_stack_top: u32,
    pub caller_line: i32,
    /// 栈上局部数组缓冲区表（V-P1-6），随函数 meta 克隆进帧。
    pub local_buffers: Vec<vitro_runtime::LocalBuffer>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    Ok,
    Paused,
    Finished,
    Trap,
    WaitingInput,
}

#[derive(Debug, Clone)]
pub struct ArrayConstructionGuard {
    /// `new[]` 分配返回给用户的指针之前 4 字节的 base 地址（存储 count）。
    pub base_addr: u32,
    /// 设置 guard 时的 call_stack 深度，用于确认 guard 设置者的栈帧仍然存在。
    pub frame_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct VariableAccess {
    pub name: String,
    pub access_type: AccessType,
}

use crate::jit_templates::CompiledTrace;

pub struct VitroVM {
    pub(crate) code: Vec<Instruction>,
    pub(crate) ip: usize,
    pub(crate) memory: Vec<u8>,
    pub(crate) stack: Vec<u64>,
    pub(crate) mem_stack_top: u32,
    pub(crate) global_count: usize,
    pub(crate) call_stack: Vec<CallFrame>,
    pub(crate) func_table: Vec<FuncMeta>,
    pub(crate) func_names: Vec<String>,
    pub(crate) symbols: Vec<VMSymbol>,
    pub(crate) vis_event_lines: Vec<(i32, i32, String)>,
    pub(crate) vis_event_queue: Vec<VisEventData>,
    pub(crate) breakpoints: HashSet<i32>,
    pub(crate) paused: bool,
    pub(crate) cancelled: bool,
    pub(crate) step_event_hit: bool,
    pub(crate) step_count: i32,
    pub(crate) max_steps: i32,
    /// 调用深度上限（会话级可配，V-P1-10）。教学场景下"可控地撞上限并拿到教学
    /// trap"优于无限等待；`vitro_set_call_depth_limit` 直接映射本字段。
    pub(crate) call_depth_limit: usize,
    pub(crate) current_line: i32,
    pub(crate) error: String,
    pub(crate) last_snapshot_step: i32,
    pub(crate) snapshot_vars: HashMap<String, u64>,
    pub(crate) finished: bool,
    pub(crate) exit_code: i32,
    pub(crate) qsort_depth: i32,
    pub(crate) f64_constants: Vec<f64>,
    pub(crate) i64_constants: Vec<i64>,
    pub(crate) last_accessed_vars: Vec<VariableAccess>,
    pub(crate) local_sym_map: HashMap<i32, String>,
    pub(crate) global_sym_map: HashMap<i32, String>,
    /// 已释放块登记（UAF / Double-Free 检测窗口）。
    /// U2#2-b（2026-09-14）：Vec → **BTreeMap<addr, info>**——churn 稳态
    /// ~16k 条（隔离区容量内每块一条），原 Vec 的每次 malloc `retain`
    /// 区间清理 / free 的 Double-Free `find` / 访存的 `check_uaf` 全是
    /// O(16k) 线性扫（U2#2-a 后 1M churn 剩余 34.8s 的归因大头）。
    /// **互不重叠不变量**（分配器保证）⟹ addr 序即 end 序：区间查询/删除
    /// 按 addr 降序扫、遇 end ≤ 查询起点即停，O(log n + 命中数)。
    /// addr 唯一（Double-Free 拦截在先）；迭代序 = addr 升序（原 Vec 为
    /// free 时间序——经查无顺序依赖消费者）。
    pub(crate) freed_logs: std::collections::BTreeMap<u32, FreedRegionInfo>,
    /// 当前未完成的 `new T[n]` 构造守卫。若构造过程中 trap，用于回滚释放内存。
    pub pending_array_construction: Option<ArrayConstructionGuard>,
    /// `main(int argc, char *argv[])` 的 argc 值。
    pub(crate) argc: i32,
    /// `main(int argc, char *argv[])` 的 argv 数组在 VM 内存中的起始地址。
    pub(crate) argv_addr: u32,
    // --- 增量快照脏页追踪 ---
    pub(crate) dirty_pages: [u64; 4], // 256 页 bitmap（4 × 64bit）
    // --- JIT ---
    pub(crate) ip_hits: HashMap<usize, u64>,
    pub(crate) trace_recorder: TraceRecorder,
    pub(crate) jit_traces: HashMap<usize, Arc<CompiledTrace>>,
    pub(crate) jit_stats: JitStats,
    /// JIT 总开关（默认开启）。关闭时 fast path 不命中、热点检测与 trace 录制
    /// 均不触发——这是结构性禁用，供基准对照（vm_bench）与诊断隔离使用；
    /// 与 `jit_traces_mut().clear()` 不同，后者清表后 `ip_hits` 仍会累积并重新录制。
    pub(crate) jit_enabled: bool,
}

impl Default for VitroVM {
    fn default() -> Self {
        Self::new()
    }
}

impl VitroVM {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            ip: 0,
            memory: vec![0; MEM_SIZE as usize],
            stack: Vec::new(),
            mem_stack_top: STACK_START,
            global_count: 0,
            call_stack: Vec::new(),
            func_table: Vec::new(),
            func_names: Vec::new(),
            symbols: Vec::new(),
            vis_event_lines: Vec::new(),
            vis_event_queue: Vec::new(),
            breakpoints: HashSet::new(),
            paused: false,
            cancelled: false,
            step_event_hit: false,
            step_count: 0,
            max_steps: 10_000_000,
            call_depth_limit: MAX_STACK_DEPTH,
            current_line: 0,
            error: String::new(),
            last_snapshot_step: 0,
            snapshot_vars: HashMap::new(),
            finished: false,
            exit_code: 0,
            qsort_depth: 0,
            f64_constants: Vec::new(),
            i64_constants: Vec::new(),
            last_accessed_vars: Vec::new(),
            local_sym_map: HashMap::new(),
            global_sym_map: HashMap::new(),
            freed_logs: std::collections::BTreeMap::new(),
            pending_array_construction: None,
            argc: 0,
            argv_addr: 0,
            dirty_pages: [0; 4],
            ip_hits: HashMap::new(),
            trace_recorder: TraceRecorder::new(),
            jit_traces: HashMap::new(),
            jit_stats: JitStats::default(),
            jit_enabled: true,
        }
    }

    pub fn reset(&mut self) {
        self.code.clear();
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.func_table.clear();
        self.func_names.clear();
        self.symbols.clear();
        self.vis_event_lines.clear();
        self.vis_event_queue.clear();
        self.breakpoints.clear();
        self.paused = false;
        self.cancelled = false;
        self.step_event_hit = false;
        self.step_count = 0;
        // max_steps / call_depth_limit 是**会话级配置**（由 capi `vitro_set_*` 设定），
        // 不随执行状态重置——否则"先设上限再运行"的配置会被本函数清掉。
        self.current_line = 0;
        self.error.clear();
        self.global_count = 0;
        self.last_snapshot_step = 0;
        self.snapshot_vars.clear();
        self.finished = false;
        self.exit_code = 0;
        self.i64_constants.clear();
        self.f64_constants.clear();
        self.last_accessed_vars.clear();
        self.qsort_depth = 0;
        self.local_sym_map.clear();
        self.global_sym_map.clear();
        self.freed_logs.clear();
        self.argc = 0;
        self.argv_addr = 0;
        self.dirty_pages = [0; 4];
        self.ip_hits.clear();
        self.trace_recorder.reset();
        self.jit_traces.clear();
        self.jit_stats = JitStats::default();
        self.memory.fill(0);
        self.mem_stack_top = STACK_START;
    }

    pub fn load_program(&mut self, code: Vec<Instruction>) {
        self.code = code;
        self.ip = 0;
    }

    pub fn set_argc(&mut self, argc: i32) {
        self.argc = argc;
    }

    pub fn set_argv_addr(&mut self, addr: u32) {
        self.argv_addr = addr;
    }

    pub fn set_globals_32(&mut self, globals: &[(u32, i32)]) {
        for &(offset, v) in globals {
            let addr = GLOBAL_START + offset;
            self.store_i32(addr, v, &SourceLoc::default());
        }
    }

    pub fn set_globals_64(&mut self, globals: &[(u32, u64)]) {
        for &(offset, v) in globals {
            let addr = GLOBAL_START + offset;
            self.store_i64(addr, v, &SourceLoc::default());
        }
    }

    /// 在全局数据区上界之下为 `main(int argc, char *argv[])` 分配 argv 内存（R1 ③）。
    ///
    /// 旧实现以 `global_count`（恒为 0）编址，argv 指针数组落在 `GLOBAL_START`，
    /// 与 codegen 全局数据重叠（预存 bug）。现改为自 `GLOBAL_REGION_LIMIT` 向下分配：
    /// 指针数组在区域底部、字符串数据在其上；占用由 `argv_region_footprint` 统一计算，
    /// 堆起点经 `compute_heap_base` 越过 argv 顶界，故堆 bump 不会覆盖参数。
    /// `global_data_end` 为全局数据末端的绝对地址（codegen 导出），冲突时明确 trap。
    pub fn setup_argv(&mut self, argc: i32, argv: &[String], global_data_end: u32) {
        if argc <= 0 || argv.is_empty() {
            self.argc = 0;
            self.argv_addr = 0;
            return;
        }
        let footprint = vitro_runtime::argv_region_footprint(argc, argv);
        let array_addr = vitro_runtime::GLOBAL_REGION_LIMIT - footprint;
        if array_addr < vitro_runtime::align4(global_data_end) {
            self.trap(
                "setup_argv: 命令行参数与全局数据区冲突（全局数据过大），无法安放 argv",
                &SourceLoc::default(),
            );
            return;
        }
        let mut string_addr = array_addr + (argc as u32) * 4;
        let count = (argc as usize).min(argv.len());
        for (i, s) in argv.iter().enumerate().take(count) {
            let next = string_addr + s.len() as u32 + 1;
            if next > vitro_runtime::GLOBAL_REGION_LIMIT {
                self.trap("setup_argv: 命令行参数过长，超出全局数据区", &SourceLoc::default());
                return;
            }
            self.write_cstring(string_addr, s);
            self.store_i32(array_addr + (i as u32) * 4, string_addr as i32, &SourceLoc::default());
            string_addr = next;
        }
        self.argc = argc;
        self.argv_addr = array_addr;
    }

    pub fn register_function(&mut self, idx: u32, meta: FuncMeta) {
        let idx = idx as usize;
        if idx >= self.func_table.len() {
            self.func_table.resize(idx + 1, FuncMeta::default());
        }
        self.func_table[idx] = meta;
    }

    pub fn register_function_name(&mut self, idx: u32, name: String) {
        let idx = idx as usize;
        if idx >= self.func_names.len() {
            self.func_names.resize(idx + 1, String::new());
        }
        self.func_names[idx] = name;
    }

    pub fn set_symbols(&mut self, symbols: Vec<VMSymbol>) {
        self.global_sym_map.clear();
        for sym in &symbols {
            if !sym.is_local {
                self.global_sym_map.insert(sym.addr as i32, sym.name.clone());
            }
        }
        self.symbols = symbols;
    }

    pub fn set_f64_constants(&mut self, constants: Vec<f64>) {
        self.f64_constants = constants;
    }

    pub fn set_i64_constants(&mut self, constants: Vec<i64>) {
        self.i64_constants = constants;
    }

    pub fn set_vis_event_lines(&mut self, lines: Vec<(i32, i32, String)>) {
        self.vis_event_lines = lines;
    }

    pub fn take_vis_events(&mut self) -> Vec<VisEventData> {
        std::mem::take(&mut self.vis_event_queue)
    }

    pub fn add_breakpoint(&mut self, line: i32) {
        self.breakpoints.insert(line);
    }

    pub fn remove_breakpoint(&mut self, line: i32) {
        self.breakpoints.remove(&line);
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.step_event_hit = false;
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Call a user-defined function from a host function context.
    /// Used by host_qsort to invoke the user-supplied comparison function.
    /// Returns the return value on success, or None if the function trapped.
    pub fn call_user_function(
        &mut self,
        session: &mut VmContext<'_>,
        func_idx: u32,
        args: &[i32],
        max_steps: i32,
    ) -> Option<i32> {
        let idx = func_idx as usize;
        if idx >= self.func_table.len() || self.func_table[idx].ip == 0 {
            return None;
        }
        let meta = self.func_table[idx].clone();
        let frame_size = meta.local_count as u64;
        if frame_size > MEM_SIZE as u64 || frame_size > self.mem_stack_top as u64 {
            return None;
        }
        let frame_size_u32 = frame_size as u32;
        if self.mem_stack_top < NULL_TRAP_SIZE + frame_size_u32 {
            return None;
        }

        // Save state
        let saved_ip = self.ip;
        let saved_call_stack = self.call_stack.clone();
        let saved_mem_stack_top = self.mem_stack_top;
        let saved_stack = self.stack.clone();
        let saved_error = self.error.clone();
        let saved_finished = self.finished;
        let saved_step_event_hit = self.step_event_hit;
        let saved_current_line = self.current_line;
        let saved_vis_event_queue = std::mem::take(&mut self.vis_event_queue);
        let saved_breakpoints = std::mem::take(&mut self.breakpoints);
        let start_step = self.step_count;

        // Setup call frame
        let original_stack_top = self.mem_stack_top;
        self.mem_stack_top -= frame_size_u32;
        let locals_base = self.mem_stack_top;
        // Arguments: args[0] is first param, args[n-1] is last param.
        // VM Call convention: first param is at locals_base + 0
        // 当前 call_user_function 仅用于 qsort 回调（参数均为 4 字节指针）。
        // 若未来扩展为 8 字节参数（double/long long），需按 type_size 选择 store_i32/store_i64。
        assert!(
            meta.param_sizes.iter().all(|&sz| sz == 1),
            "call_user_function 暂不支持非 4 字节参数（param_sizes {:?}）",
            meta.param_sizes
        );
        for i in 0..meta.param_count {
            let arg = if (i as usize) < args.len() { args[i as usize] } else { 0 };
            let arg_addr = (locals_base as u64) + (i as u64) * 4;
            self.store_i32(arg_addr as u32, arg, &SourceLoc::default());
        }
        let arg_bytes = meta.param_count as u32 * 4;
        let zero_start = locals_base + arg_bytes;
        let zero_end = locals_base + meta.local_count as u32;
        if zero_end > zero_start {
            for addr in zero_start..zero_end {
                self.store_i8(addr, 0, &SourceLoc::default());
            }
        }
        let func_name = if idx < self.func_names.len() {
            self.func_names[idx].clone()
        } else {
            format!("func_{}", func_idx)
        };
        const HOST_CALLBACK_SENTINEL: usize = usize::MAX;
        self.call_stack.push(CallFrame {
            return_ip: HOST_CALLBACK_SENTINEL,
            locals_base,
            local_count: meta.local_count,
            func_name,
            original_stack_top,
            caller_line: self.current_line,
            local_buffers: meta.local_buffers,
        });
        self.rebuild_local_sym_map();
        self.ip = meta.ip;

        // Execute until return or trap
        let mut result = None;
        loop {
            if self.step_count - start_step >= max_steps {
                break;
            }
            let step_result = self.step(session);
            match step_result {
                StepResult::Finished => {
                    result = self.stack.pop().map(|v| v as i32);
                    break;
                }
                StepResult::Trap => {
                    break; // result stays None
                }
                StepResult::Paused => {
                    self.paused = false;
                }
                StepResult::WaitingInput => {
                    break;
                }
                StepResult::Ok => {}
            }
        }

        // Restore state
        self.ip = saved_ip;
        self.call_stack = saved_call_stack;
        self.mem_stack_top = saved_mem_stack_top;
        self.stack = saved_stack;
        self.error = saved_error;
        self.finished = saved_finished;
        self.step_event_hit = saved_step_event_hit;
        self.rebuild_local_sym_map();
        self.current_line = saved_current_line;
        self.vis_event_queue = saved_vis_event_queue;
        self.breakpoints = saved_breakpoints;

        result
    }

    pub fn set_max_steps(&mut self, max: i32) {
        self.max_steps = max;
    }

    /// 当前步数上限（会话级配置回显；见 capi `vitro_set_max_steps` 与 serve `config.get`）。
    pub fn max_steps(&self) -> i32 {
        self.max_steps
    }

    /// V-P1-10 会话级保险丝：调用深度上限。
    /// 下限 16 层兜底，避免过小配置把正常程序直接判死。
    pub fn set_call_depth_limit(&mut self, limit: usize) {
        self.call_depth_limit = limit.max(16);
    }

    pub fn call_depth_limit(&self) -> usize {
        self.call_depth_limit
    }

    /// 已执行步数（capi `vitro_run_json` 的 steps_executed 字段）。
    pub fn get_step_count(&self) -> i32 {
        self.step_count
    }

    pub fn has_error(&self) -> bool {
        !self.error.is_empty()
    }

    pub fn get_error(&self) -> &str {
        &self.error
    }

    pub fn code_len(&self) -> usize {
        self.code.len()
    }

    pub fn code_ref(&self) -> &[Instruction] {
        &self.code
    }

    pub fn set_ip(&mut self, ip: usize) {
        self.ip = ip;
    }

    pub fn get_ip(&self) -> usize {
        self.ip
    }

    pub fn get_current_line(&self) -> i32 {
        self.current_line
    }

    pub fn get_executed_steps(&self) -> i32 {
        self.step_count
    }

    pub fn jit_stats(&self) -> &crate::jit_trace::JitStats {
        &self.jit_stats
    }

    pub fn set_jit_enabled(&mut self, enabled: bool) {
        self.jit_enabled = enabled;
    }

    pub fn jit_enabled(&self) -> bool {
        self.jit_enabled
    }

    pub fn jit_traces_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<usize, std::sync::Arc<crate::jit_templates::CompiledTrace>> {
        &mut self.jit_traces
    }

    /// JIT trace 批量执行时调用：增加 step_count 并检查全局限制。
    /// 返回 `true` 表示可以继续执行，`false` 表示已 trap。
    pub(crate) fn bulk_step_check(&mut self, steps: i32) -> bool {
        self.step_count = self.step_count.saturating_add(steps);
        if self.step_count >= self.max_steps {
            let msg = self.format_infinite_loop_error();
            self.trap(&msg, &SourceLoc::default());
            return false;
        }
        if self.cancelled {
            self.trap("执行已取消。", &SourceLoc::default());
            return false;
        }
        true
    }

    pub fn set_finished(&mut self, code: i32) {
        self.finished = true;
        self.exit_code = code;
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub(crate) fn call_stack_len(&self) -> usize {
        self.call_stack.len()
    }

    pub fn memory_ref(&self) -> &[u8] {
        &self.memory
    }

    pub fn memory_ref_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    /// 若存在未完成的 `new T[n]` 构造守卫，释放其底层内存块。
    /// 在 VM trap 或运行结束时调用，防止构造失败导致内存泄漏。
    pub fn rollback_pending_array_construction(&mut self, session: &mut VmContext<'_>) {
        if let Some(guard) = self.pending_array_construction.take() {
            // 仅当设置 guard 的栈帧仍然存在时才执行回滚（避免跨函数边界误释放）。
            if self.call_stack.len() >= guard.frame_depth {
                self.free_memory(session, guard.base_addr);
            }
        }
    }

    fn free_memory(&mut self, session: &mut VmContext<'_>, addr: u32) {
        if addr == 0 {
            return;
        }
        // 避免 double-free 记录重复（已释放则静默跳过）。
        if self.freed_logs.contains_key(&addr) {
            return;
        }
        let mut freed_size = 0i32;
        // U2#2：addr 索引 O(1) 定位（旧线性扫描）
        if let Some(r) = session.memory.find_region_mut(addr) {
            if !r.is_freed {
                r.is_freed = true;
                let aligned_size = ((r.size as u32) + 3) & !3;
                self.freed_logs.insert(
                    addr,
                    FreedRegionInfo {
                        addr: r.addr,
                        size: aligned_size,
                        alloc_line: r.alloc_line,
                        freed_line: self.get_current_line(),
                        alloc_step: 0,
                        freed_step: self.get_executed_steps(),
                    },
                );
                freed_size = aligned_size as i32;
            }
        }
        if freed_size > 0 {
            // 决议 §3：统一进 FIFO 隔离区
            session.memory.release_to_quarantine(vitro_runtime::FreeBlock {
                addr,
                size: freed_size,
            });
        }
    }

    pub fn qsort_depth(&self) -> i32 {
        self.qsort_depth
    }

    pub fn set_qsort_depth(&mut self, depth: i32) {
        self.qsort_depth = depth;
    }

    pub fn get_memory_size(&self) -> u32 {
        MEM_SIZE
    }

    /// 获取当前已释放的内存区域日志（用于 UAF/Double-Free 检测诊断）。
    pub fn get_freed_logs(&self) -> &std::collections::BTreeMap<u32, FreedRegionInfo> {
        &self.freed_logs
    }

    /// U2#2-b：删除与 [start, end) 重叠的释放记录（原 `retain` 全量线性扫，
    /// churn 稳态 ~16k 条 × 每次分配）。互不重叠 ⟹ addr 序即 end 序：
    /// 降序扫，遇 end ≤ start 即停（更小 addr 的块 end 只会更小）。
    pub(crate) fn freed_logs_remove_overlapping(&mut self, start: u32, end: u32) {
        let mut to_remove: Vec<u32> = Vec::new();
        for (&laddr, log) in self.freed_logs.range(..end).rev() {
            if laddr.saturating_add(log.size) <= start {
                break;
            }
            to_remove.push(laddr);
        }
        for laddr in to_remove {
            self.freed_logs.remove(&laddr);
        }
    }

    /// U2#2-b：区间重叠查询（check_uaf / trap_invalid_free 共用）——
    /// 与 [addr, addr+size) 重叠的任一释放记录。降序扫遇 end ≤ addr 即停。
    pub(crate) fn freed_logs_find_overlapping(&self, addr: u32, size: u32) -> Option<&FreedRegionInfo> {
        let end = addr.saturating_add(size);
        // 降序首块（addr 最大且 < end）即可判定：互不重叠 ⟹ 其 end 最大——
        // end ≤ addr 则后续全部不重叠（None）；end > addr 则它就是重叠块。
        match self.freed_logs.range(..end).next_back() {
            Some((&laddr, log)) if laddr.saturating_add(log.size) > addr => Some(log),
            _ => None,
        }
    }

    /// 根据函数名查找其在 VM 函数表中的索引。
    pub fn get_func_index(&self, name: &str) -> Option<u32> {
        self.func_names.iter().position(|n| n == name).map(|i| i as u32)
    }

    pub fn get_stack(&self) -> &[u64] {
        &self.stack
    }

    pub fn get_symbols(&self) -> &[VMSymbol] {
        &self.symbols
    }

    pub(crate) fn rebuild_local_sym_map(&mut self) {
        self.local_sym_map.clear();
        if let Some(frame) = self.call_stack.last() {
            for sym in &self.symbols {
                if sym.is_local && sym.func_name == frame.func_name {
                    self.local_sym_map.insert(sym.addr as i32, sym.name.clone());
                }
            }
        }
    }

    pub fn get_call_stack(&self) -> &[CallFrame] {
        &self.call_stack
    }

    pub fn get_last_accessed_vars(&self) -> &[VariableAccess] {
        &self.last_accessed_vars
    }

    pub fn was_step_event_hit(&self) -> bool {
        self.step_event_hit
    }

    // --- Stack helpers ---

    pub fn pop(&mut self) -> u64 {
        match self.stack.pop() {
            Some(v) => v,
            None => {
                self.trap("运行时错误：栈下溢", &SourceLoc::default());
                0
            }
        }
    }

    pub fn push(&mut self, val: u64) {
        if self.stack.len() >= MAX_STACK_DEPTH {
            self.trap(
                "值栈溢出：栈深度超过限制。请检查是否有无限递归或过多嵌套表达式。",
                &SourceLoc::default(),
            );
            return;
        }
        self.stack.push(val);
    }

    #[cfg(test)]
    pub fn set_test_code(&mut self, code: Vec<Instruction>) {
        self.code = code;
        self.ip = 0;
    }
}
