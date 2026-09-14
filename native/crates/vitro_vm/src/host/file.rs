use super::*;
use crate::VmContext;

pub fn host_fopen(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 参数从右到左压栈：path, mode → 栈顶是 path
    let path_addr = vm.pop() as u32;
    let mode_addr = vm.pop() as u32;
    let path = read_cstring(vm, path_addr);
    let mode = read_cstring(vm, mode_addr);
    let vfs = &mut session.vfs;
    let fd = vfs.fopen(&path, &mode, vm, session.memory);

    // 在 VM Heap 中分配 4 字节存储 fd，返回 FILE*
    if fd != 0 {
        let mut file_ptr = 0u32;
        let aligned = 4u32;
        // 统一走堆分配入口（bump + 有界隔离），不再直接推进 heap_offset
        if let Some(addr) = session.memory.allocate_raw(aligned, vm.get_memory_size()) {
            // 该地址可能刚从隔离区驱逐复用，清理对应 freed_logs
            let new_end = addr + aligned;
            vm.freed_logs.retain(|log| {
                let log_end = log.addr.saturating_add(log.size);
                log_end <= addr || log.addr >= new_end
            });
            // U2#2：复用/新增经 addr 索引 O(1)（同 host_malloc）
            match session.memory.find_region_mut(addr) {
                Some(r) if r.is_freed => {
                    r.is_freed = false;
                    r.size = 4;
                    r.name = format!("FILE:{}", path);
                    r.alloc_line = vm.get_current_line();
                    r.alloc_by = "fopen".to_string();
                }
                Some(r) => {
                    r.name = format!("FILE:{}", path);
                    r.alloc_line = vm.get_current_line();
                    r.alloc_by = "fopen".to_string();
                }
                None => {
                    session.memory.alloc_counter += 1;
                    session.memory.push_region(MemoryRegionData {
                        addr,
                        size: 4,
                        name: format!("FILE:{}", path),
                        ty: "int".to_string(),
                        is_heap: true,
                        is_freed: false,
                        alloc_line: vm.get_current_line(),
                        alloc_by: "fopen".to_string(),
                        kind: "heap".to_string(),
                    });
                }
            }
            // 写入 fd 到 FILE* 结构体
            let mem = vm.memory_ref_mut();
            let a = addr as usize;
            mem[a..a + 4].copy_from_slice(&(fd as i32).to_le_bytes());
            file_ptr = addr;
        }
        vm.push(file_ptr as u64);
    } else {
        vm.push(0);
    }
}

pub fn host_fread(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 参数从右到左压栈：buf, size, nmemb, stream → 栈顶是 buf
    let buf = vm.pop() as u32;
    let size = vm.pop() as usize;
    let nmemb = vm.pop() as usize;
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let n = vfs.fread(fd, buf, size, nmemb, vm);

    vm.push(n as u64);
}

pub fn host_fwrite(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 参数从右到左压栈：buf, size, nmemb, stream → 栈顶是 buf
    let buf = vm.pop() as u32;
    let size = vm.pop() as usize;
    let nmemb = vm.pop() as usize;
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let n = vfs.fwrite(fd, buf, size, nmemb, vm, session.memory);

    vm.push(n as u64);
}

pub fn host_fclose(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ret = vfs.fclose(fd, session.memory);

    // 释放 fopen 在 VM Heap 中为 FILE* 结构体分配的 4 字节内存。
    // stdout/stderr 等非堆分配 stream 找不到对应 region，free_region 返回 false，安全忽略。
    session.memory.free_region(stream);

    vm.push(ret as u64);
}

pub fn host_feof(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let ret = session.vfs.feof(fd);
    vm.push(ret as u64);
}

pub fn host_fgets(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 参数从右到左压栈：buf, n, stream → 栈顶是 buf
    let buf = vm.pop() as u32;
    let n = vm.pop() as usize;
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let ret = session.vfs.fgets(fd, buf, n, vm);
    vm.push(ret as u64);
}

pub fn host_fputs(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 参数从右到左压栈：s, stream → 栈顶是 s
    let s_addr = vm.pop() as u32;
    let stream = vm.pop() as u32;

    // stdout(1)/stderr(2) 直接输出到运行时会话。
    // E-P1-5：stderr 走独立通道——此前与 stdout 混在一条字节流里，程序写 stderr 时
    // 会被 Shadow Verification 误当成 stdout 差异。
    if stream == 1 || stream == 2 {
        let s = read_cstring(vm, s_addr);
        if stream == 2 {
            session.runtime.push_stderr(s);
        } else {
            session.runtime.push_stdout(s);
        }
        vm.push(0u64);
        return;
    }

    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ret = vfs.fputs(fd, s_addr, vm, session.memory);

    vm.push(ret as u64);
}

/// 从 FILE* 指针（VM Heap 地址）读取 fd 索引
pub fn host_fgetc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ch = vfs.fgetc(fd, vm);

    vm.push(ch as i64 as u64);
}

pub fn host_fputc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ret = vfs.fputc(fd, c, vm, session.memory);

    vm.push(ret as i64 as u64);
}

pub fn host_fseek(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let offset = vm.pop() as i32;
    let whence = vm.pop() as i32;
    let vfs = &mut session.vfs;
    let ret = vfs.fseek(fd, offset, whence, vm);

    vm.push(ret as u64);
}

pub fn host_ftell(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ret = vfs.ftell(fd, vm);

    vm.push(ret as i64 as u64);
}

pub fn host_rewind(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    vfs.rewind(fd);
}

// ========== String / memory extensions ==========

pub fn host_fflush(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    let ret = if fd == 0 {
        // fflush(NULL) — 刷新所有流，VFS 内存模式无缓冲，视为成功
        0
    } else {
        vfs.fflush(fd)
    };

    vm.push(ret as u64);
}

pub fn host_perror(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let s_addr = vm.pop() as u32;
    let prefix = read_cstring(vm, s_addr);
    let msg = if prefix.is_empty() {
        "Error\n".to_string()
    } else {
        format!("{}: Error\n", prefix)
    };
    // C 标准：perror 输出到 stderr（E-P1-5：独立通道，不混入 stdout）。
    session.runtime.push_stderr(msg);
}

pub fn host_clearerr(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop() as u32;
    let fd = read_fd_from_stream(vm, stream);
    let vfs = &mut session.vfs;
    vfs.clearerr(fd);
}

pub fn host_remove(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let path_addr = vm.pop() as u32;
    let path = read_cstring(vm, path_addr);
    let vfs = &mut session.vfs;
    let ret = vfs.remove(&path, session.memory);

    vm.push(ret as u64);
}

pub fn host_rename(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let old_path_addr = vm.pop() as u32;
    let new_path_addr = vm.pop() as u32;
    let old_path = read_cstring(vm, old_path_addr);
    let new_path = read_cstring(vm, new_path_addr);
    let vfs = &mut session.vfs;
    let ret = vfs.rename(&old_path, &new_path);

    vm.push(ret as u64);
}
