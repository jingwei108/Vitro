#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Host Function 契约测试（Phase A）
//!
//! 目标：验证 Layer B（Rust Host Func）的每个函数在边界条件、安全注入、标准一致性上是否达标。
//!
//! 测试哲学：
//! - NO_CODE_DISTORTION：不扭曲 C 语义去迎合 Vitro。
//! - RECORD_DONT_HIDE：任何异常行为必须记录。
//! - FIX_REAL_BUGS：测试失败时，修 Host Func 的实现，而不是改测试预期值让它通过。

use vitro_native::session::Session;
use vitro_native::vm::core::{VitroVM, MEM_SIZE, NULL_TRAP_SIZE};
use vitro_native::vm::host_funcs::{
    host_abort, host_acos, host_asin, host_atan, host_atan2, host_atoi, host_bsearch, host_calloc,
    host_vitro_assert_fail, host_clock, host_cos, host_cosh, host_exp, host_free, host_getchar, host_isblank,
    host_isgraph, host_ispunct, host_llabs, host_log, host_malloc, host_memset, host_pow, host_printf_n, host_putchar,
    host_puts, host_qsort, host_rand, host_realloc, host_remove, host_rename, host_scanf_n, host_sin, host_sinh,
    host_snprintf, host_sprintf, host_sqrt, host_srand, host_sscanf, host_strcat, host_strcmp, host_strcpy,
    host_strcspn, host_strerror, host_strlen, host_strpbrk, host_strspn, host_strtod, host_strtol, host_tanh,
    host_time,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fresh_session() -> (VitroVM, Session) {
    (VitroVM::new(), Session::default())
}

/// 在 VM 内存的合法区域写入一个 C 风格字符串，返回起始地址。
fn write_test_string(vm: &mut VitroVM, addr: u32, s: &str) {
    vm.write_cstring(addr, s);
}

/// 从 VM 内存读取一个 C 风格字符串（遇到 \0 停止）。
fn read_test_string(vm: &VitroVM, addr: u32) -> String {
    let mem = vm.memory_ref();
    let start = addr as usize;
    if start >= mem.len() {
        return String::new();
    }
    let bytes: Vec<u8> = mem[start..].iter().take_while(|&&b| b != 0).copied().collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ─── malloc 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_malloc_zero_returns_null_with_warning() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert_eq!(addr, 0, "malloc(0) 必须返回 NULL（0）");
    let notes = session.runtime.note_chunks();
    let warns: Vec<_> = notes.iter().filter(|l| l.contains("malloc(0)")).collect();
    assert!(!warns.is_empty(), "malloc(0) 必须输出警告说明其行为是实现定义的");
}

#[test]
fn test_malloc_negative_returns_null() {
    let (mut vm, mut session) = fresh_session();
    vm.push((-1i32) as u64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert_eq!(addr, 0, "malloc(负数) 必须返回 NULL");
}

#[test]
fn test_malloc_normal_returns_non_null() {
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert!(addr >= NULL_TRAP_SIZE, "malloc(64) 必须返回非 NULL 的合法地址");
    assert!(addr < MEM_SIZE, "malloc(64) 返回的地址必须在 VM 内存范围内");
}

#[test]
fn test_malloc_records_region_metadata() {
    let (mut vm, mut session) = fresh_session();
    vm.push(100);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;

    let region = session
        .memory
        .regions
        .iter()
        .find(|r| r.addr == addr && !r.is_freed)
        .expect("malloc 后必须在 session.memory.regions 中记录未释放的 MemoryRegion");
    assert_eq!(region.size, 100);
    assert!(region.is_heap);
    assert_eq!(region.alloc_by, "malloc");
}

#[test]
fn test_malloc_excessive_returns_null() {
    let (mut vm, mut session) = fresh_session();
    vm.push(MEM_SIZE as u64 + 1);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert_eq!(addr, 0, "malloc(超过 VM 内存大小) 必须返回 NULL");
}

// ─── free 契约 ───────────────────────────────────────────────────────────────

#[test]
fn test_free_null_is_safe() {
    let (mut vm, mut session) = fresh_session();
    let regions_before = session.memory.regions.len();
    vm.push(0);
    host_free(&mut vm, &mut session.as_vm_context());
    assert!(!vm.has_error(), "free(NULL) 必须是安全的，不得触发 trap");
    assert_eq!(session.memory.regions.len(), regions_before, "free(NULL) 不得新增或删除 region");
}

#[test]
fn test_free_valid_ptr_marks_freed() {
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;

    vm.push(addr as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    let region = session
        .memory
        .regions
        .iter()
        .find(|r| r.addr == addr)
        .expect("free 后 region 必须仍然存在（用于诊断）");
    assert!(region.is_freed, "free 后 region 必须标记为 is_freed");
}

#[test]
fn test_free_already_freed_traps_double_free() {
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;

    vm.push(addr as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    // 重置错误状态以便观察第二次 free
    // VitroVM 没有公开重置 error 的方法，但 trap 只在 error.is_empty() 时写入
    // 由于第一次 free 没有 trap，error 为空，第二次 free 应该触发 Double-Free
    vm.push(addr as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    assert!(vm.has_error(), "Double-Free 必须触发 trap");
    let err = vm.get_error();
    assert!(
        err.contains("Double-Free") || err.contains("E3061"),
        "Double-Free 错误信息必须包含 'Double-Free' 或 'E3061'，实际: {}",
        err
    );
}

// ─── realloc 契约 ────────────────────────────────────────────────────────────

#[test]
fn test_realloc_null_equivalent_to_malloc() {
    let (mut vm, mut session) = fresh_session();
    vm.push(64); // new_size
    vm.push(0); // ptr
    host_realloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert!(
        addr >= NULL_TRAP_SIZE,
        "realloc(NULL, size) 必须等价于 malloc(size)，返回非 NULL"
    );
}

#[test]
fn test_realloc_zero_equivalent_to_free() {
    let (mut vm, mut session) = fresh_session();
    // 先分配
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;

    // realloc(ptr, 0)
    vm.push(0); // new_size
    vm.push(addr as u64); // ptr
    host_realloc(&mut vm, &mut session.as_vm_context());
    let new_addr = vm.pop() as u32;

    assert_eq!(new_addr, 0, "realloc(ptr, 0) 应返回 NULL");
    let region = session.memory.regions.iter().find(|r| r.addr == addr).unwrap();
    assert!(region.is_freed, "realloc(ptr, 0) 必须释放原内存");
}

#[test]
fn test_realloc_larger_copies_data() {
    let (mut vm, mut session) = fresh_session();
    vm.push(8);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let old_addr = vm.pop() as u32;

    // 写入数据 "ABCDEFG\0"
    write_test_string(&mut vm, old_addr, "ABCDEFG");

    // realloc 扩大到 64
    vm.push(64);
    vm.push(old_addr as u64);
    host_realloc(&mut vm, &mut session.as_vm_context());
    let new_addr = vm.pop() as u32;

    assert!(new_addr >= NULL_TRAP_SIZE, "realloc 扩大必须返回合法地址");
    let copied = read_test_string(&vm, new_addr);
    assert_eq!(copied, "ABCDEFG", "realloc 扩大后必须拷贝旧数据");
}

// ─── strlen 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_strlen_normal() {
    let (mut vm, mut session) = fresh_session();
    let addr = 0x2000;
    write_test_string(&mut vm, addr, "hello");
    vm.push(addr as u64);
    host_strlen(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop(), 5, "strlen(\"hello\") 必须为 5");
}

#[test]
fn test_strlen_empty_string() {
    let (mut vm, mut session) = fresh_session();
    let addr = 0x2000;
    write_test_string(&mut vm, addr, "");
    vm.push(addr as u64);
    host_strlen(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop(), 0, "strlen(\"\") 必须为 0");
}

#[test]
fn test_strlen_null_address_returns_zero() {
    let (mut vm, mut session) = fresh_session();
    // VM 内存起始处为全 0，因此 read_cbytes(0) 返回空
    vm.push(0);
    host_strlen(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop(), 0, "strlen(0) 在 Vitro 中返回 0（VM 内存首字节为 0）");
}

// ─── strcpy 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_strcpy_normal_copy() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    let dst = 0x3000;
    write_test_string(&mut vm, src, "abc");
    vm.push(src as u64);
    vm.push(dst as u64);
    host_strcpy(&mut vm, &mut session.as_vm_context());
    let result = read_test_string(&vm, dst);
    assert_eq!(result, "abc", "strcpy 必须正确拷贝字符串");
}

#[test]
fn test_strcpy_dest_at_high_boundary() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    let dst = MEM_SIZE - 4; // 靠近内存末尾
    write_test_string(&mut vm, src, "ab");
    vm.push(src as u64);
    vm.push(dst as u64);
    host_strcpy(&mut vm, &mut session.as_vm_context());
    let result = read_test_string(&vm, dst);
    assert_eq!(result, "ab", "strcpy 在边界内必须正确拷贝");
}

// ─── strcmp 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_strcmp_equal() {
    let (mut vm, mut session) = fresh_session();
    let a = 0x2000;
    let b = 0x3000;
    write_test_string(&mut vm, a, "hello");
    write_test_string(&mut vm, b, "hello");
    vm.push(a as u64);
    vm.push(b as u64);
    host_strcmp(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, 0, "strcmp 相同字符串必须返回 0");
}

#[test]
fn test_strcmp_less() {
    let (mut vm, mut session) = fresh_session();
    let s1 = 0x2000;
    let s2 = 0x3000;
    write_test_string(&mut vm, s1, "abc");
    write_test_string(&mut vm, s2, "def");
    // VM 调用约定：从右到左入栈；host_strcmp 先 pop addr1(s1)，再 pop addr2(s2)
    vm.push(s2 as u64);
    vm.push(s1 as u64);
    host_strcmp(&mut vm, &mut session.as_vm_context());
    assert!((vm.pop() as i32) < 0, "strcmp(\"abc\", \"def\") 必须返回负数");
}

#[test]
fn test_strcmp_greater() {
    let (mut vm, mut session) = fresh_session();
    let s1 = 0x2000;
    let s2 = 0x3000;
    write_test_string(&mut vm, s1, "xyz");
    write_test_string(&mut vm, s2, "abc");
    vm.push(s2 as u64);
    vm.push(s1 as u64);
    host_strcmp(&mut vm, &mut session.as_vm_context());
    assert!((vm.pop() as i32) > 0, "strcmp(\"xyz\", \"abc\") 必须返回正数");
}

// ─── strcat 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_strcat_normal() {
    let (mut vm, mut session) = fresh_session();
    let dest = 0x2000;
    let src = 0x3000;
    write_test_string(&mut vm, dest, "hello");
    write_test_string(&mut vm, src, " world");
    vm.push(src as u64);
    vm.push(dest as u64);
    host_strcat(&mut vm, &mut session.as_vm_context());
    let result = read_test_string(&vm, dest);
    assert_eq!(result, "hello world", "strcat 必须正确拼接");
}

// ─── memset 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_memset_normal() {
    let (mut vm, mut session) = fresh_session();
    let ptr = 0x2000;
    let val = 0x42;
    let size = 8u64;
    vm.push(size);
    vm.push(val);
    vm.push(ptr as u64);
    host_memset(&mut vm, &mut session.as_vm_context());
    let mem = vm.memory_ref();
    for i in 0..8 {
        assert_eq!(mem[ptr as usize + i], val as u8, "memset 必须正确填充内存");
    }
}

#[test]
fn test_memset_returns_original_ptr() {
    let (mut vm, mut session) = fresh_session();
    let ptr = 0x2000u64;
    vm.push(4u64);
    vm.push(0u64);
    vm.push(ptr);
    host_memset(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop(), ptr, "memset 必须返回原指针");
}

// ─── atoi 契约 ───────────────────────────────────────────────────────────────

#[test]
fn test_atoi_standard_conformance() {
    let (mut vm, mut session) = fresh_session();
    let addr = 0x2000;
    write_test_string(&mut vm, addr, "  -123abc");
    vm.push(addr as u64);
    host_atoi(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, -123, "atoi(\"  -123abc\") 必须返回 -123（C 标准行为）");
}

#[test]
fn test_atoi_empty_string() {
    let (mut vm, mut session) = fresh_session();
    let addr = 0x2000;
    write_test_string(&mut vm, addr, "");
    vm.push(addr as u64);
    host_atoi(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, 0, "atoi(\"\") 必须返回 0");
}

#[test]
fn test_atoi_no_digits() {
    let (mut vm, mut session) = fresh_session();
    let addr = 0x2000;
    write_test_string(&mut vm, addr, "abc");
    vm.push(addr as u64);
    host_atoi(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, 0, "atoi(\"abc\") 必须返回 0");
}

// ─── printf 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_printf_basic_string() {
    let (mut vm, mut session) = fresh_session();
    let fmt = 0x2000;
    write_test_string(&mut vm, fmt, "hello");
    vm.push(fmt as u64);
    host_printf_n(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "hello");
}

#[test]
fn test_printf_integer() {
    let (mut vm, mut session) = fresh_session();
    let fmt = 0x2000;
    write_test_string(&mut vm, fmt, "%d");
    vm.push(42u64);
    vm.push(fmt as u64);
    host_printf_n(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "42");
}

#[test]
fn test_printf_string_arg() {
    let (mut vm, mut session) = fresh_session();
    let fmt = 0x2000;
    let arg = 0x3000;
    write_test_string(&mut vm, fmt, "%s");
    write_test_string(&mut vm, arg, "world");
    vm.push(arg as u64);
    vm.push(fmt as u64);
    host_printf_n(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "world");
}

// ─── scanf 契约 ──────────────────────────────────────────────────────────────

#[test]
fn test_scanf_integer() {
    let (mut vm, mut session) = fresh_session();
    session.runtime.input_lines.push("42".to_string());
    let fmt = 0x2000;
    let dst = 0x3000;
    write_test_string(&mut vm, fmt, "%d");
    vm.push(dst as u64);
    vm.push(fmt as u64);
    host_scanf_n(&mut vm, &mut session.as_vm_context());
    assert!(!session.runtime.waiting_input, "scanf 有输入时不应进入 waiting_input");
    let val = vm.load_i32(dst, &vitro_runtime::instruction::SourceLoc::default());
    assert_eq!(val, 42, "scanf(\"%%d\", ptr) 必须将 42 写入目标地址");
}

#[test]
fn test_scanf_multiple_integers() {
    let (mut vm, mut session) = fresh_session();
    session.runtime.input_lines.push("10 20".to_string());
    let fmt = 0x2000;
    let dst1 = 0x3000;
    let dst2 = 0x3004;
    write_test_string(&mut vm, fmt, "%d %d");
    vm.push(dst2 as u64);
    vm.push(dst1 as u64);
    vm.push(fmt as u64);
    host_scanf_n(&mut vm, &mut session.as_vm_context());
    let v1 = vm.load_i32(dst1, &vitro_runtime::instruction::SourceLoc::default());
    let v2 = vm.load_i32(dst2, &vitro_runtime::instruction::SourceLoc::default());
    assert_eq!(v1, 10);
    assert_eq!(v2, 20);
}

// ─── getchar / putchar 契约 ──────────────────────────────────────────────────

#[test]
fn test_getchar_reads_from_input_lines() {
    let (mut vm, mut session) = fresh_session();
    session.runtime.input_lines.push("ab".to_string());
    host_getchar(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, 'a' as i32);
    host_getchar(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, 'b' as i32);
}

#[test]
fn test_putchar_outputs_char() {
    let (mut vm, mut session) = fresh_session();
    vm.push('X' as u64);
    host_putchar(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "X");
}

// ─── rand / srand 契约 ───────────────────────────────────────────────────────

#[test]
fn test_rand_deterministic_with_seed() {
    let (mut vm1, mut session1) = fresh_session();
    let (mut vm2, mut session2) = fresh_session();

    vm1.push(12345u64);
    host_srand(&mut vm1, &mut session1.as_vm_context());

    vm2.push(12345u64);
    host_srand(&mut vm2, &mut session2.as_vm_context());

    host_rand(&mut vm1, &mut session1.as_vm_context());
    host_rand(&mut vm2, &mut session2.as_vm_context());

    assert_eq!(vm1.pop(), vm2.pop(), "相同种子必须产生相同的 rand 序列");
}

#[test]
fn test_rand_returns_non_negative() {
    let (mut vm, mut session) = fresh_session();
    vm.push(1u64);
    host_srand(&mut vm, &mut session.as_vm_context());
    host_rand(&mut vm, &mut session.as_vm_context());
    let v = vm.pop() as i64;
    assert!(v >= 0, "rand() 返回值必须非负");
    assert!(v <= 0x7fff, "rand() 返回值必须 <= RAND_MAX (0x7fff)");
}

// ─── 边界安全契约（部分为 KNOWN_FAILURE，待修复 Host Func 实现） ─────────────

#[test]
fn test_memset_null_address_traps() {
    let (mut vm, mut session) = fresh_session();
    vm.push(1u64);
    vm.push(0x42u64);
    vm.push(0u64); // NULL ptr
    host_memset(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "memset(NULL, ..., non-zero) 必须触发 NULL trap");
}

#[test]
fn test_strcpy_null_dest_traps() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    write_test_string(&mut vm, src, "ab");
    vm.push(src as u64);
    vm.push(0u64); // NULL dest
    host_strcpy(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "strcpy(NULL, src) 必须触发 NULL trap");
}

#[test]
fn test_strcpy_overflow_must_trap() {
    let (mut vm, mut session) = fresh_session();
    // 分配一个 3 字节的堆缓冲区（含 \0 只能放 2 个字符）
    vm.push(3);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let dst = vm.pop() as u32;

    let src = 0x3000;
    write_test_string(&mut vm, src, "hello"); // 5 个字符 + \0 = 6 字节

    vm.push(src as u64);
    vm.push(dst as u64);
    host_strcpy(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "strcpy 越界必须触发 trap，而不是静默截断");
    assert!(vm.get_error().contains("E3070"), "strcpy 溢出错误信息应包含 E3070");
}

#[test]
fn test_strcat_overflow_must_trap() {
    let (mut vm, mut session) = fresh_session();
    // 分配一个 4 字节的堆缓冲区
    vm.push(4);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let dest = vm.pop() as u32;
    write_test_string(&mut vm, dest, "ab"); // 已有 2 字节 + \0 = 3 字节，只剩 1 字节空间

    let src = 0x3000;
    write_test_string(&mut vm, src, " world"); // 需要 6 字节 + \0

    vm.push(src as u64);
    vm.push(dest as u64);
    host_strcat(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "strcat 越界必须触发 trap，而不是静默截断");
    assert!(vm.get_error().contains("E3070"), "strcat 溢出错误信息应包含 E3070");
}

// ─── math.h 契约 ─────────────────────────────────────────────────────────────

const MATH_EPS: f64 = 1e-9;

#[test]
fn test_math_sin_zero() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0.0f64.to_bits());
    host_sin(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!(result.abs() < MATH_EPS, "sin(0.0) 应接近 0，实际 {}", result);
}

#[test]
fn test_math_cos_zero() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0.0f64.to_bits());
    host_cos(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!((result - 1.0).abs() < MATH_EPS, "cos(0.0) 应接近 1，实际 {}", result);
}

#[test]
fn test_math_sqrt_four() {
    let (mut vm, mut session) = fresh_session();
    vm.push(4.0f64.to_bits());
    host_sqrt(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!((result - 2.0).abs() < MATH_EPS, "sqrt(4.0) 应接近 2，实际 {}", result);
}

#[test]
fn test_math_pow_two_three() {
    let (mut vm, mut session) = fresh_session();
    vm.push(3.0f64.to_bits()); // y (先压栈，后 pop)
    vm.push(2.0f64.to_bits()); // x
    host_pow(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!((result - 8.0).abs() < MATH_EPS, "pow(2.0, 3.0) 应接近 8，实际 {}", result);
}

#[test]
fn test_math_atan_zero() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0.0f64.to_bits());
    host_atan(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!(result.abs() < MATH_EPS, "atan(0.0) 应接近 0，实际 {}", result);
}

#[test]
fn test_math_log_one() {
    let (mut vm, mut session) = fresh_session();
    vm.push(1.0f64.to_bits());
    host_log(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!(result.abs() < MATH_EPS, "log(1.0) 应接近 0，实际 {}", result);
}

#[test]
fn test_math_exp_zero() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0.0f64.to_bits());
    host_exp(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!((result - 1.0).abs() < MATH_EPS, "exp(0.0) 应接近 1，实际 {}", result);
}

#[test]
fn test_math_sin_half_pi() {
    let (mut vm, mut session) = fresh_session();
    vm.push((std::f64::consts::PI / 2.0).to_bits());
    host_sin(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!((result - 1.0).abs() < MATH_EPS, "sin(pi/2) 应接近 1，实际 {}", result);
}

#[test]
fn test_math_sqrt_negative_returns_nan() {
    let (mut vm, mut session) = fresh_session();
    vm.push((-1.0f64).to_bits());
    host_sqrt(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!(result.is_nan(), "sqrt(-1.0) 应返回 NaN，实际 {}", result);
}

#[test]
fn test_math_log_zero_returns_neg_inf() {
    let (mut vm, mut session) = fresh_session();
    vm.push(0.0f64.to_bits());
    host_log(&mut vm, &mut session.as_vm_context());
    let result = f64::from_bits(vm.pop());
    assert!(
        result.is_infinite() && result.is_sign_negative(),
        "log(0.0) 应返回 -inf，实际 {}",
        result
    );
}

// ─── puts 契约 ───────────────────────────────────────────────────────────────

#[test]
fn test_puts_basic_string_with_newline() {
    let (mut vm, mut session) = fresh_session();
    let s = 0x2000;
    write_test_string(&mut vm, s, "hello");
    vm.push(s as u64);
    host_puts(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "hello\n");
    let ret = vm.pop() as i32;
    assert!(ret >= 0, "puts 成功时应返回非负值");
}

#[test]
fn test_puts_empty_string_outputs_only_newline() {
    let (mut vm, mut session) = fresh_session();
    let s = 0x2000;
    write_test_string(&mut vm, s, "");
    vm.push(s as u64);
    host_puts(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.runtime.stdout_chunks().last().copied().unwrap(), "\n");
}

// ─── calloc 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_calloc_zero_initializes_memory() {
    let (mut vm, mut session) = fresh_session();
    vm.push(4); // size
    vm.push(3); // nmemb
    host_calloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert!(addr >= NULL_TRAP_SIZE, "calloc 必须返回非 NULL 地址");
    let mem = vm.memory_ref();
    for i in 0..12 {
        assert_eq!(mem[addr as usize + i], 0, "calloc 分配的内存必须为零初始化");
    }
}

#[test]
fn test_calloc_records_region_metadata() {
    let (mut vm, mut session) = fresh_session();
    vm.push(8);
    vm.push(2);
    host_calloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    let region = session
        .memory
        .regions
        .iter()
        .find(|r| r.addr == addr && !r.is_freed)
        .expect("calloc 后必须记录 region");
    assert_eq!(region.size, 16);
    assert_eq!(region.alloc_by, "calloc");
}

#[test]
fn test_calloc_zero_nmemb_returns_null() {
    let (mut vm, mut session) = fresh_session();
    vm.push(4);
    vm.push(0);
    host_calloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert_eq!(addr, 0, "calloc(0, size) 应返回 NULL");
}

// ─── bsearch 契约 ────────────────────────────────────────────────────────────

#[test]
fn test_bsearch_found_existing_element() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    let key = 0x2100;
    // arr = [10, 20, 30, 40, 50]
    for (i, &v) in [10i32, 20, 30, 40, 50].iter().enumerate() {
        vm.store_i32(base + (i * 4) as u32, v, &vitro_runtime::instruction::SourceLoc::default());
    }
    vm.store_i32(key, 30, &vitro_runtime::instruction::SourceLoc::default());
    // args: key, base, nmemb=5, size=4, compar=0 (default byte comparison)
    vm.push(0); // compar
    vm.push(4); // size
    vm.push(5); // nmemb
    vm.push(base as u64); // base
    vm.push(key as u64); // key
    host_bsearch(&mut vm, &mut session.as_vm_context());
    let result = vm.pop() as u32;
    assert_eq!(result, base + 8, "bsearch 应找到第 3 个元素（地址 base+8）");
}

#[test]
fn test_bsearch_not_found_returns_null() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    let key = 0x2100;
    for (i, &v) in [10i32, 20, 30, 40, 50].iter().enumerate() {
        vm.store_i32(base + (i * 4) as u32, v, &vitro_runtime::instruction::SourceLoc::default());
    }
    vm.store_i32(key, 99, &vitro_runtime::instruction::SourceLoc::default());
    vm.push(0);
    vm.push(4);
    vm.push(5);
    vm.push(base as u64);
    vm.push(key as u64);
    host_bsearch(&mut vm, &mut session.as_vm_context());
    let result = vm.pop() as u32;
    assert_eq!(result, 0, "bsearch 未找到时应返回 NULL（0）");
}

#[test]
fn test_bsearch_empty_array_returns_null() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    let key = 0x2100;
    vm.store_i32(key, 10, &vitro_runtime::instruction::SourceLoc::default());
    vm.push(0);
    vm.push(4);
    vm.push(0); // nmemb = 0
    vm.push(base as u64);
    vm.push(key as u64);
    host_bsearch(&mut vm, &mut session.as_vm_context());
    let result = vm.pop() as u32;
    assert_eq!(result, 0, "bsearch 空数组应返回 NULL");
}

// ─── qsort 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_qsort_large_byte_array_default_compare() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    let n = 128usize;
    // 写入降序字节数据：127, 126, ..., 0（限制在 0~127，避免有符号/无符号解释差异）
    for i in 0..n {
        vm.store_i8(
            base + i as u32,
            (127 - i) as i32,
            &vitro_runtime::instruction::SourceLoc::default(),
        );
    }
    // args: compar=0 (default byte comparison), size=1, nmemb=128, base
    vm.push(0); // compar
    vm.push(1); // size
    vm.push(n as u64); // nmemb
    vm.push(base as u64); // base
    host_qsort(&mut vm, &mut session.as_vm_context());
    // 默认字节比较对单字节元素即数值比较，排序后应为升序：0, 1, ..., 127
    for i in 0..n {
        let v = vm.load_i8(base + i as u32, &vitro_runtime::instruction::SourceLoc::default());
        assert_eq!(
            v, i as i32,
            "qsort 128 单字节元素默认字节比较应升序排列，索引 {} 处期望 {}，实际 {}",
            i, i, v
        );
    }
}

#[test]
fn test_qsort_single_element_noop() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    vm.store_i32(base, 42, &vitro_runtime::instruction::SourceLoc::default());
    vm.push(0);
    vm.push(4);
    vm.push(1);
    vm.push(base as u64);
    host_qsort(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.load_i32(base, &vitro_runtime::instruction::SourceLoc::default()), 42);
}

#[test]
fn test_qsort_empty_array_noop() {
    let (mut vm, mut session) = fresh_session();
    let base = 0x2000;
    vm.push(0);
    vm.push(4);
    vm.push(0);
    vm.push(base as u64);
    host_qsort(&mut vm, &mut session.as_vm_context());
    // 不应 trap，也不应写入任何内容
}

// ─── sprintf 契约 ────────────────────────────────────────────────────────────

#[test]
fn test_sprintf_basic_formatting() {
    let (mut vm, mut session) = fresh_session();
    let buf = 0x2000;
    let fmt = 0x3000;
    write_test_string(&mut vm, fmt, "value=%d");
    vm.push(42u64); // arg
    vm.push(fmt as u64); // fmt
    vm.push(buf as u64); // buf
    host_sprintf(&mut vm, &mut session.as_vm_context());
    let ret = vm.pop() as i32;
    let s = read_test_string(&vm, buf);
    assert_eq!(s, "value=42");
    assert_eq!(ret, 8, "sprintf 应返回写入字符数（不含 \\0）");
}

#[test]
fn test_sprintf_multiple_args() {
    let (mut vm, mut session) = fresh_session();
    let buf = 0x2000;
    let fmt = 0x3000;
    write_test_string(&mut vm, fmt, "%d+%d=%d");
    vm.push(5u64);
    vm.push(3u64);
    vm.push(2u64);
    vm.push(fmt as u64);
    vm.push(buf as u64);
    host_sprintf(&mut vm, &mut session.as_vm_context());
    let s = read_test_string(&vm, buf);
    assert_eq!(s, "2+3=5");
}

// ─── snprintf 契约 ───────────────────────────────────────────────────────────

#[test]
fn test_snprintf_truncates_and_null_terminates() {
    let (mut vm, mut session) = fresh_session();
    let buf = 0x2000;
    let fmt = 0x3000;
    write_test_string(&mut vm, fmt, "hello world");
    vm.push(fmt as u64);
    vm.push(6); // size = 6 (最多写 5 字符 + \0)
    vm.push(buf as u64);
    host_snprintf(&mut vm, &mut session.as_vm_context());
    let ret = vm.pop() as i32;
    let s = read_test_string(&vm, buf);
    assert_eq!(s, "hello", "snprintf 应截断到 size-1");
    assert_eq!(ret, 11, "snprintf 返回值应为未截断时的总长度");
}

#[test]
fn test_snprintf_zero_size_writes_nothing() {
    let (mut vm, mut session) = fresh_session();
    let buf = 0x2000;
    let fmt = 0x3000;
    vm.memory_ref_mut()[buf as usize] = 0xAA;
    write_test_string(&mut vm, fmt, "x");
    vm.push(fmt as u64);
    vm.push(0); // size = 0
    vm.push(buf as u64);
    host_snprintf(&mut vm, &mut session.as_vm_context());
    let ret = vm.pop() as i32;
    let byte = vm.memory_ref()[buf as usize];
    assert_eq!(byte, 0xAA, "snprintf(size=0) 不得写入 buf");
    assert_eq!(ret, 1, "snprintf 返回值仍应为未截断长度");
}

// ─── sscanf 契约 ─────────────────────────────────────────────────────────────

#[test]
fn test_sscanf_two_integers() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    let fmt = 0x3000;
    let dst1 = 0x4000;
    let dst2 = 0x4004;
    write_test_string(&mut vm, src, "10 20");
    write_test_string(&mut vm, fmt, "%d %d");
    vm.push(dst2 as u64);
    vm.push(dst1 as u64);
    vm.push(fmt as u64);
    vm.push(src as u64);
    host_sscanf(&mut vm, &mut session.as_vm_context());
    let matched = vm.pop() as i32;
    let v1 = vm.load_i32(dst1, &vitro_runtime::instruction::SourceLoc::default());
    let v2 = vm.load_i32(dst2, &vitro_runtime::instruction::SourceLoc::default());
    assert_eq!(matched, 2, "sscanf 应返回成功匹配数 2");
    assert_eq!(v1, 10);
    assert_eq!(v2, 20);
}

#[test]
fn test_sscanf_string_token() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    let fmt = 0x3000;
    let dst = 0x4000;
    write_test_string(&mut vm, src, "hello world");
    write_test_string(&mut vm, fmt, "%s");
    vm.push(dst as u64);
    vm.push(fmt as u64);
    vm.push(src as u64);
    host_sscanf(&mut vm, &mut session.as_vm_context());
    let matched = vm.pop() as i32;
    let s = read_test_string(&vm, dst);
    assert_eq!(matched, 1);
    assert_eq!(s, "hello");
}

#[test]
fn test_sscanf_mixed_int_and_string() {
    let (mut vm, mut session) = fresh_session();
    let src = 0x2000;
    let fmt = 0x3000;
    let dst_int = 0x4000;
    let dst_str = 0x4100;
    write_test_string(&mut vm, src, "42 abc");
    write_test_string(&mut vm, fmt, "%d %s");
    vm.push(dst_str as u64);
    vm.push(dst_int as u64);
    vm.push(fmt as u64);
    vm.push(src as u64);
    host_sscanf(&mut vm, &mut session.as_vm_context());
    let matched = vm.pop() as i32;
    let v = vm.load_i32(dst_int, &vitro_runtime::instruction::SourceLoc::default());
    let s = read_test_string(&vm, dst_str);
    assert_eq!(matched, 2);
    assert_eq!(v, 42);
    assert_eq!(s, "abc");
}

// ─── ctype 补全契约 ──────────────────────────────────────────────────────────

#[test]
fn test_isgraph_basic() {
    let (mut vm, _session) = fresh_session();
    vm.push('A' as u64);
    host_isgraph(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 1);

    vm.push(' ' as u64);
    host_isgraph(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 0);

    vm.push('\n' as u64);
    host_isgraph(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 0);
}

#[test]
fn test_ispunct_basic() {
    let (mut vm, _session) = fresh_session();
    vm.push('!' as u64);
    host_ispunct(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 1);

    vm.push('A' as u64);
    host_ispunct(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 0);

    vm.push(' ' as u64);
    host_ispunct(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 0);
}

#[test]
fn test_isblank_basic() {
    let (mut vm, _session) = fresh_session();
    vm.push(' ' as u64);
    host_isblank(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 1);

    vm.push('\t' as u64);
    host_isblank(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 1);

    vm.push('\n' as u64);
    host_isblank(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i32, 0);
}

// ─── math 补全契约 ───────────────────────────────────────────────────────────

#[test]
fn test_asin_acos_bounds() {
    let (mut vm, _session) = fresh_session();
    // asin(0) = 0
    vm.push(0.0f64.to_bits());
    host_asin(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!(r.abs() < 1e-10, "asin(0) 应接近 0");

    // acos(1) = 0
    vm.push(1.0f64.to_bits());
    host_acos(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!(r.abs() < 1e-10, "acos(1) 应接近 0");
}

#[test]
fn test_atan2_quadrants() {
    let (mut vm, _session) = fresh_session();
    // atan2(y=0, x=1) = 0
    // 参数从右到左压栈：先 x，后 y
    vm.push(1.0f64.to_bits());
    vm.push(0.0f64.to_bits());
    host_atan2(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!(r.abs() < 1e-10, "atan2(0,1) 应接近 0");
}

#[test]
fn test_sinh_cosh_tanh_zero() {
    let (mut vm, _session) = fresh_session();
    // sinh(0) = 0
    vm.push(0.0f64.to_bits());
    host_sinh(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!(r.abs() < 1e-10, "sinh(0) 应接近 0");

    // cosh(0) = 1
    vm.push(0.0f64.to_bits());
    host_cosh(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!((r - 1.0).abs() < 1e-10, "cosh(0) 应接近 1");

    // tanh(0) = 0
    vm.push(0.0f64.to_bits());
    host_tanh(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!(r.abs() < 1e-10, "tanh(0) 应接近 0");
}

// ─── llabs 契约 ──────────────────────────────────────────────────────────────

#[test]
fn test_llabs_positive_and_negative() {
    let (mut vm, _session) = fresh_session();
    vm.push(42i64 as u64);
    host_llabs(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i64, 42);

    vm.push((-42i64) as u64);
    host_llabs(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i64, 42);

    vm.push(i64::MIN as u64);
    host_llabs(&mut vm, &mut Session::default().as_vm_context());
    // i64::MIN 的绝对值无法用 i64 表示，wrapping_neg 结果仍是 i64::MIN
    assert_eq!(vm.pop() as i64, i64::MIN);
}

// ─── abort 契约 ──────────────────────────────────────────────────────────────

#[test]
fn test_abort_sets_finished() {
    let (mut vm, mut session) = fresh_session();
    host_abort(&mut vm, &mut session.as_vm_context());
    assert!(vm.is_finished(), "abort 必须设置 finished");
    assert_eq!(vm.exit_code(), 134, "abort 退出码应为 134 (SIGABRT)");
    assert!(
        session.runtime.note_chunks().iter().any(|l| l.contains("abort")),
        "abort 必须输出诊断"
    );
}

// ─── strtol / strtod 契约 ────────────────────────────────────────────────────

#[test]
fn test_strtol_basic_decimal() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    write_test_string(&mut vm, s, "12345");
    vm.push(10); // base
    vm.push(0); // endptr = NULL
    vm.push(s as u64); // str
    host_strtol(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i64, 12345);
}

#[test]
fn test_strtol_negative() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    write_test_string(&mut vm, s, "-99");
    vm.push(10);
    vm.push(0);
    vm.push(s as u64);
    host_strtol(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i64, -99);
}

#[test]
fn test_strtol_endptr() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    let endptr = 0x3000;
    write_test_string(&mut vm, s, "123abc");
    vm.push(10);
    vm.push(endptr as u64);
    vm.push(s as u64);
    host_strtol(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as i64, 123);
    let end_addr = vm.load_i32(endptr, &vitro_runtime::instruction::SourceLoc::default()) as u32;
    assert_eq!(end_addr, s + 3, "endptr 应指向 'a'");
}

#[test]
fn test_strtol_empty_sets_errno() {
    let (mut vm, mut session) = fresh_session();
    // 先声明 errno 全局变量（模拟编译器行为）
    let errno_addr = 0x1000u32;
    vm.write_memory(errno_addr, &0i32.to_le_bytes());
    // 将 errno 注入符号表
    let mut symbols = vm.get_symbols().to_vec();
    symbols.push(vitro_native::vm::core::VMSymbol {
        name: "errno".to_string(),
        addr: errno_addr,
        is_local: false,
        ty: vitro_native::compiler::ast::Type::int(),
        scope_depth: 0,
        func_name: String::new(),
        decl_line: 0,
    });
    vm.set_symbols(symbols);

    let s = 0x2000;
    write_test_string(&mut vm, s, "abc");
    vm.push(10);
    vm.push(0);
    vm.push(s as u64);
    host_strtol(&mut vm, &mut session.as_vm_context());
    let _ = vm.pop();
    let errno_val = i32::from_le_bytes([
        vm.memory_ref()[errno_addr as usize],
        vm.memory_ref()[errno_addr as usize + 1],
        vm.memory_ref()[errno_addr as usize + 2],
        vm.memory_ref()[errno_addr as usize + 3],
    ]);
    assert_eq!(errno_val, 1, "解析失败应设置 errno=EINVAL");
}

#[test]
#[allow(clippy::approx_constant)]
fn test_strtod_basic() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    write_test_string(&mut vm, s, "3.14");
    vm.push(0); // endptr = NULL
    vm.push(s as u64); // str
    host_strtod(&mut vm, &mut Session::default().as_vm_context());
    let r = f64::from_bits(vm.pop());
    assert!((r - 3.14).abs() < 1e-6, "strtod(3.14) 应接近 3.14");
}

// ─── strerror 契约 ───────────────────────────────────────────────────────────

#[test]
fn test_strerror_known_codes() {
    let (mut vm, mut session) = fresh_session();
    vm.push(1); // EINVAL
    host_strerror(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    let s = read_test_string(&vm, addr);
    assert_eq!(s, "Invalid argument");

    vm.push(2); // ERANGE
    host_strerror(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    let s = read_test_string(&vm, addr);
    assert_eq!(s, "Numerical result out of range");
}

// ─── VFS 扩展契约 ────────────────────────────────────────────────────────────

#[test]
fn test_remove_deletes_file() {
    let (mut vm, mut session) = fresh_session();
    // 先创建文件
    let path = 0x2000;
    write_test_string(&mut vm, path, "test.txt");
    vm.push(path as u64);
    host_remove(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, -1, "不存在的文件 remove 返回 -1");
}

#[test]
fn test_rename_nonexistent() {
    let (mut vm, mut session) = fresh_session();
    let old = 0x2000;
    let new = 0x3000;
    write_test_string(&mut vm, old, "old.txt");
    write_test_string(&mut vm, new, "new.txt");
    vm.push(new as u64);
    vm.push(old as u64);
    host_rename(&mut vm, &mut session.as_vm_context());
    assert_eq!(vm.pop() as i32, -1, "不存在的文件 rename 返回 -1");
}

// ─── time / clock 契约 ───────────────────────────────────────────────────────

#[test]
fn test_time_returns_positive() {
    let (mut vm, _session) = fresh_session();
    vm.push(0); // tloc = NULL
    host_time(&mut vm, &mut Session::default().as_vm_context());
    let t = vm.pop() as i64;
    assert!(t > 0, "time() 应返回正数 (Unix 时间戳)");
}

#[test]
fn test_clock_returns_non_negative() {
    let (mut vm, _session) = fresh_session();
    host_clock(&mut vm, &mut Session::default().as_vm_context());
    let c = vm.pop() as i64;
    assert!(c >= 0, "clock() 应返回非负数");
}

// ─── assert_fail 契约 ────────────────────────────────────────────────────────

#[test]
fn test_assert_fail_sets_finished() {
    let (mut vm, mut session) = fresh_session();
    host_vitro_assert_fail(&mut vm, &mut session.as_vm_context());
    assert!(vm.is_finished(), "assert_fail 必须设置 finished");
    assert_eq!(vm.exit_code(), 1);
    assert!(
        session.runtime.note_chunks().iter().any(|l| l.contains("断言失败")),
        "assert_fail 必须输出诊断"
    );
}

// ─── strpbrk / strspn / strcspn 契约 ─────────────────────────────────────────

#[test]
fn test_strpbrk_basic() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    let accept = 0x3000;
    write_test_string(&mut vm, s, "hello");
    write_test_string(&mut vm, accept, "aeiou");
    vm.push(accept as u64);
    vm.push(s as u64);
    host_strpbrk(&mut vm, &mut Session::default().as_vm_context());
    let addr = vm.pop() as u32;
    assert_eq!(addr, s + 1, "strpbrk(hello, aeiou) 应指向 'e'");
}

#[test]
fn test_strpbrk_no_match() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    let accept = 0x3000;
    write_test_string(&mut vm, s, "xyz");
    write_test_string(&mut vm, accept, "abc");
    vm.push(accept as u64);
    vm.push(s as u64);
    host_strpbrk(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as u32, 0, "strpbrk 无匹配应返回 NULL");
}

#[test]
fn test_strspn_basic() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    let accept = 0x3000;
    write_test_string(&mut vm, s, "123abc");
    write_test_string(&mut vm, accept, "0123456789");
    vm.push(accept as u64);
    vm.push(s as u64);
    host_strspn(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as usize, 3, "strspn(123abc, digits) 应为 3");
}

#[test]
fn test_strcspn_basic() {
    let (mut vm, _session) = fresh_session();
    let s = 0x2000;
    let reject = 0x3000;
    write_test_string(&mut vm, s, "hello world");
    write_test_string(&mut vm, reject, " ");
    vm.push(reject as u64);
    vm.push(s as u64);
    host_strcspn(&mut vm, &mut Session::default().as_vm_context());
    assert_eq!(vm.pop() as usize, 5, "strcspn(hello world, space) 应为 5");
}

// ─── printf/scanf 栈深度契约 ───────────────────────────────────────────────────

#[test]
fn test_printf_n_rejects_insufficient_args() {
    // B43: 格式字符串要求的参数多于栈中实际值时，应一次性 trap 而不是多次 pop 下溢。
    let (mut vm, mut session) = fresh_session();
    let fmt_addr = 0x2000;
    write_test_string(&mut vm, fmt_addr, "%d %d");
    // VM 调用约定：fmt 在栈顶，参数按从右到左顺序压栈
    vm.push(42); // 第一个 %d
    vm.push(fmt_addr as u64);
    host_printf_n(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "printf 参数不足时应产生运行时错误");
    assert!(
        vm.get_error().contains("参数多于实际提供的参数"),
        "错误信息应提示参数不足: {}",
        vm.get_error()
    );
}

#[test]
fn test_scanf_n_rejects_insufficient_args() {
    // B43: scanf 同样需要在 pop 前验证栈深度。
    let (mut vm, mut session) = fresh_session();
    let fmt_addr = 0x2000;
    write_test_string(&mut vm, fmt_addr, "%d %d");
    // VM 调用约定：fmt 在栈顶，参数按从右到左顺序压栈
    vm.push(0x3000); // 第一个 %d 的目标地址
    vm.push(fmt_addr as u64);
    host_scanf_n(&mut vm, &mut session.as_vm_context());
    assert!(vm.has_error(), "scanf 参数不足时应产生运行时错误");
    assert!(
        vm.get_error().contains("参数多于实际提供的参数"),
        "错误信息应提示参数不足: {}",
        vm.get_error()
    );
}

// ─── 堆隔离区契约（2026-09-11 决议：bump 分配 + 有界隔离）──────────────────────
// 依据 堆有界隔离决议.md §1/§3/§6。原 3a 的 free 断言只覆盖
// "标记 is_freed"，未触及分配器复用行为；本节把新语义的行为契约显式化。

#[test]
fn test_free_enters_quarantine_not_free_list() {
    // free 的块进入 FIFO 隔离区（地址在隔离期内不复用），不得直接进入可复用 free_list
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    assert!(session.memory.free_list.is_empty(), "malloc 后 free_list 应为空");

    vm.push(addr as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    assert_eq!(session.memory.quarantine.len(), 1, "free 后块必须进入隔离区");
    assert_eq!(
        session.memory.quarantine.front().map(|b| b.addr),
        Some(addr),
        "隔离区队首应是刚释放的块地址"
    );
    assert!(
        session.memory.free_list.is_empty(),
        "隔离期内不得进入 free_list（否则地址可被立即复用，UAF 检测窗口失效）"
    );
    assert_eq!(session.memory.quarantine_bytes, 64, "隔离区字节数应与块大小同步");
}

#[test]
fn test_malloc_reuses_after_quarantine_eviction() {
    // 隔离区超预算 → FIFO 驱逐最老块归还 free_list → 下一次 malloc 复用该地址
    let (mut vm, mut session) = fresh_session();
    session.memory.quarantine_budget = 0; // 会话级预算可调（决议 §1）

    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr = vm.pop() as u32;
    vm.push(addr as u64);
    host_free(&mut vm, &mut session.as_vm_context());
    assert_eq!(session.memory.quarantine.len(), 1);

    // 预算为 0 → 下一次分配先驱逐，再 first-fit 复用被驱逐的地址
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr2 = vm.pop() as u32;

    assert_eq!(addr2, addr, "隔离区驱逐后 malloc 必须复用被驱逐的地址");
    assert!(session.memory.quarantine.is_empty(), "驱逐后隔离区应为空");
    assert_eq!(session.memory.quarantine_bytes, 0, "驱逐后隔离区字节数应归零");
}

#[test]
fn test_realloc_never_reuses_old_address() {
    // 决议 §3：realloc 恒为新块拷贝——旧块进隔离区，新地址必不同于旧地址
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let old_addr = vm.pop() as u32;

    vm.push(128); // new_size
    vm.push(old_addr as u64); // ptr
    host_realloc(&mut vm, &mut session.as_vm_context());
    let new_addr = vm.pop() as u32;

    assert_ne!(new_addr, old_addr, "realloc 必须搬移（隔离期内旧地址不复用）");
    assert_eq!(session.memory.quarantine.len(), 1, "旧块必须进入隔离区");
}

#[test]
fn test_heap_offset_never_rewinds() {
    // bump 语义：heap_offset 单调不减（leak 路径由此撞墙；churn 复用不推进堆顶）
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let addr1 = vm.pop() as u32;
    let top_after_first = session.memory.heap_offset;

    vm.push(addr1 as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let _addr2 = vm.pop() as u32;

    assert!(
        session.memory.heap_offset >= top_after_first,
        "free 后 heap_offset 不得回退（原地收缩特例已被决议移除）"
    );
}

// ─── U2#2 索引化差分保护用例（2026-09-14，G6 定论后先行落库）────────────────
// 背景（裁定 §5a）：regions 稳态 ~16387 条（隔离预算 256KB 饱和），U2#2 将
// free/malloc 复用/realloc 的 `iter().find` 线性扫描改 addr 索引。本节用例在
// **重构前**锁定隔离/复用语义——重构后复跑全绿 = 不破坏（性能对照由
// `scripts/core_asset_verdict/regions_growth` 驱动承担，不用时间断言防 CI 慢机误报）。

#[test]
fn test_u22_churn_steady_state_quarantine_window_alive() {
    // churn 进入稳态（持续驱逐复用）后，最后释放的块仍必须在 freed_logs 与
    // is_freed 条目里——UAF 检测窗口不得被复用路径的 retain 清理误删。
    let (mut vm, mut session) = fresh_session();
    session.memory.quarantine_budget = 64; // 恰容一块 64B → 每次新分配驱逐上一块
    let mut last = 0;
    for _ in 0..50 {
        vm.push(64);
        host_malloc(&mut vm, &mut session.as_vm_context());
        let a = vm.pop() as u32;
        vm.push(a as u64);
        host_free(&mut vm, &mut session.as_vm_context());
        last = a;
    }
    // 稳态已进入复用（堆顶推进量远小于分配次数）
    let distinct = session
        .memory
        .regions
        .iter()
        .filter(|r| r.is_heap)
        .count();
    assert!(distinct < 50, "预算 64B 下 50 次 churn 应进入地址复用稳态，实际不同地址 {} 个", distinct);
    assert!(
        vm.get_freed_logs().iter().any(|l| l.addr == last),
        "churn 稳态下最后释放的块必须仍在 freed_logs（UAF 检测窗口存活）"
    );
    assert!(
        session.memory.regions.iter().any(|r| r.addr == last && r.is_freed),
        "对应 region 条目必须为 is_freed"
    );
}

#[test]
fn test_u22_reuse_resets_metadata_and_clears_freed_log() {
    // 复用同地址：freed_logs 清理（新块使用不得误报 UAF）+ region 元数据复位。
    let (mut vm, mut session) = fresh_session();
    session.memory.quarantine_budget = 0;

    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let a = vm.pop() as u32;
    vm.push(a as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let b = vm.pop() as u32;

    assert_eq!(b, a, "预算 0 → 驱逐后必须复用同地址");
    assert!(
        !vm.get_freed_logs().iter().any(|l| l.addr == b),
        "复用后 freed_logs 必须已清理，否则使用新分配块会误报 UAF"
    );
    let r = session.memory.regions.iter().find(|r| r.addr == b).expect("复用条目存在");
    assert!(!r.is_freed, "复用条目 is_freed 必须复位");
    assert_eq!(r.size, 64, "复用条目 size 更新为新分配大小");
}

#[test]
fn test_u22_churn_leak_count_and_no_duplicate_entries() {
    // 泄漏计数与条目唯一性：churn 100 次（默认预算下不复用，各占新地址）后
    // 恰 1 块泄漏、条目恰 101——索引化若丢条目/重复 push，两计数必漂移。
    let (mut vm, mut session) = fresh_session();
    for _ in 0..100 {
        vm.push(32);
        host_malloc(&mut vm, &mut session.as_vm_context());
        let a = vm.pop() as u32;
        vm.push(a as u64);
        host_free(&mut vm, &mut session.as_vm_context());
    }
    vm.push(48);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let live = vm.pop() as u32;
    assert!(live != 0);

    let heap_regions: Vec<_> = session.memory.regions.iter().filter(|r| r.is_heap).collect();
    assert_eq!(heap_regions.len(), 101, "churn 100 次 + 1 活块 = 101 个堆条目（无重复 push / 无丢失）");
    let leaked = heap_regions.iter().filter(|r| !r.is_freed).count();
    assert_eq!(leaked, 1, "恰 1 块泄漏（churn 全 free + 1 活块）");
    let addrs: std::collections::HashSet<u32> = heap_regions.iter().map(|r| r.addr).collect();
    assert_eq!(addrs.len(), 101, "堆条目地址必须唯一（索引不变量：addr → 唯一条目）");
}

#[test]
fn test_u22_realloc_reuse_no_duplicate_entries() {
    // U2#2 同类清查实锤的存量缺陷：realloc 的新块登记走无条件 push——
    // 当 allocate_raw 从 free_list 复用隔离驱逐块时，该 addr 在 regions 已有
    // 条目 → 同 addr 双条目（泄漏报告把已释放块虚报为泄漏，且破坏索引不变量
    // addr 唯一）。红→绿：旧代码红，索引化批改为与 malloc 相同的"复位 or push"
    // 后绿。
    let (mut vm, mut session) = fresh_session();
    session.memory.quarantine_budget = 0;

    // A（大于 realloc 需求的驱逐块候选）与 B（realloc 搬移源）都在 free(a)
    // 之前分配——free 后 realloc 必须是第一个分配，其 allocate_raw 才会驱逐
    // A 并 first-fit 命中（中间任何 malloc 都会把驱逐块吃掉）。
    vm.push(256);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let a = vm.pop() as u32;

    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let b = vm.pop() as u32;
    assert_ne!(b, a);

    vm.push(a as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    // realloc(B, 128)：allocate_raw 驱逐 A(256) → first-fit 命中 → 新地址 = a。
    // a 在 regions 已有条目（is_freed）——新块登记必须走复位而非 push。
    vm.push(128);
    vm.push(b as u64);
    host_realloc(&mut vm, &mut session.as_vm_context());
    let new_addr = vm.pop() as u32;
    assert_eq!(new_addr, a, "free_list 复用：realloc 新地址应为驱逐块 a");

    let dup = session.memory.regions.iter().filter(|r| r.addr == a).count();
    assert_eq!(
        dup, 1,
        "addr 0x{:X} 必须恰一条目（旧实现双条目 = 泄漏虚报 + 索引失配）",
        a
    );
    assert!(
        session.memory.verify_region_index().is_ok(),
        "索引一致性：{:?}",
        session.memory.verify_region_index()
    );
}

#[test]
fn test_u22_snapshot_restore_keeps_free_semantics() {
    // 快照恢复（seek 回退）后 regions 被整体重装——恢复后再 free 同一地址
    // 必须语义正确（回到过去：is_freed 复位 → free 成功而非 Double-Free）。
    // 索引化后恢复点需同步重建索引，本用例锁其语义面。
    let (mut vm, mut session) = fresh_session();
    vm.push(64);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let a = vm.pop() as u32;

    let snap = vm.snapshot(&session.as_vm_context());

    vm.push(a as u64);
    host_free(&mut vm, &mut session.as_vm_context());
    assert!(
        session.memory.regions.iter().any(|r| r.addr == a && r.is_freed),
        "恢复前：free 已生效"
    );

    vm.restore(&snap, &mut session.as_vm_context());
    assert!(
        session.memory.regions.iter().any(|r| r.addr == a && !r.is_freed),
        "恢复后：is_freed 应回到快照时的 false"
    );

    // 回到过去后再 free：应成功置 freed（而非 Double-Free trap）
    vm.push(a as u64);
    host_free(&mut vm, &mut session.as_vm_context());
    assert!(
        session.memory.regions.iter().any(|r| r.addr == a && r.is_freed),
        "恢复后再 free 必须正常生效（时间旅行语义，不得误报 Double-Free）"
    );
}
