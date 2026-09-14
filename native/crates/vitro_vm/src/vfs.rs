//! 轻量级虚拟文件系统（VFS-lite）
//!
//! 教学友好的文件 I/O 实现：
//! - 文件元数据（fd、cursor、mode）在 Rust 端管理
//! - 文件数据存储在 VM Heap 中，前端内存 Canvas 可直接可视化
//! - 支持预设文件注入（如 test.txt），用于教学演示

use std::collections::HashMap;

use crate::core::VitroVM;
use vitro_runtime::{MemoryRegionData, MemoryState};

#[derive(Debug, Clone, Default)]
pub struct VirtualFileSystem {
    files: HashMap<String, VfsFileMeta>,
    descriptors: HashMap<u32, VfsDesc>,
    next_fd: u32,
}

#[derive(Debug, Clone)]
struct VfsFileMeta {
    heap_addr: u32,
    size: usize,
    capacity: usize,
}

#[derive(Debug, Clone)]
struct VfsDesc {
    file_name: String,
    mode: VfsMode,
    cursor: usize,
    eof: bool,
    error: bool,
    is_text_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VfsMode {
    Read,
    Write,
    Append,
}

/// 将文本模式下的逻辑位置（把 \r\n 视为一个 \n）转换为物理字节偏移。
fn logical_to_physical(data: &[u8], logical_pos: usize) -> usize {
    let mut logical = 0usize;
    let mut physical = 0usize;
    while physical < data.len() && logical < logical_pos {
        if data[physical] == b'\r' && physical + 1 < data.len() && data[physical + 1] == b'\n' {
            logical += 1;
            physical += 2;
        } else {
            logical += 1;
            physical += 1;
        }
    }
    physical
}

/// 将文本模式下的物理字节偏移转换为逻辑位置。
fn physical_to_logical(data: &[u8], physical_pos: usize) -> usize {
    let mut logical = 0usize;
    let mut physical = 0usize;
    while physical < physical_pos.min(data.len()) {
        if data[physical] == b'\r' && physical + 1 < data.len() && data[physical + 1] == b'\n' {
            logical += 1;
            physical += 2;
        } else {
            logical += 1;
            physical += 1;
        }
    }
    logical
}

/// 在文本模式下读取一个逻辑字节，返回（字节值，下一个物理位置）。
fn read_text_byte(data: &[u8], physical_pos: usize) -> (u8, usize) {
    if physical_pos >= data.len() {
        return (0, physical_pos);
    }
    if data[physical_pos] == b'\r' && physical_pos + 1 < data.len() && data[physical_pos + 1] == b'\n' {
        (b'\n', physical_pos + 2)
    } else {
        (data[physical_pos], physical_pos + 1)
    }
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入预设文件到 VM Heap。通常在 `setup_vm` 之后调用。
    pub fn inject_preset_file(&mut self, name: &str, data: &[u8], vm: &mut VitroVM, memory: &mut MemoryState) -> bool {
        if self.files.contains_key(name) {
            return false;
        }
        let size = data.len();
        let aligned = align4(size);
        let addr = match malloc_raw(memory, aligned, vm.get_memory_size()) {
            Some(a) => a,
            None => return false,
        };
        // 写入数据到 VM 内存
        if !vm.write_memory(addr, data) {
            free_raw(memory, addr);
            return false;
        }
        // 更新 region 名称
        for r in &mut memory.regions {
            if r.addr == addr {
                r.name = format!("vfs:{}", name);
                break;
            }
        }
        self.files.insert(
            name.to_string(),
            VfsFileMeta {
                heap_addr: addr,
                size,
                capacity: aligned,
            },
        );
        true
    }

    /// fopen：打开或创建文件，返回 fd（0 表示失败）
    pub fn fopen(&mut self, path: &str, mode: &str, vm: &mut VitroVM, memory: &mut MemoryState) -> u32 {
        let (vfs_mode, is_text_mode) = match mode {
            "r" => (VfsMode::Read, true),
            "rb" => (VfsMode::Read, false),
            "w" => (VfsMode::Write, true),
            "wb" => (VfsMode::Write, false),
            "a" => (VfsMode::Append, true),
            "ab" => (VfsMode::Append, false),
            _ => return 0,
        };
        let mode = vfs_mode;

        if mode == VfsMode::Read && !self.files.contains_key(path) {
            return 0; // 读模式文件不存在
        }

        if mode == VfsMode::Write {
            // 覆盖模式：清空现有文件或创建新文件
            if let Some(meta) = self.files.get_mut(path) {
                meta.size = 0;
            } else {
                let initial_cap = 256usize;
                let addr = match malloc_raw(memory, initial_cap, vm.get_memory_size()) {
                    Some(a) => a,
                    None => return 0,
                };
                // 更新 region 名称
                for r in &mut memory.regions {
                    if r.addr == addr {
                        r.name = format!("vfs:{}", path);
                        break;
                    }
                }
                self.files.insert(
                    path.to_string(),
                    VfsFileMeta {
                        heap_addr: addr,
                        size: 0,
                        capacity: initial_cap,
                    },
                );
            }

            if mode == VfsMode::Append && !self.files.contains_key(path) {
                let initial_cap = 256usize;
                let addr = match malloc_raw(memory, initial_cap, vm.get_memory_size()) {
                    Some(a) => a,
                    None => return 0,
                };
                for r in &mut memory.regions {
                    if r.addr == addr {
                        r.name = format!("vfs:{}", path);
                        break;
                    }
                }
                self.files.insert(
                    path.to_string(),
                    VfsFileMeta {
                        heap_addr: addr,
                        size: 0,
                        capacity: initial_cap,
                    },
                );
            }
        }

        let fd = self.next_fd + 1;
        self.next_fd = fd;

        let cursor = if mode == VfsMode::Append {
            self.files.get(path).map(|m| m.size).unwrap_or(0)
        } else {
            0
        };

        self.descriptors.insert(
            fd,
            VfsDesc {
                file_name: path.to_string(),
                mode,
                cursor,
                eof: false,
                error: false,
                is_text_mode,
            },
        );
        fd
    }

    /// fread：从文件读取数据到 VM 内存 buf，返回实际读取的元素数
    pub fn fread(&mut self, fd: u32, buf: u32, size: usize, nmemb: usize, vm: &mut VitroVM) -> usize {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return 0,
        };
        let meta = match self.files.get(&desc.file_name) {
            Some(m) => m,
            None => return 0,
        };
        // 溢出防护：size * nmemb 可能 wrap（如 2^32 * 2^32 = 0）导致海量分配；
        // 读取总量也不应超过 VM 线性内存大小。
        let to_read = match size.checked_mul(nmemb) {
            Some(t) if t <= vm.get_memory_size() as usize => t,
            _ => return 0,
        };
        if to_read == 0 {
            return 0;
        }

        let mut tmp = vec![0u8; to_read];
        let mut actual = 0usize;

        if desc.is_text_mode {
            // 文本模式：把 \r\n 压缩为 \n，返回逻辑字节数
            let mut data = vec![0u8; meta.size];
            if !vm.read_memory_to(meta.heap_addr, &mut data) {
                return 0;
            }
            let mut physical_cursor = desc.cursor;
            while actual < to_read && physical_cursor < meta.size {
                if data[physical_cursor] == b'\r'
                    && physical_cursor + 1 < meta.size
                    && data[physical_cursor + 1] == b'\n'
                {
                    tmp[actual] = b'\n';
                    physical_cursor += 2;
                } else {
                    tmp[actual] = data[physical_cursor];
                    physical_cursor += 1;
                }
                actual += 1;
            }
            desc.cursor = physical_cursor;
        } else {
            let remaining = meta.size.saturating_sub(desc.cursor);
            actual = to_read.min(remaining);
            if actual == 0 {
                desc.eof = true;
                return 0;
            }
            let src_addr = meta.heap_addr + desc.cursor as u32;
            if !vm.read_memory_to(src_addr, &mut tmp[..actual]) || !vm.write_memory(buf, &tmp[..actual]) {
                return 0;
            }
            desc.cursor += actual;
        }

        if actual == 0 {
            desc.eof = true;
            return 0;
        }
        if !vm.write_memory(buf, &tmp[..actual]) {
            return 0;
        }
        if desc.cursor >= meta.size {
            desc.eof = true;
        }
        actual / size
    }

    /// fwrite：从 VM 内存 buf 写入数据到文件，返回实际写入的元素数
    pub fn fwrite(
        &mut self,
        fd: u32,
        buf: u32,
        size: usize,
        nmemb: usize,
        vm: &mut VitroVM,
        memory: &mut MemoryState,
    ) -> usize {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return 0,
        };
        if desc.mode == VfsMode::Read {
            return 0;
        }
        // 溢出防护同 fread：size * nmemb 可能 wrap；写入总量不应超过 VM 线性内存大小。
        let to_write = match size.checked_mul(nmemb) {
            Some(t) if t <= vm.get_memory_size() as usize => t,
            _ => return 0,
        };
        let meta = match self.files.get_mut(&desc.file_name) {
            Some(m) => m,
            None => return 0,
        };

        // 先读取用户数据
        let mut user_data = vec![0u8; to_write];
        if !vm.read_memory_to(buf, &mut user_data) {
            return 0;
        }

        // 文本模式下将 \n 展开为 \r\n
        let data_to_write: Vec<u8> = if desc.is_text_mode {
            let mut expanded = Vec::with_capacity(to_write * 2);
            for &b in &user_data {
                if b == b'\n' {
                    expanded.push(b'\r');
                    expanded.push(b'\n');
                } else {
                    expanded.push(b);
                }
            }
            expanded
        } else {
            user_data
        };

        let write_len = data_to_write.len();
        let new_size = desc.cursor + write_len;
        if new_size > meta.capacity {
            let new_cap = align4(new_size.max(meta.capacity * 2));
            if !realloc_vfs_file(memory, vm, meta, new_cap) {
                return 0;
            }
        }
        let dst_addr = meta.heap_addr + desc.cursor as u32;
        if !vm.write_memory(dst_addr, &data_to_write) {
            return 0;
        }
        desc.cursor += write_len;
        if desc.cursor > meta.size {
            meta.size = desc.cursor;
        }
        nmemb
    }

    /// fclose：关闭文件描述符
    pub fn fclose(&mut self, fd: u32, _memory: &mut MemoryState) -> i32 {
        if self.descriptors.remove(&fd).is_some() {
            0
        } else {
            -1
        }
    }

    /// feof：检查是否到达文件末尾
    pub fn feof(&self, fd: u32) -> i32 {
        match self.descriptors.get(&fd) {
            Some(d) if d.eof => 1,
            _ => 0,
        }
    }

    /// fgets：从文件读取一行（最多 n-1 字符），写入 buf 并追加 \0
    /// 返回 buf 地址（成功）或 0（失败/EOF）
    pub fn fgets(&mut self, fd: u32, buf: u32, n: usize, vm: &mut VitroVM) -> u32 {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return 0,
        };
        let meta = match self.files.get(&desc.file_name) {
            Some(m) => m,
            None => return 0,
        };
        if n == 0 {
            return 0;
        }

        let mut data = vec![0u8; meta.size];
        if meta.size > 0 && !vm.read_memory_to(meta.heap_addr, &mut data) {
            return 0;
        }

        let mut read_count = 0usize;
        let mut tmp = Vec::new();
        while read_count < n - 1 && desc.cursor < meta.size {
            let (b, next_pos) = read_text_byte(&data, desc.cursor);
            tmp.push(b);
            desc.cursor = next_pos;
            read_count += 1;
            if b == b'\n' {
                break;
            }
        }
        if read_count == 0 {
            desc.eof = true;
            return 0;
        }
        tmp.push(0);
        if !vm.write_memory(buf, &tmp) {
            return 0;
        }
        if desc.cursor >= meta.size {
            desc.eof = true;
        }
        buf
    }

    /// fputs：将字符串 s 写入文件
    /// 返回非负数（成功）或 EOF（-1，失败）
    pub fn fputs(&mut self, fd: u32, s_addr: u32, vm: &mut VitroVM, memory: &mut MemoryState) -> i32 {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return -1,
        };
        if desc.mode == VfsMode::Read {
            return -1;
        }
        let meta = match self.files.get_mut(&desc.file_name) {
            Some(m) => m,
            None => return -1,
        };
        // 读取 C 字符串
        let mut tmp = Vec::new();
        loop {
            let mut b = [0u8; 1];
            if !vm.read_memory_to(s_addr + tmp.len() as u32, &mut b) || b[0] == 0 {
                break;
            }
            tmp.push(b[0]);
        }

        // 文本模式下将 \n 展开为 \r\n
        let data_to_write: Vec<u8> = if desc.is_text_mode {
            let mut expanded = Vec::with_capacity(tmp.len() * 2);
            for &b in &tmp {
                if b == b'\n' {
                    expanded.push(b'\r');
                    expanded.push(b'\n');
                } else {
                    expanded.push(b);
                }
            }
            expanded
        } else {
            tmp
        };

        let write_len = data_to_write.len();
        let new_size = desc.cursor + write_len;
        if new_size > meta.capacity {
            let new_cap = align4(new_size.max(meta.capacity * 2));
            if !realloc_vfs_file(memory, vm, meta, new_cap) {
                return -1;
            }
        }
        let dst_addr = meta.heap_addr + desc.cursor as u32;
        if !vm.write_memory(dst_addr, &data_to_write) {
            return -1;
        }
        desc.cursor += write_len;
        if desc.cursor > meta.size {
            meta.size = desc.cursor;
        }
        0
    }

    /// fgetc：从文件读取一个字节
    /// 返回字节值（0-255）或 EOF（-1）
    pub fn fgetc(&mut self, fd: u32, vm: &mut VitroVM) -> i32 {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return -1,
        };
        let meta = match self.files.get(&desc.file_name) {
            Some(m) => m,
            None => return -1,
        };
        if desc.cursor >= meta.size {
            desc.eof = true;
            return -1;
        }

        let mut data = vec![0u8; meta.size];
        if !vm.read_memory_to(meta.heap_addr, &mut data) {
            return -1;
        }

        let (b, next_pos) = read_text_byte(&data, desc.cursor);
        desc.cursor = next_pos;
        if desc.cursor >= meta.size {
            desc.eof = true;
        }
        b as i32
    }

    /// fputc：向文件写入一个字节
    /// 返回写入的字节值（成功）或 EOF（-1，失败）
    pub fn fputc(&mut self, fd: u32, c: i32, vm: &mut VitroVM, memory: &mut MemoryState) -> i32 {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return -1,
        };
        if desc.mode == VfsMode::Read {
            return -1;
        }
        let meta = match self.files.get_mut(&desc.file_name) {
            Some(m) => m,
            None => return -1,
        };

        let data_to_write: Vec<u8> = if desc.is_text_mode && c as u8 == b'\n' {
            vec![b'\r', b'\n']
        } else {
            vec![c as u8]
        };

        let write_len = data_to_write.len();
        let new_size = desc.cursor + write_len;
        if new_size > meta.capacity {
            let new_cap = align4(new_size.max(meta.capacity * 2));
            if !realloc_vfs_file(memory, vm, meta, new_cap) {
                return -1;
            }
        }
        let dst_addr = meta.heap_addr + desc.cursor as u32;
        if !vm.write_memory(dst_addr, &data_to_write) {
            return -1;
        }
        desc.cursor += write_len;
        if desc.cursor > meta.size {
            meta.size = desc.cursor;
        }
        c
    }

    /// fseek：移动文件光标
    /// whence: SEEK_SET=0, SEEK_CUR=1, SEEK_END=2
    /// 返回 0（成功）或 -1（失败）
    pub fn fseek(&mut self, fd: u32, offset: i32, whence: i32, vm: &mut VitroVM) -> i32 {
        let desc = match self.descriptors.get_mut(&fd) {
            Some(d) => d,
            None => return -1,
        };
        let meta = match self.files.get(&desc.file_name) {
            Some(m) => m,
            None => return -1,
        };

        // U2#13：负偏移（含 SEEK_CUR/END 计算出的负目标）统一**返回 -1 且不移动
        // 游标**（glibc EINVAL 语义）。此前二进制分支 `offset as i64 as usize`
        // 负数绕回为天文数字游标（后续读写即越界写），文本 SEEK_SET 同病，
        // 文本 SEEK_CUR/END 则把负目标钳 0 静默成功——三种形状都与 C 不符。
        let new_cursor = if desc.is_text_mode {
            let mut data = vec![0u8; meta.size];
            if meta.size > 0 && !vm.read_memory_to(meta.heap_addr, &mut data) {
                return -1;
            }
            let physical_size = meta.size;
            match whence {
                // SEEK_SET：offset 为逻辑位置，转换为物理位置
                0 => {
                    if offset < 0 {
                        return -1;
                    }
                    logical_to_physical(&data, offset as usize)
                }
                // SEEK_CUR：offset 为逻辑偏移，基于当前逻辑位置转换
                1 => {
                    let logical_cursor = physical_to_logical(&data, desc.cursor);
                    let target_logical = logical_cursor as i64 + offset as i64;
                    if target_logical < 0 {
                        return -1;
                    }
                    logical_to_physical(&data, target_logical as usize)
                }
                // SEEK_END：Windows CRT 行为：offset 基于物理文件末尾
                2 => {
                    let target = physical_size as i64 + offset as i64;
                    if target < 0 {
                        return -1;
                    }
                    target as usize
                }
                _ => return -1,
            }
        } else {
            let target = match whence {
                0 => offset as i64,
                1 => desc.cursor as i64 + offset as i64,
                2 => meta.size as i64 + offset as i64,
                _ => return -1,
            };
            if target < 0 {
                return -1;
            }
            target as usize
        };

        desc.cursor = new_cursor;
        desc.eof = false;
        0
    }

    /// ftell：返回当前文件光标位置
    /// 返回光标位置（成功）或 -1（失败）
    ///
    /// 注意：Windows CRT 文本模式下 ftell 返回物理字节偏移，而 fseek 使用逻辑偏移。
    /// 为匹配 Clang/MSVC 行为，这里统一返回物理 cursor。
    pub fn ftell(&self, fd: u32, _vm: &mut VitroVM) -> i32 {
        let desc = match self.descriptors.get(&fd) {
            Some(d) => d,
            None => return -1,
        };
        desc.cursor as i32
    }

    /// rewind：将文件光标重置到开头并清除 EOF 标志
    pub fn rewind(&mut self, fd: u32) {
        if let Some(desc) = self.descriptors.get_mut(&fd) {
            desc.cursor = 0;
            desc.eof = false;
            desc.error = false;
        }
    }

    pub fn fflush(&mut self, _fd: u32) -> i32 {
        // VFS 基于内存，无底层缓冲区，fflush 为空操作
        0
    }

    pub fn clearerr(&mut self, fd: u32) {
        if let Some(desc) = self.descriptors.get_mut(&fd) {
            desc.eof = false;
            desc.error = false;
        }
    }

    pub fn remove(&mut self, path: &str, memory: &mut MemoryState) -> i32 {
        if let Some(meta) = self.files.remove(path) {
            free_raw(memory, meta.heap_addr);
            // 同时关闭所有指向该文件的描述符
            let mut to_remove = Vec::new();
            for (fd, desc) in &self.descriptors {
                if desc.file_name == path {
                    to_remove.push(*fd);
                }
            }
            for fd in to_remove {
                self.descriptors.remove(&fd);
            }
            0
        } else {
            -1
        }
    }

    pub fn rename(&mut self, old_path: &str, new_path: &str) -> i32 {
        if let Some(meta) = self.files.remove(old_path) {
            self.files.insert(new_path.to_string(), meta);
            for desc in self.descriptors.values_mut() {
                if desc.file_name == old_path {
                    desc.file_name = new_path.to_string();
                }
            }
            0
        } else {
            -1
        }
    }

    /// 获取文件描述符对应的文件名（用于调试）
    pub fn get_file_name(&self, fd: u32) -> Option<&str> {
        self.descriptors.get(&fd).map(|d| d.file_name.as_str())
    }

    /// 序列化所有预设文件的内容（用于 Session 保存）。
    pub fn snapshot_files(&self, vm: &VitroVM) -> Vec<(String, Vec<u8>)> {
        let mut result = Vec::new();
        for (name, meta) in &self.files {
            let mut buf = vec![0u8; meta.size];
            if vm.read_memory_to(meta.heap_addr, &mut buf) {
                result.push((name.clone(), buf));
            }
        }
        result
    }

    /// 从快照恢复预设文件（用于 Session 加载）。
    pub fn restore_files(&mut self, vm: &mut VitroVM, memory: &mut MemoryState, files: &[(String, Vec<u8>)]) {
        self.files.clear();
        self.descriptors.clear();
        for (name, data) in files {
            self.inject_preset_file(name, data, vm, memory);
        }
    }
}

// ========== 内部辅助函数 ==========

fn align4(size: usize) -> usize {
    (size + 3) & !3
}

/// 从 MemoryState 分配原始内存（类似 host_malloc 但不操作 VM 栈）
fn malloc_raw(memory: &mut MemoryState, aligned_size: usize, mem_size: u32) -> Option<u32> {
    let addr = memory.allocate_raw(aligned_size as u32, mem_size)?;
    // U2#2：复用/新增经 addr 索引 O(1)（同 host_malloc）
    match memory.find_region_mut(addr) {
        Some(r) if r.is_freed => {
            r.is_freed = false;
            r.size = aligned_size as i32;
        }
        Some(_) => {}
        None => {
            memory.alloc_counter += 1;
            memory.push_region(MemoryRegionData {
                addr,
                size: aligned_size as i32,
                name: format!("heap_{}", memory.alloc_counter),
                ty: "int".to_string(),
                is_heap: true,
                is_freed: false,
                alloc_line: 0,
                alloc_by: "vfs".to_string(),
                kind: "heap".to_string(),
            });
        }
    }
    Some(addr)
}

/// 释放原始内存到 MemoryState（类似 host_free）
fn free_raw(memory: &mut MemoryState, addr: u32) {
    memory.free_region(addr);
}

/// 为 VFS 文件扩容（类似 realloc，原地缩容/扩容）
fn realloc_vfs_file(memory: &mut MemoryState, vm: &mut VitroVM, meta: &mut VfsFileMeta, new_cap: usize) -> bool {
    if new_cap <= meta.capacity {
        return true;
    }
    let new_addr = match malloc_raw(memory, new_cap, vm.get_memory_size()) {
        Some(a) => a,
        None => return false,
    };
    if !vm.copy_memory(new_addr, meta.heap_addr, meta.size) {
        free_raw(memory, new_addr);
        return false;
    }
    free_raw(memory, meta.heap_addr);
    meta.heap_addr = new_addr;
    meta.capacity = new_cap;
    // 更新 region 名称和大小
    for r in &mut memory.regions {
        if r.addr == new_addr && !r.is_freed {
            r.size = meta.size as i32;
            break;
        }
    }
    true
}
