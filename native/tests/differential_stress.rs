#![allow(clippy::unwrap_used, clippy::expect_used)]

//! 差分压力测试（Phase C）
//!
//! 核心思想：对同一功能的两种实现（Layer B Rust Host vs Layer C Bytecode）
//! 进行交叉验证。同一随机输入，两者结果必须永远一致。
//!
//! 测试哲学：
//! - 不预设哪边是对的：差分测试失败时，两边都要审查。
//! - 记录所有偏差：即使偏差极小，也要记录。
//! - 不通过删减测试用例来消除差异。

use vitro_native::engine::compile_pipeline::{run_multi_file_pipeline, setup_vm};
use vitro_native::session::{CompileUnit, Session};
use vitro_native::vm::context::VmContext;
use vitro_native::vm::core::VitroVM;
use vitro_native::vm::host_funcs::{
    host_abs, host_atoi, host_isalnum, host_isalpha, host_iscntrl, host_isdigit, host_islower, host_isprint,
    host_isspace, host_isupper, host_isxdigit, host_memcpy, host_memmove, host_strcmp, host_strlen, host_strncpy,
    host_tolower, host_toupper,
};

const BC_STDLIB: &str = include_str!("../runtime_libc/src/stdlib.c");
const BC_STRING: &str = include_str!("../runtime_libc/src/string.c");

/// 编译 Bytecode Libc 源码并加载到 VM。
fn prepare_bc_vm(sources: &[(&str, &str)]) -> (VitroVM, Session) {
    let mut units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: "int main() { return 0; }\n".to_string(),
    }];
    for (name, src) in sources {
        units.push(CompileUnit {
            filename: name.to_string(),
            source: src.to_string(),
        });
    }

    let mut session = Session::default();
    run_multi_file_pipeline(&mut session, units, false).expect("Bytecode Libc 编译失败");

    let mut vm = VitroVM::new();
    setup_vm(&mut vm, &session);

    (vm, session)
}

/// 查找 VM 中已注册函数的索引。
fn find_func_idx(vm: &VitroVM, name: &str) -> u32 {
    vm.get_func_index(name)
        .unwrap_or_else(|| panic!("函数 {} 未在 VM 中注册", name))
}

// ─── abs 差分测试 ────────────────────────────────────────────────────────────

#[test]
fn test_diff_abs() {
    let (mut vm, mut session) = prepare_bc_vm(&[("stdlib.c", BC_STDLIB)]);
    let func_idx = find_func_idx(&vm, "abs");

    let inputs = [-5i32, 0, 5, -123, 2147483647];
    for &n in &inputs {
        // Host 路径
        let mut hvm = VitroVM::new();
        let mut hsess = Session::default();
        hvm.push(n as u64);
        host_abs(&mut hvm, &mut hsess.as_vm_context());
        let host_result = hvm.pop() as i32;

        // Bytecode 路径（使用同一个 VM 实例）
        let bc_result = vm
            .call_user_function(&mut session.as_vm_context(), func_idx, &[n], 10000)
            .expect("Bytecode abs 调用失败");

        assert_eq!(
            host_result, bc_result,
            "abs({}) 出现分歧: Host={}, Bytecode={}",
            n, host_result, bc_result
        );
    }
}

// ─── strlen 差分测试 ─────────────────────────────────────────────────────────

#[test]
fn test_diff_strlen() {
    let (mut vm, mut session) = prepare_bc_vm(&[("string.c", BC_STRING)]);
    let func_idx = find_func_idx(&vm, "strlen");

    let test_strings = ["hello", "", "a", "1234567890", "Hello, World!"];
    let base_addr = 0x2000u32;

    for s in &test_strings {
        vm.write_cstring(base_addr, s);

        // Host 路径
        vm.push(base_addr as u64);
        host_strlen(&mut vm, &mut session.as_vm_context());
        let host_result = vm.pop() as i32;

        // Bytecode 路径
        let bc_result = vm
            .call_user_function(&mut session.as_vm_context(), func_idx, &[base_addr as i32], 10000)
            .expect("Bytecode strlen 调用失败");

        assert_eq!(
            host_result, bc_result,
            "strlen({:?}) 出现分歧: Host={}, Bytecode={}",
            s, host_result, bc_result
        );
    }
}

// ─── atoi 差分测试 ───────────────────────────────────────────────────────────

#[test]
fn test_diff_atoi() {
    let (mut vm, mut session) = prepare_bc_vm(&[("stdlib.c", BC_STDLIB)]);
    let func_idx = find_func_idx(&vm, "atoi");

    let test_cases = ["42", "  -123abc", "0", "  +99", "abc", "", "  007"];
    let base_addr = 0x2000u32;

    for s in &test_cases {
        vm.write_cstring(base_addr, s);

        // Host 路径
        vm.push(base_addr as u64);
        host_atoi(&mut vm, &mut session.as_vm_context());
        let host_result = vm.pop() as i32;

        // Bytecode 路径
        let bc_result = vm
            .call_user_function(&mut session.as_vm_context(), func_idx, &[base_addr as i32], 10000)
            .expect("Bytecode atoi 调用失败");

        assert_eq!(
            host_result, bc_result,
            "atoi({:?}) 出现分歧: Host={}, Bytecode={}",
            s, host_result, bc_result
        );
    }
}

// ─── strcmp 差分测试 ─────────────────────────────────────────────────────────

#[test]
fn test_diff_strcmp() {
    let (mut vm, mut session) = prepare_bc_vm(&[("string.c", BC_STRING)]);
    let func_idx = find_func_idx(&vm, "strcmp");

    let test_pairs = [
        ("hello", "hello"),
        ("abc", "def"),
        ("xyz", "abc"),
        ("", ""),
        ("a", "ab"),
        ("same", "same"),
    ];
    let addr1 = 0x2000u32;
    let addr2 = 0x3000u32;

    for (a, b) in &test_pairs {
        vm.write_cstring(addr1, a);
        vm.write_cstring(addr2, b);

        // Host 路径（注意：host_strcmp 先 pop s1 再 pop s2）
        vm.push(addr2 as u64);
        vm.push(addr1 as u64);
        host_strcmp(&mut vm, &mut session.as_vm_context());
        let host_result = vm.pop() as i32;

        // Bytecode 路径
        let bc_result = vm
            .call_user_function(&mut session.as_vm_context(), func_idx, &[addr1 as i32, addr2 as i32], 10000)
            .expect("Bytecode strcmp 调用失败");

        let sign_host = host_result.signum();
        let sign_bc = bc_result.signum();
        assert_eq!(
            sign_host, sign_bc,
            "strcmp({:?}, {:?}) 出现分歧: Host={}, Bytecode={}",
            a, b, host_result, bc_result
        );
    }
}

/// 从 VM 内存读取 C 风格字符串（遇到 \0 停止）。
fn read_test_string(vm: &VitroVM, addr: u32) -> String {
    let mem = vm.memory_ref();
    let start = addr as usize;
    if start >= mem.len() {
        return String::new();
    }
    let bytes: Vec<u8> = mem[start..].iter().take_while(|&&b| b != 0).copied().collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// ─── ctype 差分测试（批量） ─────────────────────────────────────────────────

fn diff_ctype_test(func_name: &str, inputs: &[i32], host_fn: fn(&mut VitroVM, &mut VmContext<'_>)) {
    let (mut vm, mut session) = prepare_bc_vm(&[("ctype.c", BC_CTYPE)]);
    let func_idx = find_func_idx(&vm, func_name);

    for &c in inputs {
        // Host 路径
        let mut hvm = VitroVM::new();
        let mut hsess = Session::default();
        hvm.push(c as u64);
        host_fn(&mut hvm, &mut hsess.as_vm_context());
        let host_result = hvm.pop() as i32;

        // Bytecode 路径
        let bc_result = vm
            .call_user_function(&mut session.as_vm_context(), func_idx, &[c], 10000)
            .unwrap_or_else(|| panic!("Bytecode {} 调用失败", func_name));

        assert_eq!(
            host_result, bc_result,
            "{}({}) 出现分歧: Host={}, Bytecode={}",
            func_name, c, host_result, bc_result
        );
    }
}

const BC_CTYPE: &str = include_str!("../runtime_libc/src/ctype.c");

#[test]
fn test_diff_isdigit() {
    diff_ctype_test("isdigit", &['0' as i32, '9' as i32, 'a' as i32, ' ' as i32], host_isdigit);
}

#[test]
fn test_diff_isalpha() {
    diff_ctype_test("isalpha", &['a' as i32, 'Z' as i32, '5' as i32, ' ' as i32], host_isalpha);
}

#[test]
fn test_diff_islower() {
    diff_ctype_test("islower", &['a' as i32, 'z' as i32, 'A' as i32, '5' as i32], host_islower);
}

#[test]
fn test_diff_isupper() {
    diff_ctype_test("isupper", &['A' as i32, 'Z' as i32, 'a' as i32, '5' as i32], host_isupper);
}

#[test]
fn test_diff_tolower() {
    diff_ctype_test("tolower", &['A' as i32, 'Z' as i32, 'a' as i32, '5' as i32], host_tolower);
}

#[test]
fn test_diff_toupper() {
    diff_ctype_test("toupper", &['a' as i32, 'z' as i32, 'A' as i32, '5' as i32], host_toupper);
}

#[test]
fn test_diff_isspace() {
    diff_ctype_test("isspace", &[' ' as i32, '\t' as i32, '\n' as i32, 'A' as i32], host_isspace);
}

#[test]
fn test_diff_isalnum() {
    diff_ctype_test("isalnum", &['a' as i32, '5' as i32, ' ' as i32, '!' as i32], host_isalnum);
}

#[test]
fn test_diff_isprint() {
    diff_ctype_test("isprint", &['A' as i32, ' ' as i32, '\n' as i32, 127i32], host_isprint);
}

#[test]
fn test_diff_iscntrl() {
    diff_ctype_test("iscntrl", &['\n' as i32, 127i32, 'A' as i32, ' ' as i32], host_iscntrl);
}

#[test]
fn test_diff_isxdigit() {
    diff_ctype_test("isxdigit", &['a' as i32, 'F' as i32, '5' as i32, 'g' as i32], host_isxdigit);
}

// ─── strncpy / memcpy / memmove 差分测试 ─────────────────────────────────────

#[test]
fn test_diff_strncpy() {
    let (mut vm, mut session) = prepare_bc_vm(&[("string.c", BC_STRING)]);
    let func_idx = find_func_idx(&vm, "strncpy");

    let src_addr = 0x3000u32;
    let dst_addr = 0x2000u32;
    let test_cases = [("hello world", 5, "hello"), ("hi", 8, "hi")];

    for (src_str, n, expected) in &test_cases {
        vm.write_cstring(src_addr, src_str);

        // 清零目标区域，消除残留数据对 read_test_string 的干扰
        let dst_end = (dst_addr as usize + 16).min(vm.get_memory_size() as usize);
        vm.memory_ref_mut()[dst_addr as usize..dst_end].fill(0);

        // Host 路径（C 调用约定：从右到左入栈）
        vm.push(*n as u64);
        vm.push(src_addr as u64);
        vm.push(dst_addr as u64);
        host_strncpy(&mut vm, &mut session.as_vm_context());
        let _ = vm.pop();

        let host_result = read_test_string(&vm, dst_addr);

        // Bytecode 路径
        vm.memory_ref_mut()[dst_addr as usize..dst_end].fill(0);
        let bc_ret = vm
            .call_user_function(
                &mut session.as_vm_context(),
                func_idx,
                &[dst_addr as i32, src_addr as i32, *n],
                10000,
            )
            .unwrap_or_else(|| panic!("Bytecode strncpy 调用失败: error={}", vm.get_error()));

        let bc_result = read_test_string(&vm, dst_addr);

        assert_eq!(host_result, *expected, "strncpy Host 结果不符");
        assert_eq!(bc_result, *expected, "strncpy Bytecode 结果不符");
        assert_eq!(host_result, bc_result, "strncpy({:?}, {}) 出现分歧", src_str, n);
        assert_eq!(bc_ret as u32, dst_addr, "strncpy 应返回 dest 指针");
    }
}

#[test]
fn test_diff_memcpy() {
    let (mut vm, mut session) = prepare_bc_vm(&[("string.c", BC_STRING)]);
    let func_idx = find_func_idx(&vm, "memcpy");

    let src_addr = 0x3000u32;
    let dst_addr = 0x2000u32;

    for (src_data, n) in [("ABCDEF", 6), ("XYZ", 3)] {
        vm.write_cstring(src_addr, src_data);
        vm.memory_ref_mut()[dst_addr as usize..dst_addr as usize + 16].fill(0);

        // Host 路径
        vm.push(n as u64);
        vm.push(src_addr as u64);
        vm.push(dst_addr as u64);
        host_memcpy(&mut vm, &mut session.as_vm_context());
        let _ = vm.pop();
        let host_result = read_test_string(&vm, dst_addr);

        // Bytecode 路径
        vm.memory_ref_mut()[dst_addr as usize..dst_addr as usize + 16].fill(0);
        let bc_ret = vm
            .call_user_function(
                &mut session.as_vm_context(),
                func_idx,
                &[dst_addr as i32, src_addr as i32, n],
                10000,
            )
            .expect("Bytecode memcpy 调用失败");

        let bc_result = read_test_string(&vm, dst_addr);

        assert_eq!(host_result, bc_result, "memcpy({:?}, {}) 出现分歧", src_data, n);
        assert_eq!(bc_ret as u32, dst_addr, "memcpy 应返回 dest 指针");
    }
}

#[test]
fn test_diff_memmove() {
    let (mut vm, mut session) = prepare_bc_vm(&[("string.c", BC_STRING)]);
    let func_idx = find_func_idx(&vm, "memmove");

    let buf_addr = 0x2000u32;

    let (init, src_off, dst_off, n, _expected) = ("ABCDEF", 0, 2, 4, "ABABCD"); // 重叠：向前拷贝
    vm.write_cstring(buf_addr, init);

    // Host 路径
    vm.push(n as u64);
    vm.push((buf_addr + src_off) as u64);
    vm.push((buf_addr + dst_off) as u64);
    host_memmove(&mut vm, &mut session.as_vm_context());
    let _ = vm.pop();
    let host_result = read_test_string(&vm, buf_addr);

    // Bytecode 路径
    vm.write_cstring(buf_addr, init);
    let bc_ret = vm
        .call_user_function(
            &mut session.as_vm_context(),
            func_idx,
            &[(buf_addr + dst_off) as i32, (buf_addr + src_off) as i32, n],
            10000,
        )
        .expect("Bytecode memmove 调用失败");

    let bc_result = read_test_string(&vm, buf_addr);

    assert_eq!(host_result, bc_result, "memmove 出现分歧");
    assert_eq!(bc_ret as u32, buf_addr + dst_off, "memmove 应返回 dest 指针");
}
