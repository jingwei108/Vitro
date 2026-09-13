use super::*;

/// Whether the type can be stored in a temporary local slot for `&rvalue`.
fn can_materialize_temporary(ty: &Type) -> bool {
    matches!(
        ty.kind(),
        TypeKind::Int | TypeKind::Char | TypeKind::Float | TypeKind::Double | TypeKind::LongLong | TypeKind::Pointer
    )
}

/// T-P0-3/T-P0-5：按元素类型选择自增/自减的读取-修改-写回 opcode。
/// 此前固定 LoadMem/StoreMem + int Add：double 元素读 4 字节垃圾、
/// char 元素 4 字节读改写把进位污染到相邻元素。
fn gen_mem_inc_dec(gen: &mut BytecodeGen, is_inc: bool, is_pre: bool, step: i32, elem_kind: TypeKind, loc: &SourceLoc) {
    // stack top: address
    let addr_temp = gen.get_temp_slot(2);
    gen.emit(OpCode::StoreLocal, addr_temp, loc); // save address

    let (load_op, store_op) = match elem_kind {
        TypeKind::Double => (OpCode::LoadMemD, OpCode::StoreMemD),
        // float 内存元素按 4 字节 f32 位模式存取（与 gen_index 语义一致），
        // 算术经 CastF2D/CastD2F 转换链在 double 值域进行
        TypeKind::Float => (OpCode::LoadMem, OpCode::StoreMem),
        TypeKind::LongLong => (OpCode::LoadMemQ, OpCode::StoreMemQ),
        TypeKind::Char => (OpCode::LoadMemByte, OpCode::StoreMemByte),
        _ => (OpCode::LoadMem, OpCode::StoreMem),
    };

    gen.emit(OpCode::LoadLocal, addr_temp, loc);
    gen.emit(load_op, 0, loc); // read current value
    if !is_pre {
        gen.emit(OpCode::Dup, 0, loc); // keep old value for post（float 保持 f32 位模式）
    }
    if elem_kind == TypeKind::Float {
        gen.emit(OpCode::CastF2D, 0, loc);
    }
    match elem_kind {
        TypeKind::Double | TypeKind::Float => {
            let idx = gen.push_f64_constant(step as f64);
            gen.emit(OpCode::PushConstD, idx, loc);
            gen.emit(if is_inc { OpCode::AddD } else { OpCode::SubD }, 0, loc);
        }
        TypeKind::LongLong => {
            let idx = gen.push_i64_constant(step as i64);
            gen.emit(OpCode::PushConstQ, idx, loc);
            gen.emit(if is_inc { OpCode::AddQ } else { OpCode::SubQ }, 0, loc);
        }
        _ => {
            gen.emit(OpCode::PushConst, step, loc);
            gen.emit(if is_inc { OpCode::Add } else { OpCode::Sub }, 0, loc);
        }
    }
    if elem_kind == TypeKind::Float {
        gen.emit(OpCode::CastD2F, 0, loc);
    }
    // 保存新值到 temp 槽：double/long long 的 64 位位模式必须用 8 字节专用槽
    // （4 字节槽会被 64 位写踩踏；temp_slot_64 按函数重置，见 enter_function）
    let val_temp = if matches!(elem_kind, TypeKind::Double | TypeKind::LongLong) {
        gen.get_temp_slot_64()
    } else {
        gen.get_temp_slot(3)
    };
    let (save_op, restore_op) = match elem_kind {
        TypeKind::Double | TypeKind::LongLong => (OpCode::StoreLocalD, OpCode::LoadLocalD),
        _ => (OpCode::StoreLocal, OpCode::LoadLocal),
    };
    gen.emit(save_op, val_temp, loc); // save new value
    gen.emit(OpCode::LoadLocal, addr_temp, loc);
    gen.emit(restore_op, val_temp, loc);
    gen.emit(store_op, 0, loc); // write new value
    if is_pre {
        gen.emit(OpCode::LoadLocal, addr_temp, loc);
        gen.emit(load_op, 0, loc); // return new value
    }
    // for post, old value is already on stack
}

pub(crate) fn gen_unary(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Unary { op, operand, .. } = expr {
        match op {
            UnaryOp::Neg => {
                gen.gen_expr(operand);
                if operand.ty().kind() == TypeKind::Double {
                    gen.emit(OpCode::NegD, 0, &loc);
                } else if operand.ty().kind() == TypeKind::Float {
                    gen.emit(OpCode::NegF, 0, &loc);
                } else if operand.ty().kind() == TypeKind::LongLong {
                    gen.emit(OpCode::NegQ, 0, &loc);
                } else if operand.ty().is_unsigned() {
                    gen.emit(OpCode::UNeg, 0, &loc);
                } else {
                    gen.emit(OpCode::Neg, 0, &loc);
                }
            }
            UnaryOp::Not => {
                gen.gen_expr(operand);
                gen.emit(OpCode::Not, 0, &loc);
            }
            UnaryOp::BitNot => {
                gen.gen_expr(operand);
                // E1 B 档：long long 操作数走 64 位取反
                if operand.ty().kind() == TypeKind::LongLong {
                    gen.emit(OpCode::BitNotQ, 0, &loc);
                } else {
                    gen.emit(OpCode::BitNot, 0, &loc);
                }
            }
            UnaryOp::Addr => {
                match operand.as_mut() {
                    Expr::Identifier { name, .. } => {
                        if let Some(&offset) = gen.local_indices.get(name) {
                            gen.emit(OpCode::GetFrameBase, 0, &loc);
                            gen.emit(OpCode::PushConst, offset, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                        } else if let Some(&offset) = gen.static_local_indices.get(name) {
                            gen.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + offset, &loc);
                        } else if let Some(&offset) = gen.global_indices.get(name) {
                            gen.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + offset, &loc);
                        } else if let Some(&idx) = gen.func_index.get(name) {
                            // &func_name — 取函数地址
                            gen.emit(OpCode::PushConst, idx, &loc);
                        } else {
                            gen.report_error("取地址暂不支持此表达式", &loc);
                            gen.emit(OpCode::PushConst, 0, &loc);
                        }
                    }
                    Expr::Index { array, index, ty, .. } => {
                        gen.gen_index(array, index, ty, &loc, true);
                    }
                    Expr::Member { object, member, .. } => {
                        gen.gen_member_addr(object, member, &loc);
                    }
                    Expr::Unary {
                        op: UnaryOp::Deref,
                        operand: inner,
                        ..
                    } => {
                        gen.gen_expr(inner);
                    }
                    Expr::Call { ty, .. } | Expr::CallPtr { ty, .. } if ty.is_struct() || ty.is_class() => {
                        // 函数按值返回结构体/类时，gen_expr 已经压入隐藏返回缓冲区的地址。
                        // 取地址直接复用该地址即可，避免再包一层临时变量导致源地址错位。
                        gen.gen_expr(operand);
                    }
                    Expr::CompoundLiteral { .. } => {
                        // 复合字面量是 lvalue，gen_expr 已在栈顶留下临时对象地址
                        gen.gen_expr(operand);
                    }
                    _ => {
                        // Materialize a temporary for rvalues when taking their address.
                        // This enables `const T&` parameters to bind to literals / temporaries.
                        let operand_ty = operand.ty().clone();
                        if can_materialize_temporary(&operand_ty) {
                            let sz = gen.type_size(&operand_ty);
                            let offset = gen.next_local_offset;
                            gen.next_local_offset += (sz + 3) & !3;
                            gen.gen_expr(operand);
                            match operand_ty.kind() {
                                TypeKind::Double => gen.emit(OpCode::StoreLocalD, offset, &loc),
                                TypeKind::LongLong => gen.emit(OpCode::StoreLocalQ, offset, &loc),
                                _ => gen.emit(OpCode::StoreLocal, offset, &loc),
                            }
                            gen.emit(OpCode::GetFrameBase, 0, &loc);
                            gen.emit(OpCode::PushConst, offset, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                        } else {
                            gen.report_error("取地址暂不支持此表达式", &loc);
                            gen.emit(OpCode::PushConst, 0, &loc);
                        }
                    }
                }
            }
            UnaryOp::Deref => {
                gen.gen_expr(operand);
                let base_ty = if operand.ty().is_pointer() {
                    immediate_base_kind(operand.ty())
                } else {
                    TypeKind::Int
                };
                if base_ty == TypeKind::Function {
                    // Function pointer dereference: *fp yields the function itself,
                    // which immediately decays back to the same pointer. No load.
                } else if base_ty == TypeKind::Char {
                    gen.emit(OpCode::LoadMemByte, 0, &loc);
                } else if base_ty == TypeKind::Double {
                    gen.emit(OpCode::LoadMemD, 0, &loc);
                } else if base_ty == TypeKind::LongLong {
                    gen.emit(OpCode::LoadMemQ, 0, &loc);
                } else {
                    gen.emit(OpCode::LoadMem, 0, &loc);
                }
            }
            UnaryOp::PreInc | UnaryOp::PostInc | UnaryOp::PreDec | UnaryOp::PostDec => {
                let is_inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
                let is_pre = matches!(op, UnaryOp::PreInc | UnaryOp::PreDec);
                match operand.as_mut() {
                    Expr::Identifier { name, .. } => {
                        // T-P0-3：浮点/long long 变量的 ++/-- 此前固定走 int
                        // LoadLocal+Add 路径，double d++ 完全无效。按变量类型分派。
                        // float 局部/全局槽存 f32 位模式（与 gen_expr 读取端一致），
                        // 算术经 CastF2D/CastD2F 转换链在 double 值域进行。
                        let var_ty = gen
                            .local_types
                            .get(name)
                            .or_else(|| gen.static_local_types.get(name))
                            .or_else(|| gen.global_types.get(name))
                            .cloned();
                        let step = var_ty.as_ref().map(|t| gen.ptr_step_size(t)).unwrap_or(1);
                        let kind = var_ty.as_ref().map(|t| t.kind()).unwrap_or(TypeKind::Int);
                        // 统一序列：load →(float: CastF2D)→ postDup → +1 →(float: CastD2F)→ preDup → store
                        let emit_seq = |gen: &mut BytecodeGen, load_op: OpCode, idx: i32, store_op: OpCode| {
                            gen.emit(load_op, idx, &loc);
                            if kind == TypeKind::Float {
                                gen.emit(OpCode::CastF2D, 0, &loc);
                            }
                            if !is_pre {
                                gen.emit(OpCode::Dup, 0, &loc);
                            }
                            match kind {
                                TypeKind::Double | TypeKind::Float => {
                                    let ci = gen.push_f64_constant(step as f64);
                                    gen.emit(OpCode::PushConstD, ci, &loc);
                                    gen.emit(if is_inc { OpCode::AddD } else { OpCode::SubD }, 0, &loc);
                                }
                                TypeKind::LongLong => {
                                    let ci = gen.push_i64_constant(step as i64);
                                    gen.emit(OpCode::PushConstQ, ci, &loc);
                                    gen.emit(if is_inc { OpCode::AddQ } else { OpCode::SubQ }, 0, &loc);
                                }
                                _ => {
                                    gen.emit(OpCode::PushConst, step, &loc);
                                    gen.emit(if is_inc { OpCode::Add } else { OpCode::Sub }, 0, &loc);
                                }
                            }
                            if kind == TypeKind::Float {
                                gen.emit(OpCode::CastD2F, 0, &loc);
                            }
                            if is_pre {
                                gen.emit(OpCode::Dup, 0, &loc);
                            }
                            gen.emit(store_op, idx, &loc);
                        };
                        let local_ops = match kind {
                            TypeKind::Double => (OpCode::LoadLocalD, OpCode::StoreLocalD),
                            TypeKind::Float => (OpCode::LoadLocal, OpCode::StoreLocal),
                            TypeKind::LongLong => (OpCode::LoadLocalQ, OpCode::StoreLocalQ),
                            _ => (OpCode::LoadLocal, OpCode::StoreLocal),
                        };
                        let global_ops = match kind {
                            TypeKind::Double => (OpCode::LoadGlobalD, OpCode::StoreGlobalD),
                            TypeKind::Float => (OpCode::LoadGlobal, OpCode::StoreGlobal),
                            TypeKind::LongLong => (OpCode::LoadGlobalQ, OpCode::StoreGlobalQ),
                            _ => (OpCode::LoadGlobal, OpCode::StoreGlobal),
                        };
                        if let Some(&static_idx) = gen.static_local_indices.get(name) {
                            emit_seq(gen, global_ops.0, static_idx, global_ops.1);
                        } else {
                            let local_idx = gen.resolve_local(name);
                            if local_idx >= 0 {
                                emit_seq(gen, local_ops.0, local_idx, local_ops.1);
                            } else {
                                let global_idx = gen.resolve_global(name);
                                if global_idx >= 0 {
                                    emit_seq(gen, global_ops.0, global_idx, global_ops.1);
                                } else {
                                    gen.report_error("自增/自减暂只支持简单变量", &loc);
                                    gen.emit(OpCode::PushConst, 0, &loc);
                                }
                            }
                        }
                    }
                    Expr::Index { array, index, ty, .. } => {
                        let result_ty = ty.clone();
                        let step = gen.ptr_step_size(ty);
                        gen.gen_index(array, index, &result_ty, &loc, true);
                        gen_mem_inc_dec(gen, is_inc, is_pre, step, ty.kind(), &loc);
                    }
                    Expr::Member { object, member, ty, .. } => {
                        let step = gen.ptr_step_size(ty);
                        gen.gen_member_addr(object, member, &loc);
                        gen_mem_inc_dec(gen, is_inc, is_pre, step, ty.kind(), &loc);
                    }
                    Expr::Unary {
                        op: UnaryOp::Deref,
                        operand: inner,
                        ..
                    } => {
                        // (*p)++ 是对 pointee 标量自增，步长恒为 1
                        // （指针自身的 p++ 走 Identifier 分支，才按 pointee 大小缩放；
                        //  此前误传 ptr_step_size 导致 (*double_p)++ 加 8）
                        let elem_kind = if inner.ty().is_pointer() {
                            immediate_base_kind(inner.ty())
                        } else {
                            TypeKind::Int
                        };
                        gen.gen_expr(inner);
                        gen_mem_inc_dec(gen, is_inc, is_pre, 1, elem_kind, &loc);
                    }
                    _ => {
                        gen.report_error("自增/自减暂只支持简单变量", &loc);
                        gen.emit(OpCode::PushConst, 0, &loc);
                    }
                }
            }
        }
    }
}
