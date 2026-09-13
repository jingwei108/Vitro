//! switch / case / default 语句代码生成。

use crate::expr::ExprGen;
use vitro_ast::{Expr, SourceLoc, Stmt};
use vitro_runtime::opcode::OpCode;

use super::super::BytecodeGen;
use super::StmtGen;

pub(crate) fn gen_switch(gen: &mut BytecodeGen, cond: &mut Expr, body: &mut Stmt, loc: &SourceLoc) {
    let mut cases: Vec<(Option<Expr>, Box<Stmt>)> = Vec::new();
    let mut default_case: Option<Box<Stmt>> = None;

    fn collect_cases(stmt: &mut Stmt, cases: &mut Vec<(Option<Expr>, Box<Stmt>)>, default: &mut Option<Box<Stmt>>) {
        match stmt {
            Stmt::Block { stmts, .. } => {
                for s in stmts {
                    collect_cases(s, cases, default);
                }
            }
            Stmt::Case { label, stmt, .. } => {
                if label.is_some() {
                    cases.push((label.take(), stmt.clone()));
                } else {
                    *default = Some(stmt.clone());
                }
            }
            _ => {}
        }
    }

    collect_cases(body, &mut cases, &mut default_case);

    if cases.is_empty() && default_case.is_none() {
        gen.gen_expr(cond);
        gen.emit(OpCode::Pop, 0, loc);
        return;
    }

    gen.gen_expr(cond);
    let cond_temp = gen.get_temp_slot(0);
    gen.emit(OpCode::StoreLocal, cond_temp, loc);

    let mut case_jump_ips = Vec::new();
    for (label, _) in &mut cases {
        gen.emit(OpCode::LoadLocal, cond_temp, loc);
        if let Some(ref mut l) = label {
            gen.gen_expr(l);
            if l.loc().line > 0 {
                gen.emit(OpCode::StepEvent, l.loc().line, l.loc());
            }
        }
        gen.emit(OpCode::Eq, 0, loc);
        let jump_ip = gen.current_ip();
        gen.emit(OpCode::JumpIfNotZero, 0, loc);
        case_jump_ips.push(jump_ip);
    }

    let default_or_end_jump = gen.current_ip();
    gen.emit(OpCode::Jump, 0, loc);
    let break_base = gen.break_patches.len();

    for (i, (_, ref mut stmt)) in cases.iter_mut().enumerate() {
        gen.patch_jump(case_jump_ips[i], gen.current_ip());
        gen.gen_stmt(stmt);
    }

    if let Some(ref mut d) = default_case {
        gen.patch_jump(default_or_end_jump, gen.current_ip());
        gen.gen_stmt(d);
    } else {
        gen.patch_jump(default_or_end_jump, gen.current_ip());
    }

    let end_ip = gen.current_ip();
    for i in break_base..gen.break_patches.len() {
        gen.patch_jump(gen.break_patches[i], end_ip);
    }
    gen.break_patches.resize(break_base, 0);
}
