use super::*;

impl VitroVM {
    pub(crate) fn execute_float(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::PushConstF => {
                self.push(operand as u32 as u64);
            }
            OpCode::AddF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                let r = a + b;
                self.push(r.to_bits() as u64);
            }
            OpCode::SubF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                let r = a - b;
                self.push(r.to_bits() as u64);
            }
            OpCode::MulF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                let r = a * b;
                self.push(r.to_bits() as u64);
            }
            OpCode::DivF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                if b == 0.0 {
                    self.trap("浮点数除以零", loc);
                } else {
                    let r = a / b;
                    self.push(r.to_bits() as u64);
                }
            }
            OpCode::NegF => {
                let a = f32::from_bits(self.pop() as u32);
                let r = -a;
                self.push(r.to_bits() as u64);
            }
            // E1（C23 锚定）：float 比较同改 IEEE 精确语义（与 EqD 同理，
            // IEEE 754 运算确定性，容差与 Clang golden 矛盾）
            OpCode::EqF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a == b { 1 } else { 0 });
            }
            OpCode::NeF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a != b { 1 } else { 0 });
            }
            OpCode::LtF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a < b { 1 } else { 0 });
            }
            OpCode::LeF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a <= b { 1 } else { 0 });
            }
            OpCode::GtF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a > b { 1 } else { 0 });
            }
            OpCode::GeF => {
                let b = f32::from_bits(self.pop() as u32);
                let a = f32::from_bits(self.pop() as u32);
                self.push(if a >= b { 1 } else { 0 });
            }
            OpCode::CastI2F => {
                let a = self.pop() as i32;
                self.push((a as f32).to_bits() as u64);
            }
            OpCode::CastF2I => {
                let a = f32::from_bits(self.pop() as u32);
                self.push(a as i32 as u64);
            }
            _ => {}
        }
    }
    pub(crate) fn execute_double(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::PushConstD => {
                let idx = operand as usize;
                let val = match self.f64_constants.get(idx) {
                    Some(&v) => v,
                    None => {
                        self.trap(&format!("f64常量索引越界: {}", idx), loc);
                        return;
                    }
                };
                self.push(val.to_bits());
            }
            OpCode::AddD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push((a + b).to_bits());
            }
            OpCode::SubD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push((a - b).to_bits());
            }
            OpCode::MulD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push((a * b).to_bits());
            }
            OpCode::DivD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                if b == 0.0 {
                    self.trap("double 除以零", loc);
                } else {
                    self.push((a / b).to_bits());
                }
            }
            OpCode::NegD => {
                let a = f64::from_bits(self.pop());
                self.push((-a).to_bits());
            }
            OpCode::CastI2D => {
                let a = self.pop() as i32;
                self.push((a as f64).to_bits());
            }
            OpCode::CastF2D => {
                let a = f32::from_bits(self.pop() as u32);
                self.push((a as f64).to_bits());
            }
            OpCode::CastD2I => {
                let a = f64::from_bits(self.pop());
                self.push(a as i32 as u64);
            }
            OpCode::CastD2F => {
                let a = f64::from_bits(self.pop());
                self.push((a as f32).to_bits() as u64);
            }
            // E1（C23 锚定）：double 比较改为 IEEE 精确语义。原 epsilon 容差
            //（1e-6）使 `0.1 + 0.2 == 0.3` 判真，与 C 标准和 Clang golden 直接
            // 矛盾——浮点精确性正是浮点教学的核心一课。IEEE 754 算术是确定性
            // 的，与宿主编译器逐位一致，无需容差。
            OpCode::EqD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a == b { 1 } else { 0 });
            }
            OpCode::NeD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a != b { 1 } else { 0 });
            }
            OpCode::LtD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a < b { 1 } else { 0 });
            }
            OpCode::LeD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a <= b { 1 } else { 0 });
            }
            OpCode::GtD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a > b { 1 } else { 0 });
            }
            OpCode::GeD => {
                let b = f64::from_bits(self.pop());
                let a = f64::from_bits(self.pop());
                self.push(if a >= b { 1 } else { 0 });
            }
            _ => {}
        }
    }
    pub(crate) fn execute_longlong(&mut self, op: OpCode, operand: i32, loc: &SourceLoc) {
        match op {
            OpCode::PushConstQ => {
                let idx = operand as usize;
                let val = match self.i64_constants.get(idx) {
                    Some(&v) => v,
                    None => {
                        self.trap(&format!("i64常量索引越界: {}", idx), loc);
                        return;
                    }
                };
                self.push(val as u64);
            }
            OpCode::AddQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push((a.wrapping_add(b)) as u64);
            }
            OpCode::SubQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push((a.wrapping_sub(b)) as u64);
            }
            OpCode::MulQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push((a.wrapping_mul(b)) as u64);
            }
            OpCode::DivQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                if b == 0 {
                    self.trap("long long 除以零", loc);
                } else if a == i64::MIN && b == -1 {
                    self.trap("long long 除法溢出。LLONG_MIN / -1 的结果超出了 long long 能表示的范围。", loc);
                } else {
                    self.push((a / b) as u64);
                }
            }
            OpCode::ModQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                if b == 0 {
                    self.trap("long long 取模除以零", loc);
                } else if a == i64::MIN && b == -1 {
                    self.trap(
                        "long long 取模溢出。LLONG_MIN % -1 的结果未定义，超出了 long long 能表示的范围。",
                        loc,
                    );
                } else {
                    self.push((a % b) as u64);
                }
            }
            OpCode::NegQ => {
                let a = self.pop() as i64;
                if a == i64::MIN {
                    self.trap("long long 取反溢出。-LLONG_MIN 的结果超出了 long long 能表示的范围。", loc);
                } else {
                    self.push((-a) as u64);
                }
            }
            OpCode::CastI2Q => {
                let a = self.pop() as i32;
                self.push(a as i64 as u64);
            }
            OpCode::CastQ2I => {
                let a = self.pop() as i64;
                self.push(a as i32 as u64);
            }
            OpCode::CastQ2D => {
                let a = self.pop() as i64;
                self.push((a as f64).to_bits());
            }
            OpCode::CastD2Q => {
                let a = f64::from_bits(self.pop());
                self.push(a as i64 as u64);
            }
            OpCode::EqQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a == b { 1 } else { 0 });
            }
            OpCode::NeQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a != b { 1 } else { 0 });
            }
            OpCode::LtQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a < b { 1 } else { 0 });
            }
            OpCode::LeQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a <= b { 1 } else { 0 });
            }
            OpCode::GtQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a > b { 1 } else { 0 });
            }
            OpCode::GeQ => {
                let b = self.pop() as i64;
                let a = self.pop() as i64;
                self.push(if a >= b { 1 } else { 0 });
            }
            _ => {}
        }
    }
}
