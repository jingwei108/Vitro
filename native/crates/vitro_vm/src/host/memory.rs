use super::*;
use crate::VmContext;
use vitro_runtime::{DEFAULT_QUARANTINE_BUDGET, MEM_SIZE};

pub fn host_malloc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let size = vm.pop() as i32;
    if size == 0 {
        session.runtime.push_note("[warning] malloc(0) 返回 NULL。在 C 标准中，malloc(0) 的行为是实现定义的，可能返回 NULL 也可能返回一个不可解引用的非空指针。".to_string());
        vm.push(0);
        return;
    }
    if size < 0 {
        vm.push(0);
        return;
    }
    let aligned_size = ((size as u32) + 3) & !3;
    let addr = match session.memory.allocate_raw(aligned_size, vm.get_memory_size()) {
        Some(a) => a,
        None => {
            report_heap_exhausted(session);
            vm.push(0);
            return;
        }
    };
    // 清理被新分配重用的 freed_logs
    let new_end = addr.saturating_add(aligned_size);
    vm.freed_logs.retain(|log| {
        let log_end = log.addr.saturating_add(log.size);
        log_end <= addr || log.addr >= new_end
    });
    // U2#2：复用/新增经 addr 索引 O(1) 判定（旧实现两段 `iter().find` 线性扫描，
    // G6 实测稳态 ~16387 条）。复用 = 该 addr 已有条目；复位 is_freed + 更新
    // 分配元数据一次完成（旧实现复位与元数据更新分两个循环扫两遍）。
    match session.memory.find_region_mut(addr) {
        Some(r) if r.is_freed => {
            r.is_freed = false;
            r.size = size;
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "malloc".to_string();
        }
        Some(r) => {
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "malloc".to_string();
        }
        None => {
            session.memory.alloc_counter += 1;
            session.memory.push_region(MemoryRegionData {
                addr,
                size,
                name: format!("heap_{}", session.memory.alloc_counter),
                ty: "int".to_string(),
                is_heap: true,
                is_freed: false,
                alloc_line: vm.get_current_line(),
                alloc_by: "malloc".to_string(),
                kind: "heap".to_string(),
            });
        }
    }
    vm.push(addr as u64);
}

pub fn host_free(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    if addr == 0 {
        return;
    }
    // Double-Free 检测
    if let Some(log) = vm.freed_logs.iter().find(|log| log.addr == addr) {
        let msg = format!("🔁 Double-Free (E3061)：你正在 free 一块已经在第 {} 行被释放过的内存（由第 {} 行的 malloc/realloc 分配）。\n\n💡 原因：同一块内存被释放了两次，这通常是因为 free(p) 后没有将 p 置为 NULL，或者两个指针指向同一块内存且都被释放了。\n✅ 解决方法：每次 free(p) 后立刻写 p = NULL;。对 NULL 指针重复 free 是安全的。", log.freed_line, log.alloc_line);
        vm.trap(&msg, &SourceLoc::default());
        return;
    }
    let mut freed_ok = false;
    let mut freed_size = 0i32;
    // U2#2：按 addr 索引 O(1) 定位（旧实现线性扫描，见 G6 定论）
    if let Some(r) = session.memory.find_region_mut(addr) {
        if !r.is_freed {
            r.is_freed = true;
            let aligned_size = ((r.size as u32) + 3) & !3;
            vm.freed_logs.push(FreedRegionInfo {
                addr: r.addr,
                size: aligned_size,
                alloc_line: r.alloc_line,
                freed_line: vm.get_current_line(),
                alloc_step: 0,
                freed_step: vm.get_executed_steps(),
            });
            freed_size = aligned_size as i32;
            freed_ok = true;
        }
    }
    if freed_ok {
        // 决议 §3：free 的块进入 FIFO 隔离区（地址在隔离期内不复用），
        // 由隔离预算（默认 256KB）与 FIFO 驱逐控制复用时机。
        session.memory.release_to_quarantine(FreeBlock {
            addr,
            size: freed_size,
        });
    }
    // V-P1-12：free 非分配起始地址的诊断。此前静默忽略，学生以为释放成功
    // 而泄漏报告又看不到该块，双重误导。
    if !freed_ok {
        trap_invalid_free(vm, session, addr);
    }
}

/// V-P1-12：对 free 无效地址给出分场景教学诊断并 trap。
pub(crate) fn trap_invalid_free(vm: &mut VitroVM, session: &VmContext<'_>, addr: u32) {
    if let Some(r) = session
        .memory
        .regions
        .iter()
        .find(|r| !r.is_freed && addr > r.addr && addr < r.addr + r.size as u32)
    {
        vm.trap(
            &format!(
                "🚫 无效 free (E3027)：free 的地址 0x{:X} 是第 {} 行 malloc 块（起始 0x{:X}，{} 字节）的内部地址，不是起始地址。\n\n💡 原因：free 只能释放 malloc/calloc/realloc 返回的原始指针。\n✅ 解决方法：保存分配返回的原始指针用于 free；需要偏移访问时使用单独的指针变量，不要 free 偏移后的指针。",
                addr, r.alloc_line, r.addr, r.size
            ),
            &SourceLoc::default(),
        );
    } else if let Some(log) = vm.freed_logs.iter().find(|log| addr > log.addr && addr < log.addr + log.size) {
        vm.trap(
            &format!(
                "🔁 无效 free (E3061)：地址 0x{:X} 位于第 {} 行已释放块（起始 0x{:X}）的内部。free 只能释放 malloc 返回的原始指针。",
                addr, log.freed_line, log.addr
            ),
            &SourceLoc::default(),
        );
    } else {
        vm.trap(
            &format!(
                "🚫 无效 free (E3027)：free 的地址 0x{:X} 不指向任何 malloc/calloc/realloc 分配的内存块起始地址（可能是栈地址、全局变量地址或未初始化指针的垃圾值）。\n\n💡 原因：只有 malloc 系列函数返回的指针才能被 free。\n✅ 解决方法：检查该指针是否来自 malloc 分配，且未经过指针算术运算。",
                addr
            ),
            &SourceLoc::default(),
        );
    }
}

/// 决议 §5：leak 路径撞 1MB 墙时的教学提示（按内容去重，只在首次出现时打印）。
///
/// 注意此处**返回 NULL 而不 trap**：C 标准要求分配失败返回 NULL，Clang 下同样
/// 返回 NULL；trap 会让 Vitro 偏离"检查 malloc 返回值"这一必须建立的编程习惯。
/// （诚实记录：与决议 §5"教学 trap"措辞的差异，理由如上，行为对齐 Clang。）
pub(crate) fn report_heap_exhausted(session: &mut VmContext<'_>) {
    // 末尾必须带换行：note 通道的后续 printf 输出会续接最后一个元素，
    // 缺换行会让教学提示与程序输出粘成一行。
    // R3：数值从 vitro_runtime 常量格式化（此前文本硬编码 1MB/256KB 双写）。
    let msg = format!(
        "[堆] 内存耗尽：malloc/calloc/realloc 返回 NULL（Vitro 堆上限 {}KB，其中 {}KB 为隔离区预算）。常见原因是「只分配不释放」——请确认每条 malloc 路径都有对应的 free。\n",
        MEM_SIZE / 1024,
        DEFAULT_QUARANTINE_BUDGET / 1024
    );
    if !session.runtime.note_chunks().contains(&msg.as_str()) {
        session.runtime.push_note(&msg);
    }
}

pub fn host_realloc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let ptr = vm.pop() as u32;
    let new_size = vm.pop() as i32;

    if new_size <= 0 {
        if ptr != 0 {
            // Equivalent to free：块进 FIFO 隔离区（决议 §3）
            // U2#2：addr 索引 O(1) 定位
            let mut freed_ok = false;
            let mut freed_size = 0i32;
            if let Some(r) = session.memory.find_region_mut(ptr) {
                if !r.is_freed {
                    r.is_freed = true;
                    let aligned_size = ((r.size as u32) + 3) & !3;
                    vm.freed_logs.push(FreedRegionInfo {
                        addr: ptr,
                        size: aligned_size,
                        alloc_line: r.alloc_line,
                        freed_line: vm.get_current_line(),
                        alloc_step: 0,
                        freed_step: vm.get_executed_steps(),
                    });
                    freed_size = aligned_size as i32;
                    freed_ok = true;
                }
            }
            if freed_ok {
                session.memory.release_to_quarantine(FreeBlock {
                    addr: ptr,
                    size: freed_size,
                });
            } else {
                // V-P1-12：realloc(p, 0) 等价 free，同样诊断无效地址
                trap_invalid_free(vm, session, ptr);
            }
        }
        vm.push(0);
        return;
    }

    if ptr == 0 {
        // Equivalent to malloc
        vm.push(new_size as u64);
        host_malloc(vm, session);
        return;
    }

    // Find existing region（U2#2：addr 索引 O(1)）
    let old_region = session
        .memory
        .find_region_mut(ptr)
        .filter(|r| !r.is_freed)
        .map(|r| (r.addr, r.size));

    let Some((old_addr, old_size)) = old_region else {
        vm.push(0);
        return;
    };
    let aligned_new_size = ((new_size as u32) + 3) & !3;
    let aligned_old_size = ((old_size as u32) + 3) & !3;

    // 决议 §3：realloc 恒为新块拷贝（与 glibc 常见路径一致）。
    // 原实现的"堆顶原地收缩"与"优先复用旧地址"两个特例都会破坏"隔离窗口内
    // 地址不复用"的保证：收缩会把 heap_offset 回退进隔离区，原地复用让刚 free
    // 的地址立即重新生效（UAF 检测窗口失效）。
    let Some(new_addr) = session.memory.allocate_raw(aligned_new_size, vm.get_memory_size()) else {
        report_heap_exhausted(session);
        vm.push(0);
        return;
    };

    // 清理被新分配重用的 freed_logs（必须在写入新内存之前执行，
    // 否则 store_i8 会触发 Use-After-Free 误报）
    let new_end = new_addr.saturating_add(aligned_new_size);
    vm.freed_logs.retain(|log| {
        let log_end = log.addr.saturating_add(log.size);
        log_end <= new_addr || log.addr >= new_end
    });

    // Copy old data
    let copy_size = (old_size as u32).min(aligned_new_size);
    let copy_buf = {
        let mem = vm.get_memory_slice();
        mem[old_addr as usize..(old_addr + copy_size) as usize].to_vec()
    };
    for i in 0..copy_size {
        vm.store_i8(new_addr + i, copy_buf[i as usize] as i32, &SourceLoc::default());
    }

    // Zero remaining bytes
    for i in copy_size..aligned_new_size {
        vm.store_i8(new_addr + i, 0, &SourceLoc::default());
    }

    // Free old region：进入 FIFO 隔离区（决议 §3），不直接归还 free_list
    // U2#2：addr 索引 O(1) 定位（旧线性扫描）+ alloc_line 需先读出再置 freed
    let mut old_freed = false;
    if let Some(r) = session.memory.find_region_mut(old_addr) {
        if !r.is_freed {
            r.is_freed = true;
            let alloc_line = r.alloc_line;
            vm.freed_logs.push(FreedRegionInfo {
                addr: old_addr,
                size: aligned_old_size,
                alloc_line,
                freed_line: vm.get_current_line(),
                alloc_step: 0,
                freed_step: vm.get_executed_steps(),
            });
            old_freed = true;
        }
    }
    if old_freed {
        session.memory.release_to_quarantine(FreeBlock {
            addr: old_addr,
            size: aligned_old_size as i32,
        });
    }

    // 若 realloc 恰好复用了旧地址（如 heap_offset 回退后），需清理刚添加的 freed_log
    let new_end = new_addr.saturating_add(aligned_new_size);
    vm.freed_logs.retain(|log| {
        let log_end = log.addr.saturating_add(log.size);
        log_end <= new_addr || log.addr >= new_end
    });

    // Track new region（U2#2：与 malloc 相同的"复位 or push"——新地址可能来自
    // free_list 复用隔离驱逐块，该 addr 已有条目，无条件 push 会造成同 addr
    // 双条目：泄漏虚报 + 索引失配，红锚 test_u22_realloc_reuse_no_duplicate_entries）
    match session.memory.find_region_mut(new_addr) {
        Some(r) if r.is_freed => {
            r.is_freed = false;
            r.size = new_size;
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "realloc".to_string();
        }
        Some(r) => {
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "realloc".to_string();
        }
        None => {
            session.memory.push_region(MemoryRegionData {
                addr: new_addr,
                size: new_size,
                name: String::new(),
                ty: String::new(),
                is_heap: true,
                is_freed: false,
                alloc_line: vm.get_current_line(),
                alloc_by: "realloc".to_string(),
                kind: "heap".to_string(),
            });
        }
    }

    vm.push(new_addr as u64);
}

pub fn host_calloc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let size = vm.pop() as i32;
    let nmemb = vm.pop() as i32;
    if size <= 0 || nmemb <= 0 {
        vm.push(0);
        return;
    }
    let total = (nmemb as u32).saturating_mul(size as u32);
    let aligned_size = (total + 3) & !3;
    let addr = match session.memory.allocate_raw(aligned_size, vm.get_memory_size()) {
        Some(a) => a,
        None => {
            report_heap_exhausted(session);
            vm.push(0);
            return;
        }
    };
    // zero-initialize
    for i in 0..aligned_size {
        vm.store_i8(addr + i, 0, &SourceLoc::default());
    }
    // clean freed_logs
    let new_end = addr.saturating_add(aligned_size);
    vm.freed_logs.retain(|log| {
        let log_end = log.addr.saturating_add(log.size);
        log_end <= addr || log.addr >= new_end
    });
    // U2#2：新地址可能来自 free_list 复用驱逐块——"复位 or push"（同 malloc，
    // 防同 addr 双条目）
    match session.memory.find_region_mut(addr) {
        Some(r) if r.is_freed => {
            r.is_freed = false;
            r.size = total as i32;
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "calloc".to_string();
        }
        Some(r) => {
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "calloc".to_string();
        }
        None => {
            session.memory.alloc_counter += 1;
            session.memory.push_region(MemoryRegionData {
                addr,
                size: total as i32,
                name: format!("heap_{}", session.memory.alloc_counter),
                ty: "int".to_string(),
                is_heap: true,
                is_freed: false,
                alloc_line: vm.get_current_line(),
                alloc_by: "calloc".to_string(),
                kind: "heap".to_string(),
            });
        }
    }
    vm.push(addr as u64);
}
