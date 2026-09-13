use crate::instruction::Instruction;
use crate::opcode::OpCode;
use std::collections::HashMap;

pub const JIT_THRESHOLD: u64 = 100;
pub const MAX_TRACE_LEN: usize = 256;
pub const MAX_TRACE_ITERATIONS: usize = 1000;

/// JIT 执行统计
#[derive(Debug, Clone, Default)]
pub struct JitStats {
    pub traces_compiled: u32,
    pub steps_accelerated: u64,
    pub template_hits: HashMap<String, u64>,
}

/// 录制的原始指令 trace
#[derive(Debug, Clone)]
pub struct JitTrace {
    pub start_ip: usize,
    /// trace 正常结束后应到达的下一条指令地址。
    /// 用于条件跳转（JumpIfZero/JumpIfNotZero）为假时正确退出循环。
    pub end_ip: usize,
    pub instructions: Vec<Instruction>,
}

/// Trace 录制器
#[derive(Debug, Clone)]
pub struct TraceRecorder {
    start_ip: usize,
    end_ip: usize,
    instructions: Vec<Instruction>,
    recording: bool,
}

/// trace 录制结果
pub enum RecordResult {
    Continue,
    Finish,
    Abort,
}

impl Default for TraceRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceRecorder {
    pub fn new() -> Self {
        Self {
            start_ip: 0,
            end_ip: 0,
            instructions: Vec::new(),
            recording: false,
        }
    }

    pub fn start(&mut self, ip: usize) {
        self.start_ip = ip;
        self.end_ip = ip;
        self.instructions.clear();
        self.recording = true;
    }

    /// 尝试将一条指令录入 trace。
    ///
    /// - `Continue` — 录制成功，继续下一条
    /// - `Finish`   — 自然结束（backward jump 回到起点），可以编译
    /// - `Abort`    — 遇到不应录制的指令或路径分叉，trace 应被丢弃
    pub fn record(&mut self, inst: Instruction, ip: usize, next_ip: usize) -> RecordResult {
        if !self.recording {
            return RecordResult::Abort;
        }

        // 跨函数调用不应出现在 trace 中
        if matches!(
            inst.op,
            OpCode::Call | OpCode::CallPtr | OpCode::CallHost | OpCode::Ret | OpCode::RetVoid
        ) {
            self.recording = false;
            return RecordResult::Abort;
        }

        // StepEvent 是透明指令：不录进 trace，也不终止录制
        if inst.op == OpCode::StepEvent {
            return RecordResult::Continue;
        }

        if self.instructions.len() >= MAX_TRACE_LEN {
            self.recording = false;
            // U1#5：录满上限时循环体尚未闭合（backward jump 未回到起点），
            // 半截 trace 的栈效应不平衡，编译重放会导致值栈溢出
            // （`for(...){int a[12];...}` × 300 实锤 TRAP）。丢弃而非注册。
            return RecordResult::Abort;
        }

        self.instructions.push(inst);
        self.end_ip = ip + 1;

        // 判断跳转行为：
        // 1. backward jump 回到起点 → 完成循环体录制
        // 2. 任何其他跳转被 taken（next_ip != ip+1 且不是回到起点）→ 路径分叉，abort
        if matches!(inst.op, OpCode::Jump | OpCode::JumpIfZero | OpCode::JumpIfNotZero) {
            if next_ip == self.start_ip {
                self.recording = false;
                return RecordResult::Finish;
            }
            if next_ip != ip + 1 {
                self.recording = false;
                return RecordResult::Abort;
            }
        }

        RecordResult::Continue
    }

    /// 结束录制并取出 trace。
    /// 若 trace 太短（< 2 条指令）则返回 None。
    /// 结束录制并取出 trace。
    /// 若 trace 太短（< 2 条指令）或录制以 Abort 结束时返回 None。
    pub fn finish(&mut self, aborted: bool) -> Option<JitTrace> {
        self.recording = false;
        if aborted || self.instructions.len() < 2 {
            self.instructions.clear();
            return None;
        }
        let trace = JitTrace {
            start_ip: self.start_ip,
            end_ip: self.end_ip,
            instructions: std::mem::take(&mut self.instructions),
        };
        Some(trace)
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn reset(&mut self) {
        self.recording = false;
        self.instructions.clear();
    }
}
