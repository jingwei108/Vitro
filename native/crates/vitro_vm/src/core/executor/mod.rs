use super::*;
use crate::VmContext;

mod arithmetic;
mod control;
pub mod debug;
mod float;
mod memory;
mod stack;

impl VitroVM {
    pub fn run(&mut self, session: &mut VmContext<'_>) -> i32 {
        loop {
            // --- JIT fast path ---
            // 录制期间必须禁用：否则外层 trace 推进到已 JIT 化的内层循环头时，
            // bulk 一次跑完内层，外层 trace 缺失内层指令却被注册（R-2026-09-13 静默错值）。
            // 录制期间逐条 step，遇内层回边由 `record()` Abort，与解释路径等价。
            if self.jit_enabled && !self.trace_recorder.is_recording() {
                if let Some(trace) = self.jit_traces.get(&self.ip).cloned() {
                    let (result, steps) = execute_trace_bulk(self, session, &trace);
                    self.jit_stats.steps_accelerated += steps;
                    if let Some(r) = result {
                        match r {
                            StepResult::Finished => {
                                return if self.finished {
                                    self.exit_code
                                } else {
                                    self.stack.last().copied().unwrap_or(0) as i32
                                };
                            }
                            StepResult::Trap => {
                                self.rollback_pending_array_construction(session);
                                return 0;
                            }
                            StepResult::Paused => {
                                self.trap("完整运行模式下遇到暂停状态（可能是断点配置不一致）", &SourceLoc::default());
                                return 0;
                            }
                            StepResult::WaitingInput => {
                                return 0;
                            }
                            StepResult::Ok => {}
                        }
                    }
                    // trace 正常退出（ip 已离开循环），继续外层调度
                    continue;
                }
            }

            let result = self.step(session);
            match result {
                StepResult::Finished => {
                    return if self.finished {
                        self.exit_code
                    } else {
                        self.stack.last().copied().unwrap_or(0) as i32
                    };
                }
                StepResult::Trap => {
                    self.rollback_pending_array_construction(session);
                    return 0;
                }
                StepResult::Paused => {
                    self.trap("完整运行模式下遇到暂停状态（可能是断点配置不一致）", &SourceLoc::default());
                    return 0;
                }
                StepResult::WaitingInput => {
                    return 0;
                }
                StepResult::Ok => {}
            }
        }
    }

    pub(crate) fn dispatch_single_instruction(
        &mut self,
        op: OpCode,
        operand: i32,
        loc: &SourceLoc,
        session: &mut VmContext<'_>,
    ) -> Option<StepResult> {
        match op {
            OpCode::Nop => {}

            OpCode::PushConst | OpCode::PushArgc | OpCode::PushArgv | OpCode::Pop | OpCode::Dup | OpCode::Swap => {
                self.execute_stack(op, operand, loc);
            }

            OpCode::LoadLocal
            | OpCode::StoreLocal
            | OpCode::LoadLocalD
            | OpCode::StoreLocalD
            | OpCode::LoadLocalQ
            | OpCode::StoreLocalQ
            | OpCode::GetFrameBase => {
                self.execute_local(op, operand, loc);
            }

            OpCode::LoadGlobal
            | OpCode::StoreGlobal
            | OpCode::LoadGlobalD
            | OpCode::StoreGlobalD
            | OpCode::LoadGlobalQ
            | OpCode::StoreGlobalQ => {
                self.execute_global(op, operand, loc);
            }

            OpCode::LoadMem
            | OpCode::StoreMem
            | OpCode::LoadMemD
            | OpCode::StoreMemD
            | OpCode::LoadMemByte
            | OpCode::StoreMemByte
            | OpCode::LoadMemQ
            | OpCode::StoreMemQ
            | OpCode::SplitD
            | OpCode::SplitQ
            | OpCode::StackAlloc
            | OpCode::Memcpy
            | OpCode::Memset
            | OpCode::Strlen => {
                self.execute_memory(op, operand, loc);
            }

            OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Mod
            | OpCode::Neg
            | OpCode::UDiv
            | OpCode::UMod
            | OpCode::USub
            | OpCode::UNeg
            | OpCode::UAdd
            | OpCode::UMul => {
                self.execute_arithmetic(op, operand, loc);
            }

            OpCode::Eq
            | OpCode::Ne
            | OpCode::Lt
            | OpCode::Le
            | OpCode::Gt
            | OpCode::Ge
            | OpCode::And
            | OpCode::Or
            | OpCode::Not
            | OpCode::ULt
            | OpCode::ULe
            | OpCode::UGt
            | OpCode::UGe => {
                self.execute_comparison(op, operand, loc);
            }

            OpCode::BitAnd
            | OpCode::BitOr
            | OpCode::BitXor
            | OpCode::BitNot
            | OpCode::Shl
            | OpCode::Shr
            | OpCode::LShr
            | OpCode::BitAndQ
            | OpCode::BitOrQ
            | OpCode::BitXorQ
            | OpCode::BitNotQ
            | OpCode::ShlQ
            | OpCode::ShrQ
            | OpCode::LShrQ => {
                self.execute_bitwise(op, operand, loc);
            }

            OpCode::PushConstF
            | OpCode::AddF
            | OpCode::SubF
            | OpCode::MulF
            | OpCode::DivF
            | OpCode::NegF
            | OpCode::EqF
            | OpCode::NeF
            | OpCode::LtF
            | OpCode::LeF
            | OpCode::GtF
            | OpCode::GeF
            | OpCode::CastI2F
            | OpCode::CastF2I => {
                self.execute_float(op, operand, loc);
            }

            OpCode::PushConstD
            | OpCode::AddD
            | OpCode::SubD
            | OpCode::MulD
            | OpCode::DivD
            | OpCode::NegD
            | OpCode::CastI2D
            | OpCode::CastF2D
            | OpCode::CastD2I
            | OpCode::CastD2F
            | OpCode::EqD
            | OpCode::NeD
            | OpCode::LtD
            | OpCode::LeD
            | OpCode::GtD
            | OpCode::GeD => {
                self.execute_double(op, operand, loc);
            }

            OpCode::PushConstQ
            | OpCode::AddQ
            | OpCode::SubQ
            | OpCode::MulQ
            | OpCode::DivQ
            | OpCode::ModQ
            | OpCode::NegQ
            | OpCode::CastI2Q
            | OpCode::CastQ2I
            | OpCode::CastQ2D
            | OpCode::CastD2Q
            | OpCode::EqQ
            | OpCode::NeQ
            | OpCode::LtQ
            | OpCode::LeQ
            | OpCode::GtQ
            | OpCode::GeQ => {
                self.execute_longlong(op, operand, loc);
            }

            OpCode::Jump
            | OpCode::JumpIfZero
            | OpCode::JumpIfNotZero
            | OpCode::Call
            | OpCode::CallVar
            | OpCode::CallPtr
            | OpCode::CallHost
            | OpCode::Ret
            | OpCode::RetVoid => {
                return self.execute_control_flow(op, operand, loc, session);
            }

            OpCode::StepEvent | OpCode::TrapBounds | OpCode::TrapBoundsVla => {
                return self.execute_debug(op, operand, loc);
            }
        }

        if !self.error.is_empty() {
            Some(StepResult::Trap)
        } else {
            None
        }
    }

    // --- Step (execute one instruction) ---

    pub fn step(&mut self, session: &mut VmContext<'_>) -> StepResult {
        if self.finished {
            return StepResult::Finished;
        }
        if !self.error.is_empty() {
            return StepResult::Trap;
        }
        if self.ip >= self.code.len() {
            return StepResult::Finished;
        }

        self.step_count = self.step_count.saturating_add(1);
        if self.step_count % SNAPSHOT_INTERVAL == 0 {
            self.snapshot_vars.clear();
            for sym in &self.symbols {
                if matches!(sym.ty.kind(), vitro_ast::TypeKind::Array) {
                    continue;
                }
                self.snapshot_vars.insert(sym.name.clone(), self.read_variable(sym) as u64);
            }
            self.last_snapshot_step = self.step_count;
        }
        if self.step_count >= self.max_steps {
            let msg = self.format_infinite_loop_error();
            self.trap(&msg, &SourceLoc::default());
            self.rollback_pending_array_construction(session);
            return StepResult::Trap;
        }
        if self.cancelled {
            self.trap("执行已取消。", &SourceLoc::default());
            self.rollback_pending_array_construction(session);
            return StepResult::Trap;
        }

        let inst = self.code[self.ip];
        let ip_before = self.ip;
        self.ip += 1;

        // 记录执行热力图：只统计用户主文件（file_id == 0）的源码行号，
        // 避免 Bytecode Libc / 标准库的外部行号混入导致覆盖率超过 100%。
        if inst.loc.line > 0 && inst.loc.file_id == 0 {
            std::sync::Arc::make_mut(&mut session.runtime.heatmap).record(inst.loc.line);
        }

        // 清空上一步的变量访问记录
        self.last_accessed_vars.clear();

        // --- JIT: 热点检测（backward jump 目标计数） ---
        if self.jit_enabled && matches!(inst.op, OpCode::Jump | OpCode::JumpIfZero | OpCode::JumpIfNotZero) {
            let target = inst.operand as usize;
            if target < self.ip {
                *self.ip_hits.entry(target).or_insert(0) += 1;
            }
        }

        // --- JIT: trace 录制触发 ---
        if self.jit_enabled && !self.trace_recorder.is_recording() && !self.jit_traces.contains_key(&ip_before) {
            if let Some(&hits) = self.ip_hits.get(&ip_before) {
                if hits >= JIT_THRESHOLD {
                    self.trace_recorder.start(ip_before);
                }
            }
        }

        // 执行指令
        if let Some(r) = self.dispatch_single_instruction(inst.op, inst.operand, &inst.loc, session) {
            // trace 录制：遇到中断指令时取消
            if self.trace_recorder.is_recording() {
                self.trace_recorder.reset();
            }
            if matches!(r, StepResult::Trap) {
                self.rollback_pending_array_construction(session);
            }
            return r;
        }

        // --- JIT: trace 录制 ---
        if self.trace_recorder.is_recording() {
            match self.trace_recorder.record(inst, ip_before, self.ip) {
                crate::jit_trace::RecordResult::Continue => {}
                crate::jit_trace::RecordResult::Finish => {
                    if let Some(trace) = self.trace_recorder.finish(false) {
                        let compiled = crate::jit_templates::compile_trace(&trace);
                        self.jit_traces.insert(trace.start_ip, Arc::new(compiled));
                        self.jit_stats.traces_compiled += 1;
                    }
                }
                crate::jit_trace::RecordResult::Abort => {
                    // Abort 时丢弃已录制内容，避免生成不完整的 trace。
                    let _ = self.trace_recorder.finish(true);
                }
            }
        }

        if !self.error.is_empty() {
            self.rollback_pending_array_construction(session);
            StepResult::Trap
        } else {
            StepResult::Ok
        }
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::*;
    use crate::context::VmContext;
    use crate::instruction::{Instruction, SourceLoc};
    use crate::opcode::OpCode;

    fn exec_single(vm: &mut VitroVM, session: &mut VmContext<'_>, op: OpCode) {
        vm.set_test_code(vec![Instruction::new(op, 0, SourceLoc { line: 1, column: 1, file_id: 0 })]);
        let result = vm.step(session);
        assert!(
            !vm.has_error() || matches!(result, StepResult::Trap),
            "指令意外失败: {}",
            vm.get_error()
        );
    }

    #[test]
    fn test_builtin_strlen() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let addr = 0x2000u32;
        let s = b"hello\0";
        vm.memory_ref_mut()[addr as usize..addr as usize + s.len()].copy_from_slice(s);
        vm.push(addr as u64);
        exec_single(&mut vm, &mut session, OpCode::Strlen);
        assert_eq!(vm.get_stack().last().copied().unwrap() as usize, 5);
    }

    #[test]
    fn test_builtin_strlen_empty() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let addr = 0x2000u32;
        vm.memory_ref_mut()[addr as usize] = 0;
        vm.push(addr as u64);
        exec_single(&mut vm, &mut session, OpCode::Strlen);
        assert_eq!(vm.get_stack().last().copied().unwrap() as usize, 0);
    }

    #[test]
    fn test_builtin_strlen_out_of_bounds() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        vm.push((1024 * 1024) as u64); // MEM_SIZE
        exec_single(&mut vm, &mut session, OpCode::Strlen);
        assert_eq!(vm.get_stack().last().copied().unwrap() as usize, 0);
    }

    #[test]
    fn test_builtin_memset() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let addr = 0x2000u32;
        // 参数入栈顺序与 codegen 一致（从右到左：size, value, ptr）
        vm.push(5); // size
        vm.push(0x41); // value ('A')
        vm.push(addr as u64); // ptr
        exec_single(&mut vm, &mut session, OpCode::Memset);
        assert_eq!(vm.get_stack().last().copied().unwrap() as u32, addr);
        let mem = vm.memory_ref();
        for i in 0..5 {
            assert_eq!(mem[addr as usize + i], b'A');
        }
    }

    #[test]
    fn test_builtin_memset_null_trap() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let addr = 0x100u32; // NULL trap area
        vm.push(5);
        vm.push(0x41);
        vm.push(addr as u64);
        exec_single(&mut vm, &mut session, OpCode::Memset);
        assert!(vm.has_error(), "Memset 应对 NULL 指针区域触发 trap");
    }

    #[test]
    fn test_builtin_memcpy() {
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let src = 0x2000u32;
        let dst = 0x3000u32;
        let data = b"world\0";
        vm.memory_ref_mut()[src as usize..src as usize + data.len()].copy_from_slice(data);
        // 参数入栈顺序与 codegen 一致（从右到左：n, src, dest）
        vm.push(5); // n
        vm.push(src as u64); // src
        vm.push(dst as u64); // dest
        exec_single(&mut vm, &mut session, OpCode::Memcpy);
        assert_eq!(vm.get_stack().last().copied().unwrap() as u32, dst);
        let mem = vm.memory_ref();
        assert_eq!(&mem[dst as usize..dst as usize + 5], b"world");
    }

    #[test]
    fn test_builtin_memcpy_overlap_safety() {
        // Builtin Memcpy 不做重叠处理（与 host_memcpy 一致，未定义行为）。
        // 此处仅验证它能正确复制无重叠区域。
        let mut vm = VitroVM::new();
        let mut runtime = vitro_runtime::RuntimeState::default();
        let mut memory = vitro_runtime::MemoryState::default();
        let mut vfs = crate::vfs::VirtualFileSystem::default();
        let mut session = VmContext::new(&mut runtime, &mut memory, &mut vfs);
        let src = 0x2000u32;
        let dst = 0x2005u32;
        let data = b"abcde";
        vm.memory_ref_mut()[src as usize..src as usize + data.len()].copy_from_slice(data);
        vm.push(5);
        vm.push(src as u64);
        vm.push(dst as u64);
        exec_single(&mut vm, &mut session, OpCode::Memcpy);
        let mem = vm.memory_ref();
        assert_eq!(&mem[dst as usize..dst as usize + 5], b"abcde");
    }
}
