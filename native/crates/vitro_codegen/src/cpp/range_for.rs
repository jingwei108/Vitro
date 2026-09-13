use vitro_ast::*;
use vitro_cpp_frontend::type_map::is_builtin_container;
use vitro_runtime::opcode::OpCode;
use vitro_runtime::GLOBAL_START;

use super::super::stmt::StmtGen;
use super::super::BytecodeGen;

impl BytecodeGen {
    pub(crate) fn gen_range_for(
        &mut self,
        var: &str,
        var_type: &Type,
        iter: &mut Expr,
        body: &mut Stmt,
        loc: &SourceLoc,
    ) {
        let iter_ty = iter.ty().clone();
        let is_container = matches!(&iter_ty, Type::Class { name, .. }
            if is_builtin_container(name)
                || name.starts_with("vitro_vec_")
                || name == "vitro_string"
                || name == "vitro_list_int");
        let is_array = matches!(&iter_ty, Type::Array { .. });
        if !is_array && !is_container {
            self.report_error("RangeFor 目前只支持数组和内置容器类型", loc);
            return;
        }
        self.enter_scope();
        // Index temp
        let idx_offset = self.next_local_offset;
        self.next_local_offset += 4;
        self.emit(OpCode::PushConst, 0, loc);
        self.emit(OpCode::StoreLocal, idx_offset, loc);
        // Loop variable
        let var_sz = (self.type_size(var_type) + 3) & !3;
        let var_offset = self.next_local_offset;
        self.next_local_offset += var_sz;
        self.local_indices.insert(var.to_string(), var_offset);
        self.local_types.insert(var.to_string(), var_type.clone());

        let start_ip = self.current_ip();
        self.loop_start_ips.push(start_ip);
        self.loop_scope_depths.push(self.local_scope_stack.len());
        self.loop_break_has_init_frame.push(true);
        let break_base = self.break_patches.len();
        let continue_base = self.continue_patches.len();

        // Condition: idx < count
        self.emit(OpCode::LoadLocal, idx_offset, loc);
        if is_array {
            let elem_count = if let Type::Array { array_size, .. } = &iter_ty {
                *array_size
            } else {
                0
            };
            self.emit(OpCode::PushConst, elem_count, loc);
        } else {
            // Container: call {container}__size(&iter)
            let class_name = iter_ty.name();
            let size_func = format!("{}__size", class_name);
            if let Some(&idx) = self.func_index.get(&size_func) {
                // Push &iter
                if let Expr::Identifier { name, .. } = iter {
                    if let Some(&offset) = self.local_indices.get(name) {
                        self.emit(OpCode::GetFrameBase, 0, loc);
                        self.emit(OpCode::PushConst, offset, loc);
                        self.emit(OpCode::Add, 0, loc);
                    } else if let Some(&offset) = self.global_indices.get(name) {
                        self.emit(OpCode::PushConst, GLOBAL_START as i32 + offset, loc);
                    } else {
                        self.report_error("RangeFor: 未声明的容器变量", loc);
                        self.exit_scope();
                        return;
                    }
                } else {
                    self.report_error("RangeFor: 复杂的迭代表达式暂不支持", loc);
                    self.exit_scope();
                    return;
                }
                self.emit(OpCode::Call, idx, loc);
            } else {
                self.report_error(&format!("RangeFor: 未找到容器函数 '{}'", size_func), loc);
                self.exit_scope();
                return;
            }
        }
        self.emit(OpCode::Lt, 0, loc);
        let cond_jump = self.current_ip();
        self.emit(OpCode::JumpIfZero, 0, loc);

        // Load element: var = iter[idx]
        if is_array {
            let elem_ty = if let Type::Array { element, .. } = &iter_ty {
                element.clone()
            } else {
                Box::new(Type::int())
            };
            let elem_sz = self.type_size(&elem_ty);
            if let Expr::Identifier { name, .. } = iter {
                if let Some(&offset) = self.local_indices.get(name) {
                    self.emit(OpCode::GetFrameBase, 0, loc);
                    self.emit(OpCode::PushConst, offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                } else if let Some(&offset) = self.global_indices.get(name) {
                    self.emit(OpCode::PushConst, GLOBAL_START as i32 + offset, loc);
                } else {
                    self.report_error("RangeFor: 未声明的数组变量", loc);
                    self.exit_scope();
                    return;
                }
            } else {
                self.report_error("RangeFor: 复杂的迭代表达式暂不支持", loc);
                self.exit_scope();
                return;
            }
            self.emit(OpCode::LoadLocal, idx_offset, loc);
            self.emit(OpCode::PushConst, elem_sz, loc);
            self.emit(OpCode::Mul, 0, loc);
            self.emit(OpCode::Add, 0, loc);
            if var_type.is_reference() || var_type.is_rvalue_ref() {
                // Reference loop variable for array: bind to element address
                self.emit(OpCode::StoreLocal, var_offset, loc);
            } else {
                self.emit(OpCode::LoadMem, 0, loc);
                self.emit(OpCode::StoreLocal, var_offset, loc);
            }
        } else {
            // Container: call {container}__get(&iter, idx)
            let class_name = iter_ty.name();
            let get_func = format!("{}__get", class_name);
            if let Some(&idx) = self.func_index.get(&get_func) {
                // Push idx first (will be second on stack after &iter)
                self.emit(OpCode::LoadLocal, idx_offset, loc);
                // Push &iter
                if let Expr::Identifier { name, .. } = iter {
                    if let Some(&offset) = self.local_indices.get(name) {
                        self.emit(OpCode::GetFrameBase, 0, loc);
                        self.emit(OpCode::PushConst, offset, loc);
                        self.emit(OpCode::Add, 0, loc);
                    } else if let Some(&offset) = self.global_indices.get(name) {
                        self.emit(OpCode::PushConst, GLOBAL_START as i32 + offset, loc);
                    } else {
                        self.report_error("RangeFor: 未声明的容器变量", loc);
                        self.exit_scope();
                        return;
                    }
                } else {
                    self.report_error("RangeFor: 复杂的迭代表达式暂不支持", loc);
                    self.exit_scope();
                    return;
                }
                self.emit(OpCode::Call, idx, loc);
                self.emit(OpCode::StoreLocal, var_offset, loc);
            } else {
                self.report_error(&format!("RangeFor: 未找到容器函数 '{}'", get_func), loc);
                self.exit_scope();
                return;
            }
        }

        self.gen_stmt(body);

        // Continue: ++idx
        let continue_ip = self.current_ip();
        self.emit(OpCode::LoadLocal, idx_offset, loc);
        self.emit(OpCode::PushConst, 1, loc);
        self.emit(OpCode::Add, 0, loc);
        self.emit(OpCode::StoreLocal, idx_offset, loc);
        self.emit(OpCode::Jump, start_ip as i32, loc);

        let end_ip = self.current_ip();
        self.exit_scope();
        self.patch_jump(cond_jump, end_ip);
        for i in break_base..self.break_patches.len() {
            self.patch_jump(self.break_patches[i], end_ip);
        }
        self.break_patches.resize(break_base, 0);
        for i in continue_base..self.continue_patches.len() {
            self.patch_jump(self.continue_patches[i], continue_ip);
        }
        self.continue_patches.resize(continue_base, 0);
        self.loop_start_ips.pop();
        self.loop_scope_depths.pop();
        self.loop_break_has_init_frame.pop();
    }
}
