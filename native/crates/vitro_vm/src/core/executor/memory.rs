use super::*;

impl VitroVM {
    pub(crate) fn execute_global(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::LoadGlobal => {
                let var_name = self
                    .global_sym_map
                    .get(&operand)
                    .cloned()
                    .unwrap_or_else(|| format!("global_{}", operand));
                self.last_accessed_vars.push(VariableAccess {
                    name: var_name,
                    access_type: AccessType::Read,
                });
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("LoadGlobal: 地址越界", loc);
                } else {
                    let val = self.load_i32(addr, loc);
                    self.push(val as u64);
                }
            }
            OpCode::StoreGlobal => {
                let var_name = self
                    .global_sym_map
                    .get(&operand)
                    .cloned()
                    .unwrap_or_else(|| format!("global_{}", operand));
                self.last_accessed_vars.push(VariableAccess {
                    name: var_name,
                    access_type: AccessType::Write,
                });
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("StoreGlobal: 地址越界", loc);
                } else {
                    let val = self.pop() as i32;
                    self.store_i32(addr, val, loc);
                }
            }
            OpCode::LoadGlobalD => {
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("LoadGlobalD: 地址越界", loc);
                } else {
                    let val = self.load_i64(addr, loc);
                    self.push(val);
                }
            }
            OpCode::StoreGlobalD => {
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("StoreGlobalD: 地址越界", loc);
                } else {
                    let val = self.pop();
                    self.store_i64(addr, val, loc);
                }
            }
            OpCode::LoadGlobalQ => {
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("LoadGlobalQ: 地址越界", loc);
                } else {
                    let val = self.load_i64(addr, loc);
                    self.push(val);
                }
            }
            OpCode::StoreGlobalQ => {
                let addr = GLOBAL_START + operand as u32;
                if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                    self.trap("StoreGlobalQ: 地址越界", loc);
                } else {
                    let val = self.pop();
                    self.store_i64(addr, val, loc);
                }
            }
            _ => {}
        }
    }
    pub(crate) fn execute_memory(&mut self, op: OpCode, _operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::LoadMem => {
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 4) {
                    let msg = self.format_uaf_message(log, false);
                    self.trap(&msg, loc);
                    return;
                }
                let val = self.load_i32(addr, loc);
                self.push(val as u64);
            }
            OpCode::StoreMem => {
                let val = self.pop() as i32;
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 4) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    return;
                }
                self.store_i32(addr, val, loc);
            }
            OpCode::LoadMemD => {
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 8) {
                    let msg = self.format_uaf_message(log, false);
                    self.trap(&msg, loc);
                    return;
                }
                let val = self.load_i64(addr, loc);
                self.push(val);
            }
            OpCode::StoreMemD => {
                let val = self.pop();
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 8) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    return;
                }
                self.store_i64(addr, val, loc);
            }
            OpCode::SplitD => {
                let val = self.pop();
                let low = (val & 0xFFFFFFFF) as i32;
                let high = ((val >> 32) & 0xFFFFFFFF) as i32;
                self.push(low as u64);
                self.push(high as u64);
            }
            OpCode::LoadMemByte => {
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 1) {
                    let msg = self.format_uaf_message(log, false);
                    self.trap(&msg, loc);
                    return;
                }
                let val = self.load_i8(addr, loc);
                self.push(val as u64);
            }
            OpCode::StoreMemByte => {
                let val = self.pop() as i32;
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 1) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    return;
                }
                self.store_i8(addr, val, loc);
            }
            OpCode::LoadMemQ => {
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 8) {
                    let msg = self.format_uaf_message(log, false);
                    self.trap(&msg, loc);
                    return;
                }
                let val = self.load_i64(addr, loc);
                self.push(val);
            }
            OpCode::StoreMemQ => {
                let val = self.pop();
                let addr = self.pop() as u32;
                if let Some(log) = self.check_uaf(addr, 8) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    return;
                }
                self.store_i64(addr, val, loc);
            }
            OpCode::SplitQ => {
                let val = self.pop();
                let low = (val & 0xFFFFFFFF) as i32;
                let high = ((val >> 32) & 0xFFFFFFFF) as i32;
                self.push(low as u64);
                self.push(high as u64);
            }
            OpCode::StackAlloc => {
                let size = self.pop() as i32;
                if size < 0 {
                    self.trap("StackAlloc: 分配的栈空间大小为负数", loc);
                    return;
                }
                let aligned_size = ((size + 3) & !3) as u32;
                if aligned_size > self.mem_stack_top {
                    self.trap("StackAlloc: 栈溢出", loc);
                    return;
                }
                if self.mem_stack_top - aligned_size < NULL_TRAP_SIZE {
                    self.trap("StackAlloc: 栈溢出（触及保留区）", loc);
                    return;
                }
                self.mem_stack_top -= aligned_size;
                self.push(self.mem_stack_top as u64);
            }
            OpCode::Memcpy => {
                let dest = self.pop() as u32;
                let src = self.pop() as u32;
                let n = self.pop();
                let n_u32 = n as u32;
                // 统一边界检查：NULL 区、上界、UAF。
                if !self.check_mem_access(dest, n_u32, loc, true) || !self.check_mem_access(src, n_u32, loc, false) {
                    self.push(dest as u64);
                    return;
                }
                if let Some(log) = self.check_uaf(dest, n_u32) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    self.push(dest as u64);
                    return;
                }
                if let Some(log) = self.check_uaf(src, n_u32) {
                    let msg = self.format_uaf_message(log, false);
                    self.trap(&msg, loc);
                    self.push(dest as u64);
                    return;
                }
                let mem_size = self.memory_ref().len();
                let copy_len = (n as usize).min(mem_size - dest as usize).min(mem_size - src as usize);
                if copy_len > 0 {
                    let buf = {
                        let mem = self.memory_ref();
                        mem[src as usize..src as usize + copy_len].to_vec()
                    };
                    let mem = self.memory_ref_mut();
                    for i in 0..copy_len {
                        mem[dest as usize + i] = buf[i];
                    }
                }
                self.push(dest as u64);
            }
            OpCode::Memset => {
                let ptr = self.pop() as u32;
                let value = self.pop();
                let size = self.pop();
                let size_u32 = size as u32;
                // 统一边界检查：NULL 区、上界、UAF；越界时 trap 而非静默截断。
                if !self.check_mem_access(ptr, size_u32, loc, true) {
                    self.push(ptr as u64);
                    return;
                }
                if let Some(log) = self.check_uaf(ptr, size_u32) {
                    let msg = self.format_uaf_message(log, true);
                    self.trap(&msg, loc);
                    self.push(ptr as u64);
                    return;
                }
                let mem_size = self.memory_ref().len();
                let max_write = mem_size - ptr as usize;
                let write_len = (size as usize).min(max_write);
                let byte_val = (value & 0xFF) as u8;
                let mem = self.memory_ref_mut();
                mem[ptr as usize..ptr as usize + write_len].fill(byte_val);
                self.push(ptr as u64);
            }
            OpCode::Strlen => {
                let addr = self.pop() as u32;
                // 统一 NULL 区检查；上界在扫描时自然处理。
                if addr < NULL_TRAP_SIZE {
                    self.trap(
                        &format!(
                            "访问了 NULL 指针区域（地址 0x{:04X}）。NULL 指针不能解引用。请确认指针已被正确初始化。",
                            addr
                        ),
                        loc,
                    );
                    self.push(0);
                } else {
                    let mem = self.memory_ref();
                    let start = addr as usize;
                    if start >= mem.len() {
                        self.push(0);
                    } else {
                        let len = mem[start..].iter().take_while(|&&b| b != 0).count();
                        self.push(len as u64);
                    }
                }
            }
            _ => {}
        }
    }
}
