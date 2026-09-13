#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::vm::core::{VitroVM, NULL_TRAP_SIZE};

#[test]
fn test_write_cstring_basic() {
    let mut vm = VitroVM::default();
    let addr = NULL_TRAP_SIZE;
    vm.write_cstring(addr, "hello");
    assert_eq!(&vm.memory_ref()[addr as usize..addr as usize + 6], b"hello\0");
}

#[test]
fn test_write_cstring_at_offset() {
    let mut vm = VitroVM::default();
    let addr = NULL_TRAP_SIZE + 10;
    vm.write_cstring(addr, "world");
    assert_eq!(&vm.memory_ref()[addr as usize..addr as usize + 6], b"world\0");
    // 之前的区域不应被修改
    assert_eq!(vm.memory_ref()[NULL_TRAP_SIZE as usize..addr as usize].iter().sum::<u8>(), 0);
}

#[test]
fn test_write_cstring_exact_fit() {
    let mut vm = VitroVM::default();
    let addr = NULL_TRAP_SIZE + 100;
    vm.write_cstring(addr, "hello");
    // "hello" is 5 bytes + null = 6
    let mem = vm.memory_ref();
    assert_eq!(&mem[addr as usize..addr as usize + 6], b"hello\0");
}

#[test]
fn test_write_cstring_boundary_rejected() {
    let mut vm = VitroVM::default();
    let addr = (vm.get_memory_size() - 2) as usize; // 只剩 2 字节空间
    let before = vm.memory_ref()[addr..].to_vec();
    vm.write_cstring(addr as u32, "hello"); // 5+1=6 字节，超出边界
                                            // 不应写入
    assert_eq!(vm.memory_ref()[addr..], before);
}

#[test]
fn test_write_cstring_offset_boundary() {
    let mut vm = VitroVM::default();
    let addr = (vm.get_memory_size() - 3) as usize;
    let before = vm.memory_ref()[addr..].to_vec();
    // addr + 6 > MEM_SIZE, should not write
    vm.write_cstring(addr as u32, "hello");
    assert_eq!(vm.memory_ref()[addr..], before);
}

#[test]
fn test_write_cstring_empty_string() {
    let mut vm = VitroVM::default();
    let addr = NULL_TRAP_SIZE;
    vm.write_cstring(addr, "");
    // empty string writes just '\0' at addr
    assert_eq!(vm.memory_ref()[addr as usize], 0);
}

#[test]
fn test_write_cstring_null_trap_rejected() {
    let mut vm = VitroVM::default();
    let before = vm.memory_ref()[0..16].to_vec();
    vm.write_cstring(0, "hello"); // NULL 区应拒绝写入
    assert_eq!(&vm.memory_ref()[0..16], &before[..]);
}

#[test]
fn test_write_cstring_chinese() {
    let mut vm = VitroVM::default();
    let s = "你好";
    let addr = NULL_TRAP_SIZE;
    vm.write_cstring(addr, s);
    let base = addr as usize;
    let written = &vm.memory_ref()[base..base + s.len() + 1];
    assert_eq!(&written[..s.len()], s.as_bytes());
    assert_eq!(written[s.len()], 0);
}
