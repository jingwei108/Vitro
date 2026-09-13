use super::*;
use crate::instruction::SourceLoc;
use vitro_runtime::base_kind;

impl VitroVM {
    pub(crate) fn format_bounds_error(&self, addr: u32) -> String {
        let mut best_sym: Option<(&VMSymbol, u32, i32)> = None;
        let mut best_dist = i32::MAX;

        for sym in &self.symbols {
            if !matches!(sym.ty.kind(), vitro_ast::TypeKind::Array) || sym.ty.array_size() <= 0 {
                continue;
            }
            let mut base = sym.addr;
            if sym.is_local {
                if let Some(frame) = self.call_stack.last() {
                    base = frame.locals_base + sym.addr;
                } else {
                    continue;
                }
            }
            let elem_size = match base_kind(&sym.ty) {
                vitro_ast::TypeKind::Char => 1,
                vitro_ast::TypeKind::Double => 8,
                _ => 4,
            };
            let size = (sym.ty.array_size() as u32) * elem_size as u32;
            let dist = if addr >= base && addr < base + size {
                0
            } else if addr >= base + size && addr < base + size + 64 {
                (addr - (base + size)) as i32
            } else if addr + 64 >= base && addr < base {
                (base - addr) as i32
            } else {
                continue;
            };
            if dist < best_dist {
                best_dist = dist;
                best_sym = Some((sym, base, dist));
            }
        }

        if let Some((sym, base, _)) = best_sym {
            let elem_size = match base_kind(&sym.ty) {
                vitro_ast::TypeKind::Char => 1,
                vitro_ast::TypeKind::Double => 8,
                _ => 4,
            };
            let index = ((addr as i64 - base as i64) / elem_size as i64) as i32;
            format!(
                "🚫 数组越界：你访问了 {}[{}]，但数组 '{}' 只有 {} 个元素，有效索引是 0~{}。\n\n📍 发生在第 {} 行\n💡 原因：数组索引超出了合法范围。\n✅ 检查方法：确认索引变量值在 0 到 {} 之间。",
                sym.name, index, sym.name, sym.ty.array_size(), sym.ty.array_size() - 1,
                self.current_line, sym.ty.array_size() - 1
            )
        } else {
            format!(
                "🚫 内存访问越界：你访问了地址 0x{:04X}，但合法内存范围是 0x{:04X} ~ 0x{:04X}。\n\n✅ 检查方法：\n  • 确认数组索引小于数组大小\n  • 确认指针已经指向有效的内存地址\n  • 确认没有使用已经 free 的指针",
                addr, NULL_TRAP_SIZE, MEM_SIZE
            )
        }
    }

    pub(crate) fn format_div_zero_error(&self, a: i32, _b: i32) -> String {
        let mut diag = format!("😵 除零错误：你试图用 {} 除以 0。\n\n", a);
        let zero_vars: Vec<String> = self
            .symbols
            .iter()
            .filter_map(|sym| {
                if matches!(sym.ty.kind(), vitro_ast::TypeKind::Array) {
                    return None;
                }
                let mut vaddr = sym.addr;
                if sym.is_local {
                    let frame = self.call_stack.last()?;
                    vaddr = frame.locals_base + sym.addr;
                }
                if vaddr + 4 <= MEM_SIZE && vaddr >= NULL_TRAP_SIZE {
                    let val = i32::from_le_bytes([
                        self.memory[vaddr as usize],
                        self.memory[vaddr as usize + 1],
                        self.memory[vaddr as usize + 2],
                        self.memory[vaddr as usize + 3],
                    ]);
                    if val == 0 {
                        Some(sym.name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if !zero_vars.is_empty() {
            diag.push_str("🔍 当前作用域内值为 0 的变量：");
            diag.push_str(&zero_vars.join("、"));
            diag.push_str("。请检查除法表达式中是否使用了这些变量。\n\n");
        }
        diag.push_str("💡 原因：除数不能为 0。\n✅ 检查你的除法表达式，确保除数在被除之前不是 0。\n📝 示例：如果变量 b 可能为 0，先用 if 判断：\n    if (b != 0) {\n        result = a / b;\n    }");
        diag
    }

    pub(crate) fn format_infinite_loop_error(&self) -> String {
        let mut diag = format!("🔄 程序执行步数超过限制（{} 步），可能包含无限循环。\n\n", self.max_steps);
        let mut stale_vars = Vec::new();
        let mut changed_vars = Vec::new();
        for sym in &self.symbols {
            if matches!(sym.ty.kind(), vitro_ast::TypeKind::Array) {
                continue;
            }
            let cur_val = self.read_variable(sym);
            if let Some(&old_val) = self.snapshot_vars.get(&sym.name) {
                if old_val == cur_val as u64 {
                    stale_vars.push(format!("{} = {}", sym.name, cur_val));
                } else {
                    changed_vars.push(format!("{} = {}", sym.name, cur_val));
                }
            }
        }
        if !stale_vars.is_empty() {
            diag.push_str("🔍 在最近 ");
            diag.push_str(&SNAPSHOT_INTERVAL.to_string());
            diag.push_str(" 步内没有变化的变量：");
            let shown: Vec<_> = stale_vars.iter().take(6).cloned().collect();
            diag.push_str(&shown.join("，"));
            if stale_vars.len() > 6 {
                diag.push_str(" 等");
            }
            diag.push_str("。\n\n");
        }
        if !changed_vars.is_empty() {
            diag.push_str("🔍 发生变化的变量：");
            let shown: Vec<_> = changed_vars.iter().take(4).cloned().collect();
            diag.push_str(&shown.join("，"));
            if changed_vars.len() > 4 {
                diag.push_str(" 等");
            }
            diag.push_str("。\n\n");
        }
        diag.push_str("💡 原因：程序执行了太多步数但没有结束。常见原因：\n  • 循环条件永远为真（如 while(1)）\n  • 循环变量没有更新（如忘了写 i++）\n  • 递归函数没有正确的终止条件\n✅ 检查方法：确认循环体中有改变循环条件的语句。");
        diag
    }

    pub fn trap(&mut self, msg: &str, loc: &SourceLoc) {
        if self.error.is_empty() {
            self.error = msg.to_string();
            let line = if loc.line > 0 { loc.line } else { self.current_line };
            if line > 0 {
                self.error.push_str(&format!("\n📍 发生在第 {} 行", line));
                if loc.column > 0 {
                    self.error.push_str(&format!(" 第 {} 列", loc.column));
                }
            }
        }
    }
}
