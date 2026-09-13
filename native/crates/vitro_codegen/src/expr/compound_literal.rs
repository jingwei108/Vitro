use super::*;

pub(crate) fn gen_compound_literal_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::CompoundLiteral { target_type, init, .. } = expr {
        let sz = gen.type_size(target_type);
        let aligned_sz = (sz + 3) & !3;
        let local_offset = gen.next_local_offset;
        gen.next_local_offset += aligned_sz;

        match target_type.kind() {
            TypeKind::Array => {
                gen.emit_local_array_init(target_type, init, local_offset, &loc);
            }
            TypeKind::Struct => {
                gen.emit_local_struct_init(target_type, init, local_offset, &loc);
            }
            TypeKind::Int
            | TypeKind::Char
            | TypeKind::Float
            | TypeKind::Double
            | TypeKind::LongLong
            | TypeKind::Pointer => {
                if let Expr::InitList { elements, .. } = init.as_mut() {
                    if let Some(elem) = elements.first_mut() {
                        gen.gen_expr(&mut elem.value);
                        gen.emit_scalar_cast_store(target_type, &elem.value, local_offset, &loc);
                    }
                } else {
                    gen.report_error("标量复合字面量必须使用初始化列表", &loc);
                }
            }
            _ => {
                gen.report_error(&format!("复合字面量暂不支持类型 '{}'", target_type), &loc);
            }
        }

        // 复合字面量为 lvalue，在栈顶留下临时对象地址。
        gen.emit(OpCode::GetFrameBase, 0, &loc);
        gen.emit(OpCode::PushConst, local_offset, &loc);
        gen.emit(OpCode::Add, 0, &loc);
        return;
    }
    gen.report_error("非复合字面量进入 compound_literal 生成器", &loc);
    gen.emit(OpCode::PushConst, 0, &loc);
}
