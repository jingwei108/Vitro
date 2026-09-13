use super::*;
use crate::VmContext;

pub fn host_strlen(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let bytes = read_cbytes(vm, addr);
    vm.push(bytes.len() as u64);
}

pub fn host_strdup(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let src = vm.pop() as u32;
    let src_bytes = read_cbytes(vm, src);
    let len = src_bytes.len();
    let size = (len + 1) as i32;
    if size <= 0 {
        vm.push(0);
        return;
    }
    let aligned_size = ((size as u32) + 3) & !3;
    let addr = match session.memory.allocate_raw(aligned_size, vm.get_memory_size()) {
        Some(a) => a,
        None => {
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
    // 复制字符串内容（含终止符）
    let mem_size = vm.get_memory_slice().len();
    if (addr as usize) < mem_size {
        let max_write = mem_size - addr as usize;
        let write_len = len.min(max_write.saturating_sub(1));
        for (i, &b) in src_bytes.iter().enumerate().take(write_len) {
            vm.store_i8(addr + i as u32, b as i32, &SourceLoc::default());
        }
        let end = addr as usize + write_len;
        if end < mem_size {
            vm.store_i8(end as u32, 0, &SourceLoc::default());
        }
    }
    // reuse or add region
    let mut reused = false;
    for r in &mut session.memory.regions {
        if r.addr == addr && r.is_freed {
            r.is_freed = false;
            r.size = size;
            reused = true;
            break;
        }
    }
    if !reused {
        session.memory.alloc_counter += 1;
        session.memory.regions.push(MemoryRegionData {
            addr,
            size,
            name: format!("heap_{}", session.memory.alloc_counter),
            ty: "int".to_string(),
            is_heap: true,
            is_freed: false,
            alloc_line: vm.get_current_line(),
            alloc_by: "strdup".to_string(),
            kind: "heap".to_string(),
        });
    } else {
        for r in &mut session.memory.regions {
            if r.addr == addr && !r.is_freed {
                r.alloc_line = vm.get_current_line();
                r.alloc_by = "strdup".to_string();
                break;
            }
        }
    }
    vm.push(addr as u64);
}

pub fn host_strcpy(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let src_bytes = read_cbytes(vm, src);
    let src_len = src_bytes.len();
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size {
        vm.push(0);
        return;
    }

    // 注入边界检查：如果 dest 落在已知的堆分配区域内，验证容量
    for r in &session.memory.regions {
        if !r.is_freed && dest >= r.addr && dest < r.addr + r.size as u32 {
            let offset = (dest - r.addr) as usize;
            let available = (r.size as usize).saturating_sub(offset);
            if src_len + 1 > available {
                vm.trap(
                    &format!(
                        "💥 Buffer Overflow (E3070)：strcpy 目标缓冲区溢出。源字符串长度 {} 字节（含终止符 {} 字节），但目标区域 '{}' 从偏移 {} 开始仅剩 {} 字节。\n\n💡 原因：strcpy 不会检查目标缓冲区大小。\n✅ 解决方法：使用 strncpy 或确保目标缓冲区足够大（至少 {} 字节）。",
                        src_len, src_len + 1, r.name, offset, available, src_len + 1
                    ),
                    &SourceLoc::default(),
                );
                return;
            }
            break;
        }
    }

    // V-P1-6：栈上缓冲区容量校验（此前只查堆 region，栈上 char buf[N] 被静默覆写）
    if check_stack_buffer_capacity(vm, dest, src_len + 1, "strcpy") {
        return;
    }

    let max_copy = mem_size - dest as usize;
    let copy_len = src_len.min(max_copy.saturating_sub(1));
    for (i, &b) in src_bytes.iter().enumerate().take(copy_len) {
        vm.store_i8(dest + i as u32, b as i32, &SourceLoc::default());
    }
    let end = dest as usize + copy_len;
    vm.store_i8(end as u32, 0, &SourceLoc::default());
    vm.push(dest as u64);
}

pub fn host_strcmp(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr1 = vm.pop() as u32;
    let addr2 = vm.pop() as u32;
    let mem = vm.get_memory_slice();
    let mut i = 0usize;
    let result = loop {
        let a = if addr1 as usize + i < mem.len() {
            mem[addr1 as usize + i]
        } else {
            0
        };
        let b = if addr2 as usize + i < mem.len() {
            mem[addr2 as usize + i]
        } else {
            0
        };
        if a != b {
            break (a as i8).wrapping_sub(b as i8) as i32;
        }
        if a == 0 {
            break 0;
        }
        i += 1;
    };
    vm.push(result as u64);
}

// Helper trait to get memory slice safely
pub fn host_memset(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let ptr = vm.pop() as u32;
    let value = vm.pop();
    let size = vm.pop();
    let mem_size = vm.get_memory_slice().len();
    if ptr as usize >= mem_size {
        vm.push(ptr as u64);
        return;
    }
    let max_write = mem_size - ptr as usize;
    let write_len = (size as usize).min(max_write);
    // 注入 NULL 指针安全检查（与 VM store_i8 保持一致）
    if ptr < NULL_TRAP_SIZE && write_len > 0 {
        let msg = format!("向 NULL 指针区域写入（地址 0x{:04X}）。请确认指针已被正确初始化。", ptr);
        vm.trap(&msg, &SourceLoc::default());
        vm.push(ptr as u64);
        return;
    }
    let byte_val = (value & 0xFF) as u8;
    let mem = vm.memory_ref_mut();
    mem[ptr as usize..ptr as usize + write_len].fill(byte_val);
    vm.push(ptr as u64);
}

pub fn host_strcat(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size {
        vm.push(dest as u64);
        return;
    }
    let mut end = dest as usize;
    while end < mem_size && vm.get_memory_slice()[end] != 0 {
        end += 1;
    }
    let dest_len = end - dest as usize;
    let src_bytes = read_cbytes(vm, src);
    let src_len = src_bytes.len();

    // 注入边界检查
    for r in &session.memory.regions {
        if !r.is_freed && dest >= r.addr && dest < r.addr + r.size as u32 {
            let offset = (dest - r.addr) as usize;
            let available = (r.size as usize).saturating_sub(offset);
            if dest_len + src_len + 1 > available {
                vm.trap(
                    &format!(
                        "💥 Buffer Overflow (E3070)：strcat 目标缓冲区溢出。已有 {} 字节 + 源字符串 {} 字节 + 终止符 1 字节 = {} 字节，但目标区域 '{}' 从偏移 {} 开始仅剩 {} 字节。\n\n💡 原因：strcat 不会检查目标缓冲区剩余容量。\n✅ 解决方法：使用 strncat 或确保目标缓冲区足够大。",
                        dest_len, src_len, dest_len + src_len + 1, r.name, offset, available
                    ),
                    &SourceLoc::default(),
                );
                return;
            }
            break;
        }
    }

    // V-P1-6：栈上缓冲区容量校验（end 为 dest 起已有内容末尾）
    if check_stack_buffer_capacity(vm, dest, dest_len + src_len + 1, "strcat") {
        return;
    }

    let max_copy = mem_size.saturating_sub(end).saturating_sub(1);
    let copy_len = src_len.min(max_copy);
    for (i, &b) in src_bytes.iter().enumerate().take(copy_len) {
        vm.store_i8((end + i) as u32, b as i32, &SourceLoc::default());
    }
    vm.store_i8((end + copy_len) as u32, 0, &SourceLoc::default());
    vm.push(dest as u64);
}

pub fn host_strncpy(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let n = vm.pop() as i32;
    let src_bytes = read_cbytes(vm, src);
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size {
        vm.push(dest as u64);
        return;
    }
    let max_write = mem_size - dest as usize;
    let write_len = (n as usize).min(max_write);
    let copy_len = src_bytes.len().min(write_len);
    for (i, &byte) in src_bytes.iter().enumerate().take(copy_len) {
        vm.store_i8(dest + i as u32, byte as i32, &SourceLoc::default());
    }
    for i in copy_len..write_len {
        vm.store_i8(dest + i as u32, 0, &SourceLoc::default());
    }
    vm.push(dest as u64);
}

pub fn host_memcpy(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let n = vm.pop() as i32;
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size || src as usize >= mem_size {
        vm.push(dest as u64);
        return;
    }
    let copy_len = (n as usize).min(mem_size - dest as usize).min(mem_size - src as usize);
    if copy_len > 0 {
        let buf = {
            let mem = vm.memory_ref();
            mem[src as usize..src as usize + copy_len].to_vec()
        };
        let mem = vm.memory_ref_mut();
        for i in 0..copy_len {
            mem[dest as usize + i] = buf[i];
        }
    }
    vm.push(dest as u64);
}

pub fn host_memmove(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let n = vm.pop() as i32;
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size || src as usize >= mem_size {
        vm.push(dest as u64);
        return;
    }
    let copy_len = (n as usize).min(mem_size - dest as usize).min(mem_size - src as usize);
    if copy_len > 0 {
        let buf = {
            let mem = vm.memory_ref();
            mem[src as usize..src as usize + copy_len].to_vec()
        };
        let mem = vm.memory_ref_mut();
        for i in 0..copy_len {
            mem[dest as usize + i] = buf[i];
        }
    }
    vm.push(dest as u64);
}

pub fn host_strncat(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let dest = vm.pop() as u32;
    let src = vm.pop() as u32;
    let n = vm.pop() as usize;
    let mem_size = vm.get_memory_slice().len();
    if dest as usize >= mem_size {
        vm.push(dest as u64);
        return;
    }
    let mut end = dest as usize;
    while end < mem_size && vm.get_memory_slice()[end] != 0 {
        end += 1;
    }
    let src_bytes = read_cbytes(vm, src);
    let copy_len = src_bytes.len().min(n);
    let max_copy = mem_size.saturating_sub(end).saturating_sub(1);
    let actual_copy = copy_len.min(max_copy);
    for (i, &b) in src_bytes.iter().enumerate().take(actual_copy) {
        vm.store_i8((end + i) as u32, b as i32, &SourceLoc::default());
    }
    vm.store_i8((end + actual_copy) as u32, 0, &SourceLoc::default());
    vm.push(dest as u64);
}

pub fn host_strncmp(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr1 = vm.pop() as u32;
    let addr2 = vm.pop() as u32;
    let n = vm.pop() as usize;
    let mem = vm.get_memory_slice();
    let mut i = 0usize;
    let result = loop {
        if i >= n {
            break 0;
        }
        let a = if addr1 as usize + i < mem.len() {
            mem[addr1 as usize + i]
        } else {
            0
        };
        let b = if addr2 as usize + i < mem.len() {
            mem[addr2 as usize + i]
        } else {
            0
        };
        if a != b {
            break (a as i8).wrapping_sub(b as i8) as i32;
        }
        if a == 0 {
            break 0;
        }
        i += 1;
    };
    vm.push(result as u64);
}

pub fn host_memcmp(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr1 = vm.pop() as u32;
    let addr2 = vm.pop() as u32;
    let n = vm.pop() as usize;
    let mem = vm.get_memory_slice();
    let mut result = 0i32;
    for i in 0..n {
        let a = if addr1 as usize + i < mem.len() {
            mem[addr1 as usize + i]
        } else {
            0
        };
        let b = if addr2 as usize + i < mem.len() {
            mem[addr2 as usize + i]
        } else {
            0
        };
        if a != b {
            result = (a as i8).wrapping_sub(b as i8) as i32;
            break;
        }
    }
    vm.push(result as u64);
}

pub fn host_strchr(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let c = vm.pop() as i32;
    let mem = vm.get_memory_slice();
    let mut i = 0usize;
    let result = loop {
        let b = if addr as usize + i < mem.len() {
            mem[addr as usize + i]
        } else {
            0
        };
        if b as i32 == c {
            break addr + i as u32;
        }
        if b == 0 {
            break 0;
        }
        i += 1;
    };
    vm.push(result as u64);
}

pub fn host_strrchr(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let c = vm.pop() as i32;
    let mem = vm.get_memory_slice();
    let mut result = 0u32;
    let mut i = 0usize;
    while addr as usize + i < mem.len() {
        let b = mem[addr as usize + i];
        if b as i32 == c {
            result = addr + i as u32;
        }
        if b == 0 {
            break;
        }
        i += 1;
    }
    vm.push(result as u64);
}

pub fn host_strstr(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let haystack = vm.pop() as u32;
    let needle = vm.pop() as u32;
    let needle_bytes = read_cbytes(vm, needle);
    let haystack_bytes = read_cbytes(vm, haystack);
    if needle_bytes.is_empty() {
        vm.push(haystack as u64);
        return;
    }
    let result = haystack_bytes
        .windows(needle_bytes.len())
        .position(|window| window == needle_bytes);
    vm.push(match result {
        Some(pos) => (haystack + pos as u32) as u64,
        None => 0,
    });
}

pub fn host_memchr(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let c = vm.pop() as i32;
    let n = vm.pop() as usize;
    let mem = vm.get_memory_slice();
    let mut result = 0u32;
    for i in 0..n {
        if addr as usize + i >= mem.len() {
            break;
        }
        if mem[addr as usize + i] as i32 == c {
            result = addr + i as u32;
            break;
        }
    }
    vm.push(result as u64);
}

// ========== Conversion extensions ==========

pub fn host_strpbrk(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let s_addr = vm.pop() as u32;
    let accept_addr = vm.pop() as u32;
    let s = read_cstring(vm, s_addr);
    let accept = read_cstring(vm, accept_addr);
    for (i, c) in s.char_indices() {
        if accept.contains(c) {
            vm.push((s_addr + i as u32) as u64);
            return;
        }
    }
    vm.push(0);
}

pub fn host_strspn(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let s_addr = vm.pop() as u32;
    let accept_addr = vm.pop() as u32;
    let s = read_cstring(vm, s_addr);
    let accept = read_cstring(vm, accept_addr);
    let mut count = 0usize;
    for c in s.chars() {
        if accept.contains(c) {
            count += 1;
        } else {
            break;
        }
    }
    vm.push(count as u64);
}

pub fn host_strcspn(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let s_addr = vm.pop() as u32;
    let reject_addr = vm.pop() as u32;
    let s = read_cstring(vm, s_addr);
    let reject = read_cstring(vm, reject_addr);
    let mut count = 0usize;
    for c in s.chars() {
        if reject.contains(c) {
            break;
        }
        count += 1;
    }
    vm.push(count as u64);
}
