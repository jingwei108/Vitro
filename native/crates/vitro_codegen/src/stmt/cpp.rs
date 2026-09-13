//! C++ 专属语句代码生成（RangeFor / Try 占位）。

use vitro_ast::{Expr, SourceLoc, Stmt, Type};

use super::super::BytecodeGen;

impl BytecodeGen {
    pub(crate) fn gen_range_for_stmt(
        &mut self,
        var: &str,
        var_type: &Type,
        iter: &mut Expr,
        body: &mut Stmt,
        loc: &SourceLoc,
    ) {
        self.gen_range_for(var, var_type, iter, body, loc);
    }

    pub(crate) fn gen_try_stmt(&mut self, loc: &SourceLoc) {
        self.report_error("Try/Catch 语句代码生成尚未实现（VM 不支持异常）", loc);
    }
}
