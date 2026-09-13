use super::*;
use crate::is_lambda_closure_type;

pub(crate) fn gen_call(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Call { name, args, ty, .. } = expr {
        let ret_temp_offset = if ty.is_struct() || ty.is_class() {
            let sz = (gen.type_size(ty) + 3) & !3;
            let offset = gen.next_local_offset;
            gen.next_local_offset += sz;
            Some(offset)
        } else {
            None
        };
        let is_variadic_callee =
            gen.func_table.get(name).map(|m| m.is_variadic).unwrap_or(false) && gen.func_index.contains_key(name);
        // 记录需要在本条完整表达式结束后销毁的临时类对象（按值/按 const 引用传递的右值）。
        let mut temp_cleanups: Vec<(i32, String)> = Vec::new();
        for arg in args.iter_mut().rev() {
            let arg_ty_kind = arg.ty().kind();
            let arg_ty = arg.ty().clone();
            // const T& / T& 绑定到按值返回的类右值时，先生成临时对象、记录析构，再传地址。
            if arg_ty.is_reference() {
                if let Expr::Unary { op: UnaryOp::Addr, operand, .. } = arg {
                    let (class_name, has_dtor) = {
                        let operand_ty = operand.ty();
                        if (operand_ty.is_class() || operand_ty.is_struct())
                            && matches!(operand.as_ref(), Expr::Call { .. } | Expr::CallPtr { .. })
                        {
                            let class_name = operand_ty.name().to_string();
                            let dtor_name = format!("__dtor__{}", class_name);
                            let has = gen.func_index.contains_key(&dtor_name);
                            (class_name, has)
                        } else {
                            (String::new(), false)
                        }
                    };
                    if has_dtor {
                        let cleanup_slot = gen.next_local_offset;
                        gen.next_local_offset += 4;
                        gen.gen_expr(operand);
                        gen.emit(OpCode::StoreLocal, cleanup_slot, &loc);
                        temp_cleanups.push((cleanup_slot, class_name));
                        gen.emit(OpCode::LoadLocal, cleanup_slot, &loc);
                    } else {
                        gen.gen_expr(arg);
                    }
                    continue;
                }
            }
            if arg_ty.is_struct() || arg_ty.is_class() {
                let is_lambda = is_lambda_closure_type(&arg_ty);
                let sz = gen.type_size(&arg_ty);
                // lambda 以闭包对象地址（1 word）传参，与闭包类字段大小无关（Issue B2）
                let words = if is_lambda { 1 } else { (sz + 3) / 4 };

                // 按值传递的类右值来自函数返回的临时对象：直接复用其隐藏返回缓冲区，
                // 并在本完整表达式结束时调用析构函数，避免内存泄漏。
                if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                    let class_name = arg_ty.name().to_string();
                    let dtor_name = format!("__dtor__{}", class_name);
                    let cleanup_slot = gen.next_local_offset;
                    gen.next_local_offset += 4;
                    gen.gen_expr(arg);
                    gen.emit(OpCode::StoreLocal, cleanup_slot, &loc);
                    if gen.func_index.contains_key(&dtor_name) {
                        temp_cleanups.push((cleanup_slot, class_name));
                    }
                    for i in 0..words {
                        gen.emit(OpCode::LoadLocal, cleanup_slot, &loc);
                        if i > 0 {
                            gen.emit(OpCode::PushConst, i * 4, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                        }
                        gen.emit(OpCode::LoadMem, 0, &loc);
                    }
                    continue;
                }

                // 如果类有自定义拷贝构造函数且实参是左值，先调用拷贝构造生成临时对象，
                // 再按值传递该临时对象。这避免了按字段位拷贝导致的双重释放问题。
                if !is_lambda {
                    if let Type::Class { name: class_name, .. } = arg_ty {
                        let copy_ctor_name = format!("__ctor__{}__copy", class_name);
                        if gen.func_index.contains_key(&copy_ctor_name) && is_lvalue_expr(arg) {
                            let aligned_sz = (sz + 3) & !3;
                            let temp_offset = gen.next_local_offset;
                            gen.next_local_offset += aligned_sz;
                            // other = source address
                            gen.gen_addr(arg, &loc);
                            // this = temp address
                            gen.emit(OpCode::GetFrameBase, 0, &loc);
                            gen.emit(OpCode::PushConst, temp_offset, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                            if let Some(&idx) = gen.func_index.get(&copy_ctor_name) {
                                gen.emit(OpCode::Call, idx, &loc);
                            }
                            // push temp object words onto the stack for the outer call
                            for i in 0..words {
                                gen.emit(OpCode::GetFrameBase, 0, &loc);
                                gen.emit(OpCode::PushConst, temp_offset + i * 4, &loc);
                                gen.emit(OpCode::Add, 0, &loc);
                                gen.emit(OpCode::LoadMem, 0, &loc);
                            }
                            continue;
                        }
                    }
                }

                if let Expr::Identifier { name: arg_name, .. } = arg {
                    if let Some(&offset) = gen.local_indices.get(arg_name) {
                        if is_lambda {
                            // Lambda closure variables store the closure address, not the object itself.
                            gen.emit(OpCode::LoadLocal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadLocal, offset + i * 4, &loc);
                            }
                        }
                    } else if let Some(&offset) = gen.static_local_indices.get(arg_name) {
                        if is_lambda {
                            gen.emit(OpCode::LoadGlobal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadGlobal, offset + i * 4, &loc);
                            }
                        }
                    } else if let Some(&offset) = gen.global_indices.get(arg_name) {
                        if is_lambda {
                            gen.emit(OpCode::LoadGlobal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadGlobal, offset + i * 4, &loc);
                            }
                        }
                    } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                        gen.gen_expr(arg);
                        let addr_temp = gen.get_temp_slot(0);
                        gen.emit(OpCode::StoreLocal, addr_temp, &loc);
                        for i in 0..words {
                            gen.emit(OpCode::LoadLocal, addr_temp, &loc);
                            if i > 0 {
                                gen.emit(OpCode::PushConst, i * 4, &loc);
                                gen.emit(OpCode::Add, 0, &loc);
                            }
                            gen.emit(OpCode::LoadMem, 0, &loc);
                        }
                    } else {
                        gen.gen_expr(arg);
                        for _ in 1..words {
                            gen.emit(OpCode::PushConst, 0, &loc);
                        }
                    }
                } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                    gen.gen_expr(arg);
                    let addr_temp = gen.get_temp_slot(0);
                    gen.emit(OpCode::StoreLocal, addr_temp, &loc);
                    for i in 0..words {
                        gen.emit(OpCode::LoadLocal, addr_temp, &loc);
                        if i > 0 {
                            gen.emit(OpCode::PushConst, i * 4, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                        }
                        gen.emit(OpCode::LoadMem, 0, &loc);
                    }
                } else {
                    gen.gen_expr(arg);
                    for _ in 1..words {
                        gen.emit(OpCode::PushConst, 0, &loc);
                    }
                }
            } else if arg_ty.kind() == TypeKind::Double {
                gen.gen_expr(arg);
                if gen.func_index.contains_key(name) {
                    if is_variadic_callee {
                        // 变参函数：按 VM 变参循环顺序 push（低地址在前），
                        // 因此先压入高 32 位，再压入低 32 位。
                        let temp0 = gen.get_temp_slot(0);
                        let _temp1 = gen.get_temp_slot(1);
                        gen.emit(OpCode::StoreLocalD, temp0, &loc);
                        gen.emit(OpCode::LoadLocal, temp0 + 4, &loc);
                        gen.emit(OpCode::LoadLocal, temp0, &loc);
                    } else {
                        gen.emit(OpCode::SplitD, 0, &loc);
                    }
                }
            } else if arg_ty.kind() == TypeKind::LongLong {
                gen.gen_expr(arg);
                if gen.func_index.contains_key(name) {
                    if is_variadic_callee {
                        let temp0 = gen.get_temp_slot(0);
                        let _temp1 = gen.get_temp_slot(1);
                        gen.emit(OpCode::StoreLocalQ, temp0, &loc);
                        gen.emit(OpCode::LoadLocal, temp0 + 4, &loc);
                        gen.emit(OpCode::LoadLocal, temp0, &loc);
                    } else {
                        gen.emit(OpCode::SplitQ, 0, &loc);
                    }
                }
            } else {
                gen.gen_expr(arg);
                if (name == "printf" || name == "fprintf") && arg_ty_kind == TypeKind::Float {
                    gen.emit(OpCode::CastF2D, 0, &loc);
                }
            }
        }
        if let Some(offset) = ret_temp_offset {
            gen.emit(OpCode::GetFrameBase, 0, &loc);
            gen.emit(OpCode::PushConst, offset, &loc);
            gen.emit(OpCode::Add, 0, &loc);
        }
        if let Some(&idx) = gen.func_index.get(name) {
            if let Some(meta) = gen.func_table.get(name) {
                if meta.is_variadic {
                    let named_words: i32 = meta.param_sizes.iter().sum();
                    let user_named_count = if meta.return_type.is_struct() || meta.return_type.is_class() {
                        (meta.param_count - 1).max(0)
                    } else {
                        meta.param_count
                    };
                    let extra_variadic_words: i32 = args[user_named_count as usize..]
                        .iter()
                        .map(|a| (gen.type_size(a.ty()) + 3) / 4)
                        .sum();
                    let total_arg_words = named_words + extra_variadic_words;
                    gen.emit(OpCode::PushConst, total_arg_words, &loc);
                    gen.emit(OpCode::CallVar, idx, &loc);
                } else {
                    gen.emit(OpCode::Call, idx, &loc);
                }
            } else {
                gen.emit(OpCode::Call, idx, &loc);
            }
        } else {
            if let Some(host_id) = vitro_runtime::host_func_id::by_user_name(name.as_str()) {
                gen.emit(OpCode::CallHost, host_id as i32, &loc);
            } else {
                gen.report_error(&format!("未定义的函数 '{}'", name), &loc);
                gen.emit(OpCode::PushConst, 0, &loc);
            }
        }
        // 销毁本函数调用中为按值/按引用传递而生成的临时类对象。
        // 构造顺序与参数求值顺序一致，析构按逆序进行。
        for (slot, class_name) in temp_cleanups.iter().rev() {
            let dtor_name = format!("__dtor__{}", class_name);
            if let Some(&idx) = gen.func_index.get(&dtor_name) {
                // slot 中保存的是临时对象本身的地址，直接作为 this 传入析构函数。
                gen.emit(OpCode::LoadLocal, *slot, &loc);
                gen.emit(OpCode::Call, idx, &loc);
            }
        }
        if let Some(offset) = ret_temp_offset {
            gen.emit(OpCode::GetFrameBase, 0, &loc);
            gen.emit(OpCode::PushConst, offset, &loc);
            gen.emit(OpCode::Add, 0, &loc);
        }
    }
}

pub(crate) fn gen_call_ptr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::CallPtr { callee, args, ty, .. } = expr {
        // std::move(x) is a compile-time cast to RValueRef; no function call.
        if let Expr::Identifier { name, .. } = callee.as_ref() {
            if name == "std__move" && args.len() == 1 {
                gen.gen_expr(&mut args[0]);
                return;
            }
        }
        let ret_temp_offset = if ty.is_struct() || ty.is_class() {
            let sz = (gen.type_size(ty) + 3) & !3;
            let offset = gen.next_local_offset;
            gen.next_local_offset += sz;
            Some(offset)
        } else {
            None
        };
        // Determine if this CallPtr will resolve to a user function call (needs SplitD/SplitQ)
        // or a host/built-in call (passes 64-bit values directly).
        let is_user_call = if let Expr::Identifier { name, .. } = callee.as_ref() {
            gen.func_index.contains_key(name) || gen.resolve_host_func_id(name) < 0
        } else {
            true // indirect calls are always user calls
        };
        let is_variadic_callee = if let Expr::Identifier { name, .. } = callee.as_ref() {
            gen.func_index.contains_key(name) && gen.func_table.get(name).map(|m| m.is_variadic).unwrap_or(false)
        } else {
            false
        };
        for arg in args.iter_mut().rev() {
            let arg_ty = arg.ty().clone();
            if arg_ty.is_struct() || arg_ty.is_class() {
                let is_lambda = is_lambda_closure_type(&arg_ty);
                let sz = gen.type_size(&arg_ty);
                // lambda 以闭包对象地址（1 word）传参，与闭包类字段大小无关（Issue B2）
                let words = if is_lambda { 1 } else { (sz + 3) / 4 };
                if let Expr::Identifier { name: arg_name, .. } = arg {
                    if let Some(&offset) = gen.local_indices.get(arg_name) {
                        if is_lambda {
                            gen.emit(OpCode::LoadLocal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadLocal, offset + i * 4, &loc);
                            }
                        }
                    } else if let Some(&offset) = gen.static_local_indices.get(arg_name) {
                        if is_lambda {
                            gen.emit(OpCode::LoadGlobal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadGlobal, offset + i * 4, &loc);
                            }
                        }
                    } else if let Some(&offset) = gen.global_indices.get(arg_name) {
                        if is_lambda {
                            gen.emit(OpCode::LoadGlobal, offset, &loc);
                        } else {
                            for i in 0..words {
                                gen.emit(OpCode::LoadGlobal, offset + i * 4, &loc);
                            }
                        }
                    } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                        gen.gen_expr(arg);
                        let addr_temp = gen.get_temp_slot(0);
                        gen.emit(OpCode::StoreLocal, addr_temp, &loc);
                        for i in 0..words {
                            gen.emit(OpCode::LoadLocal, addr_temp, &loc);
                            if i > 0 {
                                gen.emit(OpCode::PushConst, i * 4, &loc);
                                gen.emit(OpCode::Add, 0, &loc);
                            }
                            gen.emit(OpCode::LoadMem, 0, &loc);
                        }
                    } else {
                        gen.gen_expr(arg);
                        for _ in 1..words {
                            gen.emit(OpCode::PushConst, 0, &loc);
                        }
                    }
                } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                    gen.gen_expr(arg);
                    let addr_temp = gen.get_temp_slot(0);
                    gen.emit(OpCode::StoreLocal, addr_temp, &loc);
                    for i in 0..words {
                        gen.emit(OpCode::LoadLocal, addr_temp, &loc);
                        if i > 0 {
                            gen.emit(OpCode::PushConst, i * 4, &loc);
                            gen.emit(OpCode::Add, 0, &loc);
                        }
                        gen.emit(OpCode::LoadMem, 0, &loc);
                    }
                } else {
                    gen.gen_expr(arg);
                    for _ in 1..words {
                        gen.emit(OpCode::PushConst, 0, &loc);
                    }
                }
            } else if arg_ty.kind() == TypeKind::Double {
                gen.gen_expr(arg);
                if is_user_call {
                    if is_variadic_callee {
                        // 变参函数：按 VM 变参循环顺序 push（低地址在前），
                        // 因此先压入高 32 位，再压入低 32 位。
                        let temp0 = gen.get_temp_slot(0);
                        let _temp1 = gen.get_temp_slot(1);
                        gen.emit(OpCode::StoreLocalD, temp0, &loc);
                        gen.emit(OpCode::LoadLocal, temp0 + 4, &loc);
                        gen.emit(OpCode::LoadLocal, temp0, &loc);
                    } else {
                        gen.emit(OpCode::SplitD, 0, &loc);
                    }
                }
            } else if arg_ty.kind() == TypeKind::LongLong {
                gen.gen_expr(arg);
                if is_user_call {
                    if is_variadic_callee {
                        let temp0 = gen.get_temp_slot(0);
                        let _temp1 = gen.get_temp_slot(1);
                        gen.emit(OpCode::StoreLocalQ, temp0, &loc);
                        gen.emit(OpCode::LoadLocal, temp0 + 4, &loc);
                        gen.emit(OpCode::LoadLocal, temp0, &loc);
                    } else {
                        gen.emit(OpCode::SplitQ, 0, &loc);
                    }
                }
            } else {
                gen.gen_expr(arg);
                if let Expr::Identifier { name, .. } = callee.as_ref() {
                    if (name == "printf" || name == "fprintf") && arg_ty.kind() == TypeKind::Float {
                        gen.emit(OpCode::CastF2D, 0, &loc);
                    }
                }
            }
        }
        if let Some(offset) = ret_temp_offset {
            gen.emit(OpCode::GetFrameBase, 0, &loc);
            gen.emit(OpCode::PushConst, offset, &loc);
            gen.emit(OpCode::Add, 0, &loc);
        }
        if let Expr::Identifier { name, .. } = callee.as_ref() {
            if let Some(&idx) = gen.func_index.get(name) {
                if let Some(meta) = gen.func_table.get(name) {
                    if meta.is_variadic {
                        let named_words: i32 = meta.param_sizes.iter().sum();
                        let user_named_count = if meta.return_type.is_struct() || meta.return_type.is_class() {
                            (meta.param_count - 1).max(0)
                        } else {
                            meta.param_count
                        };
                        let extra_variadic_words: i32 = args[user_named_count as usize..]
                            .iter()
                            .map(|a| (gen.type_size(a.ty()) + 3) / 4)
                            .sum();
                        let total_arg_words = named_words + extra_variadic_words;
                        gen.emit(OpCode::PushConst, total_arg_words, &loc);
                        gen.emit(OpCode::CallVar, idx, &loc);
                    } else {
                        gen.emit(OpCode::Call, idx, &loc);
                    }
                } else {
                    gen.emit(OpCode::Call, idx, &loc);
                }
                if let Some(offset) = ret_temp_offset {
                    gen.emit(OpCode::GetFrameBase, 0, &loc);
                    gen.emit(OpCode::PushConst, offset, &loc);
                    gen.emit(OpCode::Add, 0, &loc);
                }
                return;
            }
            // Host function: direct CallHost
            let host_id = gen.resolve_host_func_id(name);
            if host_id >= 0 {
                gen.emit(OpCode::CallHost, host_id, &loc);
                if let Some(offset) = ret_temp_offset {
                    gen.emit(OpCode::GetFrameBase, 0, &loc);
                    gen.emit(OpCode::PushConst, offset, &loc);
                    gen.emit(OpCode::Add, 0, &loc);
                }
                return;
            }
        }
        gen.gen_expr(callee);
        gen.emit(OpCode::CallPtr, args.len() as i32, &loc);
        if let Some(offset) = ret_temp_offset {
            gen.emit(OpCode::GetFrameBase, 0, &loc);
            gen.emit(OpCode::PushConst, offset, &loc);
            gen.emit(OpCode::Add, 0, &loc);
        }
    }
}
