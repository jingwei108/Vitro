//! 统一模式 / 时间旅行相关的数据结构基础版本。
//!
//! `vitro_native::unified::types` 中定义了带 `#[frb]` 的 Dart 绑定类型；
//! 本模块提供不含 `#[frb]` 的基础数据结构，供 `vitro_vm` 生成原始数据后
//! 在 `vitro_native` 层转换为 FRB 友好类型。

/// 数组变量快照（用于算法可视化条形图）。
/// 数组快照 payload 级元素上限（U2#10）：可视化条形图消费上限 + 内存有界
///（`int a[50000]` × 2000 帧窗口的 GB 形态根治）。
pub const MAX_ARRAY_SNAPSHOT_ELEMENTS: usize = 256;

#[derive(Debug, Clone)]
pub struct ArraySnapshotData {
    pub name: String,
    pub element_ty: String,
    pub elements: Vec<String>,
    /// U2#10：元素数超出 [`crate::MAX_ARRAY_SNAPSHOT_ELEMENTS`] 时置位
    ///（payload 级截断可见，消费方不得把 elements 当全量）。
    pub truncated: bool,
}

/// 指针变量快照基础数据。
#[derive(Debug, Clone)]
pub struct PointerSnapshotData {
    pub name: String,
    pub addr: u32,
    pub ty_name: String,
    pub target_addr: u32,
    pub target_name: String,
    pub status: PointerStatusData,
}

/// 指针状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerStatusData {
    Valid,
    Freed,
    Null,
    Dangling,
}

/// 当前步访问的变量基础数据。
#[derive(Debug, Clone)]
pub struct AccessedVarData {
    pub name: String,
    pub access_type: String, // "Read" | "Write"
}
