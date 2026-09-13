use super::*;

pub(crate) fn gen_this_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_this(expr, &loc);
}

pub(crate) fn gen_new_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_new(expr, &loc);
}

pub(crate) fn gen_delete_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_delete(expr, &loc);
}

pub(crate) fn gen_move_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_move(expr, &loc);
}

pub(crate) fn gen_lambda_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_lambda(expr, &loc);
}

impl BytecodeGen {
    pub(crate) fn gen_this(&mut self, _expr: &mut Expr, loc: &SourceLoc) {
        let this_offset = self.resolve_local("this");
        if this_offset >= 0 {
            self.emit(OpCode::LoadLocal, this_offset, loc);
        } else {
            self.report_error("this 在静态上下文中不可用", loc);
            self.emit(OpCode::PushConst, 0, loc);
        }
    }

    pub(crate) fn gen_new(&mut self, expr: &mut Expr, loc: &SourceLoc) {
        let Expr::New { elem_type, size_expr, init, .. } = expr else {
            self.report_error("gen_new 期望 New 表达式", loc);
            self.emit(OpCode::PushConst, 0, loc);
            return;
        };

        let elem_sz = self.type_size(elem_type);
        let is_class = elem_type.is_class();

        if let Some(size_expr) = size_expr {
            // new T[n] 数组形式
            self.gen_expr(size_expr);
            if is_class {
                // 需要为 delete[] 保存元素个数：多分配 4 字节，将 count 存于返回地址之前
                let count_temp = self.get_temp_slot(0);
                let ptr_temp = self.get_temp_slot(1);
                let user_ptr_temp = self.get_temp_slot(2);
                let i_temp = self.get_temp_slot(3);

                // count = n
                self.emit(OpCode::Dup, 0, loc);
                self.emit(OpCode::StoreLocal, count_temp, loc);

                // total = n * elem_sz + 4
                self.emit(OpCode::PushConst, elem_sz, loc);
                self.emit(OpCode::Mul, 0, loc);
                self.emit(OpCode::PushConst, 4, loc);
                self.emit(OpCode::Add, 0, loc);
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::MALLOC as i32, loc);

                // ptr = base
                self.emit(OpCode::Dup, 0, loc);
                self.emit(OpCode::StoreLocal, ptr_temp, loc);

                // user_ptr = base + 4
                self.emit(OpCode::PushConst, 4, loc);
                self.emit(OpCode::Add, 0, loc);
                self.emit(OpCode::Dup, 0, loc);
                self.emit(OpCode::StoreLocal, user_ptr_temp, loc);

                // *base = count
                self.emit(OpCode::LoadLocal, ptr_temp, loc);
                self.emit(OpCode::LoadLocal, count_temp, loc);
                self.emit(OpCode::StoreMem, 0, loc);

                // 设置构造守卫：构造失败时 VM 可回滚释放 base 内存。
                self.emit(OpCode::LoadLocal, ptr_temp, loc);
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::SET_ARRAY_GUARD as i32, loc);

                if let Type::Class { name, .. } = elem_type {
                    let ctor_name = if let Some(ref init_expr) = init {
                        if let Expr::Call { name: ctor_name, .. } = init_expr.as_ref() {
                            ctor_name.clone()
                        } else {
                            format!("__ctor__{}", name)
                        }
                    } else {
                        format!("__ctor__{}", name)
                    };
                    if self.func_index.contains_key(&ctor_name) {
                        // for (int i = 0; i < count; i++)
                        self.emit(OpCode::PushConst, 0, loc);
                        self.emit(OpCode::StoreLocal, i_temp, loc);

                        let loop_start = self.current_ip();
                        self.emit(OpCode::LoadLocal, i_temp, loc);
                        self.emit(OpCode::LoadLocal, count_temp, loc);
                        self.emit(OpCode::Lt, 0, loc);
                        let cond_jump = self.current_ip();
                        self.emit(OpCode::JumpIfZero, 0, loc);

                        // this = user_ptr + i * elem_sz
                        self.emit(OpCode::LoadLocal, user_ptr_temp, loc);
                        self.emit(OpCode::LoadLocal, i_temp, loc);
                        self.emit(OpCode::PushConst, elem_sz, loc);
                        self.emit(OpCode::Mul, 0, loc);
                        self.emit(OpCode::Add, 0, loc);
                        if let Some(&idx) = self.func_index.get(&ctor_name) {
                            self.emit(OpCode::Call, idx, loc);
                        }

                        // i++
                        self.emit(OpCode::LoadLocal, i_temp, loc);
                        self.emit(OpCode::PushConst, 1, loc);
                        self.emit(OpCode::Add, 0, loc);
                        self.emit(OpCode::StoreLocal, i_temp, loc);
                        self.emit(OpCode::Jump, loop_start as i32, loc);

                        let loop_end = self.current_ip();
                        self.patch_jump(cond_jump, loop_end);
                    }
                }

                // 构造成功，清除守卫
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::CLEAR_ARRAY_GUARD as i32, loc);

                // 返回 user_ptr
                self.emit(OpCode::LoadLocal, user_ptr_temp, loc);
            } else {
                // 非类类型数组：保持原有行为
                self.emit(OpCode::PushConst, elem_sz, loc);
                self.emit(OpCode::Mul, 0, loc);
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::MALLOC as i32, loc);
            }
        } else {
            // new T 单个对象形式
            self.emit(OpCode::PushConst, elem_sz, loc);
            self.emit(OpCode::CallHost, vitro_runtime::host_func_id::MALLOC as i32, loc);
            if let Type::Class { name, .. } = elem_type {
                let ptr_temp = self.get_temp_slot(0);
                self.emit(OpCode::StoreLocal, ptr_temp, loc);
                // Initialize vptr if class has a vtable
                if let Some(&vtable_offset) = self.class_vtables.get(name) {
                    self.emit(OpCode::LoadLocal, ptr_temp, loc);
                    self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + vtable_offset as i32, loc);
                    self.emit(OpCode::StoreMem, 0, loc);
                }
                let ctor_name = if let Some(ref init_expr) = init {
                    if let Expr::Call { name: ctor_name, .. } = init_expr.as_ref() {
                        ctor_name.clone()
                    } else {
                        format!("__ctor__{}", name)
                    }
                } else {
                    format!("__ctor__{}", name)
                };
                if self.func_index.contains_key(&ctor_name) {
                    if let Some(init_expr) = init {
                        if let Expr::Call { args, .. } = init_expr.as_mut() {
                            for arg in args.iter_mut().rev() {
                                self.gen_expr(arg);
                            }
                        }
                    }
                    self.emit(OpCode::LoadLocal, ptr_temp, loc);
                    if let Some(&idx) = self.func_index.get(&ctor_name) {
                        self.emit(OpCode::Call, idx, loc);
                    }
                    self.emit(OpCode::LoadLocal, ptr_temp, loc);
                } else {
                    self.emit(OpCode::LoadLocal, ptr_temp, loc);
                }
            } else {
                // Non-class type: if init is present, store it directly
                if let Some(init_expr) = init {
                    self.emit(OpCode::Dup, 0, loc);
                    self.gen_expr(init_expr);
                    self.emit(OpCode::StoreMem, 0, loc);
                }
            }
        }
    }

    pub(crate) fn gen_delete(&mut self, expr: &mut Expr, loc: &SourceLoc) {
        let Expr::Delete { expr: inner, is_array, .. } = expr else {
            self.report_error("gen_delete 期望 Delete 表达式", loc);
            return;
        };

        self.gen_expr(inner);

        // C++ 语义：delete/delete[] nullptr 是 no-op（空容器析构高频场景）。
        // 此前未判空：delete[] 的 ptr-4 会 wrap 为 0xFFFFFFFC 再 free，
        // 触发无效 free 诊断。JumpIfZero pop 判断值，null 路径短路到尾部。
        self.emit(OpCode::Dup, 0, loc);
        let null_jump = self.current_ip();
        self.emit(OpCode::JumpIfZero, 0, loc);

        // 判断元素类型是否为 class（支持直接 Class 或 Pointer to Class）
        let inner_ty = inner.ty().clone();
        let (is_class, class_name, elem_type) = if let Type::Class { name, .. } = &inner_ty {
            (true, name.clone(), inner_ty.clone())
        } else if let Type::Pointer { pointee, .. } = &inner_ty {
            if let Type::Class { name, .. } = pointee.as_ref() {
                (true, name.clone(), (**pointee).clone())
            } else {
                (false, String::new(), inner_ty.clone())
            }
        } else {
            (false, String::new(), inner_ty.clone())
        };

        let elem_sz = self.type_size(&elem_type);

        if is_class {
            let dtor_name = format!("__dtor__{}", class_name);
            let has_dtor = self.func_index.contains_key(&dtor_name);
            if *is_array {
                // delete[]: 无论元素是否有显式析构函数，都需要释放 base 地址。
                // new[] 在返回地址前 4 字节保存元素个数；释放时必须 free(base)。
                let user_ptr_temp = self.get_temp_slot(2);
                let base_temp = self.get_temp_slot(1);
                let count_temp = self.get_temp_slot(0);
                let i_temp = self.get_temp_slot(3);

                self.emit(OpCode::Dup, 0, loc);
                self.emit(OpCode::StoreLocal, user_ptr_temp, loc);

                // base = user_ptr - 4
                self.emit(OpCode::PushConst, 4, loc);
                self.emit(OpCode::Sub, 0, loc);

                if has_dtor {
                    self.emit(OpCode::Dup, 0, loc);
                    self.emit(OpCode::StoreLocal, base_temp, loc);

                    // count = *base
                    self.emit(OpCode::LoadMem, 0, loc);
                    self.emit(OpCode::StoreLocal, count_temp, loc);

                    // for (int i = count - 1; i >= 0; i--)
                    self.emit(OpCode::LoadLocal, count_temp, loc);
                    self.emit(OpCode::PushConst, 1, loc);
                    self.emit(OpCode::Sub, 0, loc);
                    self.emit(OpCode::StoreLocal, i_temp, loc);

                    let loop_check = self.current_ip();
                    self.emit(OpCode::LoadLocal, i_temp, loc);
                    self.emit(OpCode::PushConst, 0, loc);
                    self.emit(OpCode::Lt, 0, loc);
                    let cond_jump = self.current_ip();
                    self.emit(OpCode::JumpIfNotZero, 0, loc);

                    // this = user_ptr + i * elem_sz
                    self.emit(OpCode::LoadLocal, user_ptr_temp, loc);
                    self.emit(OpCode::LoadLocal, i_temp, loc);
                    self.emit(OpCode::PushConst, elem_sz, loc);
                    self.emit(OpCode::Mul, 0, loc);
                    self.emit(OpCode::Add, 0, loc);
                    if let Some(&idx) = self.func_index.get(&dtor_name) {
                        self.emit(OpCode::Call, idx, loc);
                    }

                    // i--
                    self.emit(OpCode::LoadLocal, i_temp, loc);
                    self.emit(OpCode::PushConst, 1, loc);
                    self.emit(OpCode::Sub, 0, loc);
                    self.emit(OpCode::StoreLocal, i_temp, loc);
                    self.emit(OpCode::Jump, loop_check as i32, loc);

                    let loop_end = self.current_ip();
                    self.patch_jump(cond_jump, loop_end);

                    // free(base)
                    self.emit(OpCode::LoadLocal, base_temp, loc);
                }

                // free(base): base is already on the stack
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::FREE as i32, loc);
            } else {
                // delete: 直接调用析构函数后 free(ptr)
                if has_dtor {
                    self.emit(OpCode::Dup, 0, loc);
                    if let Some(&idx) = self.func_index.get(&dtor_name) {
                        self.emit(OpCode::Call, idx, loc);
                    }
                }
                self.emit(OpCode::CallHost, vitro_runtime::host_func_id::FREE as i32, loc);
            }
        } else {
            self.emit(OpCode::CallHost, vitro_runtime::host_func_id::FREE as i32, loc);
        }

        // null 短路汇合点：正常路径（free 已消费指针）跳过 Pop；
        // null 路径栈上残留一份 0，Pop 丢弃后与正常路径汇合。
        let skip_pop = self.current_ip();
        self.emit(OpCode::Jump, 0, loc);
        let null_target = self.current_ip();
        self.emit(OpCode::Pop, 0, loc);
        self.patch_jump(skip_pop, self.current_ip());
        self.patch_jump(null_jump, null_target);
    }

    pub(crate) fn gen_move(&mut self, expr: &mut Expr, _loc: &SourceLoc) {
        let Expr::Move { expr: inner, .. } = expr else {
            return;
        };
        self.gen_expr(inner);
    }

    pub(crate) fn gen_lambda(&mut self, expr: &mut Expr, loc: &SourceLoc) {
        let Expr::Lambda { unique_id, capture, .. } = expr else {
            self.report_error("gen_lambda 期望 Lambda 表达式", loc);
            self.emit(OpCode::PushConst, 0, loc);
            return;
        };

        let lambda_name = format!("__lambda_{}", unique_id);
        let mut by_ref_fields = std::collections::HashSet::new();
        for cap in capture.iter() {
            if let CaptureMode::ByReference(name) = cap {
                by_ref_fields.insert(name.clone());
            }
        }
        self.lambda_by_ref_fields.insert(lambda_name.clone(), by_ref_fields);
        let class_decl = self.class_defs.get(&lambda_name);

        // Compute closure size and field offsets (no vptr for lambda)
        let mut class_size = 0i32;
        let mut field_offsets = Vec::new();
        if let Some(decl) = class_decl {
            for member in &decl.members {
                if let ClassMember::Field { name, ty, .. } = member {
                    field_offsets.push((name.clone(), class_size, ty.clone()));
                    class_size += self.type_size(ty);
                }
            }
        }
        class_size = (class_size + 3) & !3;
        // Issue B2：无捕获闭包没有任何字段，size 为 0，但其**地址**仍会被压栈并
        // 作为 `this` 传给 `__lambda_N__call`。保底 4 字节，使闭包对象落在帧内
        // 有效内存（与 lambda 变量槽位的 4 字节口径一致，见 is_lambda_closure_type）。
        if class_size == 0 {
            class_size = 4;
        }

        // Allocate closure on stack as a temporary
        let closure_offset = self.next_local_offset;
        self.next_local_offset += class_size;

        // Initialize capture fields
        for (field_name, field_offset, _field_ty) in field_offsets {
            let cap_mode = capture.iter().find(|cap| match cap {
                CaptureMode::ByValue(n) | CaptureMode::ByReference(n) => n == &field_name,
                _ => false,
            });

            if let Some(cap) = cap_mode {
                // Compute destination address: frame_base + closure_offset + field_offset
                self.emit(OpCode::GetFrameBase, 0, loc);
                self.emit(OpCode::PushConst, closure_offset + field_offset, loc);
                self.emit(OpCode::Add, 0, loc);

                if matches!(cap, CaptureMode::ByReference(_)) {
                    // Store address of captured variable
                    if let Some(&local_offset) = self.local_indices.get(&field_name) {
                        self.emit(OpCode::GetFrameBase, 0, loc);
                        self.emit(OpCode::PushConst, local_offset, loc);
                        self.emit(OpCode::Add, 0, loc);
                    } else if let Some(&global_offset) = self.global_indices.get(&field_name) {
                        self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + global_offset, loc);
                    } else if let Some(&static_offset) = self.static_local_indices.get(&field_name) {
                        self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + static_offset, loc);
                    } else {
                        self.report_error(&format!("Lambda 捕获变量 '{}' 未找到", field_name), loc);
                        self.emit(OpCode::PushConst, 0, loc);
                    }
                } else {
                    // Store value of captured variable
                    let mut id_expr = Expr::Identifier {
                        name: field_name.clone(),
                        loc: *loc,
                        ty: Type::int(),
                    };
                    self.gen_expr(&mut id_expr);
                }

                self.emit(OpCode::StoreMem, 0, loc);
            }
        }

        // Push closure address
        self.emit(OpCode::GetFrameBase, 0, loc);
        self.emit(OpCode::PushConst, closure_offset, loc);
        self.emit(OpCode::Add, 0, loc);
    }
}
