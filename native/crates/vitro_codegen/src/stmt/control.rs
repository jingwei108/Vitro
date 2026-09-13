//! 控制流语句代码生成（if / while / do-while / for / break / continue / return）。

use crate::expr::{is_lvalue_expr, ExprGen};
use vitro_ast::{Expr, SourceLoc, Stmt, Type, TypeKind};
use vitro_runtime::opcode::OpCode;

use super::super::BytecodeGen;
use super::StmtGen;

impl BytecodeGen {
    pub(crate) fn gen_if(
        &mut self,
        cond: &mut Expr,
        then_stmt: &mut Stmt,
        else_stmt: &mut Option<Box<Stmt>>,
        loc: &SourceLoc,
    ) {
        self.gen_expr(cond);
        let else_jump = self.current_ip();
        self.emit(OpCode::JumpIfZero, 0, loc);
        self.gen_stmt(then_stmt);
        let skip_else_jump = self.current_ip();
        self.emit(OpCode::Jump, 0, loc);
        let else_ip = self.current_ip();
        self.patch_jump(else_jump, else_ip);
        if let Some(ref mut e) = else_stmt {
            self.gen_stmt(e);
        }
        let end_ip = self.current_ip();
        self.patch_jump(skip_else_jump, end_ip);
    }

    pub(crate) fn gen_while(&mut self, cond: &mut Expr, body: &mut Stmt, loc: &SourceLoc) {
        let start_ip = self.current_ip();
        self.gen_expr(cond);
        let end_jump = self.current_ip();
        self.emit(OpCode::JumpIfZero, 0, loc);
        self.loop_start_ips.push(start_ip);
        self.loop_scope_depths.push(self.local_scope_stack.len());
        self.loop_break_has_init_frame.push(false);
        let break_base = self.break_patches.len();
        let continue_base = self.continue_patches.len();
        self.gen_stmt(body);
        self.emit(OpCode::Jump, start_ip as i32, loc);
        let end_ip = self.current_ip();
        self.patch_jump(end_jump, end_ip);
        self.patch_loop_patches(end_ip, start_ip, break_base, continue_base);
        self.loop_start_ips.pop();
        self.loop_scope_depths.pop();
        self.loop_break_has_init_frame.pop();
    }

    pub(crate) fn gen_do_while(&mut self, body: &mut Stmt, cond: &mut Expr, loc: &SourceLoc) {
        let start_ip = self.current_ip();
        self.loop_start_ips.push(start_ip);
        self.loop_scope_depths.push(self.local_scope_stack.len());
        self.loop_break_has_init_frame.push(false);
        let break_base = self.break_patches.len();
        let continue_base = self.continue_patches.len();
        self.gen_stmt(body);
        let cond_ip = self.current_ip();
        self.gen_expr(cond);
        self.emit(OpCode::JumpIfNotZero, start_ip as i32, loc);
        let end_ip = self.current_ip();
        self.patch_loop_patches(end_ip, cond_ip, break_base, continue_base);
        self.loop_start_ips.pop();
        self.loop_scope_depths.pop();
        self.loop_break_has_init_frame.pop();
    }

    pub(crate) fn gen_for(
        &mut self,
        init: &mut Option<Box<Stmt>>,
        cond: &mut Option<Expr>,
        step: &mut Vec<Expr>,
        body: &mut Stmt,
        loc: &SourceLoc,
    ) {
        self.enter_scope();
        if let Some(ref mut i) = init {
            self.gen_stmt(i);
        }
        let start_ip = self.current_ip();
        let mut cond_jump = 0;
        if let Some(ref mut c) = cond {
            self.gen_expr(c);
            cond_jump = self.current_ip();
            self.emit(OpCode::JumpIfZero, 0, loc);
        }
        self.loop_start_ips.push(start_ip);
        self.loop_scope_depths.push(self.local_scope_stack.len());
        self.loop_break_has_init_frame.push(true);
        let break_base = self.break_patches.len();
        let continue_base = self.continue_patches.len();
        self.gen_stmt(body);
        let continue_ip = self.current_ip();
        for s in step {
            self.gen_expr(s);
            self.emit(OpCode::Pop, 0, loc);
        }
        self.emit(OpCode::Jump, start_ip as i32, loc);
        let end_ip = self.current_ip();
        self.exit_scope();
        if cond.is_some() {
            self.patch_jump(cond_jump, end_ip);
        }
        self.patch_loop_patches(end_ip, continue_ip, break_base, continue_base);
        self.loop_start_ips.pop();
        self.loop_scope_depths.pop();
        self.loop_break_has_init_frame.pop();
    }

    pub(crate) fn gen_return(&mut self, value: &mut Option<Expr>, loc: &SourceLoc) {
        if let Some(ref mut v) = value {
            let ret_is_struct = self
                .func_table
                .get(&self.current_func)
                .map(|m| m.return_type.is_struct() || m.return_type.is_class())
                .unwrap_or(false);
            if ret_is_struct {
                let ret_ptr_offset = self.resolve_local("__ret_ptr");
                let size = self.type_size(v.ty());
                if size > 0 {
                    // 如果类有自定义拷贝构造函数且返回值是左值，调用拷贝构造。
                    let copy_ctor_name = if let Type::Class { name: class_name, .. } = v.ty() {
                        let name = format!("__ctor__{}__copy", class_name);
                        if self.func_index.contains_key(&name) && is_lvalue_expr(v) {
                            Some(name)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(copy_ctor_name) = copy_ctor_name {
                        // 拷贝构造参数：other（源地址，先压栈）、this（目标地址，后压栈）
                        self.gen_addr(v, loc);
                        self.emit(OpCode::LoadLocal, ret_ptr_offset, loc);
                        if let Some(&idx) = self.func_index.get(&copy_ctor_name) {
                            self.emit(OpCode::Call, idx, loc);
                        }
                    } else {
                        let src_temp = self.get_temp_slot(0);
                        self.gen_addr(v, loc);
                        self.emit(OpCode::StoreLocal, src_temp, loc);
                        for i in 0..size / 4 {
                            self.emit(OpCode::LoadLocal, ret_ptr_offset, loc);
                            if i > 0 {
                                self.emit(OpCode::PushConst, i * 4, loc);
                                self.emit(OpCode::Add, 0, loc);
                            }
                            self.emit(OpCode::LoadLocal, src_temp, loc);
                            if i > 0 {
                                self.emit(OpCode::PushConst, i * 4, loc);
                                self.emit(OpCode::Add, 0, loc);
                            }
                            self.emit(OpCode::LoadMem, 0, loc);
                            self.emit(OpCode::StoreMem, 0, loc);
                        }
                    }
                }
                // C++ 栈对象 RAII：return 前按 LIFO 调用所有活跃 scope 的析构函数
                self.emit_dtors_for_scope_exit(0, loc);
                self.emit(OpCode::RetVoid, 0, loc);
            } else {
                let ret_is_ref = self
                    .func_table
                    .get(&self.current_func)
                    .map(|m| m.return_type.is_reference() || m.return_type.is_rvalue_ref())
                    .unwrap_or(false);
                if ret_is_ref {
                    self.gen_addr(v, loc);
                } else {
                    self.gen_expr(v);
                    let ret_is_float = self
                        .func_table
                        .get(&self.current_func)
                        .map(|m| m.return_type.kind() == TypeKind::Float || m.return_type.kind() == TypeKind::Double)
                        .unwrap_or(false);
                    if ret_is_float && v.ty().kind() != TypeKind::Float && v.ty().kind() != TypeKind::Double {
                        self.emit(OpCode::CastI2F, 0, loc);
                    } else if !ret_is_float && (v.ty().kind() == TypeKind::Float || v.ty().kind() == TypeKind::Double) {
                        self.emit(OpCode::CastF2I, 0, loc);
                    }
                }
                // C++ 栈对象 RAII：return 前按 LIFO 调用所有活跃 scope 的析构函数
                self.emit_dtors_for_scope_exit(0, loc);
                self.emit(OpCode::Ret, 0, loc);
            }
        } else {
            // C++ 栈对象 RAII：return 前按 LIFO 调用所有活跃 scope 的析构函数
            self.emit_dtors_for_scope_exit(0, loc);
            self.emit(OpCode::RetVoid, 0, loc);
        }
    }

    pub(crate) fn gen_break(&mut self, loc: &SourceLoc) {
        // break 跳出整个循环语句：while/do-while 从循环体 frame 起析构；
        // for/range-for 的 init/临时 frame 也随循环销毁，多退一层。
        let body_frame = self.loop_scope_depths.last().copied().unwrap_or(0);
        let start = match self.loop_break_has_init_frame.last().copied() {
            Some(true) => body_frame.saturating_sub(1),
            _ => body_frame,
        };
        self.emit_dtors_for_scope_exit(start, loc);
        let ip = self.current_ip();
        self.emit(OpCode::Jump, 0, loc);
        self.break_patches.push(ip);
    }

    pub(crate) fn gen_continue(&mut self, loc: &SourceLoc) {
        // continue 跳回 step/cond，循环体外的 init/临时 frame 仍存活，
        // 一律从循环体自身 frame 起析构。
        let start = self.loop_scope_depths.last().copied().unwrap_or(0);
        self.emit_dtors_for_scope_exit(start, loc);
        let ip = self.current_ip();
        self.emit(OpCode::Jump, 0, loc);
        self.continue_patches.push(ip);
    }

    fn patch_loop_patches(&mut self, end_ip: usize, continue_ip: usize, break_base: usize, continue_base: usize) {
        for i in break_base..self.break_patches.len() {
            self.patch_jump(self.break_patches[i], end_ip);
        }
        self.break_patches.resize(break_base, 0);
        for i in continue_base..self.continue_patches.len() {
            self.patch_jump(self.continue_patches[i], continue_ip);
        }
        self.continue_patches.resize(continue_base, 0);
    }
}
