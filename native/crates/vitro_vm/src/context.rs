use crate::vfs::VirtualFileSystem;
use vitro_runtime::{MemoryState, RuntimeState};

/// VM 执行上下文：替代对 `Session` 的直接依赖。
///
/// `VitroVM` 在执行过程中只需要访问 runtime 状态、内存状态与 VFS，
/// 无需了解 `Session` 中编译期状态（如 bytecode、func_table 等）。
/// 该结构体将 VM 所需的可变引用聚合在一起，使 `vitro_vm` crate 可以独立。
pub struct VmContext<'a> {
    pub runtime: &'a mut RuntimeState,
    pub memory: &'a mut MemoryState,
    pub vfs: &'a mut VirtualFileSystem,
}

impl<'a> VmContext<'a> {
    /// 构造新的 VM 上下文。
    pub fn new(runtime: &'a mut RuntimeState, memory: &'a mut MemoryState, vfs: &'a mut VirtualFileSystem) -> Self {
        Self { runtime, memory, vfs }
    }
}
