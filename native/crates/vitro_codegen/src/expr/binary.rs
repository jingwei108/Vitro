use super::*;

// TODO(#D08): gen_binary 接近 300 行，未来可按运算符大类（算术/比较/位运算/指针）拆分。
pub(crate) fn gen_binary(
    gen: &mut BytecodeGen,
    op: &BinaryOp,
    left: &mut Expr,
    right: &mut Expr,
    ty: &Type,
    loc: &SourceLoc,
) {
    let left_is_ptr = left.ty().is_pointer() || left.ty().is_array();
    let right_is_ptr = right.ty().is_pointer() || right.ty().is_array();
    let result_is_double = ty.kind() == TypeKind::Double;
    let result_is_float = ty.kind() == TypeKind::Float;
    let result_is_long_long = ty.kind() == TypeKind::LongLong;
    let any_fp = result_is_double || result_is_float;

    // For comparison ops, result type is always int, so we must look at operand types.
    let is_comparison = matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    );
    let op_is_double = if is_comparison {
        left.ty().kind() == TypeKind::Double || right.ty().kind() == TypeKind::Double
    } else {
        result_is_double
    };
    let op_is_float = if is_comparison {
        !op_is_double && (left.ty().kind() == TypeKind::Float || right.ty().kind() == TypeKind::Float)
    } else {
        result_is_float
    };
    let op_is_long_long = if is_comparison {
        !op_is_double
            && !op_is_float
            && (left.ty().kind() == TypeKind::LongLong || right.ty().kind() == TypeKind::LongLong)
    } else {
        result_is_long_long
    };
    let any_op_fp = op_is_double || op_is_float;
    let is_unsigned = if is_comparison {
        (matches!(left.ty().kind(), TypeKind::Int | TypeKind::Char) && left.ty().is_unsigned())
            || (matches!(right.ty().kind(), TypeKind::Int | TypeKind::Char) && right.ty().is_unsigned())
    } else {
        matches!(ty.kind(), TypeKind::Int | TypeKind::Char) && ty.is_unsigned()
    };

    // Short-circuit evaluation for && and ||
    if *op == BinaryOp::And || *op == BinaryOp::Or {
        gen.gen_expr(left);
        match left.ty().kind() {
            TypeKind::Float => gen.emit(OpCode::CastF2I, 0, loc),
            TypeKind::Double => gen.emit(OpCode::CastD2I, 0, loc),
            TypeKind::LongLong => gen.emit(OpCode::CastQ2I, 0, loc),
            _ => {}
        }
        gen.emit(OpCode::Dup, 0, loc);
        let end_jump = gen.current_ip();
        if *op == BinaryOp::And {
            gen.emit(OpCode::JumpIfZero, 0, loc);
        } else {
            gen.emit(OpCode::JumpIfNotZero, 0, loc);
        }
        gen.emit(OpCode::Pop, 0, loc);
        gen.gen_expr(right);
        match right.ty().kind() {
            TypeKind::Float => gen.emit(OpCode::CastF2I, 0, loc),
            TypeKind::Double => gen.emit(OpCode::CastD2I, 0, loc),
            TypeKind::LongLong => gen.emit(OpCode::CastQ2I, 0, loc),
            _ => {}
        }
        let end_ip = gen.current_ip();
        gen.patch_jump(end_jump, end_ip);
        // T-P1-1：C 标准要求 && / || 的结果规范化为 0/1，
        // 此前短路结构把操作数原始值留作结果（5&&3 得 3、0||7 得 7）
        gen.emit(OpCode::PushConst, 0, loc);
        gen.emit(OpCode::Ne, 0, loc);
        return;
    }

    gen.gen_expr(left);
    let any_fp_for_cast = if is_comparison { any_op_fp } else { any_fp };
    let cast_is_double = if is_comparison { op_is_double } else { result_is_double };
    let cast_is_long_long = if is_comparison {
        op_is_long_long
    } else {
        result_is_long_long
    };
    if any_fp_for_cast
        && !left_is_ptr
        && left.ty().kind() != TypeKind::Float
        && left.ty().kind() != TypeKind::Double
        && left.ty().kind() != TypeKind::LongLong
    {
        if cast_is_double {
            gen.emit(OpCode::CastI2D, 0, loc);
        } else {
            gen.emit(OpCode::CastI2F, 0, loc);
        }
    } else if cast_is_double && left.ty().kind() == TypeKind::Float {
        gen.emit(OpCode::CastF2D, 0, loc);
    } else if cast_is_double && left.ty().kind() == TypeKind::LongLong {
        gen.emit(OpCode::CastQ2D, 0, loc);
    } else if cast_is_long_long && left.ty().kind() == TypeKind::Int {
        gen.emit(OpCode::CastI2Q, 0, loc);
    }
    gen.gen_expr(right);
    if any_fp_for_cast
        && !right_is_ptr
        && right.ty().kind() != TypeKind::Float
        && right.ty().kind() != TypeKind::Double
        && right.ty().kind() != TypeKind::LongLong
    {
        if cast_is_double {
            gen.emit(OpCode::CastI2D, 0, loc);
        } else {
            gen.emit(OpCode::CastI2F, 0, loc);
        }
    } else if cast_is_double && right.ty().kind() == TypeKind::Float {
        gen.emit(OpCode::CastF2D, 0, loc);
    } else if cast_is_double && right.ty().kind() == TypeKind::LongLong {
        gen.emit(OpCode::CastQ2D, 0, loc);
    } else if cast_is_long_long && right.ty().kind() == TypeKind::Int {
        gen.emit(OpCode::CastI2Q, 0, loc);
    }

    match op {
        BinaryOp::Add => {
            if left_is_ptr && !right_is_ptr {
                let step = gen.ptr_step_size(left.ty());
                gen.emit(OpCode::PushConst, step, loc);
                gen.emit(OpCode::Mul, 0, loc);
                gen.emit(OpCode::Add, 0, loc);
            } else if !left_is_ptr && right_is_ptr {
                let step = gen.ptr_step_size(right.ty());
                gen.emit(OpCode::Swap, 0, loc);
                gen.emit(OpCode::PushConst, step, loc);
                gen.emit(OpCode::Mul, 0, loc);
                gen.emit(OpCode::Swap, 0, loc);
                gen.emit(OpCode::Add, 0, loc);
            } else if result_is_double {
                gen.emit(OpCode::AddD, 0, loc);
            } else if result_is_float {
                gen.emit(OpCode::AddF, 0, loc);
            } else if result_is_long_long {
                gen.emit(OpCode::AddQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UAdd, 0, loc);
            } else {
                gen.emit(OpCode::Add, 0, loc);
            }
        }
        BinaryOp::Sub => {
            if left_is_ptr && right_is_ptr {
                let step = gen.ptr_step_size(left.ty());
                gen.emit(OpCode::Sub, 0, loc);
                gen.emit(OpCode::PushConst, step, loc);
                gen.emit(OpCode::Div, 0, loc);
            } else if left_is_ptr && !right_is_ptr {
                let step = gen.ptr_step_size(left.ty());
                gen.emit(OpCode::PushConst, step, loc);
                gen.emit(OpCode::Mul, 0, loc);
                gen.emit(OpCode::Sub, 0, loc);
            } else if result_is_double {
                gen.emit(OpCode::SubD, 0, loc);
            } else if result_is_float {
                gen.emit(OpCode::SubF, 0, loc);
            } else if result_is_long_long {
                gen.emit(OpCode::SubQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::USub, 0, loc);
            } else {
                gen.emit(OpCode::Sub, 0, loc);
            }
        }
        BinaryOp::Mul => {
            if result_is_double {
                gen.emit(OpCode::MulD, 0, loc);
            } else if result_is_float {
                gen.emit(OpCode::MulF, 0, loc);
            } else if result_is_long_long {
                gen.emit(OpCode::MulQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UMul, 0, loc);
            } else {
                gen.emit(OpCode::Mul, 0, loc);
            }
        }
        BinaryOp::Div => {
            if result_is_double {
                gen.emit(OpCode::DivD, 0, loc);
            } else if result_is_float {
                gen.emit(OpCode::DivF, 0, loc);
            } else if result_is_long_long {
                gen.emit(OpCode::DivQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UDiv, 0, loc);
            } else {
                gen.emit(OpCode::Div, 0, loc);
            }
        }
        BinaryOp::Mod => {
            if result_is_long_long {
                gen.emit(OpCode::ModQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UMod, 0, loc);
            } else {
                gen.emit(OpCode::Mod, 0, loc);
            }
        }
        BinaryOp::Eq => {
            if op_is_double {
                gen.emit(OpCode::EqD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::EqF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::EqQ, 0, loc);
            } else {
                gen.emit(OpCode::Eq, 0, loc);
            }
        }
        BinaryOp::Ne => {
            if op_is_double {
                gen.emit(OpCode::NeD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::NeF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::NeQ, 0, loc);
            } else {
                gen.emit(OpCode::Ne, 0, loc);
            }
        }
        BinaryOp::Lt => {
            if op_is_double {
                gen.emit(OpCode::LtD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::LtF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::LtQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::ULt, 0, loc);
            } else {
                gen.emit(OpCode::Lt, 0, loc);
            }
        }
        BinaryOp::Le => {
            if op_is_double {
                gen.emit(OpCode::LeD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::LeF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::LeQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::ULe, 0, loc);
            } else {
                gen.emit(OpCode::Le, 0, loc);
            }
        }
        BinaryOp::Gt => {
            if op_is_double {
                gen.emit(OpCode::GtD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::GtF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::GtQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UGt, 0, loc);
            } else {
                gen.emit(OpCode::Gt, 0, loc);
            }
        }
        BinaryOp::Ge => {
            if op_is_double {
                gen.emit(OpCode::GeD, 0, loc);
            } else if op_is_float {
                gen.emit(OpCode::GeF, 0, loc);
            } else if op_is_long_long {
                gen.emit(OpCode::GeQ, 0, loc);
            } else if is_unsigned {
                gen.emit(OpCode::UGe, 0, loc);
            } else {
                gen.emit(OpCode::Ge, 0, loc);
            }
        }
        BinaryOp::BitAnd => {
            // E1 B 档：long long 位运算走 64 位指令（Int 操作数已由上方
            // cast_is_long_long 机制 CastI2Q 扩展）
            if result_is_long_long {
                gen.emit(OpCode::BitAndQ, 0, loc);
            } else {
                gen.emit(OpCode::BitAnd, 0, loc);
            }
        }
        BinaryOp::BitOr => {
            if result_is_long_long {
                gen.emit(OpCode::BitOrQ, 0, loc);
            } else {
                gen.emit(OpCode::BitOr, 0, loc);
            }
        }
        BinaryOp::BitXor => {
            if result_is_long_long {
                gen.emit(OpCode::BitXorQ, 0, loc);
            } else {
                gen.emit(OpCode::BitXor, 0, loc);
            }
        }
        BinaryOp::Shl => {
            if result_is_long_long {
                gen.emit(OpCode::ShlQ, 0, loc);
            } else {
                gen.emit(OpCode::Shl, 0, loc);
            }
        }
        BinaryOp::Shr => {
            if result_is_long_long {
                // unsigned long long 用逻辑右移（Type::is_unsigned 覆盖 LongLong 变体）
                if ty.is_unsigned() {
                    gen.emit(OpCode::LShrQ, 0, loc);
                } else {
                    gen.emit(OpCode::ShrQ, 0, loc);
                }
            } else if is_unsigned {
                gen.emit(OpCode::LShr, 0, loc);
            } else {
                gen.emit(OpCode::Shr, 0, loc);
            }
        }
        BinaryOp::And | BinaryOp::Or => {} // handled above
        BinaryOp::Comma => {
            // Stack: ... left_result right_result
            // Discard left, keep right
            gen.emit(OpCode::Swap, 0, loc);
            gen.emit(OpCode::Pop, 0, loc);
        }
    }
}
