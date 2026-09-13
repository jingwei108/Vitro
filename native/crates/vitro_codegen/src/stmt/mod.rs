//! BytecodeGen 语句级别代码生成入口。
//!
//! 将 `gen_stmt` 按语句类型分派到 `stmt/` 子模块，降低单文件认知负荷。

use super::*;

mod block;
mod control;
mod cpp;
mod expr_stmt;
mod switch;
mod var_decl;

pub(crate) trait StmtGen {
    fn gen_stmt(&mut self, stmt: &mut Stmt);
    fn gen_switch(&mut self, cond: &mut Expr, body: &mut Stmt, loc: &SourceLoc);
}

impl StmtGen for BytecodeGen {
    fn gen_stmt(&mut self, stmt: &mut Stmt) {
        let loc = stmt_loc(stmt);
        if loc.line > 0 {
            self.emit(OpCode::StepEvent, loc.line, &loc);
        }
        match stmt {
            Stmt::Block { stmts, .. } => self.gen_block(stmts),
            Stmt::VarDecl {
                var_type,
                name,
                init,
                extra_vars,
                is_static,
                loc,
            } => self.gen_var_decl(var_type, name, init, extra_vars, *is_static, loc),
            Stmt::Expr { expr, .. } => self.gen_expr_stmt(expr, &loc),
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                loc,
            } => self.gen_if(cond, then_stmt, else_stmt, loc),
            Stmt::While { cond, body, loc } => self.gen_while(cond, body, loc),
            Stmt::DoWhile { body, cond, loc } => self.gen_do_while(body, cond, loc),
            Stmt::For { init, cond, step, body, loc } => self.gen_for(init, cond, step, body, loc),
            Stmt::Return { value, loc } => self.gen_return(value, loc),
            Stmt::Break { loc } => self.gen_break(loc),
            Stmt::Continue { loc } => self.gen_continue(loc),
            Stmt::Switch { cond, body, loc } => self.gen_switch(cond, body, loc),
            Stmt::Case { .. } => {}
            Stmt::Goto { label, loc } => self.gen_goto(label, loc),
            Stmt::Label { label, stmt, .. } => self.gen_label(label, stmt.as_mut()),
            Stmt::RangeFor { var, var_type, iter, body, .. } => {
                self.gen_range_for_stmt(var, var_type, iter.as_mut(), body.as_mut(), &loc)
            }
            Stmt::Try { .. } => self.gen_try_stmt(&loc),
        }
    }

    fn gen_switch(&mut self, cond: &mut Expr, body: &mut Stmt, loc: &SourceLoc) {
        switch::gen_switch(self, cond, body, loc);
    }
}

impl BytecodeGen {
    pub(crate) fn gen_goto(&mut self, label: &str, loc: &SourceLoc) {
        if let Some(&target_ip) = self.label_ips.get(label) {
            self.emit(OpCode::Jump, target_ip as i32, loc);
        } else {
            let ip = self.current_ip();
            self.emit(OpCode::Jump, 0, loc);
            self.goto_patches.entry(label.to_string()).or_default().push(ip);
        }
    }

    pub(crate) fn gen_label(&mut self, label: &str, stmt: &mut Stmt) {
        let ip = self.current_ip();
        self.label_ips.insert(label.to_string(), ip);
        if let Some(patches) = self.goto_patches.remove(label) {
            for patch_ip in patches {
                self.patch_jump(patch_ip, ip);
            }
        }
        self.gen_stmt(stmt);
    }
}
