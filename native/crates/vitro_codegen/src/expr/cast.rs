use super::*;

pub(crate) fn gen_cast_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Cast { expr: inner, target_type, .. } = expr {
        gen.gen_expr(inner);
        if target_type.kind() == TypeKind::Double
            && inner.ty().kind() != TypeKind::Float
            && inner.ty().kind() != TypeKind::Double
            && inner.ty().kind() != TypeKind::LongLong
        {
            gen.emit(OpCode::CastI2D, 0, &loc);
        } else if target_type.kind() == TypeKind::Double && inner.ty().kind() == TypeKind::Float {
            gen.emit(OpCode::CastF2D, 0, &loc);
        } else if target_type.kind() == TypeKind::Double && inner.ty().kind() == TypeKind::LongLong {
            gen.emit(OpCode::CastQ2D, 0, &loc);
        } else if target_type.kind() == TypeKind::Float
            && inner.ty().kind() != TypeKind::Float
            && inner.ty().kind() != TypeKind::Double
            && inner.ty().kind() != TypeKind::LongLong
        {
            gen.emit(OpCode::CastI2F, 0, &loc);
        } else if target_type.kind() == TypeKind::Float && inner.ty().kind() == TypeKind::Double {
            gen.emit(OpCode::CastD2F, 0, &loc);
        } else if target_type.kind() == TypeKind::LongLong
            && inner.ty().kind() != TypeKind::LongLong
            && inner.ty().kind() != TypeKind::Double
            && inner.ty().kind() != TypeKind::Float
        {
            gen.emit(OpCode::CastI2Q, 0, &loc);
        } else if target_type.kind() == TypeKind::LongLong && inner.ty().kind() == TypeKind::Double {
            gen.emit(OpCode::CastD2Q, 0, &loc);
        } else if target_type.kind() != TypeKind::Float
            && target_type.kind() != TypeKind::Double
            && target_type.kind() != TypeKind::LongLong
            && inner.ty().kind() == TypeKind::Double
        {
            gen.emit(OpCode::CastD2I, 0, &loc);
        } else if target_type.kind() != TypeKind::Float
            && target_type.kind() != TypeKind::Double
            && target_type.kind() != TypeKind::LongLong
            && inner.ty().kind() == TypeKind::Float
        {
            gen.emit(OpCode::CastF2I, 0, &loc);
        } else if target_type.kind() != TypeKind::Float
            && target_type.kind() != TypeKind::Double
            && target_type.kind() != TypeKind::LongLong
            && inner.ty().kind() == TypeKind::LongLong
        {
            gen.emit(OpCode::CastQ2I, 0, &loc);
        }
    }
}

pub(crate) fn gen_sizeof_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Sizeof { target_type, operand, .. } = expr {
        let is_vla = target_type.as_ref().map(|t| t.is_vla()).unwrap_or(false)
            || operand.as_ref().map(|op| op.ty().is_vla()).unwrap_or(false);
        if is_vla {
            let array_info = if let Some(ref op) = operand {
                if let Type::Array { dims, vla_dims, .. } = op.ty() {
                    Some((dims.clone(), vla_dims.clone(), gen.elem_type_size(op.ty())))
                } else {
                    None
                }
            } else if let Some(ref t) = target_type {
                if let Type::Array { dims, vla_dims, .. } = t {
                    Some((dims.clone(), vla_dims.clone(), gen.elem_type_size(t)))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some((dims, mut vla_dims, elem_size)) = array_info {
                if dims.is_empty() {
                    gen.emit(OpCode::PushConst, 0, &loc);
                } else {
                    let mut vla_idx = 0;
                    for &dim in dims.iter() {
                        if dim > 0 {
                            gen.emit(OpCode::PushConst, dim, &loc);
                        } else if let Some(dim_expr) = vla_dims.get_mut(vla_idx) {
                            gen.gen_expr(dim_expr);
                            vla_idx += 1;
                        } else {
                            gen.emit(OpCode::PushConst, 0, &loc);
                        }
                    }
                    for _ in 1..dims.len() {
                        gen.emit(OpCode::Mul, 0, &loc);
                    }
                    if elem_size > 1 {
                        gen.emit(OpCode::PushConst, elem_size, &loc);
                        gen.emit(OpCode::Mul, 0, &loc);
                    }
                }
            } else {
                gen.emit(OpCode::PushConst, 4, &loc);
            }
        } else {
            let size = if let Some(ref t) = target_type {
                gen.type_size(t)
            } else if let Some(ref op) = operand {
                gen.type_size(op.ty())
            } else {
                0
            };
            gen.emit(OpCode::PushConst, size, &loc);
        }
    }
}

/// C23 alignof（E1）：对齐值由 [`BytecodeGen::type_align`] 计算（VLA 本体按
/// 指针 4 字节对齐，无 sizeof 的 VLA 维度展开特例）。
pub(crate) fn gen_alignof_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Alignof { target_type, operand, .. } = expr {
        let align = if let Some(ref t) = target_type {
            gen.type_align(t)
        } else if let Some(ref op) = operand {
            gen.type_align(op.ty())
        } else {
            4
        };
        gen.emit(OpCode::PushConst, align, &loc);
    }
}

pub(crate) fn gen_offsetof_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Offsetof { target_type, field, .. } = expr {
        let mut offset = 0;
        let mut found = false;
        if let Some(fields) = gen.struct_defs.get(target_type.name()) {
            for f in fields {
                if f.name == *field {
                    found = true;
                    break;
                }
                offset += gen.type_size(&f.ty);
            }
        } else if let Some(fields) = gen.union_defs.get(target_type.name()) {
            if fields.iter().any(|f| f.name == *field) {
                offset = 0;
                found = true;
            }
        }
        if !found {
            gen.report_error(
                &format!("offsetof: 未知的结构体/联合体 '{}' 或字段 '{}'", target_type.name(), field),
                &loc,
            );
        }
        gen.emit(OpCode::PushConst, offset, &loc);
    }
}

impl BytecodeGen {
    pub(crate) fn gen_expr_with_cast(&mut self, expr: &mut Expr, target_ty: &Type, loc: &SourceLoc) {
        self.gen_expr(expr);
        let target_is_fp = matches!(target_ty.kind(), TypeKind::Float | TypeKind::Double);
        let target_is_double = target_ty.kind() == TypeKind::Double;
        let target_is_long_long = target_ty.kind() == TypeKind::LongLong;
        if target_is_double
            && expr.ty().kind() != TypeKind::Float
            && expr.ty().kind() != TypeKind::Double
            && expr.ty().kind() != TypeKind::LongLong
        {
            self.emit(OpCode::CastI2D, 0, loc);
        } else if target_is_double && expr.ty().kind() == TypeKind::Float {
            self.emit(OpCode::CastF2D, 0, loc);
        } else if target_is_double && expr.ty().kind() == TypeKind::LongLong {
            self.emit(OpCode::CastQ2D, 0, loc);
        } else if target_is_fp
            && expr.ty().kind() != TypeKind::Float
            && expr.ty().kind() != TypeKind::Double
            && expr.ty().kind() != TypeKind::LongLong
        {
            self.emit(OpCode::CastI2F, 0, loc);
        } else if !target_is_fp && !target_is_long_long && expr.ty().kind() == TypeKind::Double {
            self.emit(OpCode::CastD2I, 0, loc);
        } else if !target_is_fp && !target_is_long_long && expr.ty().kind() == TypeKind::Float {
            self.emit(OpCode::CastF2I, 0, loc);
        } else if !target_is_fp && !target_is_long_long && expr.ty().kind() == TypeKind::LongLong {
            self.emit(OpCode::CastQ2I, 0, loc);
        }
    }
}
