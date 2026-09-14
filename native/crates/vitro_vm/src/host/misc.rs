use super::*;
use crate::VmContext;

pub fn host_rand(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let seed = session.runtime.rand_seed;
    let next = seed.wrapping_mul(1103515245).wrapping_add(12345);
    session.runtime.rand_seed = next;
    vm.push((next & 0x7fff) as u64);
}

pub fn host_srand(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let seed = vm.pop();
    session.runtime.rand_seed = seed as u32;
}

pub fn host_exit(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let code = vm.pop() as i32;
    vm.set_finished(code);
}

pub fn host_isdigit(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= '0' as i32 && c <= '9' as i32 { 1 } else { 0 });
}

pub fn host_isalpha(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(
        if (c >= 'a' as i32 && c <= 'z' as i32) || (c >= 'A' as i32 && c <= 'Z' as i32) {
            1
        } else {
            0
        },
    );
}

pub fn host_islower(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= 'a' as i32 && c <= 'z' as i32 { 1 } else { 0 });
}

pub fn host_isupper(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= 'A' as i32 && c <= 'Z' as i32 { 1 } else { 0 });
}

pub fn host_tolower(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= 'A' as i32 && c <= 'Z' as i32 {
        c + ('a' as i32 - 'A' as i32)
    } else {
        c
    } as u64);
}

pub fn host_toupper(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= 'a' as i32 && c <= 'z' as i32 {
        c + ('A' as i32 - 'a' as i32)
    } else {
        c
    } as u64);
}

pub fn host_isspace(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(
        if c == ' ' as i32
            || c == '\t' as i32
            || c == '\n' as i32
            || c == '\r' as i32
            || c == '\x0C' as i32
            || c == '\x0B' as i32
        {
            1
        } else {
            0
        },
    );
}

pub fn host_isalnum(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    let alpha = (c >= 'a' as i32 && c <= 'z' as i32) || (c >= 'A' as i32 && c <= 'Z' as i32);
    let digit = c >= '0' as i32 && c <= '9' as i32;
    vm.push(if alpha || digit { 1 } else { 0 });
}

pub fn host_isprint(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c >= ' ' as i32 && c <= '~' as i32 { 1 } else { 0 });
}

pub fn host_iscntrl(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if (0..=31).contains(&c) || c == 127 { 1 } else { 0 });
}

pub fn host_isxdigit(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    let digit = c >= '0' as i32 && c <= '9' as i32;
    let lower = c >= 'a' as i32 && c <= 'f' as i32;
    let upper = c >= 'A' as i32 && c <= 'F' as i32;
    vm.push(if digit || lower || upper { 1 } else { 0 });
}

pub fn host_isgraph(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c > ' ' as i32 && c <= '~' as i32 { 1 } else { 0 });
}

pub fn host_ispunct(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    let is_print = c >= ' ' as i32 && c <= '~' as i32;
    let is_alnum = (c >= 'a' as i32 && c <= 'z' as i32)
        || (c >= 'A' as i32 && c <= 'Z' as i32)
        || (c >= '0' as i32 && c <= '9' as i32);
    vm.push(if is_print && !is_alnum && c != ' ' as i32 { 1 } else { 0 });
}

pub fn host_isblank(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let c = vm.pop() as i32;
    vm.push(if c == ' ' as i32 || c == '\t' as i32 { 1 } else { 0 });
}

pub fn host_atoi(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let s = read_cstring(vm, addr);
    let trimmed = s.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut sign = 1i32;
    if let Some(&c) = chars.peek() {
        if c == '-' {
            sign = -1;
            chars.next();
        } else if c == '+' {
            chars.next();
        }
    }
    let mut val: i32 = 0;
    for c in chars {
        if c.is_ascii_digit() {
            val = val.wrapping_mul(10).wrapping_add(c as i32 - '0' as i32);
        } else {
            break;
        }
    }
    vm.push((sign.wrapping_mul(val)) as u64);
}

const MAX_QSORT_DEPTH: i32 = 8;

pub fn host_qsort(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let base = vm.pop() as u32;
    let nmemb = vm.pop() as usize;
    let size = vm.pop() as usize;
    let compar = vm.pop() as u32;

    if vm.qsort_depth() >= MAX_QSORT_DEPTH {
        session.runtime.push_note("[qsort] 嵌套深度超过限制，防止栈溢出");
        return;
    }

    if nmemb <= 1 || size == 0 || base == 0 {
        return;
    }

    let mem_size = vm.get_memory_slice().len();
    // usize 乘法/加法溢出防护：恶意 nmemb/size（如 2^32 * 2^32 wrap 为 0）会绕过
    // 边界检查并触发海量分配（OOM abort，debug 构建直接 panic）。
    if nmemb
        .checked_mul(size)
        .and_then(|t| (base as usize).checked_add(t))
        .is_none_or(|t| t > mem_size)
    {
        return;
    }

    vm.set_qsort_depth(vm.qsort_depth() + 1);

    let mut indices: Vec<usize> = (0..nmemb).collect();
    const MAX_COMPARE_STEPS: i32 = 1000;

    if compar == 0 {
        // No comparison function, use default byte comparison
        let mem = vm.get_memory_slice();
        indices.sort_by(|&i, &j| {
            let a_start = (base as usize) + i * size;
            let b_start = (base as usize) + j * size;
            let a = &mem[a_start..a_start + size];
            let b = &mem[b_start..b_start + size];
            a.cmp(b)
        });
    } else {
        indices.sort_by(|&i, &j| {
            let addr_a = (base as i32) + (i as i32) * (size as i32);
            let addr_b = (base as i32) + (j as i32) * (size as i32);
            let result = vm.call_user_function(session, compar, &[addr_a, addr_b], MAX_COMPARE_STEPS);
            match result {
                Some(v) => v.cmp(&0),
                None => std::cmp::Ordering::Equal,
            }
        });
    }

    // Reorder memory according to sorted indices.
    // Use a temporary buffer to avoid overwriting data during in-place reordering,
    // then write back in chunks via `write_memory` instead of byte-by-byte store_i8.
    let mut temp: Vec<u8> = vec![0; nmemb * size];
    {
        let mem = vm.get_memory_slice();
        for (new_pos, &old_idx) in indices.iter().enumerate() {
            let src_start = (base as usize) + old_idx * size;
            let dst_start = new_pos * size;
            temp[dst_start..dst_start + size].copy_from_slice(&mem[src_start..src_start + size]);
        }
    }
    for i in 0..nmemb {
        let src_start = i * size;
        let dst_start = (base as usize) + i * size;
        if !vm.write_memory(dst_start as u32, &temp[src_start..src_start + size]) {
            vm.trap("qsort: write back out of bounds", &SourceLoc::default());
            vm.set_qsort_depth(vm.qsort_depth() - 1);
            return;
        }
    }
    vm.set_qsort_depth(vm.qsort_depth() - 1);
}

// ========== VFS File I/O Host Functions ==========

const MAX_BSEARCH_DEPTH: i32 = 8;

pub fn host_bsearch(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let key = vm.pop() as u32;
    let base = vm.pop() as u32;
    let nmemb = vm.pop() as usize;
    let size = vm.pop() as usize;
    let compar = vm.pop() as u32;

    if vm.qsort_depth() >= MAX_BSEARCH_DEPTH {
        session.runtime.push_note("[bsearch] 嵌套深度超过限制，防止栈溢出");
        vm.push(0);
        return;
    }

    if nmemb == 0 || size == 0 || base == 0 || key == 0 {
        vm.push(0);
        return;
    }

    let mem_size = vm.get_memory_slice().len();
    // 溢出防护同 host_qsort：checked_mul/checked_add，恶意参数按"未找到"处理。
    if nmemb
        .checked_mul(size)
        .and_then(|t| (base as usize).checked_add(t))
        .is_none_or(|t| t > mem_size)
    {
        vm.push(0);
        return;
    }

    // U2#13：key 侧边界——base 侧已检查，key 指向 [key, key+size) 越界时
    //（学生常见野指针）此前直接切片 panic；按"未找到"返回 NULL。
    // 检查必须在 set_qsort_depth(+1) 之前：qsort_depth 与 qsort 共用（门限 8），
    // 早退不减深度会让计数器只增不减——8 次野指针 key 后 bsearch 永久 NULL、
    // qsort 静默 no-op（审阅 P1-a，静默错值）。
    if (key as u64) + size as u64 > mem_size as u64 {
        vm.push(0);
        return;
    }

    vm.set_qsort_depth(vm.qsort_depth() + 1);

    const MAX_COMPARE_STEPS: i32 = 1000;
    let mut lo: usize = 0;
    let mut hi: usize = nmemb;
    let mut result_ptr: u32 = 0;

    if compar == 0 {
        let mem = vm.get_memory_slice();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let a_start = key as usize;
            let b_start = (base as usize) + mid * size;
            let a = &mem[a_start..a_start + size];
            let b = &mem[b_start..b_start + size];
            match a.cmp(b) {
                std::cmp::Ordering::Less => {
                    lo = mid + 1;
                }
                std::cmp::Ordering::Greater => {
                    hi = mid;
                }
                std::cmp::Ordering::Equal => {
                    result_ptr = b_start as u32;
                    break;
                }
            }
        }
    } else {
        while lo < hi {
            let mid = (lo + hi) / 2;
            let addr_key = key as i32;
            let addr_mid = (base as i32) + (mid as i32) * (size as i32);
            let cmp_result = vm.call_user_function(session, compar, &[addr_key, addr_mid], MAX_COMPARE_STEPS);
            match cmp_result {
                Some(v) if v < 0 => {
                    hi = mid;
                }
                Some(v) if v > 0 => {
                    lo = mid + 1;
                }
                Some(_) => {
                    result_ptr = addr_mid as u32;
                    break;
                }
                None => {
                    break;
                }
            }
        }
    }

    vm.set_qsort_depth(vm.qsort_depth() - 1);
    vm.push(result_ptr as u64);
}

pub fn host_atof(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let s = read_cstring(vm, addr);
    let val: f64 = s.trim().parse().unwrap_or(0.0);
    vm.push(val.to_bits());
}

pub fn host_atol(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let addr = vm.pop() as u32;
    let s = read_cstring(vm, addr);
    let trimmed = s.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut sign = 1i64;
    if let Some(&c) = chars.peek() {
        if c == '-' {
            sign = -1;
            chars.next();
        } else if c == '+' {
            chars.next();
        }
    }
    let mut val: i64 = 0;
    for c in chars {
        if c.is_ascii_digit() {
            val = val.wrapping_mul(10).wrapping_add(c as i64 - '0' as i64);
        } else {
            break;
        }
    }
    vm.push((sign.wrapping_mul(val)) as u64);
}

// ========== Math extensions ==========

pub fn host_abort(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    session.runtime.push_note("[abort] 程序异常终止 (SIGABRT)");
    vm.set_finished(134);
}

pub fn host_strtol(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let str_addr = vm.pop() as u32;
    let endptr_addr = vm.pop() as u32;
    let base = vm.pop() as i32;
    let s = read_cstring(vm, str_addr);
    let bytes = s.as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let mut sign = 1i64;
    if pos < bytes.len() {
        if bytes[pos] == b'-' {
            sign = -1;
            pos += 1;
        } else if bytes[pos] == b'+' {
            pos += 1;
        }
    }
    let start_pos = pos;
    let mut val: i64 = 0;
    let effective_base = if base == 0 { 10 } else { base };
    while pos < bytes.len() {
        let c = bytes[pos];
        let digit = if c.is_ascii_digit() {
            (c - b'0') as i64
        } else if c.is_ascii_lowercase() {
            (c - b'a' + 10) as i64
        } else if c.is_ascii_uppercase() {
            (c - b'A' + 10) as i64
        } else {
            break;
        };
        if digit >= effective_base as i64 {
            break;
        }
        val = val.wrapping_mul(effective_base as i64).wrapping_add(digit);
        pos += 1;
    }
    if pos == start_pos {
        set_errno(vm, 1); // EINVAL
    }
    if endptr_addr != 0 {
        let stop_addr = str_addr + pos as u32;
        vm.write_memory(endptr_addr, &stop_addr.to_le_bytes());
    }
    vm.push((sign.wrapping_mul(val)) as u64);
}

pub fn host_strtod(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let str_addr = vm.pop() as u32;
    let endptr_addr = vm.pop() as u32;
    let s = read_cstring(vm, str_addr);
    let bytes = s.as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let start = pos;
    if pos < bytes.len() && (bytes[pos] == b'+' || bytes[pos] == b'-') {
        pos += 1;
    }
    let mut has_dot = false;
    let mut has_exp = false;
    while pos < bytes.len() {
        let c = bytes[pos];
        if c.is_ascii_digit() {
            pos += 1;
        } else if c == b'.' && !has_dot {
            has_dot = true;
            pos += 1;
        } else if (c == b'e' || c == b'E') && !has_exp {
            has_exp = true;
            pos += 1;
            if pos < bytes.len() && (bytes[pos] == b'+' || bytes[pos] == b'-') {
                pos += 1;
            }
        } else {
            break;
        }
    }
    let num_str = std::str::from_utf8(&bytes[start..pos]).unwrap_or("");
    let val: f64 = num_str.parse().unwrap_or(0.0);
    if pos == start {
        set_errno(vm, 1); // EINVAL
    }
    if endptr_addr != 0 {
        let stop_addr = str_addr + pos as u32;
        vm.write_memory(endptr_addr, &stop_addr.to_le_bytes());
    }
    vm.push(val.to_bits());
}

pub fn host_strerror(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let errnum = vm.pop() as i32;
    let msg = match errnum {
        1 => "Invalid argument\0",
        2 => "Numerical result out of range\0",
        3 => "Domain error\0",
        4 => "No such file or directory\0",
        5 => "Permission denied\0",
        _ => "Unknown error\0",
    };
    let size = msg.len() as i32;
    let aligned = ((size as u32) + 3) & !3;
    let addr = match session.memory.allocate_raw(aligned, vm.get_memory_size()) {
        Some(a) => a,
        None => {
            vm.push(0);
            return;
        }
    };
    // U2#13：与 host_strdup 逐字同型的分配三步契约（审阅 P1-b：无条件
    // push_region 只做了第一步——复用路径（free_list 取回已用地址）会造同址
    // 双条目 + 索引失配；stale freed_logs 未清会让下方 write_memory 被 UAF
    // 检查拦截，返回全零缓冲）。
    let new_end = addr.saturating_add(aligned);
    vm.freed_logs_remove_overlapping(addr, new_end);
    match session.memory.find_region_mut(addr) {
        Some(r) if r.is_freed => {
            r.is_freed = false;
            r.size = size;
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "strerror".to_string();
        }
        Some(r) => {
            r.alloc_line = vm.get_current_line();
            r.alloc_by = "strerror".to_string();
        }
        None => {
            session.memory.alloc_counter += 1;
            session.memory.push_region(vitro_runtime::MemoryRegionData {
                addr,
                size,
                name: format!("heap_{}", session.memory.alloc_counter),
                ty: "char".to_string(),
                is_heap: true,
                is_freed: false,
                alloc_line: vm.get_current_line(),
                alloc_by: "strerror".to_string(),
                kind: "heap".to_string(),
            });
        }
    }
    if !vm.write_memory(addr, msg.as_bytes()) {
        // 与 strdup 同口径：缓冲写入失败（理论上不可达——地址刚从分配器取出）
        // 不得静默返回半初始化缓冲
        vm.trap("strerror: 输出缓冲写入失败", &SourceLoc::default());
        return;
    }
    vm.push(addr as u64);
}

pub fn host_time(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let _tloc = vm.pop() as u32;
    // 判分确定性最小形态（Phase 1）：deterministic 下时间固定为 0，
    // 否则同一份代码每次判分得到不同的 time() 值（完整 step 派生伪时钟留 Phase 3）。
    let secs = if session.runtime.deterministic {
        0i64
    } else {
        (current_time_millis() / 1000) as i64
    };
    vm.push(secs as u64);
}

pub fn host_clock(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 返回一个近似值：使用当前时间戳的微秒数作为单调时钟
    // CLOCKS_PER_SEC 通常定义为 1_000_000，返回微秒级精度
    // 判分确定性模式下固定为 0（同 host_time）。
    let clocks = if session.runtime.deterministic {
        0i64
    } else {
        (current_time_millis() * 1000) as i64
    };
    vm.push(clocks as u64);
}

pub fn host_vitro_assert_fail(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    session.runtime.push_note("🚫 断言失败 (assertion failed)");
    vm.set_finished(1);
}

pub fn host_set_array_guard(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let base_addr = vm.pop() as u32;
    vm.pending_array_construction = Some(ArrayConstructionGuard {
        base_addr,
        frame_depth: vm.call_stack_len(),
    });
}

pub fn host_clear_array_guard(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    vm.pending_array_construction = None;
}

// ============================================================================
// stdarg.h 变参支持
// ============================================================================

pub fn host_va_start(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let ap_addr = vm.pop() as i32;
    let last_addr = vm.pop() as i32;
    let last_size = vm.pop() as i32;
    let loc = SourceLoc::default();
    // ap 指向最后一个命名参数之后的位置（即变参区域起始）
    let va_base = last_addr + last_size;
    vm.store_i32(ap_addr as u32, va_base, &loc);
}

pub fn host_va_arg(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let ap_addr = vm.pop() as i32;
    let size = vm.pop() as i32;
    let loc = SourceLoc::default();
    let current_ap = vm.load_i32(ap_addr as u32, &loc);
    let new_ap = current_ap + size;
    vm.store_i32(ap_addr as u32, new_ap, &loc);
    // __vitro_va_arg 返回 void*，指向当前变参参数的地址
    vm.push(current_ap as u64);
}

pub fn host_va_end(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let _ap_addr = vm.pop() as i32;
    // 无操作；保留函数以匹配标准接口
}

/// E3：C23 `unreachable()`（<stddef.h>）——执行到即教学 trap。
/// 死代码中的调用不执行、不影响输出；典型用法是标记"逻辑上不可达"的分支。
pub fn host_unreachable(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    vm.trap(
        "执行了标注为不可达的代码（unreachable）。请检查分支条件是否覆盖了这条路径——这不是防御式编程的兜底，而是逻辑错误的信号。",
        &SourceLoc::default(),
    );
}

/// E1 B 档：va_copy —— 复制 va_list（本实现中即 char* 游标值）。
/// 宏展开为 `__vitro_va_copy(&dst, &src)`，参数顺序与 host_va_start 一致：先 &dst 后 &src。
pub fn host_va_copy(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let dst_addr = vm.pop() as i32;
    let src_addr = vm.pop() as i32;
    let loc = SourceLoc::default();
    let value = vm.load_i32(src_addr as u32, &loc);
    vm.store_i32(dst_addr as u32, value, &loc);
}
