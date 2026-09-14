//! 输出通道：把"程序自己写出的字节"与"引擎附注"在源头分开。
//!
//! 背景（E-P1-5）：此前所有输出都追加进同一个 `Vec<String>`（`RuntimeState::output_lines`），
//! 于是引擎的"程序运行完成，返回值：N"、内存泄漏报告、安全检测提示与学生的 `printf`
//! 混在同一条字节流里。消费方（Shadow Verification 的 Python/Rust 驱动、E2E 测试、
//! 第三方）只能靠文本正则把附注"洗掉"，同一套清洗规则散落十余处且语义互不一致：
//! 程序自己打印 `程序运行完成，返回值：7` 时会被整段删除，导出假阳性 `output_gap`。
//!
//! 现在改为在源头按 [`OutputKind`] 打标：
//! - 需要与 Clang 比对的消费方读 [`crate::RuntimeState::stdout`]，拿到的是**程序真实输出**；
//! - 需要展示的消费方读 [`crate::RuntimeState::display`]，拿到按写入顺序拼接的全量视图；
//! - 教学附注单独走 [`crate::RuntimeState::notes`]，不再污染 stdout。
//!
//! 之所以用"带 kind 的单序列"而不是"stdout/notes 两个独立缓冲区"：附注并非总在末尾
//! （如"堆内存耗尽"提示在 malloc 失败处插入，程序后续还会继续输出），双缓冲会丢失真实
//! 交错顺序，UI 展示会错位。

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 单个输出片段的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// 程序自身写到 stdout 的字节（`printf` / `puts` / `putchar` / `fputs(stdout)` 等）。
    ///
    /// 这是 Shadow Verification 与 Clang golden 比对的**唯一**合法来源。
    Stdout,
    /// 程序自身写到 stderr 的字节（`fputs(stderr)` / `perror` 等）。
    ///
    /// C 标准下 stderr 无缓冲且与 stdout 分流；与 Clang 比对 stdout 时不得混入。
    Stderr,
    /// 引擎附注：教学诊断、运行完成提示、内存泄漏报告、堆上限提示、安全检测提示等。
    ///
    /// **不属于程序输出**，任何 stdout 比对都必须排除。
    Note,
}

impl OutputKind {
    /// 稳定的字符串标识，用于 JSON 出口（serve / capi 消费方）与文档。
    pub fn as_str(self) -> &'static str {
        match self {
            OutputKind::Stdout => "stdout",
            OutputKind::Stderr => "stderr",
            OutputKind::Note => "note",
        }
    }

    /// 从字符串解析通道标识；`None` 表示未知通道。
    ///
    /// `"display"` 不是单一片段的 kind，而是"全部按序拼接"的投影，由调用方单独处理。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stdout" => Some(OutputKind::Stdout),
            "stderr" => Some(OutputKind::Stderr),
            "note" | "notes" => Some(OutputKind::Note),
            _ => None,
        }
    }

    fn index(self) -> usize {
        match self {
            OutputKind::Stdout => 0,
            OutputKind::Stderr => 1,
            OutputKind::Note => 2,
        }
    }
}

/// 一段带来源标记的输出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChunk {
    pub kind: OutputKind,
    pub text: String,
}

impl OutputChunk {
    pub fn new(kind: OutputKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into() }
    }

    pub fn stdout(text: impl Into<String>) -> Self {
        Self::new(OutputKind::Stdout, text)
    }

    pub fn stderr(text: impl Into<String>) -> Self {
        Self::new(OutputKind::Stderr, text)
    }

    pub fn note(text: impl Into<String>) -> Self {
        Self::new(OutputKind::Note, text)
    }
}

/// 输出日志默认总预算（16MB）。
///
/// U2#3：真实教学/防线用例的输出最大仅几十 KB，16MB 内**完全保真**（shadow
/// 比对不受影响）；失控程序（如 10M 步上限内逐字符 putchar）超预算后丢最旧
/// 并附截断注记——有界且截断可见，替代此前的无界 `Vec` 累积形态。
pub const OUTPUT_LOG_BUDGET: usize = 16 * 1024 * 1024;

/// stdout/stderr 连续小段合并阈值：写入 ≤ 此长度的程序片段时，与末尾同通道
/// chunk 拼接（putchar 逐字符分配合并，评估 R11）；大于此长度独立成段。
const MERGE_THRESHOLD: usize = 64;
/// 合并后单 chunk 的长度上限（防止合并本身无界）。
const MERGE_LIMIT: usize = 4096;

fn default_budget() -> usize {
    OUTPUT_LOG_BUDGET
}

/// 带预算的输出日志（U2#3：output_chunks 有界化）。
///
/// - **预算内保真**：顺序、通道、字节全部保持（shadow stdout 比对不受影响）；
/// - **超预算丢最旧**：尾部保真（学生看到的最新输出不丢），并附一条截断
///   引擎附注（截断可见，不静默）；
/// - **O(1) 长度**：总量与逐通道字节长度增量维护（`vitro_get_output_length`
///   等出口不再全量拼接）；
/// - **小段合并**：stdout/stderr 的连续小写入（≤64B，如逐字符 putchar）
///   拼进末尾同通道 chunk，消除逐字符 String 分配。
///
/// **已知残余（诚实记录）**：引擎附注（note）不占预算、不参与丢弃——它是
/// 输出侧唯一无界通道，当前仅靠 `host/memory.rs` 的 O(n) 文本去重控制条数
///（"堆内存耗尽"等高频附注场景理论上仍可累积）；若实际语料出现失控形态，
/// 再为 note 增设独立条数上限（截断泄漏报告不可接受，需按"条"而非"字节"）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputLog {
    chunks: VecDeque<OutputChunk>,
    total_bytes: usize,
    kind_bytes: [usize; 3],
    /// 环形丢弃的累计字节数（诊断口径，clear 时复位）。
    dropped_bytes: usize,
    /// 首次触发丢弃后置位，避免重复附截断注记。
    #[serde(skip)]
    truncation_noted: bool,
    /// 总字节预算（运行配置，不参与序列化；反序列化取默认 16MB）。
    #[serde(skip, default = "default_budget")]
    budget: usize,
}

impl Default for OutputLog {
    fn default() -> Self {
        Self {
            chunks: VecDeque::new(),
            total_bytes: 0,
            kind_bytes: [0; 3],
            dropped_bytes: 0,
            truncation_noted: false,
            budget: OUTPUT_LOG_BUDGET,
        }
    }
}

impl OutputLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// 调整总预算（测试/特殊场景用）。只影响后续写入的裁剪判定。
    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    /// 现存全部字节总数（O(1)，展示视图长度）。
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// 指定通道现存字节数（O(1)）。
    pub fn len_of(&self, kind: OutputKind) -> usize {
        self.kind_bytes[kind.index()]
    }

    /// 环形丢弃的累计字节数。
    pub fn dropped_bytes(&self) -> usize {
        self.dropped_bytes
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &OutputChunk> {
        self.chunks.iter()
    }

    /// 按通道筛选片段文本（保持写入顺序）。
    pub fn chunks_of(&self, kind: OutputKind) -> Vec<&str> {
        self.chunks.iter().filter(|c| c.kind == kind).map(|c| c.text.as_str()).collect()
    }

    fn push_raw(&mut self, kind: OutputKind, text: String) {
        let n = text.len();
        self.chunks.push_back(OutputChunk { kind, text });
        self.total_bytes += n;
        self.kind_bytes[kind.index()] += n;
    }

    fn on_bytes_removed(&mut self, kind: OutputKind, n: usize) {
        self.total_bytes -= n;
        self.kind_bytes[kind.index()] -= n;
        self.dropped_bytes += n;
    }

    /// 程序输出（stdout+stderr）现存字节数——预算只约束程序输出，
    /// 引擎附注不占预算、不参与环形丢弃（截断提示被程序输出挤掉就失去意义）。
    fn program_bytes(&self) -> usize {
        self.total_bytes - self.kind_bytes[OutputKind::Note.index()]
    }

    /// 追加一段输出。stdout/stderr 的小段会尝试与末尾同通道 chunk 合并。
    pub fn push(&mut self, kind: OutputKind, text: impl Into<String>) {
        let text = text.into();
        if kind != OutputKind::Note && text.len() <= MERGE_THRESHOLD {
            if let Some(back) = self.chunks.back_mut() {
                if back.kind == kind && back.text.len() + text.len() <= MERGE_LIMIT {
                    back.text.push_str(&text);
                    self.total_bytes += text.len();
                    self.kind_bytes[kind.index()] += text.len();
                    self.enforce_budget();
                    return;
                }
            }
        }
        self.push_raw(kind, text);
        self.enforce_budget();
    }

    /// 追加程序 stdout 片段。
    pub fn push_stdout(&mut self, text: impl Into<String>) {
        self.push(OutputKind::Stdout, text);
    }

    /// 追加程序 stderr 片段。
    pub fn push_stderr(&mut self, text: impl Into<String>) {
        self.push(OutputKind::Stderr, text);
    }

    /// 追加引擎附注：保证每段以 `\n` 收尾（段落语义，不参与小段合并）。
    pub fn push_note(&mut self, text: impl Into<String>) {
        let mut text = text.into();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        if text.is_empty() {
            return;
        }
        self.push_raw(OutputKind::Note, text);
        self.enforce_budget();
    }

    /// 超预算时从最旧端丢弃**程序输出**（尾部保真）；单条超预算的 chunk 截其
    /// 头部保留尾部。引擎附注（note）不参与丢弃且**位置不动**——按索引跳过
    /// note 找到最旧的可丢弃程序块（若摘下重排到队尾，display 的真实交错顺序
    /// 会被破坏，违反模块头"附注并非总在末尾"的承诺；截断提示也不能被程序
    /// 输出逐字节吃掉）。首次触发丢弃时附一条截断注记。
    fn enforce_budget(&mut self) {
        /// 单次处置：部分截除（保留尾部）或整条丢弃。
        enum Trim {
            Partial(usize),
            Whole,
        }
        while self.program_bytes() > self.budget {
            // 最旧的非 note 程序块（note 原地跳过）
            let Some(idx) = self.chunks.iter().position(|c| c.kind != OutputKind::Note) else {
                break;
            };
            let action = {
                let target = &self.chunks[idx];
                let over = self.program_bytes() - self.budget;
                if target.text.len() > over {
                    // 从头部移除 over 字节（UTF-8 边界向后对齐），保留尾部
                    let mut boundary = over;
                    while boundary < target.text.len() && !target.text.is_char_boundary(boundary) {
                        boundary += 1;
                    }
                    Trim::Partial(boundary.min(target.text.len()))
                } else {
                    Trim::Whole
                }
            };
            match action {
                Trim::Partial(removed) => {
                    let kind = self.chunks[idx].kind;
                    self.chunks[idx].text.drain(..removed);
                    self.on_bytes_removed(kind, removed);
                    break;
                }
                Trim::Whole => {
                    let kind = self.chunks[idx].kind;
                    let n = self.chunks.remove(idx).expect("idx checked").text.len();
                    self.on_bytes_removed(kind, n);
                }
            }
        }
        if self.dropped_bytes > 0 && !self.truncation_noted {
            self.truncation_noted = true;
            let note = format!(
                "[引擎] 程序输出超出 {} 字节预算，更早的输出已截断丢弃，仅保留最新部分。\n",
                self.budget
            );
            self.push_raw(OutputKind::Note, note);
        }
    }

    /// 拼接指定通道的全部文本。
    pub fn join(&self, kind: OutputKind) -> String {
        let mut out = String::with_capacity(self.kind_bytes[kind.index()]);
        for chunk in self.chunks.iter().filter(|c| c.kind == kind) {
            out.push_str(&chunk.text);
        }
        out
    }

    /// 展示视图：所有通道按写入顺序拼接。
    pub fn join_all(&self) -> String {
        let mut out = String::with_capacity(self.total_bytes);
        for chunk in &self.chunks {
            out.push_str(&chunk.text);
        }
        out
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.total_bytes = 0;
        self.kind_bytes = [0; 3];
        self.dropped_bytes = 0;
        self.truncation_noted = false;
    }
}
