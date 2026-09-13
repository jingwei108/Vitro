//! Block 语句代码生成。

use vitro_ast::Stmt;

use super::super::BytecodeGen;
use super::StmtGen;

impl BytecodeGen {
    pub(crate) fn gen_block(&mut self, stmts: &mut Vec<Stmt>) {
        self.enter_scope();
        for s in stmts {
            self.gen_stmt(s);
        }
        self.exit_scope();
    }
}
