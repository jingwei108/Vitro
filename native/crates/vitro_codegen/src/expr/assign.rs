use super::*;

pub(crate) fn gen_ternary_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Ternary {
        cond, then_branch, else_branch, ..
    } = expr
    {
        gen.gen_expr(cond);
        let else_jump = gen.current_ip();
        gen.emit(OpCode::JumpIfZero, 0, &loc);
        gen.gen_expr(then_branch);
        let end_jump = gen.current_ip();
        gen.emit(OpCode::Jump, 0, &loc);
        let else_ip = gen.current_ip();
        gen.patch_jump(else_jump, else_ip);
        gen.gen_expr(else_branch);
        let end_ip = gen.current_ip();
        gen.patch_jump(end_jump, end_ip);
    }
}

pub(crate) fn gen_assign_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Assign { op, left, right, .. } = expr {
        gen.gen_assign(op, left, right, &loc);
    }
}

impl BytecodeGen {
    // TODO(#D08): gen_assign 超过 500 行，未来可按赋值目标类型（标量/结构体/数组）拆分子函数。
    /// T-P0-6：入口递增嵌套深度，使各层赋值的目标地址槽互不冲突后调用 inner。
    pub(crate) fn gen_assign(&mut self, op: &AssignOp, left: &mut Expr, right: &mut Expr, loc: &SourceLoc) {
        self.assign_nest_depth += 1;
        self.gen_assign_inner(op, left, right, loc);
        self.assign_nest_depth -= 1;
    }

    #[allow(clippy::too_many_lines)]
    fn gen_assign_inner(&mut self, op: &AssignOp, left: &mut Expr, right: &mut Expr, loc: &SourceLoc) {
        let left_ty = left.ty().clone();
        let left_is_double = left_ty.kind() == TypeKind::Double;
        let left_is_float = left_ty.kind() == TypeKind::Float;
        let left_is_long_long = left_ty.kind() == TypeKind::LongLong;
        // T-P0-4：char 左值按 1 字节读写（Member 分支此前缺 char 分支写 4 字节）
        let left_is_char = left_ty.kind() == TypeKind::Char;
        let left_is_unsigned = left_ty.is_unsigned();
        let left_is_ptr = left_ty.is_pointer();
        let is_ptr_compound = matches!(*op, AssignOp::AddAssign | AssignOp::SubAssign);
        let ptr_step = if left_is_ptr {
            if let Type::Pointer { pointee, .. } = &left_ty {
                if pointee.is_void() {
                    1
                } else {
                    self.ptr_step_size(&left_ty)
                }
            } else {
                self.ptr_step_size(&left_ty)
            }
        } else {
            0
        };
        let mut gen_right = |this: &mut Self| {
            if left_is_ptr && is_ptr_compound {
                this.gen_expr(right);
            } else {
                this.gen_expr_with_cast(right, &left_ty, loc);
            }
        };
        if (left.ty().is_struct() || left.ty().is_class()) && *op == AssignOp::Assign {
            self.gen_struct_copy(left, right, loc);
            return;
        }
        let emit_compound = |this: &mut Self, loc: &SourceLoc| match op {
            AssignOp::AddAssign => {
                if left_is_ptr {
                    this.emit(OpCode::PushConst, ptr_step, loc);
                    this.emit(OpCode::Mul, 0, loc);
                    this.emit(OpCode::Add, 0, loc);
                } else if left_is_double {
                    this.emit(OpCode::AddD, 0, loc);
                } else if left_is_float {
                    this.emit(OpCode::AddF, 0, loc);
                } else if left_is_long_long {
                    this.emit(OpCode::AddQ, 0, loc);
                } else if left_is_unsigned {
                    this.emit(OpCode::UAdd, 0, loc);
                } else {
                    this.emit(OpCode::Add, 0, loc);
                }
            }
            AssignOp::SubAssign => {
                if left_is_ptr {
                    this.emit(OpCode::PushConst, ptr_step, loc);
                    this.emit(OpCode::Mul, 0, loc);
                    this.emit(OpCode::Sub, 0, loc);
                } else if left_is_double {
                    this.emit(OpCode::SubD, 0, loc);
                } else if left_is_float {
                    this.emit(OpCode::SubF, 0, loc);
                } else if left_is_long_long {
                    this.emit(OpCode::SubQ, 0, loc);
                } else if left_is_unsigned {
                    this.emit(OpCode::USub, 0, loc);
                } else {
                    this.emit(OpCode::Sub, 0, loc);
                }
            }
            AssignOp::MulAssign => {
                if left_is_double {
                    this.emit(OpCode::MulD, 0, loc);
                } else if left_is_float {
                    this.emit(OpCode::MulF, 0, loc);
                } else if left_is_long_long {
                    this.emit(OpCode::MulQ, 0, loc);
                } else if left_is_unsigned {
                    this.emit(OpCode::UMul, 0, loc);
                } else {
                    this.emit(OpCode::Mul, 0, loc);
                }
            }
            AssignOp::DivAssign => {
                if left_is_double {
                    this.emit(OpCode::DivD, 0, loc);
                } else if left_is_float {
                    this.emit(OpCode::DivF, 0, loc);
                } else if left_is_long_long {
                    this.emit(OpCode::DivQ, 0, loc);
                } else if left_is_unsigned {
                    this.emit(OpCode::UDiv, 0, loc);
                } else {
                    this.emit(OpCode::Div, 0, loc);
                }
            }
            AssignOp::ModAssign => {
                if left_is_long_long {
                    this.emit(OpCode::ModQ, 0, loc);
                } else if left_is_unsigned {
                    this.emit(OpCode::UMod, 0, loc);
                } else {
                    this.emit(OpCode::Mod, 0, loc);
                }
            }
            AssignOp::AndAssign => {
                this.emit(OpCode::BitAnd, 0, loc);
            }
            AssignOp::OrAssign => {
                this.emit(OpCode::BitOr, 0, loc);
            }
            AssignOp::XorAssign => {
                this.emit(OpCode::BitXor, 0, loc);
            }
            AssignOp::ShlAssign => {
                this.emit(OpCode::Shl, 0, loc);
            }
            AssignOp::ShrAssign => {
                if left_is_unsigned {
                    this.emit(OpCode::LShr, 0, loc);
                } else {
                    this.emit(OpCode::Shr, 0, loc);
                }
            }
            _ => {}
        };

        // C++ 引用返回的函数调用 / 成员调用作为左值：调用结果已在栈顶留下目标地址
        if (matches!(left, Expr::Call { .. } | Expr::CallPtr { .. } | Expr::MemberCall { .. }))
            && (left.ty().is_reference() || left.ty().is_rvalue_ref())
        {
            if *op != AssignOp::Assign {
                self.report_error("复合赋值暂不支持引用返回的调用结果", loc);
            }
            let base_ty = left.ty().reference_base().cloned().unwrap_or(Type::int());
            self.gen_addr(left, loc);
            self.emit(OpCode::Dup, 0, loc);
            let addr_temp = self.get_assign_addr_slot();
            self.emit(OpCode::StoreLocal, addr_temp, loc);
            self.gen_expr_with_cast(right, &base_ty, loc);
            match base_ty.kind() {
                TypeKind::Char => self.emit(OpCode::StoreMemByte, 0, loc),
                TypeKind::Double => self.emit(OpCode::StoreMemD, 0, loc),
                TypeKind::LongLong => self.emit(OpCode::StoreMemQ, 0, loc),
                _ => self.emit(OpCode::StoreMem, 0, loc),
            }
            self.emit(OpCode::LoadLocal, addr_temp, loc);
            match base_ty.kind() {
                TypeKind::Char => self.emit(OpCode::LoadMemByte, 0, loc),
                TypeKind::Double => self.emit(OpCode::LoadMemD, 0, loc),
                TypeKind::LongLong => self.emit(OpCode::LoadMemQ, 0, loc),
                _ => self.emit(OpCode::LoadMem, 0, loc),
            }
            return;
        }

        if let Expr::Identifier { name, .. } = left {
            let is_ref = self
                .local_types
                .get(name)
                .map(|t| t.is_reference() || t.is_rvalue_ref())
                .unwrap_or(false)
                || self
                    .global_types
                    .get(name)
                    .map(|t| t.is_reference() || t.is_rvalue_ref())
                    .unwrap_or(false)
                || self
                    .static_local_types
                    .get(name)
                    .map(|t| t.is_reference() || t.is_rvalue_ref())
                    .unwrap_or(false);
            if is_ref {
                let base_ty = self
                    .local_types
                    .get(name)
                    .or_else(|| self.global_types.get(name))
                    .or_else(|| self.static_local_types.get(name))
                    .and_then(|t| t.reference_base().cloned())
                    .unwrap_or(Type::int());
                let base_is_double = base_ty.kind() == TypeKind::Double;
                let base_is_float = base_ty.kind() == TypeKind::Float;
                let base_is_long_long = base_ty.kind() == TypeKind::LongLong;
                self.gen_addr(left, loc);
                if *op != AssignOp::Assign {
                    self.emit(OpCode::Dup, 0, loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::LoadMemByte, 0, loc),
                        TypeKind::Double => self.emit(OpCode::LoadMemD, 0, loc),
                        TypeKind::LongLong => self.emit(OpCode::LoadMemQ, 0, loc),
                        _ => self.emit(OpCode::LoadMem, 0, loc),
                    }
                    self.emit(OpCode::Swap, 0, loc);
                    let addr_temp = self.get_assign_addr_slot();
                    self.emit(OpCode::StoreLocal, addr_temp, loc);
                    self.gen_expr_with_cast(right, &base_ty, loc);
                    match op {
                        AssignOp::AddAssign => {
                            if base_is_double {
                                self.emit(OpCode::AddD, 0, loc);
                            } else if base_is_float {
                                self.emit(OpCode::AddF, 0, loc);
                            } else if base_ty.is_unsigned() {
                                self.emit(OpCode::UAdd, 0, loc);
                            } else {
                                self.emit(OpCode::Add, 0, loc);
                            }
                        }
                        AssignOp::SubAssign => {
                            if base_is_double {
                                self.emit(OpCode::SubD, 0, loc);
                            } else if base_is_float {
                                self.emit(OpCode::SubF, 0, loc);
                            } else if base_ty.is_unsigned() {
                                self.emit(OpCode::USub, 0, loc);
                            } else {
                                self.emit(OpCode::Sub, 0, loc);
                            }
                        }
                        AssignOp::MulAssign => {
                            if base_is_double {
                                self.emit(OpCode::MulD, 0, loc);
                            } else if base_is_float {
                                self.emit(OpCode::MulF, 0, loc);
                            } else if base_ty.is_unsigned() {
                                self.emit(OpCode::UMul, 0, loc);
                            } else {
                                self.emit(OpCode::Mul, 0, loc);
                            }
                        }
                        AssignOp::DivAssign => {
                            if base_is_double {
                                self.emit(OpCode::DivD, 0, loc);
                            } else if base_is_float {
                                self.emit(OpCode::DivF, 0, loc);
                            } else if base_ty.is_unsigned() {
                                self.emit(OpCode::UDiv, 0, loc);
                            } else {
                                self.emit(OpCode::Div, 0, loc);
                            }
                        }
                        AssignOp::ModAssign => {
                            if base_is_long_long {
                                self.emit(OpCode::ModQ, 0, loc);
                            } else if base_ty.is_unsigned() {
                                self.emit(OpCode::UMod, 0, loc);
                            } else {
                                self.emit(OpCode::Mod, 0, loc);
                            }
                        }
                        AssignOp::AndAssign => {
                            self.emit(OpCode::BitAnd, 0, loc);
                        }
                        AssignOp::OrAssign => {
                            self.emit(OpCode::BitOr, 0, loc);
                        }
                        AssignOp::XorAssign => {
                            self.emit(OpCode::BitXor, 0, loc);
                        }
                        AssignOp::ShlAssign => {
                            self.emit(OpCode::Shl, 0, loc);
                        }
                        AssignOp::ShrAssign => {
                            if base_ty.is_unsigned() {
                                self.emit(OpCode::LShr, 0, loc);
                            } else {
                                self.emit(OpCode::Shr, 0, loc);
                            }
                        }
                        _ => {}
                    }
                    self.emit(OpCode::LoadLocal, addr_temp, loc);
                    self.emit(OpCode::Swap, 0, loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::StoreMemByte, 0, loc),
                        TypeKind::Double => self.emit(OpCode::StoreMemD, 0, loc),
                        TypeKind::LongLong => self.emit(OpCode::StoreMemQ, 0, loc),
                        _ => self.emit(OpCode::StoreMem, 0, loc),
                    }
                    self.emit(OpCode::LoadLocal, addr_temp, loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::LoadMemByte, 0, loc),
                        TypeKind::Double => self.emit(OpCode::LoadMemD, 0, loc),
                        TypeKind::LongLong => self.emit(OpCode::LoadMemQ, 0, loc),
                        _ => self.emit(OpCode::LoadMem, 0, loc),
                    }
                } else {
                    self.emit(OpCode::Dup, 0, loc);
                    let addr_temp = self.get_assign_addr_slot();
                    self.emit(OpCode::StoreLocal, addr_temp, loc);
                    self.gen_expr_with_cast(right, &base_ty, loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::StoreMemByte, 0, loc),
                        TypeKind::Double => self.emit(OpCode::StoreMemD, 0, loc),
                        TypeKind::LongLong => self.emit(OpCode::StoreMemQ, 0, loc),
                        _ => self.emit(OpCode::StoreMem, 0, loc),
                    }
                    self.emit(OpCode::LoadLocal, addr_temp, loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::LoadMemByte, 0, loc),
                        TypeKind::Double => self.emit(OpCode::LoadMemD, 0, loc),
                        TypeKind::LongLong => self.emit(OpCode::LoadMemQ, 0, loc),
                        _ => self.emit(OpCode::LoadMem, 0, loc),
                    }
                }
                return;
            }
            if let Some(&static_offset) = self.static_local_indices.get(name) {
                if *op != AssignOp::Assign {
                    if left_is_double {
                        self.emit(OpCode::LoadGlobalD, static_offset, loc);
                    } else if left_is_long_long {
                        self.emit(OpCode::LoadGlobalQ, static_offset, loc);
                    } else {
                        self.emit(OpCode::LoadGlobal, static_offset, loc);
                    }
                    gen_right(self);
                    emit_compound(self, loc);
                } else {
                    gen_right(self);
                }
                if left_is_double {
                    self.emit(OpCode::StoreGlobalD, static_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreGlobalQ, static_offset, loc);
                } else {
                    self.emit(OpCode::StoreGlobal, static_offset, loc);
                }
                if left_is_double {
                    self.emit(OpCode::LoadGlobalD, static_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadGlobalQ, static_offset, loc);
                } else {
                    self.emit(OpCode::LoadGlobal, static_offset, loc);
                }
                return;
            }
            let local_offset = self.resolve_local(name);
            if local_offset >= 0 {
                if *op != AssignOp::Assign {
                    if left_is_double {
                        self.emit(OpCode::LoadLocalD, local_offset, loc);
                    } else if left_is_long_long {
                        self.emit(OpCode::LoadLocalQ, local_offset, loc);
                    } else {
                        self.emit(OpCode::LoadLocal, local_offset, loc);
                    }
                    gen_right(self);
                    emit_compound(self, loc);
                } else {
                    gen_right(self);
                }
                if left_is_double {
                    self.emit(OpCode::StoreLocalD, local_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreLocalQ, local_offset, loc);
                } else {
                    self.emit(OpCode::StoreLocal, local_offset, loc);
                }
                if left_is_double {
                    self.emit(OpCode::LoadLocalD, local_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadLocalQ, local_offset, loc);
                } else {
                    self.emit(OpCode::LoadLocal, local_offset, loc);
                }
                return;
            }
            let global_offset = self.resolve_global(name);
            if global_offset >= 0 {
                if *op != AssignOp::Assign {
                    if left_is_double {
                        self.emit(OpCode::LoadGlobalD, global_offset, loc);
                    } else if left_is_long_long {
                        self.emit(OpCode::LoadGlobalQ, global_offset, loc);
                    } else {
                        self.emit(OpCode::LoadGlobal, global_offset, loc);
                    }
                    gen_right(self);
                    emit_compound(self, loc);
                } else {
                    gen_right(self);
                }
                if left_is_double {
                    self.emit(OpCode::StoreGlobalD, global_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreGlobalQ, global_offset, loc);
                } else {
                    self.emit(OpCode::StoreGlobal, global_offset, loc);
                }
                if left_is_double {
                    self.emit(OpCode::LoadGlobalD, global_offset, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadGlobalQ, global_offset, loc);
                } else {
                    self.emit(OpCode::LoadGlobal, global_offset, loc);
                }
                return;
            }
        } else if let Expr::Index { array, index, ty, .. } = left {
            let result_ty = ty.clone();
            self.gen_index(array, index, &result_ty, loc, true);
            if *op != AssignOp::Assign {
                self.emit(OpCode::Dup, 0, loc);
                if result_ty.kind() == TypeKind::Char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if result_ty.kind() == TypeKind::Double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if result_ty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
                self.emit(OpCode::Swap, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                emit_compound(self, loc);
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                self.emit(OpCode::Swap, 0, loc);
                if result_ty.kind() == TypeKind::Char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if result_ty.kind() == TypeKind::Double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if result_ty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if result_ty.kind() == TypeKind::Char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if result_ty.kind() == TypeKind::Double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if result_ty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            } else {
                self.emit(OpCode::Dup, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                if result_ty.kind() == TypeKind::Char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if result_ty.kind() == TypeKind::Double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if result_ty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if result_ty.kind() == TypeKind::Char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if result_ty.kind() == TypeKind::Double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if result_ty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            }
            return;
        } else if let Expr::Unary {
            op: UnaryOp::Deref, operand, ..
        } = left
        {
            self.gen_expr(operand);
            let left_is_char = left.ty().kind() == TypeKind::Char;
            if *op != AssignOp::Assign {
                self.emit(OpCode::Dup, 0, loc);
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
                self.emit(OpCode::Swap, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                emit_compound(self, loc);
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                self.emit(OpCode::Swap, 0, loc);
                if left_is_char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            } else {
                self.emit(OpCode::Dup, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                if left_is_char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            }
            return;
        } else if let Expr::Member { object, member, .. } = left {
            self.gen_member_addr(object, member, loc);
            // Lambda by-reference capture: load the captured pointer so StoreMem writes through it
            if object.ty().is_pointer() {
                if let Type::Pointer { pointee, .. } = object.ty() {
                    if let Type::Class { name, .. } = pointee.as_ref() {
                        if let Some(by_ref_fields) = self.lambda_by_ref_fields.get(name) {
                            if by_ref_fields.contains(member) {
                                self.emit(OpCode::LoadMem, 0, loc);
                            }
                        }
                    }
                }
            }
            if *op != AssignOp::Assign {
                self.emit(OpCode::Dup, 0, loc);
                // T-P0-4：char 成员按 1 字节读写，此前 StoreMem 写 4 字节
                // 破坏相邻内存（gq.c='B' 越界覆盖后续全局变量）
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
                self.emit(OpCode::Swap, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                emit_compound(self, loc);
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                self.emit(OpCode::Swap, 0, loc);
                if left_is_char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            } else {
                self.emit(OpCode::Dup, 0, loc);
                let addr_temp = self.get_assign_addr_slot();
                self.emit(OpCode::StoreLocal, addr_temp, loc);
                gen_right(self);
                if left_is_char {
                    self.emit(OpCode::StoreMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::StoreMemQ, 0, loc);
                } else {
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                self.emit(OpCode::LoadLocal, addr_temp, loc);
                if left_is_char {
                    self.emit(OpCode::LoadMemByte, 0, loc);
                } else if left_is_double {
                    self.emit(OpCode::LoadMemD, 0, loc);
                } else if left_is_long_long {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                } else {
                    self.emit(OpCode::LoadMem, 0, loc);
                }
            }
            return;
        }

        self.report_error("赋值目标不支持", loc);
        gen_right(self);
        self.emit(OpCode::Pop, 0, loc);
        self.emit(OpCode::PushConst, 0, loc);
    }
}
