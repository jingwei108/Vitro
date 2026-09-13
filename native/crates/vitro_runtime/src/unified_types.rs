//! 统一模式 / 时间旅行相关的数据结构基础版本。
//!
//! `vitro_native::unified::types` 中定义了带 `#[frb]` 的 Dart 绑定类型；
//! 本模块提供不含 `#[frb]` 的基础数据结构，供 `vitro_vm` 生成原始数据后
//! 在 `vitro_native` 层转换为 FRB 友好类型。

/// 数组变量快照（用于算法可视化条形图）。
#[derive(Debug, Clone)]
pub struct ArraySnapshotData {
    pub name: String,
    pub element_ty: String,
    pub elements: Vec<String>,
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
