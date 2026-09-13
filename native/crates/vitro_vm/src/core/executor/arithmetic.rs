use super::*;

impl VitroVM {
    pub(crate) fn execute_arithmetic(&mut self, op: OpCode, _operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::Add => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                let r = (a as i64) + (b as i64);
                if r > i32::MAX as i64 || r < i32::MIN as i64 {
                    self.trap("整数加法溢出。两个很大的正数（或很小的负数）相加超出了 int 能表示的范围。", loc);
                } else {
                    self.push(r as u64);
                }
            }
            OpCode::UAdd => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(a.wrapping_add(b) as u64);
            }
            OpCode::Sub => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                let r = (a as i64) - (b as i64);
                if r > i32::MAX as i64 || r < i32::MIN as i64 {
                    self.trap("整数减法溢出。被减数太小而减数太大，结果超出了 int 能表示的范围。", loc);
                } else {
                    self.push(r as u64);
                }
            }
            OpCode::USub => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(a.wrapping_sub(b) as u64);
            }
            OpCode::Mul => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                let r = (a as i64) * (b as i64);
                if r > i32::MAX as i64 || r < i32::MIN as i64 {
                    self.trap("整数乘法溢出。乘积太大，超出了 int 能表示的范围。", loc);
                } else {
                    self.push(r as u64);
                }
            }
            OpCode::UMul => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(a.wrapping_mul(b) as u64);
            }
            OpCode::Div => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                if b == 0 {
                    let msg = self.format_div_zero_error(a, b);
                    self.trap(&msg, loc);
                } else if a == i32::MIN && b == -1 {
                    self.trap("整数除法溢出。INT_MIN / -1 的结果超出了 int 能表示的范围。", loc);
                } else {
                    self.push((a / b) as u64);
                }
            }
            OpCode::UDiv => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                if let Some(res) = a.checked_div(b) {
                    self.push(res as u64);
                } else {
                    let msg = self.format_div_zero_error(a as i32, b as i32);
                    self.trap(&msg, loc);
                }
            }
            OpCode::Mod => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                if b == 0 {
                    let msg = self.format_div_zero_error(a, b);
                    self.trap(&msg, loc);
                } else if a == i32::MIN && b == -1 {
                    // Rust 的 % 在 MIN % -1 时溢出 panic，必须先拦截
                    self.trap("整数取模溢出。INT_MIN % -1 的结果未定义，超出了 int 能表示的范围。", loc);
                } else {
                    self.push((a % b) as u64);
                }
            }
            OpCode::UMod => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                if b == 0 {
                    let msg = self.format_div_zero_error(a as i32, b as i32);
                    self.trap(&msg, loc);
                } else {
                    self.push((a % b) as u64);
                }
            }
            OpCode::Neg => {
                let a = self.pop() as i32;
                if a == i32::MIN {
                    self.trap("整数取反溢出。-INT_MIN 的结果超出了 int 能表示的范围。", loc);
                } else {
                    self.push((-a) as u64);
                }
            }
            OpCode::UNeg => {
                let a = self.pop() as u32;
                self.push(a.wrapping_neg() as u64);
            }
            _ => {}
        }
    }
    pub(crate) fn execute_comparison(&mut self, op: OpCode, _operand: i32, _loc: &SourceLoc) {
        match op {
            OpCode::Eq => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a == b { 1 } else { 0 });
            }
            OpCode::Ne => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a != b { 1 } else { 0 });
            }
            OpCode::Lt => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a < b { 1 } else { 0 });
            }
            OpCode::Le => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a <= b { 1 } else { 0 });
            }
            OpCode::Gt => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a > b { 1 } else { 0 });
            }
            OpCode::Ge => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a >= b { 1 } else { 0 });
            }
            OpCode::ULt => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                self.push(if a < b { 1 } else { 0 });
            }
            OpCode::ULe => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                self.push(if a <= b { 1 } else { 0 });
            }
            OpCode::UGt => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                self.push(if a > b { 1 } else { 0 });
            }
            OpCode::UGe => {
                let b = self.pop() as u32;
                let a = self.pop() as u32;
                self.push(if a >= b { 1 } else { 0 });
            }
            OpCode::And => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a != 0 && b != 0 { 1 } else { 0 });
            }
            OpCode::Or => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push(if a != 0 || b != 0 { 1 } else { 0 });
            }
            OpCode::Not => {
                let a = self.pop() as i32;
                self.push(if a != 0 { 0 } else { 1 });
            }
            _ => {}
        }
    }
    pub(crate) fn execute_bitwise(&mut self, op: OpCode, _operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::BitAnd => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push((a & b) as u64);
            }
            OpCode::BitOr => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push((a | b) as u64);
            }
            OpCode::BitXor => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                self.push((a ^ b) as u64);
            }
            OpCode::BitNot => {
                let a = self.pop() as i32;
                self.push((!a) as u64);
            }
            OpCode::Shl => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                if !(0..32).contains(&b) {
                    self.trap(&format!("Shl 移位量越界：{}（必须是 0~31）", b), loc);
                } else {
                    self.push((a << b) as u64);
                }
            }
            OpCode::Shr => {
                let b = self.pop() as i32;
                let a = self.pop() as i32;
                if !(0..32).contains(&b) {
                    self.trap(&format!("Shr 移位量越界：{}（必须是 0~31）", b), loc);
                } else {
                    self.push((a >> b) as u64);
                }
            }
            OpCode::LShr => {
                let b = self.pop() as i32;
                let a = self.pop() as u32;
                if !(0..32).contains(&b) {
                    self.trap(&format!("LShr 移位量越界：{}（必须是 0~31）", b), loc);
                } else {
                    self.push((a >> b) as u64);
                }
            }
            // E1 B 档：64 位位运算（long long / unsigned long long）。
            // 栈值本就是 u64 位模式：与/或/异或按位组合与符号性无关；
            // 算术右移按 i64 解释，逻辑右移按 u64。
            OpCode::BitAndQ => {
                let b = self.pop();
                let a = self.pop();
                self.push(a & b);
            }
            OpCode::BitOrQ => {
                let b = self.pop();
                let a = self.pop();
                self.push(a | b);
            }
            OpCode::BitXorQ => {
                let b = self.pop();
                let a = self.pop();
                self.push(a ^ b);
            }
            OpCode::BitNotQ => {
                let a = self.pop();
                self.push(!a);
            }
            OpCode::ShlQ => {
                let b = self.pop() as i64;
                let a = self.pop();
                if !(0..64).contains(&b) {
                    self.trap(&format!("Shl 移位量越界：{}（必须是 0~63）", b), loc);
                } else {
                    self.push(a << b);
                }
            }
            OpCode::ShrQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                if !(0..64).contains(&b) {
                    self.trap(&format!("Shr 移位量越界：{}（必须是 0~63）", b), loc);
                } else {
                    self.push((a >> b) as u64);
                }
            }
            OpCode::LShrQ => {
                let b = self.pop() as i64;
                let a = self.pop();
                if !(0..64).contains(&b) {
                    self.trap(&format!("LShr 移位量越界：{}（必须是 0~63）", b), loc);
                } else {
                    self.push(a >> b);
                }
            }
            _ => {}
        }
    }
}
