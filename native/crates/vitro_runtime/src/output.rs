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
