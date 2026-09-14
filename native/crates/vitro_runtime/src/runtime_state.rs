use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Arc;

use crate::output::{OutputKind, OutputLog};
use vitro_ast::Type;

/// 执行轨迹条目基础数据：`vitro_native` 会定义带 `#[frb]` 的同名包装。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceEntryData {
    pub line: i32,
    pub operation: String,
}

/// 可视化事件基础数据：`vitro_native` 会定义带 `#[frb]` 的同名包装。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisEventData {
    pub ty: i32,
    pub line: i32,
    pub extra0: i32,
    pub extra1: i32,
    pub extra2: i32,
    pub context: String,
}

/// 变量快照基础数据：`vitro_native` 会定义带 `#[frb]` 的同名包装。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariableSnapshotData {
    pub name: String,
    pub addr: u32,
    pub is_local: bool,
    pub ty: Type,
    pub value: i64,
}

/// 执行路径热力图：记录每行源代码被执行的次数。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExecutionHeatmap {
    pub line_counts: HashMap<i32, u64>,
}

impl ExecutionHeatmap {
    pub fn record(&mut self, line: i32) {
        if line > 0 {
            *self.line_counts.entry(line).or_insert(0) += 1;
        }
    }

    pub fn max_count(&self) -> u64 {
        self.line_counts.values().copied().max().unwrap_or(0)
    }

    pub fn clear(&mut self) {
        self.line_counts.clear();
    }
}

/// 输入模式：交互式或批量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum InputMode {
    #[default]
    Interactive,
    Batch,
}

/// 执行轨迹环形上限（U2#4）：`host_step` 每步写一条，无读者——上限后丢最旧，
/// 消除 10M 步数百 MB 的只写不读累积。尾部（最近的轨迹）保真。
pub const TRACE_LIMIT: usize = 4096;

/// 运行时状态：记录 VM 执行过程中的输出、trace、输入、变量快照等。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RuntimeState {
    pub error: String,
    /// 最近一次 `vitro_get_runtime_error` 返回的 C 字符串缓存，避免返回 `String` 内部指针导致悬垂。
    pub last_error_cstring: Option<CString>,
    pub error_buffer: String,
    /// 输出日志（按写入顺序）：程序 stdout / 程序 stderr / 引擎附注各自打标，
    /// 预算内有界（U2#3）+ 通道长度 O(1)。
    ///
    /// E-P1-5 修复：此前这里是 `Vec<String> output_lines`，三种语义混在一条字节流里，
    /// 消费方只能靠文本正则清洗（十余处、口径不一，程序打印"程序运行完成，返回值：N"
    /// 会被误删）。需要纯程序输出请用 [`RuntimeState::stdout`]，需要展示视图用
    /// [`RuntimeState::display`]。
    ///
    /// U2#4：`Arc` + 写时复制——检查点快照（`snapshot_into` 每步、`save` 每 20 步）
    /// 浅共享此结构而非深拷贝，消除 50× 克隆放大；运行时首次写入时 `make_mut`
    /// 自动分叉（旧快照不受影响）。
    #[serde(default)]
    pub output: Arc<OutputLog>,
    pub running: bool,
    /// 执行轨迹（U2#4：环形上限 [`TRACE_LIMIT`]，超限丢最旧）。
    /// 同样 Arc+COW（快照浅共享）。
    #[serde(default)]
    pub trace: Arc<Vec<TraceEntryData>>,
    pub current_line: i32,
    pub input_lines: Vec<String>,
    pub input_index: usize,
    pub step_mode: bool,
    pub step_count: i32,
    pub variable_snapshot: Vec<VariableSnapshotData>,
    pub vis_event_cache: Vec<VisEventData>,
    pub rand_seed: u32,
    pub input_char_offset: usize,
    pub waiting_input: bool,
    /// 标准输入已到达 EOF 的粘滞标志（C11 7.21.5.1：`feof` 一旦置位，后续读恒返回 EOF）。
    ///
    /// 修复下游需求 A1 的**遗留分支**：此前所有读取路径各自重算"流是否还有内容"，
    /// 判定 EOF 后也**不推进游标**，于是 `scanf` 触发的 EOF 不被 `getchar` 感知
    /// （`getchar` 会把 scanf 未消费的尾部 `'\n'` 当成普通字符返回 10），
    /// 且尾部空白会在下一次读取时"复活"。真实 C 语义下 stdin 到 EOF 即永久 EOF。
    ///
    /// - 置位：任一路径判定"无更多输入且处于 Batch 模式"时，经 [`RuntimeState::set_stdin_eof`]；
    /// - 清位：`set_stdin` / `push_stdin_text` 重新喂入内容时（交互续跑是合法的"新内容到达"）。
    #[serde(default)]
    pub stdin_eof: bool,
    /// 执行热力图（U2#4 同口径 Arc+COW：快照浅共享、record 走 make_mut 分叉）。
    #[serde(default)]
    pub heatmap: Arc<ExecutionHeatmap>,
    pub input_mode: InputMode,
    pub ungetc_char: Option<i32>,
    /// 命令行参数个数（供 `main(int argc, char *argv[])` 使用）。
    pub argc: i32,
    /// 命令行参数字符串数组（供 `main(int argc, char *argv[])` 使用）。
    pub argv: Vec<String>,
    /// 判分确定性模式（Phase 1 最小形态）：`time()`/`clock()` 固定返回 0，
    /// `rand()` 使用固定起始种子。与 Phase 3 的"完整 step 派生伪时钟"分层——
    /// 本项服务判分可复现，后者服务时间旅行重放确定性。
    pub deterministic: bool,
}

impl RuntimeState {
    /// 追加程序 stdout 片段。
    pub fn push_stdout(&mut self, text: impl Into<String>) {
        Arc::make_mut(&mut self.output).push_stdout(text);
    }

    /// 追加程序 stderr 片段。
    pub fn push_stderr(&mut self, text: impl Into<String>) {
        Arc::make_mut(&mut self.output).push_stderr(text);
    }

    /// 追加引擎附注（教学诊断 / 运行完成提示 / 泄漏报告等），不进入 stdout。
    ///
    /// 附注是"给人读的段落"：本方法保证每段以 `\n` 收尾，避免多段附注在
    /// [`RuntimeState::display`] 的零分隔顺序拼接中粘连成一行（泄漏报告曾因此
    /// 输出为 `===== 内存泄漏检测报告 =====发现 1 处…`）。**程序 stdout / stderr
    /// 不做任何加工** —— 那必须逐字节保真。
    pub fn push_note(&mut self, text: impl Into<String>) {
        Arc::make_mut(&mut self.output).push_note(text);
    }

    /// 追加一条执行轨迹（环形上限 [`TRACE_LIMIT`]，超限丢最旧）。
    pub fn push_trace(&mut self, entry: TraceEntryData) {
        let trace = Arc::make_mut(&mut self.trace);
        trace.push(entry);
        let overflow = trace.len().saturating_sub(TRACE_LIMIT);
        if overflow > 0 {
            trace.drain(..overflow);
        }
    }

    /// 清空全部输出通道。
    pub fn clear_output(&mut self) {
        Arc::make_mut(&mut self.output).clear();
    }

    /// 按通道筛选片段（保持写入顺序）。
    pub fn chunks_of(&self, kind: OutputKind) -> Vec<&str> {
        self.output.chunks_of(kind)
    }

    /// 纯程序 stdout —— Shadow Verification / 差分对比的**唯一**合法来源。
    ///
    /// 不含引擎附注、不含 stderr。
    pub fn stdout(&self) -> String {
        self.output.join(OutputKind::Stdout)
    }

    /// 程序 stderr 文本（教学场景下单独展示，不参与 stdout 比对）。
    pub fn stderr(&self) -> String {
        self.output.join(OutputKind::Stderr)
    }

    /// 引擎附注文本（运行完成提示、泄漏报告、安全提示等）。
    pub fn notes(&self) -> String {
        self.output.join(OutputKind::Note)
    }

    /// 程序 stdout 片段列表（stdout/stderr 小段会合并，元素≠单次调用）。
    pub fn stdout_chunks(&self) -> Vec<&str> {
        self.chunks_of(OutputKind::Stdout)
    }

    /// 引擎附注片段列表。
    pub fn note_chunks(&self) -> Vec<&str> {
        self.chunks_of(OutputKind::Note)
    }

    /// 展示视图：所有通道按写入顺序拼接（原 `output()` 语义，UI / CLI 使用）。
    pub fn display(&self) -> String {
        self.output.join_all()
    }

    /// 兼容别名：等同于 [`RuntimeState::display`]。
    ///
    /// 保留旧名是为了让 `flutter_bridge` / `session_api` / `vitro_cli` 等展示型出口
    /// 无需改动；**需要与 Clang 比对的消费方请改用 [`RuntimeState::stdout`]**。
    pub fn output(&self) -> String {
        self.display()
    }

    /// 把一段标准输入文本拆成"**保留换行**"的行序列（stdin 行语义的单一来源）。
    ///
    /// C 的 stdin 是字节流：`getchar()` 必须能读到 `'\n'`（K&R 用例 `kr_1_8`
    /// 统计换行数即依赖此项），`scanf` 的流式游标也按真实字节推进。
    /// 此前 capi / FRB / serve 三处各自用 `str::lines()` 拆分、把行尾换行丢掉，
    /// 而 E2E 防线用 `split_inclusive('\n')` —— 同一份输入在不同出口语义不一致；
    /// Shadow 防线首次注入用例自带的 `.in` 后立刻暴露（`nl` 恒为 0、逆波兰计算器无输出）。
    ///
    /// `\r\n` 先规整为 `\n`（Windows 文本输入的等价处理）；最后一行无换行时保持原样。
    pub fn split_stdin(input: &str) -> Vec<String> {
        input
            .replace("\r\n", "\n")
            .split_inclusive('\n')
            .map(|s| s.to_string())
            .collect()
    }

    /// 设置标准输入并复位读取游标（等价于 `input_lines = split_stdin(input)`）。
    pub fn set_stdin(&mut self, input: &str) {
        self.input_lines = Self::split_stdin(input);
        self.input_index = 0;
        self.input_char_offset = 0;
        // 重新喂入内容 = "新内容到达"，清除 EOF 粘滞位
        self.stdin_eof = false;
    }

    /// 向标准输入**追加**文本，保留读取游标（交互续跑 `input.feed` 使用）。
    ///
    /// 与新内容到达同步清除 EOF 粘滞位：C 里管道关闭后不可复活，但教学场景下
    /// 交互式续跑本质是"学生又键入了一行"，属预期语义。
    pub fn push_stdin_text(&mut self, text: &str) {
        self.input_lines.extend(Self::split_stdin(text));
        self.stdin_eof = false;
    }

    /// 置位 EOF 并把读取游标**推到底**。
    ///
    /// 游标必须一并推到底，否则未消费的尾部空白会在下一次读取时"复活"
    /// （实测：输入 `7\n`，`scanf("%d")` 消费 `7` 后判定 EOF，`getchar()`
    /// 仍读到 `'\n'` = 10，而 Clang 返回 -1）。
    pub fn set_stdin_eof(&mut self) {
        self.stdin_eof = true;
        self.input_index = self.input_lines.len();
        self.input_char_offset = 0;
    }
}
