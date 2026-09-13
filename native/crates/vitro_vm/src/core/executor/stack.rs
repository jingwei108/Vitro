use super::*;

impl VitroVM {
    pub(crate) fn execute_stack(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::PushConst => {
                self.push(operand as u64);
            }
            OpCode::PushArgc => {
                self.push(self.argc as u64);
            }
            OpCode::PushArgv => {
                self.push(self.argv_addr as u64);
            }
            OpCode::Pop => {
                self.pop();
            }
            OpCode::Dup => {
                if let Some(&v) = self.stack.last() {
                    self.push(v);
                } else {
                    self.trap("Dup: 栈空", loc);
                }
            }
            OpCode::Swap => {
                let len = self.stack.len();
                if len >= 2 {
                    self.stack.swap(len - 1, len - 2);
                } else {
                    self.trap("Swap: 栈不足", loc);
                }
            }
            _ => {}
        }
    }
    pub(crate) fn execute_local(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::LoadLocal => {
                let var_name = self
                    .local_sym_map
                    .get(&operand)
                    .cloned()
                    .unwrap_or_else(|| format!("local_{}", operand));
                self.last_accessed_vars.push(VariableAccess {
                    name: var_name,
                    access_type: AccessType::Read,
                });
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("LoadLocal: 地址越界", loc);
                    } else {
                        let val = self.load_i32(addr, loc);
                        self.push(val as u64);
                    }
                } else {
                    self.trap("LoadLocal: 无调用帧", loc);
                }
            }
            OpCode::StoreLocal => {
                let var_name = self
                    .local_sym_map
                    .get(&operand)
                    .cloned()
                    .unwrap_or_else(|| format!("local_{}", operand));
                self.last_accessed_vars.push(VariableAccess {
                    name: var_name,
                    access_type: AccessType::Write,
                });
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("StoreLocal: 地址越界", loc);
                    } else {
                        let val = self.pop() as i32;
                        self.store_i32(addr, val, loc);
                    }
                } else {
                    self.trap("StoreLocal: 无调用帧", loc);
                }
            }
            OpCode::LoadLocalD => {
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("LoadLocalD: 地址越界", loc);
                    } else {
                        let val = self.load_i64(addr, loc);
                        self.push(val);
                    }
                } else {
                    self.trap("LoadLocalD: 无调用帧", loc);
                }
            }
            OpCode::StoreLocalD => {
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("StoreLocalD: 地址越界", loc);
                    } else {
                        let val = self.pop();
                        self.store_i64(addr, val, loc);
                    }
                } else {
                    self.trap("StoreLocalD: 无调用帧", loc);
                }
            }
            OpCode::GetFrameBase => {
                if let Some(frame) = self.call_stack.last() {
                    self.push(frame.locals_base as u64);
                } else {
                    self.trap("GetFrameBase: 无调用帧", loc);
                }
            }
            OpCode::LoadLocalQ => {
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("LoadLocalQ: 地址越界", loc);
                    } else {
                        let val = self.load_i64(addr, loc);
                        self.push(val);
                    }
                } else {
                    self.trap("LoadLocalQ: 无调用帧", loc);
                }
            }
            OpCode::StoreLocalQ => {
                if let Some(frame) = self.call_stack.last() {
                    let addr = frame.locals_base + operand as u32;
                    if addr as u64 + 8 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
                        self.trap("StoreLocalQ: 地址越界", loc);
                    } else {
                        let val = self.pop();
                        self.store_i64(addr, val, loc);
                    }
                } else {
                    self.trap("StoreLocalQ: 无调用帧", loc);
                }
            }
            _ => {}
        }
    }
}
