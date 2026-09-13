//! 模板 JIT：将 JitTrace 编译为预优化的函数指针序列（超级指令）。
//!
//! 由于 crate 启用 `#![forbid(unsafe_code)]`，无法动态生成机器码。
//! 此处采用安全 Rust 内的 "模板超级指令" 策略：
//! 常见字节码模式（如 i++、数组访问）直接映射到预编译的 Rust 函数，
//! 跳过两层 match 分支；不匹配的指令回退到 `dispatch_single_instruction`。

use crate::context::VmContext;
use crate::core::{VitroVM, StepResult, MEM_SIZE, NULL_TRAP_SIZE};
use crate::instruction::SourceLoc;
use crate::jit_trace::{JitTrace, MAX_TRACE_ITERATIONS};
use crate::opcode::OpCode;

pub type JitFn = fn(&mut VitroVM, i32, i32, &SourceLoc, &mut VmContext<'_>) -> Option<StepResult>;

/// 编译后的 trace 条目
pub struct JitEntry {
    pub func: JitFn,
    pub arg0: i32,
    pub arg1: i32,
    pub loc: SourceLoc,
    /// 是否为条件跳转（JumpIfZero / JumpIfNotZero）。
    /// 用于 bulk 执行时区分循环继续/退出，避免依赖函数指针地址比较。
    pub is_conditional_jump: bool,
}

/// 编译后的 trace
pub struct CompiledTrace {
    pub start_ip: usize,
    /// trace 正常结束后应到达的下一条指令地址。
    pub end_ip: usize,
    pub entries: Vec<JitEntry>,
}

// =============================================================================
// 模板函数（按 opcode 分类）
// =============================================================================

fn tpl_nop(_vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    None
}

fn tpl_push_const(
    vm: &mut VitroVM,
    val: i32,
    _b: i32,
    _loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    vm.push(val as u64);
    None
}

fn tpl_pop(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    vm.pop();
    None
}

fn tpl_dup(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    if let Some(&v) = vm.get_stack().last() {
        vm.push(v);
    } else {
        vm.trap("Dup: 栈空", loc);
        return Some(StepResult::Trap);
    }
    None
}

fn tpl_swap(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let len = vm.get_stack().len();
    if len >= 2 {
        // SAFETY: 我们在安全 Rust 内，但无法直接修改私有字段。
        // 通过 vm 提供的公共/内部方法操作。
        let a = vm.pop();
        let b = vm.pop();
        vm.push(a);
        vm.push(b);
    } else {
        vm.trap("Swap: 栈不足", loc);
        return Some(StepResult::Trap);
    }
    None
}

// --- Local ---

fn tpl_load_local(
    vm: &mut VitroVM,
    offset: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    if let Some(frame) = vm.get_call_stack().last() {
        let addr = frame.locals_base + offset as u32;
        if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
            vm.trap("LoadLocal: 地址越界", loc);
            return Some(StepResult::Trap);
        }
        let val = vm.load_i32(addr, loc);
        vm.push(val as u64);
    } else {
        vm.trap("LoadLocal: 无调用帧", loc);
        return Some(StepResult::Trap);
    }
    None
}

fn tpl_store_local(
    vm: &mut VitroVM,
    offset: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    if let Some(frame) = vm.get_call_stack().last() {
        let addr = frame.locals_base + offset as u32;
        if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
            vm.trap("StoreLocal: 地址越界", loc);
            return Some(StepResult::Trap);
        }
        let val = vm.pop() as i32;
        vm.store_i32(addr, val, loc);
    } else {
        vm.trap("StoreLocal: 无调用帧", loc);
        return Some(StepResult::Trap);
    }
    None
}

fn tpl_get_frame_base(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    if let Some(frame) = vm.get_call_stack().last() {
        vm.push(frame.locals_base as u64);
    } else {
        vm.trap("GetFrameBase: 无调用帧", loc);
        return Some(StepResult::Trap);
    }
    None
}

// --- Global ---

fn tpl_load_global(
    vm: &mut VitroVM,
    offset: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let addr = crate::core::GLOBAL_START + offset as u32;
    if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
        vm.trap("LoadGlobal: 地址越界", loc);
        return Some(StepResult::Trap);
    }
    let val = vm.load_i32(addr, loc);
    vm.push(val as u64);
    None
}

fn tpl_store_global(
    vm: &mut VitroVM,
    offset: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let addr = crate::core::GLOBAL_START + offset as u32;
    if addr as u64 + 4 > MEM_SIZE as u64 || addr < NULL_TRAP_SIZE {
        vm.trap("StoreGlobal: 地址越界", loc);
        return Some(StepResult::Trap);
    }
    let val = vm.pop() as i32;
    vm.store_i32(addr, val, loc);
    None
}

// --- Memory ---

fn tpl_load_mem(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let addr = vm.pop() as u32;
    let val = vm.load_i32(addr, loc);
    vm.push(val as u64);
    None
}

fn tpl_store_mem(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    // 注意：解释器 executor.rs 中 StoreMem 的栈约定是“值在下、地址在上”，
    // 因此模板必须同样先 pop 值，再 pop 地址，否则 JIT 加速下的数组赋值会
    // 把数值当成目标地址写入 NULL trap 区。
    let val = vm.pop() as i32;
    let addr = vm.pop() as u32;
    vm.store_i32(addr, val, loc);
    None
}

fn tpl_load_mem_byte(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let addr = vm.pop() as u32;
    let val = vm.load_i8(addr, loc);
    vm.push(val as u64);
    None
}

fn tpl_store_mem_byte(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    // 与 StoreMem 保持一致：先 pop 值，再 pop 地址。
    let val = vm.pop() as i32;
    let addr = vm.pop() as u32;
    vm.store_i8(addr, val, loc);
    None
}

// --- Arithmetic (i32) ---

fn tpl_add(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(a.wrapping_add(b) as u64);
    None
}

fn tpl_sub(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(a.wrapping_sub(b) as u64);
    None
}

fn tpl_mul(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(a.wrapping_mul(b) as u64);
    None
}

fn tpl_div(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    if b == 0 {
        vm.trap("除零错误：整数除法的除数不能为 0。", loc);
        return Some(StepResult::Trap);
    }
    if a == i32::MIN && b == -1 {
        vm.trap("整数除法溢出。INT_MIN / -1 的结果超出了 int 能表示的范围。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a / b) as u64);
    None
}

fn tpl_mod(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    if b == 0 {
        vm.trap("取模错误：取模运算的除数不能为 0。", loc);
        return Some(StepResult::Trap);
    }
    if a == i32::MIN && b == -1 {
        vm.trap("整数取模溢出。INT_MIN % -1 的结果未定义，超出了 int 能表示的范围。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a % b) as u64);
    None
}

fn tpl_neg(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let a = vm.pop() as i32;
    if a == i32::MIN {
        vm.trap("整数取反溢出。-INT_MIN 的结果超出了 int 能表示的范围。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((-a) as u64);
    None
}

fn tpl_uneg(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let a = vm.pop() as u32;
    vm.push(a.wrapping_neg() as u64);
    None
}

// --- Comparison / Logic ---

fn tpl_eq(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a == b { 1 } else { 0 });
    None
}

fn tpl_ne(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a != b { 1 } else { 0 });
    None
}

fn tpl_lt(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a < b { 1 } else { 0 });
    None
}

fn tpl_le(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a <= b { 1 } else { 0 });
    None
}

fn tpl_gt(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a > b { 1 } else { 0 });
    None
}

fn tpl_ge(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a >= b { 1 } else { 0 });
    None
}

fn tpl_and(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a != 0 && b != 0 { 1 } else { 0 });
    None
}

fn tpl_or(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push(if a != 0 || b != 0 { 1 } else { 0 });
    None
}

fn tpl_not(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let a = vm.pop() as i32;
    vm.push(if a == 0 { 1 } else { 0 });
    None
}

// --- Bitwise ---

fn tpl_bit_and(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    _loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push((a & b) as u64);
    None
}

fn tpl_bit_or(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push((a | b) as u64);
    None
}

fn tpl_bit_xor(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    _loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    vm.push((a ^ b) as u64);
    None
}

fn tpl_bit_not(
    vm: &mut VitroVM,
    _a: i32,
    _b: i32,
    _loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let a = vm.pop() as i32;
    vm.push((!a) as u64);
    None
}

fn tpl_shl(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    if !(0..32).contains(&b) {
        vm.trap("移位操作越界：左移位数必须在 0~31 之间。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a.wrapping_shl(b as u32)) as u64);
    None
}

fn tpl_shr(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as i32;
    if !(0..32).contains(&b) {
        vm.trap("移位操作越界：右移位数必须在 0~31 之间。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a.wrapping_shr(b as u32)) as u64);
    None
}

fn tpl_ult(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    vm.push(if a < b { 1 } else { 0 });
    None
}

fn tpl_ule(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    vm.push(if a <= b { 1 } else { 0 });
    None
}

fn tpl_ugt(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    vm.push(if a > b { 1 } else { 0 });
    None
}

fn tpl_uge(vm: &mut VitroVM, _a: i32, _b: i32, _loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    vm.push(if a >= b { 1 } else { 0 });
    None
}

fn tpl_udiv(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    if b == 0 {
        vm.trap("除零错误：整数除法的除数不能为 0。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a / b) as u64);
    None
}

fn tpl_umod(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as u32;
    let a = vm.pop() as u32;
    if b == 0 {
        vm.trap("取模错误：取模运算的除数不能为 0。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a % b) as u64);
    None
}

fn tpl_lshr(vm: &mut VitroVM, _a: i32, _b: i32, loc: &SourceLoc, _session: &mut VmContext<'_>) -> Option<StepResult> {
    let b = vm.pop() as i32;
    let a = vm.pop() as u32;
    if !(0..32).contains(&b) {
        vm.trap("移位操作越界：右移位数必须在 0~31 之间。", loc);
        return Some(StepResult::Trap);
    }
    vm.push((a.wrapping_shr(b as u32)) as u64);
    None
}

// --- Control Flow ---

fn tpl_jump(
    vm: &mut VitroVM,
    target: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let t = target as usize;
    if t >= vm.code_len() {
        vm.trap(&format!("Jump 目标越界：{}（代码长度：{}）", t, vm.code_len()), loc);
        return Some(StepResult::Trap);
    }
    vm.set_ip(t);
    None
}

fn tpl_jump_if_zero(
    vm: &mut VitroVM,
    target: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let val = vm.pop();
    if val == 0 {
        let t = target as usize;
        if t >= vm.code_len() {
            vm.trap(&format!("JumpIfZero 目标越界：{}（代码长度：{}）", t, vm.code_len()), loc);
            return Some(StepResult::Trap);
        }
        vm.set_ip(t);
        // Side-exit：跳转被 taken，退出 trace，回到正常解释执行
        return Some(StepResult::Ok);
    }
    None
}

fn tpl_jump_if_not_zero(
    vm: &mut VitroVM,
    target: i32,
    _b: i32,
    loc: &SourceLoc,
    _session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let val = vm.pop();
    if val != 0 {
        let t = target as usize;
        if t >= vm.code_len() {
            vm.trap(&format!("JumpIfNotZero 目标越界：{}（代码长度：{}）", t, vm.code_len()), loc);
            return Some(StepResult::Trap);
        }
        vm.set_ip(t);
        // Side-exit：跳转被 taken，退出 trace，回到正常解释执行
        return Some(StepResult::Ok);
    }
    None
}

// --- Generic fallback ---

/// 对于尚未编写专用模板的 opcode，回退到 VM 的内部分发逻辑。
fn tpl_generic(
    vm: &mut VitroVM,
    operand: i32,
    opcode_val: i32,
    loc: &SourceLoc,
    session: &mut VmContext<'_>,
) -> Option<StepResult> {
    let op = OpCode::from_u8(opcode_val as u8)?;
    vm.dispatch_single_instruction(op, operand, loc, session)
}

// =============================================================================
// Opcode → 模板函数 映射
// =============================================================================

fn opcode_to_jit_fn(op: OpCode) -> JitFn {
    match op {
        OpCode::Nop => tpl_nop,
        OpCode::PushConst => tpl_push_const,
        OpCode::Pop => tpl_pop,
        OpCode::Dup => tpl_dup,
        OpCode::Swap => tpl_swap,
        OpCode::LoadLocal => tpl_load_local,
        OpCode::StoreLocal => tpl_store_local,
        OpCode::GetFrameBase => tpl_get_frame_base,
        OpCode::LoadGlobal => tpl_load_global,
        OpCode::StoreGlobal => tpl_store_global,
        OpCode::LoadMem => tpl_load_mem,
        OpCode::StoreMem => tpl_store_mem,
        OpCode::LoadMemByte => tpl_load_mem_byte,
        OpCode::StoreMemByte => tpl_store_mem_byte,
        OpCode::Add => tpl_add,
        OpCode::UAdd => tpl_add,
        OpCode::Sub => tpl_sub,
        OpCode::USub => tpl_sub,
        OpCode::Mul => tpl_mul,
        OpCode::UMul => tpl_mul,
        OpCode::Div => tpl_div,
        OpCode::Mod => tpl_mod,
        OpCode::Neg => tpl_neg,
        OpCode::UNeg => tpl_uneg,
        OpCode::Eq => tpl_eq,
        OpCode::Ne => tpl_ne,
        OpCode::Lt => tpl_lt,
        OpCode::Le => tpl_le,
        OpCode::Gt => tpl_gt,
        OpCode::Ge => tpl_ge,
        OpCode::And => tpl_and,
        OpCode::Or => tpl_or,
        OpCode::Not => tpl_not,
        OpCode::BitAnd => tpl_bit_and,
        OpCode::BitOr => tpl_bit_or,
        OpCode::BitXor => tpl_bit_xor,
        OpCode::BitNot => tpl_bit_not,
        OpCode::Shl => tpl_shl,
        OpCode::Shr => tpl_shr,
        OpCode::ULt => tpl_ult,
        OpCode::ULe => tpl_ule,
        OpCode::UGt => tpl_ugt,
        OpCode::UGe => tpl_uge,
        OpCode::UDiv => tpl_udiv,
        OpCode::UMod => tpl_umod,
        OpCode::LShr => tpl_lshr,
        OpCode::Jump => tpl_jump,
        OpCode::JumpIfZero => tpl_jump_if_zero,
        OpCode::JumpIfNotZero => tpl_jump_if_not_zero,
        // Float / Double / LongLong / Call / Ret / StepEvent 等回退到 generic
        _ => tpl_generic,
    }
}

// =============================================================================
// Trace 编译
// =============================================================================

pub fn compile_trace(trace: &JitTrace) -> CompiledTrace {
    let mut entries = Vec::with_capacity(trace.instructions.len());
    for inst in &trace.instructions {
        let func = opcode_to_jit_fn(inst.op);
        // 对于 generic fallback，arg1 携带 opcode 的 u8 值
        let arg1 = if func as usize == (tpl_generic as *const ()) as usize {
            inst.op as u8 as i32
        } else {
            0
        };
        let is_conditional_jump = matches!(inst.op, OpCode::JumpIfZero | OpCode::JumpIfNotZero);
        entries.push(JitEntry {
            func,
            arg0: inst.operand,
            arg1,
            loc: inst.loc,
            is_conditional_jump,
        });
    }
    CompiledTrace {
        start_ip: trace.start_ip,
        end_ip: trace.end_ip,
        entries,
    }
}

// =============================================================================
// Trace 执行
// =============================================================================

/// 执行一条编译后的 trace。
///
/// 返回值：
/// - `Some(StepResult)` — trace 执行过程中遇到 Trap / Paused / Finished / WaitingInput
/// - `None` — trace 正常完成一轮（此时 `vm.ip` 可能回到 trace 起点，也可能前进到 trace 之外）
pub fn execute_trace_once(vm: &mut VitroVM, session: &mut VmContext<'_>, trace: &CompiledTrace) -> Option<StepResult> {
    for entry in &trace.entries {
        if let Some(result) = (entry.func)(vm, entry.arg0, entry.arg1, &entry.loc, session) {
            return Some(result);
        }
        if !vm.get_error().is_empty() {
            return Some(StepResult::Trap);
        }
    }
    None
}

/// 批量执行 trace，直到循环条件不满足或遇到外部事件。
///
/// 返回值：`(Option<StepResult>, 加速步数)`
/// 这是 JIT 的核心加速逻辑：一次函数调用执行多轮循环迭代，
/// 跳过逐步的辅助操作（heatmap、last_accessed_vars、step_count 逐次递增）。
pub fn execute_trace_bulk(
    vm: &mut VitroVM,
    session: &mut VmContext<'_>,
    trace: &CompiledTrace,
) -> (Option<StepResult>, u64) {
    let start_ip = trace.start_ip;
    let steps_per_iter = trace.entries.len() as u64;
    let entry_steps = trace.entries.len() as i32;
    let mut total_steps = 0u64;
    let ends_with_conditional = trace.entries.last().is_some_and(|e| e.is_conditional_jump);

    for _ in 0..MAX_TRACE_ITERATIONS {
        // 执行一轮 trace
        match execute_trace_once(vm, session, trace) {
            Some(StepResult::Ok) => {
                // 条件跳转被 taken：
                // - 若跳回 trace 起点，循环继续；
                // - 若跳到其他地址（如循环外），退出 bulk。
                if vm.get_ip() != start_ip {
                    total_steps += steps_per_iter;
                    return (None, total_steps);
                }
            }
            Some(r) => {
                // Trap / Paused / Finished / WaitingInput 等真实外部事件，退出 bulk。
                total_steps += steps_per_iter;
                return (Some(r), total_steps);
            }
            None => {
                // trace 正常执行完，ip 未被任何模板函数修改。
                if vm.get_ip() != start_ip {
                    // 例如 trace 中间包含 unconditional Jump 离开了循环
                    total_steps += steps_per_iter;
                    return (None, total_steps);
                }
                // ip 仍在起点：
                // - 若 trace 以条件跳转结尾，说明条件为假（未 taken），循环结束，
                //   需将 ip 推进到 trace 结束后的第一条指令。
                // - 否则是无条件跳转回起点，继续下一轮循环。
                if ends_with_conditional {
                    vm.set_ip(trace.end_ip);
                    total_steps += steps_per_iter;
                    return (None, total_steps);
                }
            }
        }

        // 批量检查 step_count / cancelled
        if !vm.bulk_step_check(entry_steps) {
            total_steps += steps_per_iter;
            return (Some(StepResult::Trap), total_steps);
        }

        total_steps += steps_per_iter;
    }

    total_steps += steps_per_iter * MAX_TRACE_ITERATIONS as u64;
    (None, total_steps)
}
