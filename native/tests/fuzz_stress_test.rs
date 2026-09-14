#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Fuzz 压力测试（Phase E）
//!
//! 目标：随机内存状态 + 随机标准库调用序列，验证安全检测不泄漏。
//!
//! 测试哲学（不可妥协）：
//! - NO_CODE_DISTORTION：不扭曲 C 语义去迎合 Vitro。
//! - RECORD_DONT_HIDE：任何异常行为（包括未定义行为）必须记录。
//! - FIX_REAL_BUGS：测试失败时，修 Host Func 或 VM 的实现，而不是改测试预期值让它通过。
//! - 不通过删减测试用例来消除差异。
//!
//! 实施方式：
//! - 使用确定性 SplitMix64 RNG，保证可复现。
//! - Fuzz A：malloc/free/realloc 随机序列 + UAF/Double-Free 检测验证。
//! - Fuzz B：字符串操作随机序列（strcpy/strcat/strncpy/memcpy/memmove）+ 缓冲区溢出检测验证。
//! - Fuzz C：IO 操作随机序列（printf/scanf）+ 不崩溃验证。
//! - Fuzz D：混合恶意序列（malloc/free/字符串/IO/rand）+ 整体稳定性验证。
//! - Fuzz E：内存泄漏检测验证（随机分配后不释放，验证泄漏报告）。

use vitro_native::engine::session_ops::append_leak_report;
use vitro_native::session::Session;
use vitro_native::vm::core::VitroVM;
use vitro_native::vm::host_funcs::{
    host_atoi, host_free, host_getchar, host_malloc, host_memcpy, host_memmove, host_memset, host_printf_n,
    host_putchar, host_rand, host_realloc, host_scanf_n, host_srand, host_strcat, host_strcmp, host_strcpy,
    host_strlen, host_strncpy,
};
use vitro_runtime::instruction::SourceLoc;

// ─── 确定性 RNG（SplitMix64）──────────────────────────────────────────────────

struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        min + (self.next() % (max - min) as u64) as i32
    }

    fn range_usize(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next() as usize) % (max - min)
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn choice<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.next() as usize % items.len()])
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fresh_session() -> (VitroVM, Session) {
    (VitroVM::new(), Session::default())
}

fn write_test_string(vm: &mut VitroVM, addr: u32, s: &str) {
    vm.write_cstring(addr, s);
}

#[allow(dead_code)]
fn read_test_string(vm: &VitroVM, addr: u32) -> String {
    let mem = vm.memory_ref();
    let start = addr as usize;
    if start >= mem.len() {
        return String::new();
    }
    let bytes: Vec<u8> = mem[start..].iter().take_while(|&&b| b != 0).copied().collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ─── Fuzz A：Malloc/Free/Realloc 序列 + 安全检测验证 ─────────────────────────

/// 运行一轮 malloc/free/realloc 随机序列 fuzz。
/// 所有内存块状态直接来源于 VM/session.memory.regions，避免跟踪不一致。
/// 返回该轮发现的所有 issue 描述。
fn fuzz_round_malloc_free(rng: &mut FuzzRng, max_ops: usize) -> Vec<String> {
    let mut issues = Vec::new();
    let mut vm = VitroVM::new();
    let mut session = Session::default();

    for op_idx in 0..max_ops {
        // 如果 VM 已 trap（非预期），记录并重建
        if vm.has_error() {
            issues.push(format!(
                "op {}: VM 已处于 trap 状态，可能上一轮未正确清理: {}",
                op_idx,
                vm.get_error()
            ));
            let (nv, ns) = fresh_session();
            vm = nv;
            session = ns;
            continue;
        }

        // 从 session.memory.regions 获取当前活跃块（排除 vfs 分配）
        let active_regions: Vec<usize> = session
            .memory
            .regions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_heap && !r.is_freed && r.alloc_by != "vfs")
            .map(|(i, _)| i)
            .collect();
        // 已释放块必须以 vm.freed_logs 为准，因为 session.memory.regions
        // 保留历史记录，可能与当前实际分配状态不一致（如 realloc 后
        // region 地址重叠、malloc 清理 freed_logs 但未清理旧 region）。
        let freed_addrs: Vec<u32> = vm.get_freed_logs().values().map(|log| log.addr).collect();

        let op = rng.range_i32(0, 12);
        match op {
            // ── malloc ──────────────────────────────────────────────────────
            0..=4 => {
                let size = rng.range_i32(1, 256);
                vm.push(size as u64);
                host_malloc(&mut vm, &mut session.as_vm_context());
                let addr = vm.pop() as u32;
                if addr != 0 {
                    // 写入可辨识的填充模式，便于后续数据完整性检查
                    let pattern = (rng.next() & 0xFF) as u8;
                    let mem = vm.memory_ref_mut();
                    let start = addr as usize;
                    let end = (start + size as usize).min(mem.len());
                    mem[start..end].fill(pattern);
                }
            }
            // ── free active ─────────────────────────────────────────────────
            5..=6 => {
                if let Some(&idx) = rng.choice(&active_regions) {
                    let addr = session.memory.regions[idx].addr;
                    vm.push(addr as u64);
                    host_free(&mut vm, &mut session.as_vm_context());
                    if vm.has_error() {
                        issues.push(format!(
                            "op {}: free 合法指针意外触发 trap: addr=0x{:x} err={}",
                            op_idx,
                            addr,
                            vm.get_error()
                        ));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                    }
                }
            }
            // ── realloc active ──────────────────────────────────────────────
            7 => {
                if let Some(&idx) = rng.choice(&active_regions) {
                    let addr = session.memory.regions[idx].addr;
                    let new_size = rng.range_i32(0, 256);
                    vm.push(new_size as u64);
                    vm.push(addr as u64);
                    host_realloc(&mut vm, &mut session.as_vm_context());
                    let _new_addr = vm.pop() as u32;
                    if vm.has_error() {
                        issues.push(format!(
                            "op {}: realloc 合法指针意外触发 trap: addr=0x{:x} err={}",
                            op_idx,
                            addr,
                            vm.get_error()
                        ));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                    }
                }
            }
            // ── double-free（预期必须触发 E3061）────────────────────────────
            8 => {
                if let Some(&addr) = rng.choice(&freed_addrs) {
                    vm.push(addr as u64);
                    host_free(&mut vm, &mut session.as_vm_context());
                    if !vm.has_error() {
                        issues.push(format!("op {}: Double-Free 未检测: addr=0x{:x}", op_idx, addr));
                    } else if !vm.get_error().contains("E3061") {
                        issues.push(format!("op {}: Double-Free 触发但不含 E3061: {}", op_idx, vm.get_error()));
                    }
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            // ── UAF 写（预期必须触发 E3060）─────────────────────────────────
            9 => {
                if let Some(&addr) = rng.choice(&freed_addrs) {
                    let loc = SourceLoc::default();
                    vm.store_i8(addr, 0x42, &loc);
                    if !vm.has_error() {
                        issues.push(format!("op {}: UAF 写未检测: addr=0x{:x}", op_idx, addr));
                    } else if !vm.get_error().contains("E3060") {
                        issues.push(format!("op {}: UAF 写触发但不含 E3060: {}", op_idx, vm.get_error()));
                    }
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            // ── UAF 读（预期必须触发 E3060）─────────────────────────────────
            10 => {
                if let Some(&addr) = rng.choice(&freed_addrs) {
                    let loc = SourceLoc::default();
                    let _ = vm.load_i8(addr, &loc);
                    if !vm.has_error() {
                        issues.push(format!("op {}: UAF 读未检测: addr=0x{:x}", op_idx, addr));
                    } else if !vm.get_error().contains("E3060") {
                        issues.push(format!("op {}: UAF 读触发但不含 E3060: {}", op_idx, vm.get_error()));
                    }
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            // ── free NULL（必须安全）────────────────────────────────────────
            11 => {
                vm.push(0u64);
                host_free(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!("op {}: free(NULL) 意外触发 trap: {}", op_idx, vm.get_error()));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            _ => {}
        }
    }

    issues
}

#[test]
fn test_fuzz_malloc_free_uaf_double_free() {
    let mut rng = FuzzRng::new(0xDEADBEEF_CAFE0001);
    let mut total_issues = Vec::new();
    let rounds = if cfg!(debug_assertions) { 50 } else { 200 };
    for round in 0..rounds {
        let seed = rng.next();
        let mut round_rng = FuzzRng::new(seed);
        let issues = fuzz_round_malloc_free(&mut round_rng, 100);
        for issue in &issues {
            total_issues.push(format!("[round {} seed=0x{:016x}] {}", round, seed, issue));
        }
    }
    assert!(
        total_issues.is_empty(),
        "Fuzz A (malloc/free/UAF/Double-Free) 发现 {} 个问题:\n{}",
        total_issues.len(),
        total_issues.join("\n")
    );
}

// ─── Fuzz B：字符串操作随机序列 + 缓冲区溢出检测 ─────────────────────────────

fn fuzz_round_string_ops(rng: &mut FuzzRng, max_ops: usize) -> Vec<String> {
    let mut issues = Vec::new();
    let mut vm = VitroVM::new();
    let mut session = Session::default();
    let mut bufs: Vec<(u32, i32)> = Vec::new();

    for op_idx in 0..max_ops {
        if vm.has_error() {
            let (nv, ns) = fresh_session();
            vm = nv;
            session = ns;
            bufs.clear();
            continue;
        }

        let op = rng.range_i32(0, 14);
        match op {
            0..=3 => {
                let size = rng.range_i32(4, 64);
                vm.push(size as u64);
                host_malloc(&mut vm, &mut session.as_vm_context());
                let addr = vm.pop() as u32;
                if addr != 0 {
                    let loc = SourceLoc::default();
                    vm.store_i8(addr, 0, &loc);
                    bufs.push((addr, size));
                }
            }
            4 => {
                if let Some(&(addr, size)) = rng.choice(&bufs) {
                    let src_str = ["hi", "hello", "A", "12345678901234567890"][rng.range_usize(0, 4)];
                    let src_addr = 0x3000u32;
                    write_test_string(&mut vm, src_addr, src_str);
                    vm.push(src_addr as u64);
                    vm.push(addr as u64);
                    host_strcpy(&mut vm, &mut session.as_vm_context());
                    let src_len = src_str.len();
                    if src_len + 1 > size as usize {
                        if !vm.has_error() {
                            issues.push(format!(
                                "op {}: strcpy 溢出未检测: dst_size={} src_len={}",
                                op_idx, size, src_len
                            ));
                        } else if !vm.get_error().contains("E3070") {
                            issues.push(format!("op {}: strcpy 溢出触发但不含 E3070: {}", op_idx, vm.get_error()));
                        }
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                        bufs.clear();
                    }
                }
            }
            5 => {
                if let Some(&(addr, _size)) = rng.choice(&bufs) {
                    let src_str = [" world", "!!", "12345678901234567890"][rng.range_usize(0, 3)];
                    let src_addr = 0x3000u32;
                    write_test_string(&mut vm, src_addr, src_str);
                    vm.push(src_addr as u64);
                    vm.push(addr as u64);
                    host_strcat(&mut vm, &mut session.as_vm_context());
                    if vm.has_error() && !vm.get_error().contains("E3070") {
                        issues.push(format!("op {}: strcat 溢出触发但不含 E3070: {}", op_idx, vm.get_error()));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                        bufs.clear();
                    }
                }
            }
            6 => {
                let src_addr = 0x3000u32;
                write_test_string(&mut vm, src_addr, "test");
                vm.push(src_addr as u64);
                vm.push(0u64);
                host_strcpy(&mut vm, &mut session.as_vm_context());
                if !vm.has_error() {
                    issues.push(format!("op {}: strcpy(NULL, src) 未触发 trap", op_idx));
                }
                let (nv, ns) = fresh_session();
                vm = nv;
                session = ns;
                bufs.clear();
            }
            7 => {
                let src_addr = 0x3000u32;
                write_test_string(&mut vm, src_addr, "test");
                vm.push(src_addr as u64);
                vm.push(0u64);
                host_strcat(&mut vm, &mut session.as_vm_context());
                if !vm.has_error() {
                    issues.push(format!("op {}: strcat(NULL, src) 未触发 trap", op_idx));
                }
                let (nv, ns) = fresh_session();
                vm = nv;
                session = ns;
                bufs.clear();
            }
            8 => {
                if let Some(&(addr, _size)) = rng.choice(&bufs) {
                    let src_str = ["hi", "hello world"][rng.range_usize(0, 2)];
                    let n = rng.range_i32(0, 32);
                    let src_addr = 0x3000u32;
                    write_test_string(&mut vm, src_addr, src_str);
                    vm.push(n as u64);
                    vm.push(src_addr as u64);
                    vm.push(addr as u64);
                    host_strncpy(&mut vm, &mut session.as_vm_context());
                    if vm.has_error() {
                        issues.push(format!("op {}: strncpy 意外触发 trap: n={} err={}", op_idx, n, vm.get_error()));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                        bufs.clear();
                    }
                }
            }
            9 => {
                if let Some(&(addr, _size)) = rng.choice(&bufs) {
                    let n = rng.range_i32(0, 64);
                    let src_addr = 0x3000u32;
                    let mem = vm.memory_ref_mut();
                    mem[src_addr as usize..src_addr as usize + 64].fill(0xAB);
                    vm.push(n as u64);
                    vm.push(src_addr as u64);
                    vm.push(addr as u64);
                    host_memcpy(&mut vm, &mut session.as_vm_context());
                    if vm.has_error() {
                        issues.push(format!("op {}: memcpy 意外触发 trap: n={} err={}", op_idx, n, vm.get_error()));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                        bufs.clear();
                    }
                }
            }
            10 => {
                let buf_addr = 0x2000u32;
                let init = "ABCDEFGHIJ";
                write_test_string(&mut vm, buf_addr, init);
                let src_off = rng.range_i32(0, 5);
                let dst_off = rng.range_i32(0, 5);
                let n = rng.range_i32(0, 10);
                vm.push(n as u64);
                vm.push((buf_addr + src_off as u32) as u64);
                vm.push((buf_addr + dst_off as u32) as u64);
                host_memmove(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!(
                        "op {}: memmove 意外触发 trap: src_off={} dst_off={} n={} err={}",
                        op_idx,
                        src_off,
                        dst_off,
                        n,
                        vm.get_error()
                    ));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    bufs.clear();
                }
            }
            11 => {
                if let Some(&(addr, size)) = rng.choice(&bufs) {
                    let n = rng.range_i32(0, size + 10);
                    vm.push(n as u64);
                    vm.push(rng.range_i32(0, 256) as u64);
                    vm.push(addr as u64);
                    host_memset(&mut vm, &mut session.as_vm_context());
                    if vm.has_error() {
                        issues.push(format!(
                            "op {}: memset 意外触发 trap: n={} size={} err={}",
                            op_idx,
                            n,
                            size,
                            vm.get_error()
                        ));
                        let (nv, ns) = fresh_session();
                        vm = nv;
                        session = ns;
                        bufs.clear();
                    }
                }
            }
            12 => {
                let addr = if rng.bool() && !bufs.is_empty() {
                    bufs[rng.range_usize(0, bufs.len())].0
                } else {
                    0x3000u32
                };
                write_test_string(&mut vm, addr, ["abc", "", "x"][rng.range_usize(0, 3)]);
                vm.push(addr as u64);
                host_strlen(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!("op {}: strlen 意外触发 trap: err={}", op_idx, vm.get_error()));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    bufs.clear();
                }
            }
            13 => {
                let a = 0x2000u32;
                let b = 0x2800u32;
                let s1 = ["abc", "def", "", "same"][rng.range_usize(0, 4)];
                let s2 = ["abc", "def", "", "same"][rng.range_usize(0, 4)];
                write_test_string(&mut vm, a, s1);
                write_test_string(&mut vm, b, s2);
                vm.push(b as u64);
                vm.push(a as u64);
                host_strcmp(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!("op {}: strcmp 意外触发 trap: err={}", op_idx, vm.get_error()));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    bufs.clear();
                }
            }
            _ => {}
        }
    }

    issues
}

#[test]
fn test_fuzz_string_ops() {
    let mut rng = FuzzRng::new(0xCAFEBABE_DEAD0002);
    let mut total_issues = Vec::new();
    let rounds = if cfg!(debug_assertions) { 50 } else { 200 };
    for round in 0..rounds {
        let seed = rng.next();
        let mut round_rng = FuzzRng::new(seed);
        let issues = fuzz_round_string_ops(&mut round_rng, 80);
        for issue in &issues {
            total_issues.push(format!("[round {} seed=0x{:016x}] {}", round, seed, issue));
        }
    }
    assert!(
        total_issues.is_empty(),
        "Fuzz B (字符串操作) 发现 {} 个问题:\n{}",
        total_issues.len(),
        total_issues.join("\n")
    );
}

// ─── Fuzz C：IO 操作随机序列 + 不崩溃验证 ────────────────────────────────────

fn fuzz_round_io(rng: &mut FuzzRng, max_ops: usize) -> Vec<String> {
    let mut issues = Vec::new();
    let mut vm = VitroVM::new();
    let mut session = Session::default();

    // 预置一些输入行
    session.runtime.input_lines = vec![
        "42".to_string(),
        "hello world".to_string(),
        "3.14".to_string(),
        "123 456".to_string(),
    ];

    let fmt_pool = ["%d", "%s", "%c", "%f", "%%", "%d %d", "hello", "%d\n", "%s %d"];

    for op_idx in 0..max_ops {
        if vm.has_error() {
            let (nv, ns) = fresh_session();
            vm = nv;
            session = ns;
            session.runtime.input_lines = vec![
                "42".to_string(),
                "hello world".to_string(),
                "3.14".to_string(),
                "123 456".to_string(),
            ];
            continue;
        }

        let op = rng.range_i32(0, 8);
        match op {
            // ── printf 随机格式 ─────────────────────────────────────────────
            0..=3 => {
                let fmt = fmt_pool[rng.range_usize(0, fmt_pool.len())];
                let fmt_addr = 0x2000u32;
                write_test_string(&mut vm, fmt_addr, fmt);
                let arg_count = fmt.matches('%').count() - fmt.matches("%%").count();
                for _ in 0..arg_count {
                    vm.push(rng.next());
                }
                vm.push(fmt_addr as u64);
                host_printf_n(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!(
                        "op {}: printf 意外触发 trap: fmt={:?} err={}",
                        op_idx,
                        fmt,
                        vm.get_error()
                    ));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    session.runtime.input_lines = vec![
                        "42".to_string(),
                        "hello world".to_string(),
                        "3.14".to_string(),
                        "123 456".to_string(),
                    ];
                }
            }
            // ── scanf 随机格式 ─────────────────────────────────────────────
            4..=5 => {
                let fmt = ["%d", "%d %d", "%f", "%c"][rng.range_usize(0, 4)];
                let fmt_addr = 0x2000u32;
                let dst_addr = 0x3000u32;
                write_test_string(&mut vm, fmt_addr, fmt);
                let arg_count = fmt.matches('%').count() - fmt.matches("%%").count();
                for _ in 0..arg_count {
                    vm.push((dst_addr + rng.range_i32(0, 64) as u32) as u64);
                }
                vm.push(fmt_addr as u64);
                host_scanf_n(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!(
                        "op {}: scanf 意外触发 trap: fmt={:?} err={}",
                        op_idx,
                        fmt,
                        vm.get_error()
                    ));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    session.runtime.input_lines = vec![
                        "42".to_string(),
                        "hello world".to_string(),
                        "3.14".to_string(),
                        "123 456".to_string(),
                    ];
                }
            }
            // ── getchar ─────────────────────────────────────────────────────
            6 => {
                host_getchar(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!("op {}: getchar 意外触发 trap: err={}", op_idx, vm.get_error()));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    session.runtime.input_lines = vec![
                        "42".to_string(),
                        "hello world".to_string(),
                        "3.14".to_string(),
                        "123 456".to_string(),
                    ];
                }
            }
            // ── putchar ─────────────────────────────────────────────────────
            7 => {
                vm.push(rng.range_i32(32, 127) as u64);
                host_putchar(&mut vm, &mut session.as_vm_context());
                if vm.has_error() {
                    issues.push(format!("op {}: putchar 意外触发 trap: err={}", op_idx, vm.get_error()));
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                    session.runtime.input_lines = vec![
                        "42".to_string(),
                        "hello world".to_string(),
                        "3.14".to_string(),
                        "123 456".to_string(),
                    ];
                }
            }
            _ => {}
        }
    }

    issues
}

#[test]
fn test_fuzz_io_ops() {
    let mut rng = FuzzRng::new(0xCAFEBABE_DEAD0003);
    let mut total_issues = Vec::new();
    let rounds = if cfg!(debug_assertions) { 50 } else { 200 };
    for round in 0..rounds {
        let seed = rng.next();
        let mut round_rng = FuzzRng::new(seed);
        let issues = fuzz_round_io(&mut round_rng, 60);
        for issue in &issues {
            total_issues.push(format!("[round {} seed=0x{:016x}] {}", round, seed, issue));
        }
    }
    assert!(
        total_issues.is_empty(),
        "Fuzz C (IO 操作) 发现 {} 个问题:\n{}",
        total_issues.len(),
        total_issues.join("\n")
    );
}

// ─── Fuzz D：混合恶意序列 + 整体稳定性验证 ───────────────────────────────────

fn fuzz_round_mixed(rng: &mut FuzzRng, max_ops: usize) -> Vec<String> {
    let mut issues = Vec::new();
    let mut vm = VitroVM::new();
    let mut session = Session::default();

    for op_idx in 0..max_ops {
        if vm.has_error() {
            let (nv, ns) = fresh_session();
            vm = nv;
            session = ns;
            continue;
        }

        let op = rng.range_i32(0, 16);
        match op {
            0..=2 => {
                vm.push(rng.range_i32(1, 128) as u64);
                host_malloc(&mut vm, &mut session.as_vm_context());
                let _ = vm.pop();
            }
            3 => {
                let active: Vec<u32> = session
                    .memory
                    .regions
                    .iter()
                    .filter(|r| r.is_heap && !r.is_freed && r.alloc_by != "vfs")
                    .map(|r| r.addr)
                    .collect();
                if let Some(&addr) = rng.choice(&active) {
                    vm.push(addr as u64);
                    host_free(&mut vm, &mut session.as_vm_context());
                }
            }
            4 => {
                let fmt_addr = 0x2000u32;
                write_test_string(&mut vm, fmt_addr, "%d");
                vm.push(rng.next());
                vm.push(fmt_addr as u64);
                host_printf_n(&mut vm, &mut session.as_vm_context());
            }
            5 => {
                vm.push(rng.range_i32(1, 65535) as u64);
                host_srand(&mut vm, &mut session.as_vm_context());
            }
            6 => {
                host_rand(&mut vm, &mut session.as_vm_context());
                let _ = vm.pop();
            }
            7 => {
                let addr = 0x2000u32;
                write_test_string(&mut vm, addr, "test");
                vm.push(addr as u64);
                host_strlen(&mut vm, &mut session.as_vm_context());
                let _ = vm.pop();
            }
            8 => {
                let addr = 0x2000u32;
                write_test_string(&mut vm, addr, "123");
                vm.push(addr as u64);
                host_atoi(&mut vm, &mut session.as_vm_context());
                let _ = vm.pop();
            }
            9 => {
                let a = 0x2000u32;
                let b = 0x2100u32;
                write_test_string(&mut vm, a, "abc");
                write_test_string(&mut vm, b, "def");
                vm.push(b as u64);
                vm.push(a as u64);
                host_strcmp(&mut vm, &mut session.as_vm_context());
                let _ = vm.pop();
            }
            10 => {
                let ptr = 0x2000u32;
                let loc = SourceLoc::default();
                vm.store_i8(ptr, rng.range_i32(0, 256), &loc);
            }
            11 => {
                let ptr = 0x2000u32 + rng.range_i32(0, 256) as u32;
                let loc = SourceLoc::default();
                let _ = vm.load_i8(ptr, &loc);
            }
            12 => {
                let freed: Vec<u32> = vm.get_freed_logs().values().map(|log| log.addr).collect();
                if let Some(&addr) = rng.choice(&freed) {
                    let loc = SourceLoc::default();
                    vm.store_i8(addr, 0x42, &loc);
                    if !vm.has_error() {
                        issues.push(format!("op {}: 混合序列中 UAF 写未检测: addr=0x{:x}", op_idx, addr));
                    }
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            13 => {
                let freed: Vec<u32> = vm.get_freed_logs().values().map(|log| log.addr).collect();
                if let Some(&addr) = rng.choice(&freed) {
                    vm.push(addr as u64);
                    host_free(&mut vm, &mut session.as_vm_context());
                    if !vm.has_error() {
                        issues.push(format!("op {}: 混合序列中 Double-Free 未检测: addr=0x{:x}", op_idx, addr));
                    }
                    let (nv, ns) = fresh_session();
                    vm = nv;
                    session = ns;
                }
            }
            14 => {
                vm.push(0u64);
                host_putchar(&mut vm, &mut session.as_vm_context());
            }
            15 => {
                let addr = 0x2000u32;
                write_test_string(&mut vm, addr, "%d");
                let dst = 0x3000u32;
                vm.push(dst as u64);
                vm.push(addr as u64);
                host_scanf_n(&mut vm, &mut session.as_vm_context());
            }
            _ => {}
        }
    }

    issues
}

#[test]
fn test_fuzz_mixed_malicious() {
    let mut rng = FuzzRng::new(0x1234BEEF_00040004);
    let mut total_issues = Vec::new();
    let rounds = if cfg!(debug_assertions) { 50 } else { 200 };
    for round in 0..rounds {
        let seed = rng.next();
        let mut round_rng = FuzzRng::new(seed);
        let issues = fuzz_round_mixed(&mut round_rng, 100);
        for issue in &issues {
            total_issues.push(format!("[round {} seed=0x{:016x}] {}", round, seed, issue));
        }
    }
    assert!(
        total_issues.is_empty(),
        "Fuzz D (混合恶意序列) 发现 {} 个问题:\n{}",
        total_issues.len(),
        total_issues.join("\n")
    );
}

// ─── Fuzz E：内存泄漏检测验证 ────────────────────────────────────────────────

fn fuzz_round_leak_detection(rng: &mut FuzzRng) -> Vec<String> {
    let mut issues = Vec::new();
    let mut vm = VitroVM::new();
    let mut session = Session::default();

    // 随机分配若干块，部分释放，部分泄漏
    let total_allocs = rng.range_i32(3, 12);
    let mut alloc_addrs: Vec<u32> = Vec::new();
    for _ in 0..total_allocs {
        let size = rng.range_i32(8, 64);
        vm.push(size as u64);
        host_malloc(&mut vm, &mut session.as_vm_context());
        let addr = vm.pop() as u32;
        if addr != 0 {
            alloc_addrs.push(addr);
        }
    }

    // 随机释放一部分
    let free_count = rng.range_i32(0, alloc_addrs.len() as i32);
    for _ in 0..free_count {
        if alloc_addrs.is_empty() {
            break;
        }
        let idx = rng.range_usize(0, alloc_addrs.len());
        let addr = alloc_addrs.swap_remove(idx);
        vm.push(addr as u64);
        host_free(&mut vm, &mut session.as_vm_context());
        if vm.has_error() {
            issues.push(format!(
                "leak fuzz: free 意外触发 trap: addr=0x{:x} err={}",
                addr,
                vm.get_error()
            ));
            return issues;
        }
    }

    // 获取泄漏报告前的输出行数
    let lines_before = session.runtime.note_chunks().len();
    append_leak_report(&mut session);
    let lines_after = session.runtime.note_chunks().len();

    let leaked = alloc_addrs.len();
    if leaked > 0 {
        if lines_after == lines_before {
            issues.push(format!("leak fuzz: {} 个泄漏块但 append_leak_report 未输出任何内容", leaked));
        } else {
            // 检查所有新增行中是否包含泄漏数量信息
            let new_lines: Vec<&str> = session.runtime.note_chunks()[lines_before..lines_after].to_vec();
            let report_text = new_lines.join("\n");
            let count_in_report = report_text.matches("分配了").count();
            if count_in_report < leaked {
                issues.push(format!(
                    "leak fuzz: {} 个泄漏块但报告仅提及 {} 个: {}",
                    leaked, count_in_report, report_text
                ));
            }
        }
    } else {
        // 无泄漏时，不应输出泄漏报告（或应输出"无泄漏"）
        // 当前实现中 append_leak_report 在无泄漏时可能不输出，这是可接受的
    }

    issues
}

#[test]
fn test_fuzz_leak_detection() {
    let mut rng = FuzzRng::new(0x5678DEAD_00050005);
    let mut total_issues = Vec::new();
    let rounds = if cfg!(debug_assertions) { 50 } else { 200 };
    for round in 0..rounds {
        let seed = rng.next();
        let mut round_rng = FuzzRng::new(seed);
        let issues = fuzz_round_leak_detection(&mut round_rng);
        for issue in &issues {
            total_issues.push(format!("[round {} seed=0x{:016x}] {}", round, seed, issue));
        }
    }
    assert!(
        total_issues.is_empty(),
        "Fuzz E (泄漏检测) 发现 {} 个问题:\n{}",
        total_issues.len(),
        total_issues.join("\n")
    );
}
