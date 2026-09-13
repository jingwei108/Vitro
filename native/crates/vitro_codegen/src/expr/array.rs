use super::*;

pub(crate) fn gen_index_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Index { array, index, ty, .. } = expr {
        gen.gen_index(array, index, ty, &loc, false);
    }
}
