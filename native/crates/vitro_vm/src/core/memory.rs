//! VM 内存访问：安全读写、脏页追踪、变量快照与数组快照。

use super::state::{VitroVM, FreedRegionInfo, VMSymbol, MEM_SIZE, NULL_TRAP_SIZE};
use crate::instruction::SourceLoc;
use vitro_ast::TypeKind;
use vitro_runtime::base_kind;

impl VitroVM {
    /// 将 C 风格字符串安全写入 VM 内存的指定地址（含 null 终止符）。
    /// 若目标地址超出边界则静默跳过。
    ///
    /// 注意：边界检查 `a + bytes.len() < len` 已隐含为末尾的 null 终止符预留了 1 字节空间，
    /// 因此当 `addr + bytes.len() == MEM_SIZE` 时会正确拒绝写入，避免越界。
    pub fn write_cstring(&mut self, addr: u32, s: &str) {
        let a = addr as usize;
        let bytes = s.as_bytes();
        let total = bytes.len() + 1;
        // 统一边界检查：NULL 区、上界、UAF。
        if !self.check_mem_access(addr, total as u32, &SourceLoc::default(), true) {
            return;
        }
        if let Some(log) = self.check_uaf(addr, total as u32) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, &SourceLoc::default());
            return;
        }
        self.memory[a..a + bytes.len()].copy_from_slice(bytes);
        self.memory[a + bytes.len()] = 0;
        self.mark_dirty_page(addr, total as u32);
    }

    /// 安全写入字节切片到 VM 内存（带 NULL Trap 和边界检查）
    pub fn write_memory(&mut self, addr: u32, data: &[u8]) -> bool {
        let end = addr as usize + data.len();
        if addr < NULL_TRAP_SIZE || end > self.memory.len() {
            return false;
        }
        if let Some(log) = self.check_uaf(addr, data.len() as u32) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, &SourceLoc::default());
            return false;
        }
        self.memory[addr as usize..end].copy_from_slice(data);
        self.mark_dirty_page(addr, data.len() as u32);
        true
    }

    /// 安全读取字节切片从 VM 内存到外部缓冲区
    pub fn read_memory_to(&self, addr: u32, buf: &mut [u8]) -> bool {
        let end = addr as usize + buf.len();
        if addr < NULL_TRAP_SIZE || end > self.memory.len() {
            return false;
        }
        buf.copy_from_slice(&self.memory[addr as usize..end]);
        true
    }

    /// 安全地在 VM 内存内部复制（src → dst），带完整边界检查
    pub fn copy_memory(&mut self, dst: u32, src: u32, len: usize) -> bool {
        let dst_end = dst as usize + len;
        let src_end = src as usize + len;
        if dst < NULL_TRAP_SIZE || src < NULL_TRAP_SIZE || dst_end > self.memory.len() || src_end > self.memory.len() {
            return false;
        }
        let len_u32 = len as u32;
        if let Some(log) = self.check_uaf(dst, len_u32) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, &SourceLoc::default());
            return false;
        }
        if let Some(log) = self.check_uaf(src, len_u32) {
            let msg = self.format_uaf_message(log, false);
            self.trap(&msg, &SourceLoc::default());
            return false;
        }
        // 用临时 buffer 避免重叠问题
        let tmp = self.memory[src as usize..src_end].to_vec();
        self.memory[dst as usize..dst_end].copy_from_slice(&tmp);
        self.mark_dirty_page(dst, len as u32);
        true
    }

    pub fn load_i32(&mut self, addr: u32, loc: &SourceLoc) -> i32 {
        if !self.check_mem_access(addr, 4, loc, false) {
            return 0;
        }
        if let Some(log) = self.check_uaf(addr, 4) {
            let msg = self.format_uaf_message(log, false);
            self.trap(&msg, loc);
            return 0;
        }
        i32::from_le_bytes([
            self.memory[addr as usize],
            self.memory[addr as usize + 1],
            self.memory[addr as usize + 2],
            self.memory[addr as usize + 3],
        ])
    }

    pub fn store_i32(&mut self, addr: u32, val: i32, loc: &SourceLoc) {
        if !self.check_mem_access(addr, 4, loc, true) {
            return;
        }
        if let Some(log) = self.check_uaf(addr, 4) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, loc);
            return;
        }
        let bytes = val.to_le_bytes();
        self.memory[addr as usize..addr as usize + 4].copy_from_slice(&bytes);
        self.mark_dirty_page(addr, 4);
    }

    pub fn load_i64(&mut self, addr: u32, loc: &SourceLoc) -> u64 {
        if !self.check_mem_access(addr, 8, loc, false) {
            return 0;
        }
        if let Some(log) = self.check_uaf(addr, 8) {
            let msg = self.format_uaf_message(log, false);
            self.trap(&msg, loc);
            return 0;
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.memory[addr as usize..addr as usize + 8]);
        u64::from_le_bytes(bytes)
    }

    pub fn store_i64(&mut self, addr: u32, val: u64, loc: &SourceLoc) {
        if !self.check_mem_access(addr, 8, loc, true) {
            return;
        }
        if let Some(log) = self.check_uaf(addr, 8) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, loc);
            return;
        }
        let bytes = val.to_le_bytes();
        self.memory[addr as usize..addr as usize + 8].copy_from_slice(&bytes);
        self.mark_dirty_page(addr, 8);
    }

    pub fn load_i8(&mut self, addr: u32, loc: &SourceLoc) -> i32 {
        if !self.check_mem_access(addr, 1, loc, false) {
            return 0;
        }
        if let Some(log) = self.check_uaf(addr, 1) {
            let msg = self.format_uaf_message(log, false);
            self.trap(&msg, loc);
            return 0;
        }
        self.memory[addr as usize] as i8 as i32
    }

    pub fn store_i8(&mut self, addr: u32, val: i32, loc: &SourceLoc) {
        if !self.check_mem_access(addr, 1, loc, true) {
            return;
        }
        if let Some(log) = self.check_uaf(addr, 1) {
            let msg = self.format_uaf_message(log, true);
            self.trap(&msg, loc);
            return;
        }
        self.memory[addr as usize] = val as u8;
        self.mark_dirty_page(addr, 1);
    }

    /// 标记脏页（ addr 为起始地址，len 为字节数 ）。
    pub(crate) fn mark_dirty_page(&mut self, addr: u32, len: u32) {
        if len == 0 {
            return;
        }
        let start_page = (addr as usize) / crate::snapshot::PAGE_SIZE;
        let end_page = ((addr as usize) + (len as usize) - 1) / crate::snapshot::PAGE_SIZE;
        for p in start_page..=end_page {
            if p < crate::snapshot::PAGE_COUNT {
                let word = p / 64;
                let bit = p % 64;
                self.dirty_pages[word] |= 1u64 << bit;
            }
        }
    }

    /// 清空脏页记录。
    pub fn clear_dirty_pages(&mut self) {
        self.dirty_pages = [0; 4];
    }

    pub(crate) fn check_uaf(&self, addr: u32, size: u32) -> Option<&FreedRegionInfo> {
        // U2#2-b：原全量线性扫（churn 稳态 ~16k 条 × 每次访存）——
        // BTreeMap 互不重叠单调性下的降序扫描，O(log n + 命中数)。
        self.freed_logs_find_overlapping(addr, size)
    }

    pub(crate) fn format_uaf_message(&self, log: &FreedRegionInfo, is_write: bool) -> String {
        let action = if is_write { "写入" } else { "读取" };
        format!(
            "💥 Use-After-Free (E3060)：你正在向一块已经在第 {} 行（第 {} 步）被 free 的内存{}（由第 {} 行的 malloc/realloc 分配）。\n\n⏱️ 时间轴：分配 → 释放（第 {} 步）→ 继续访问（第 {} 步）。\n\n💡 原因：指针在 free 后没有置为 NULL，或者存在别名指针（另一个指针也指向同一块内存）。\n✅ 解决方法：free(p) 后立即写 p = NULL;，并确保不再通过其他指针访问这块内存。",
            log.freed_line, log.freed_step, action, log.alloc_line, log.freed_step, self.step_count
        )
    }

    pub(crate) fn check_mem_access(&mut self, addr: u32, size: u32, loc: &SourceLoc, is_write: bool) -> bool {
        if addr < NULL_TRAP_SIZE {
            let msg = if is_write {
                format!("向 NULL 指针区域写入（地址 0x{:04X}）。请确认指针已被正确初始化。", addr)
            } else {
                format!(
                    "访问了 NULL 指针区域（地址 0x{:04X}）。NULL 指针不能解引用。请确认指针已被正确初始化。",
                    addr
                )
            };
            self.trap(&msg, loc);
            return false;
        }
        if addr as u64 + size as u64 > MEM_SIZE as u64 {
            self.trap(&self.format_bounds_error(addr), loc);
            return false;
        }
        true
    }

    pub(crate) fn read_variable(&self, sym: &VMSymbol) -> i64 {
        let vaddr = if sym.is_local {
            if let Some(frame) = self.call_stack.last() {
                frame.locals_base + sym.addr
            } else {
                return 0;
            }
        } else {
            super::state::GLOBAL_START + sym.addr
        };
        if vaddr + 4 > MEM_SIZE || vaddr < NULL_TRAP_SIZE {
            return 0;
        }
        if matches!(sym.ty.kind(), TypeKind::Double | TypeKind::LongLong) {
            if vaddr + 8 > MEM_SIZE {
                return 0;
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&self.memory[vaddr as usize..vaddr as usize + 8]);
            u64::from_le_bytes(bytes) as i64
        } else {
            i32::from_le_bytes([
                self.memory[vaddr as usize],
                self.memory[vaddr as usize + 1],
                self.memory[vaddr as usize + 2],
                self.memory[vaddr as usize + 3],
            ]) as i64
        }
    }

    /// 当前**可见**变量的快照（供 `StepPayload.local_vars`）。
    ///
    /// 可见性规则（P2-7a + 跨函数混入修复）：
    /// 1. **按函数归属过滤**：函数内声明的符号（`func_name` 非空，含静态局部）只在该函数
    ///    栈帧活跃时可见。此前不过滤，`helper` 的局部变量会出现在 `main` 的 payload 里，
    ///    并且是用 `main` 的 `locals_base` 去读 `helper` 的偏移——地址错位、值无意义；
    /// 2. **按声明行过滤**：`decl_line` 大于当前执行行的符号尚未进入作用域，不可见
    ///    （`decl_line == 0` 表示未知，不过滤）。这是"两个 `for` 各声明一个 `i`"时
    ///    唯一能判定谁活跃的依据——符号表顺序与分配地址都无法排除"尚未执行到声明处"；
    /// 3. **同名取最近声明**：同一名字在多个块中声明时保留 `decl_line` 最大者
    ///    （与调试器"显示当前作用域可见变量"的惯例一致），条目的相对顺序保持不变。
    pub fn get_variable_snapshot(&self) -> Vec<vitro_runtime::VariableSnapshotData> {
        let frame = self.call_stack.last();
        let current_func = frame.map(|f| f.func_name.as_str()).unwrap_or("");
        let current_line = self.get_current_line();

        let mut result: Vec<vitro_runtime::VariableSnapshotData> = Vec::new();
        // 名字 → (已选条目的声明行, 在 result 中的下标)
        let mut chosen: std::collections::HashMap<&str, (i32, usize)> = std::collections::HashMap::new();

        for sym in self.symbols.iter() {
            if !sym.func_name.is_empty() && sym.func_name != current_func {
                continue;
            }
            // 无有效执行位置（如预热步 code_line == 0）时无法判定作用域可见性：
            // 保守地不输出局部变量，否则同名变量会退化为"取声明最晚者"而误导消费方。
            if sym.is_local && current_line <= 0 {
                continue;
            }
            if sym.decl_line > 0 && current_line > 0 && sym.decl_line > current_line {
                continue;
            }
            let vaddr = if sym.is_local {
                match frame {
                    Some(f) => f.locals_base + sym.addr,
                    None => continue,
                }
            } else {
                super::state::GLOBAL_START + sym.addr
            };
            if vaddr + 4 > MEM_SIZE || vaddr < NULL_TRAP_SIZE {
                continue;
            }
            let val = if matches!(sym.ty.kind(), TypeKind::Double) {
                if vaddr + 8 > MEM_SIZE {
                    continue;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&self.memory[vaddr as usize..vaddr as usize + 8]);
                u64::from_le_bytes(bytes) as i64
            } else {
                i32::from_le_bytes([
                    self.memory[vaddr as usize],
                    self.memory[vaddr as usize + 1],
                    self.memory[vaddr as usize + 2],
                    self.memory[vaddr as usize + 3],
                ]) as i64
            };
            let snap = vitro_runtime::VariableSnapshotData {
                name: sym.name.clone(),
                addr: vaddr,
                is_local: sym.is_local,
                ty: sym.ty.clone(),
                value: val,
            };
            match chosen.get(sym.name.as_str()).copied() {
                Some((prev_line, idx)) => {
                    if sym.decl_line >= prev_line {
                        result[idx] = snap;
                        chosen.insert(sym.name.as_str(), (sym.decl_line, idx));
                    }
                }
                None => {
                    chosen.insert(sym.name.as_str(), (sym.decl_line, result.len()));
                    result.push(snap);
                }
            }
        }
        result
    }

    /// 跨帧解析：`addr` 归属的变量名（下游需求清单 D3 / SharpTutor S3 §6 观测 #2）。
    ///
    /// `get_variable_snapshot` 只回当前帧可见变量；作为**指针快照的 target_name 源**
    /// 它不够用：`swap(int *a, int *b)` 体内 `a` 指向调用者的 `x`，当前帧里根本没有
    /// `x` 这个符号，于是 `target_addr` 正确而 `target_name == ""`（实测现象）。
    /// 本函数把搜索范围扩到**全局 + 全部活跃帧**（由内向外，调试器"最近定义优先"惯例）。
    ///
    /// 可见性判定与 `get_variable_snapshot` 同源（函数归属 + 声明行作用域）：
    /// - 每个帧的"当前执行行"：顶层帧取 `current_line`，其余帧取**被调帧记录的
    ///   `caller_line`**（`call_stack[i+1].caller_line` 正是帧 `i` 的当前行）；
    /// - 命中规则：地址**等于变量起始地址**，或落在局部/全局**数组**的元素区间内
    ///   （`&arr[2]` 这类取中间元素地址也能归属到 `arr`）。其他复合类型（struct 字段
    ///   取址）无布局信息，不参与区间匹配——已知限制，记于 schema §2.5。
    pub fn find_variable_name_at_addr(&self, addr: u32) -> Option<String> {
        for sym in self.symbols.iter().filter(|s| !s.is_local) {
            if slot_covers(sym, super::state::GLOBAL_START + sym.addr, addr) {
                return Some(sym.name.clone());
            }
        }
        for idx in (0..self.call_stack.len()).rev() {
            let frame = &self.call_stack[idx];
            let frame_line = match self.call_stack.get(idx + 1) {
                Some(callee) => callee.caller_line,
                None => self.current_line,
            };
            for sym in self.symbols.iter() {
                if !sym.is_local || sym.func_name != frame.func_name {
                    continue;
                }
                if sym.decl_line > 0 && frame_line > 0 && sym.decl_line > frame_line {
                    continue;
                }
                if slot_covers(sym, frame.locals_base + sym.addr, addr) {
                    return Some(sym.name.clone());
                }
            }
        }
        None
    }

    /// 获取所有数组变量的元素快照（用于算法可视化条形图）。
    pub fn get_array_snapshots(&self) -> Vec<vitro_runtime::ArraySnapshotData> {
        let mut result = Vec::new();
        for sym in &self.symbols {
            if sym.ty.kind() != TypeKind::Array {
                continue;
            }
            let vaddr = if sym.is_local {
                if let Some(frame) = self.call_stack.last() {
                    frame.locals_base + sym.addr
                } else {
                    continue;
                }
            } else {
                super::state::GLOBAL_START + sym.addr
            };
            let array_size = sym.ty.array_size();
            if array_size <= 0 {
                continue;
            }
            let base_kind = base_kind(&sym.ty);
            let elem_size = match base_kind {
                TypeKind::Char => 1,
                TypeKind::Int | TypeKind::Pointer | TypeKind::Float => 4,
                TypeKind::Double | TypeKind::LongLong => 8,
                _ => 4,
            };
            let mut elements = Vec::with_capacity(array_size as usize);
            for i in 0..array_size {
                let addr = vaddr + (i as u32) * (elem_size as u32);
                let elem_size_u32 = elem_size as u32;
                // NULL 区检查；上界在后续索引前已经判断。
                if addr < NULL_TRAP_SIZE || addr + elem_size_u32 > MEM_SIZE {
                    break;
                }
                let val_str = match base_kind {
                    TypeKind::Char => {
                        let b = self.memory[addr as usize];
                        if (32..=126).contains(&b) {
                            format!("'{}'", b as char)
                        } else {
                            format!("{}", b as i8)
                        }
                    }
                    TypeKind::Int => {
                        let bytes = [
                            self.memory[addr as usize],
                            self.memory[addr as usize + 1],
                            self.memory[addr as usize + 2],
                            self.memory[addr as usize + 3],
                        ];
                        i32::from_le_bytes(bytes).to_string()
                    }
                    TypeKind::Float => {
                        let bytes = [
                            self.memory[addr as usize],
                            self.memory[addr as usize + 1],
                            self.memory[addr as usize + 2],
                            self.memory[addr as usize + 3],
                        ];
                        format!("{:.2}", f32::from_le_bytes(bytes))
                    }
                    TypeKind::Double => {
                        let mut bytes = [0u8; 8];
                        bytes.copy_from_slice(&self.memory[addr as usize..addr as usize + 8]);
                        format!("{:.2}", f64::from_le_bytes(bytes))
                    }
                    TypeKind::LongLong => {
                        let mut bytes = [0u8; 8];
                        bytes.copy_from_slice(&self.memory[addr as usize..addr as usize + 8]);
                        i64::from_le_bytes(bytes).to_string()
                    }
                    _ => "?".to_string(),
                };
                elements.push(val_str);
            }
            // 类型名走与 local_vars 同一来源（`vitro_runtime::type_display_name`），
            // 避免两处手写映射漂移。
            let element_ty = vitro_runtime::type_display_name(vitro_ast::base_element_type(&sym.ty));
            result.push(vitro_runtime::ArraySnapshotData {
                name: sym.name.clone(),
                element_ty,
                elements,
            });
        }
        result
    }
}

/// 变量槽 `base` 是否覆盖地址 `addr`（D3 的归属判据）。
///
/// - 精确命中起始地址：恒真；
/// - 数组类型额外接受**元素区间**内的地址（`&arr[2]` 归属到 `arr`）——元素宽度按
///   `vitro_runtime::base_kind` 判定（char 1 / double·long long 8 / 其余 4），
///   与 `get_array_snapshots` 的取值口径同源；
/// - 其余复合类型（struct/union/class）无布局信息，只做精确匹配。
fn slot_covers(sym: &VMSymbol, base: u32, addr: u32) -> bool {
    if base == addr {
        return true;
    }
    if sym.ty.kind() != TypeKind::Array {
        return false;
    }
    let count = sym.ty.array_size();
    if count <= 0 {
        return false;
    }
    let elem_size = match base_kind(&sym.ty) {
        TypeKind::Char => 1u32,
        TypeKind::Double | TypeKind::LongLong => 8,
        _ => 4,
    };
    addr > base && addr < base + count as u32 * elem_size
}
