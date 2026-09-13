//! E2 模块化预处理器（皮肤与内核分离）。
//!
//! 架构落点（重构计划 §3）：学生写的仍是标准 C 预处理语法；引擎内部不做文本
//! 变换黑魔法。各子模块职责：
//! - [`macro_table`]：`MacroTable`——宏定义表（对象式/参数式）+ 遮蔽诊断 +
//!   预定义宏族（`__STDC_VERSION__=202311L` 名义锚点 / `__VITRO_SUBSET__`）；
//! - [`resolver`]：include 解析——ModuleGraph 语义（include-once + 依赖图 +
//!   环检测诊断）+ 标准库存根加载；
//! - [`cond`]：`#if` / `#elif` 条件求值器——`defined()`、`__has_include`、
//!   整数常量表达式（非图灵完备），分支选择原因记录（白箱教学层）；
//! - [`expander`]：token 树转录展开器——展开深度保险丝（64）+ 展开链诊断 +
//!   宏参数副作用检测 + 自引用停止（展开栈查重）；
//! - [`splice`]：受限 token 操作——`#` 字符串化、`##` 拼接（结果必须合法，
//!   按 C99 6.10.3.1/3.3 规则：操作数不预先展开）。
//!
//! 行为契约（v3 定案）：参数化宏调用后跟分号的 `do { ... } while(0)` 自动包装
//! （H01）保留，落在本模块展开器的后处理阶段。

pub mod cond;
pub mod directives;
pub mod expander;
pub mod macro_table;
pub mod resolver;
pub mod splice;

pub use cond::CondOutcome;
pub use macro_table::MacroTable;
pub use resolver::IncludeResolver;

use crate::token::{LexerError, LexerWarning, Token};
use vitro_shared::ErrorCode;

/// 宏定义（兼容旧公开形态）。
#[derive(Debug, Clone)]
pub struct MacroDef {
    pub params: Vec<String>,
    pub body: Vec<Token>,
}

/// 条件编译状态。
///
/// `taken` 记录本组条件中是否已有真分支——`#elif` 依赖它（`#else` 保持既有
/// `has_else` 语义）。
#[derive(Debug, Clone, Copy)]
pub struct ConditionalState {
    pub active: bool,
    pub has_else: bool,
    pub taken: bool,
}

/// 宏定义表。
pub type Macros = MacroTable;

/// 展开深度保险丝（行为契约化：防止自引用/互引用宏把编译器拖死）。
/// U1#6：检查点在 `expand_inner` 每层递归入口（此前只在 depth=0 的公开入口
/// 检查 = 死代码——不同名对象宏链 `A0→A1→…` 5000 层实测栈溢出）。
pub const EXPAND_DEPTH_FUSE: usize = 64;

/// 展开规模保险丝：深度限深不限宽（`REP(x) x x` 嵌套 64 层 = 2^64 token），
/// 累计展开产出超过预算即熔断——同报 E1017。
/// U1#6：计费口径从 token 个数改为**累计字节数**（token 文本长度 +1 分隔）。
/// 旧口径实测失效：`S(x) x x` + 4KB 字面量 × 14 层 = 16384 个 token 远低于
/// 26 万 token 预算，但实际产出 67MB 且零诊断。16MB 对正常教学程序
/// （数千行源码 ≈ 数百 KB）余量充足。
pub const EXPAND_BYTE_BUDGET: usize = 16 * 1024 * 1024;

/// 展开链/分支原因教学记录的容量上限（防止病态程序无限膨胀）。
pub const TEACHING_TRACE_CAP: usize = 64;

/// 单条教学追踪记录的长度上限（U1#6）：条数封顶不防单条——67MB 展开的
/// 结果拼写拼进一条 trace 同样是病态内存。超长截断加省略号。
pub const TRACE_ENTRY_MAX_CHARS: usize = 2048;

/// 展开上下文：宏表 + 各诊断/教学收集通道。
pub(crate) struct ExpandCtx<'a> {
    pub macros: &'a MacroTable,
    pub errors: &'a mut Vec<LexerError>,
    pub warnings: &'a mut Vec<LexerWarning>,
    /// 白箱教学层：展开链（`#if` 表达式内不记录，见 `expr_mode`）。
    pub trace: &'a mut Vec<String>,
    /// `#if` 表达式求值场景：展开链噪声不记录。
    pub expr_mode: bool,
    /// 已展开产出的累计字节数（规模预算，U1#6 改字节口径）。
    pub emitted: usize,
    /// 预算已熔断（错误只报一次）。
    pub budget_exhausted: bool,
    /// 深度保险丝已熔断（错误只报一次，U1#6）。
    pub depth_exceeded: bool,
}

pub(crate) fn lex_error(message: impl Into<String>, line: i32, column: i32, code: ErrorCode) -> LexerError {
    LexerError { message: message.into(), line, column, code: code as i32 }
}

/// 教学追踪封顶推送（展开链 / `#if` 分支原因共用）。
pub(crate) fn push_trace(trace: &mut Vec<String>, entry: String) {
    if trace.len() < TEACHING_TRACE_CAP {
        trace.push(entry);
    }
}
