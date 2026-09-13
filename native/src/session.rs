use crate::engine::completion::CompletionSnapshot;
use crate::vm::core::CideVM;
use crate::vm::vfs::VirtualFileSystem;
use cide_runtime::instruction::Instruction;
use std::collections::HashMap;
use std::ffi::CString;

pub use cide_runtime::{
    CodeFile, CompileUnit, FreeBlock, FuncMeta, InputMode, MemoryState, RuntimeState, Symbol, GLOBAL_START, HEAP_START,
    MAX_STACK_DEPTH, MEM_SIZE, NULL_TRAP_SIZE, SNAPSHOT_INTERVAL, STACK_START,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub line: i32,
    pub column: i32,
    pub error_code: i32,
    pub severity: i32,
    pub message: String,
    pub fix_suggestion: String,
    pub fix_kind: i32,
    pub replace_start_line: i32,
    pub replace_start_column: i32,
    pub replace_end_line: i32,
    pub replace_end_column: i32,
    pub replacement_text: String,
    pub filename: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlgorithmMatch {
    pub name: String,
    pub display_name: String,
    pub func_name: String,
    pub confidence: i32,
    pub suggestion: String,
    pub line: i32,
    pub vis_events: Vec<VisEvent>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompileState {
    pub errors: String,
    /// 最近一次 `cide_get_compile_errors` 返回的 C 字符串缓存，避免返回 `String` 内部指针导致悬垂。
    pub last_errors_cstring: Option<CString>,
    pub compile_units: Vec<CompileUnit>,
    pub compiled: bool,
    pub bytecode: Vec<Instruction>,
    pub globals_init: Vec<(u32, i32)>,
    pub globals_init_64: Vec<(u32, u64)>,
    pub f64_constants: Vec<f64>,
    pub i64_constants: Vec<i64>,
    pub diagnostics: Vec<Diagnostic>,
    pub source_map: Vec<(u32, cide_shared::source_loc::SourceLoc)>,
    pub func_table: HashMap<String, FuncMeta>,
    pub func_index: HashMap<String, i32>,
    pub string_data: Vec<(u32, String)>,
    pub symbols: Vec<Symbol>,
    pub algorithm_matches: Vec<AlgorithmMatch>,
    pub struct_fields: HashMap<String, Vec<(String, i32)>>,
    /// 智能补全快照：每次成功编译后从 AST 提取的符号表
    pub completion_snapshot: CompletionSnapshot,
    /// 多文件会话的**全局行号 → 文件**映射（由 `merge_compile_units` 产出）。
    ///
    /// P0-4：多文件编译会把各编译单元合并成一份源码，因此字节码里的行号是**全局行号**；
    /// 单文件管线（`run_compile_pipeline`）为空 —— 此时全局行号即文件内行号。
    /// 语义标注 / 源码行查询必须经此映射换算，不能再假设"第一个编译单元"。
    #[serde(default)]
    pub file_ranges: Vec<crate::engine::compile_pipeline::FileRange>,
    /// 全局数据区末端的**绝对地址**（R1：codegen 导出，含 Bytecode Libc 预留段）。
    /// 运行入口据此计算动态堆起点 `max(HEAP_START, align4(global_data_end))`。
    #[serde(default)]
    pub global_data_end: u32,
    /// E2 白箱教学层：预处理追踪（宏展开链 + `#if` 分支选择原因，容量封顶）。
    #[serde(default)]
    pub preprocessor_trace: Vec<String>,
}

impl Session {
    /// 按**全局行号**取源码行（P0-4：多文件会话安全的行号定位）。
    ///
    /// 多文件编译会把各编译单元合并成一份源码（`merge_compile_units`），因此字节码与
    /// `code_line` 里的是**全局行号**。此前多处直接拿它去查 `compile_units.first()` ——
    /// 单文件时两者恰好一致（所以问题长期未暴露），多文件时必然错配（实测 `main.c` 仅
    /// 13 行却报出 `line 20..25`），语义标注会产出与本行执行内容无关的"看似合理"描述。
    ///
    /// 单文件会话（`file_ranges` 为空）保持"全局行号 == 文件内行号"的原语义。
    pub fn source_line_at(&self, global_line: i32) -> Option<String> {
        if global_line <= 0 {
            return None;
        }
        if let Some(range) = self
            .compile
            .file_ranges
            .iter()
            .find(|r| global_line >= r.start_line && global_line <= r.end_line)
        {
            let in_file = (global_line - range.start_line) as usize;
            let unit = self
                .compile
                .compile_units
                .iter()
                .find(|u| u.filename == range.filename)?;
            return unit.source.lines().nth(in_file).map(|s| s.to_string());
        }
        self.compile
            .compile_units
            .first()
            .and_then(|u| u.source.lines().nth((global_line - 1) as usize).map(|s| s.to_string()))
    }

    /// 步数保险丝（会话级）：**与 VM 是否已创建解耦**。
    ///
    /// 此前 capi 的 `cide_set_max_steps` 与 serve 的 `config.set` 都写成
    /// `if let Some(vm) = session.vm.as_mut() { vm.set_max_steps(..) }` 并返回"成功" ——
    /// 会话尚未编译（`vm == None`）时配置被**静默丢弃**。实测：serve 里把
    /// `config.set {"max_steps": 2000}` 写在 `compile` 之前，程序一路跑到默认的
    /// 1000 万步才 trap（API 报告成功，配置完全不生效）。
    ///
    /// 现在无 VM 时先建立一个承载配置的 VM：`compile` → `run` 会 take 它，
    /// `setup_vm` 内部的 `reset()` 保留会话级保险丝。
    pub fn set_max_steps(&mut self, max: i32) {
        let vm = self.vm.get_or_insert_with(CideVM::default);
        vm.set_max_steps(max.max(1));
    }

    /// 调用深度保险丝（会话级）：同 [`Session::set_max_steps`]，与 VM 是否已创建解耦。
    pub fn set_call_depth_limit(&mut self, limit: usize) {
        let vm = self.vm.get_or_insert_with(CideVM::default);
        vm.set_call_depth_limit(limit);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceEntry {
    pub line: i32,
    pub operation: String,
}

impl From<cide_runtime::TraceEntryData> for TraceEntry {
    fn from(value: cide_runtime::TraceEntryData) -> Self {
        Self {
            line: value.line,
            operation: value.operation,
        }
    }
}

impl From<TraceEntry> for cide_runtime::TraceEntryData {
    fn from(value: TraceEntry) -> Self {
        Self {
            line: value.line,
            operation: value.operation,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariableSnapshot {
    pub name: String,
    pub addr: u32,
    pub is_local: bool,
    pub ty: cide_ast::Type,
    pub value: i64,
}

impl From<cide_runtime::VariableSnapshotData> for VariableSnapshot {
    fn from(value: cide_runtime::VariableSnapshotData) -> Self {
        Self {
            name: value.name,
            addr: value.addr,
            is_local: value.is_local,
            ty: value.ty,
            value: value.value,
        }
    }
}

impl From<VariableSnapshot> for cide_runtime::VariableSnapshotData {
    fn from(value: VariableSnapshot) -> Self {
        Self {
            name: value.name,
            addr: value.addr,
            is_local: value.is_local,
            ty: value.ty,
            value: value.value,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisEvent {
    pub ty: i32,
    pub line: i32,
    pub extra0: i32,
    pub extra1: i32,
    pub extra2: i32,
    pub context: String,
}

impl From<cide_runtime::VisEventData> for VisEvent {
    fn from(value: cide_runtime::VisEventData) -> Self {
        Self {
            ty: value.ty,
            line: value.line,
            extra0: value.extra0,
            extra1: value.extra1,
            extra2: value.extra2,
            context: value.context,
        }
    }
}

impl From<VisEvent> for cide_runtime::VisEventData {
    fn from(value: VisEvent) -> Self {
        Self {
            ty: value.ty,
            line: value.line,
            extra0: value.extra0,
            extra1: value.extra1,
            extra2: value.extra2,
            context: value.context,
        }
    }
}

/// 执行路径热力图：记录每行源代码被执行的次数。
pub use cide_runtime::ExecutionHeatmap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryRegion {
    pub addr: u32,
    pub size: i32,
    pub name: String,
    pub ty: String,
    pub is_heap: bool,
    pub is_freed: bool,
    /// 分配时的源码行号（教学用途）
    pub alloc_line: i32,
    /// 分配方式，如 "malloc" / "realloc" / "fopen"
    pub alloc_by: String,
    /// 段标识：`global` / `stack` / `heap`（C2 三段式内存地图；见 `MemoryRegionData::kind`）
    #[serde(default = "default_memory_region_kind")]
    pub kind: String,
}

fn default_memory_region_kind() -> String {
    "heap".to_string()
}

impl From<cide_runtime::MemoryRegionData> for MemoryRegion {
    fn from(value: cide_runtime::MemoryRegionData) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            name: value.name,
            ty: value.ty,
            is_heap: value.is_heap,
            is_freed: value.is_freed,
            alloc_line: value.alloc_line,
            alloc_by: value.alloc_by,
            kind: value.kind,
        }
    }
}

impl From<MemoryRegion> for cide_runtime::MemoryRegionData {
    fn from(value: MemoryRegion) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            name: value.name,
            ty: value.ty,
            is_heap: value.is_heap,
            is_freed: value.is_freed,
            alloc_line: value.alloc_line,
            alloc_by: value.alloc_by,
            kind: value.kind,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryFragment {
    pub addr: u32,
    pub size: i32,
}

impl From<cide_runtime::MemoryFragmentData> for MemoryFragment {
    fn from(value: cide_runtime::MemoryFragmentData) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
        }
    }
}

impl From<MemoryFragment> for cide_runtime::MemoryFragmentData {
    fn from(value: MemoryFragment) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeapStats {
    /// 本次运行的堆起点（R1：动态堆起点，原为常量 HEAP_START）
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

impl From<cide_runtime::HeapStatsData> for HeapStats {
    fn from(value: cide_runtime::HeapStatsData) -> Self {
        Self {
            heap_base: value.heap_base,
            total_heap: value.total_heap,
            allocated: value.allocated,
            fragmented: value.fragmented,
            fragmentation_rate: value.fragmentation_rate,
        }
    }
}

impl From<HeapStats> for cide_runtime::HeapStatsData {
    fn from(value: HeapStats) -> Self {
        Self {
            heap_base: value.heap_base,
            total_heap: value.total_heap,
            allocated: value.allocated,
            fragmented: value.fragmented,
            fragmentation_rate: value.fragmentation_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub status: StepStatus,
    pub current_line: i32,
    pub output: String,
    pub waiting_input: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Paused,
    WaitingInput,
    Finished,
    Trap,
}

pub struct Session {
    pub compile: CompileState,
    pub runtime: RuntimeState,
    pub memory: MemoryState,
    pub vm: Option<CideVM>,
    pub vfs: VirtualFileSystem,
    /// 统一模式（时间旅行）引擎。由 `cide_step_begin` 初始化，
    /// 供 `cide_step_next_json` / `cide_get_step_payloads_json` 消费。
    pub unified: Option<crate::unified::engine::UnifiedEngine>,
    /// U1#1 P0-1：serve step.next 的一帧发布缓冲——流式协议下"行末帧"
    /// 判定需要未来信息（下一帧是否同行），故当前帧暂存，下一帧到来时
    /// 回改（同行 → 清其 algorithm_step，它是语句中间帧、数值为旧值）
    /// 后再发布。
    pub unified_pending: Option<crate::unified::types::StepPayload>,
}

impl Session {
    /// 构造 VM 执行上下文，将 VM 所需的运行时/内存/VFS 可变引用聚合起来。
    pub fn as_vm_context(&mut self) -> crate::vm::context::VmContext<'_> {
        crate::vm::context::VmContext {
            runtime: &mut self.runtime,
            memory: &mut self.memory,
            vfs: &mut self.vfs,
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self {
            compile: CompileState::default(),
            runtime: RuntimeState::default(),
            memory: MemoryState::default(),
            vm: Some(CideVM::default()),
            vfs: VirtualFileSystem::new(),
            unified: None,
            unified_pending: None,
        }
    }
}

impl cide_algorithm_steps::AlgorithmContext for Session {
    fn source_line(&self, line: i32) -> Option<String> {
        // P0-4：统一走多文件安全的行号定位（此前固定查第一个编译单元）
        self.source_line_at(line).map(|s| s.trim().to_string())
    }

    fn find_algorithm(&self, func_name: &str) -> Option<cide_algorithm_steps::AlgorithmMatch> {
        self.compile
            .algorithm_matches
            .iter()
            .find(|m| m.func_name == func_name)
            .map(|m| cide_algorithm_steps::AlgorithmMatch {
                name: m.name.clone(),
                display_name: m.display_name.clone(),
                func_name: m.func_name.clone(),
                confidence: m.confidence,
                suggestion: m.suggestion.clone(),
                line: m.line,
            })
    }
}
