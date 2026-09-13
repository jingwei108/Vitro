use super::*;

pub(crate) fn gen_generic_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Generic {
        control, associations, default, ..
    } = expr
    {
        let control_type = control.ty().clone();
        // 与控制表达式类型检查阶段保持一致：数组退化为指针。
        let control_decayed = if let Type::Array { element, .. } = &control_type {
            Type::pointer_to(*element.clone())
        } else {
            control_type
        };

        let mut selected: Option<&mut Expr> = None;
        for (assoc_ty, assoc_expr) in associations.iter_mut() {
            if *assoc_ty == control_decayed {
                selected = Some(assoc_expr);
                break;
            }
        }

        if let Some(sel) = selected {
            gen.gen_expr(sel);
        } else if let Some(default_expr) = default.as_deref_mut() {
            gen.gen_expr(default_expr);
        } else {
            gen.report_error("_Generic 无匹配分支", &loc);
            gen.emit(OpCode::PushConst, 0, &loc);
        }
    } else {
        gen.report_error("非 _Generic 表达式进入 generic 生成器", &loc);
        gen.emit(OpCode::PushConst, 0, &loc);
    }
}
