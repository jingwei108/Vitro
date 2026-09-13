use super::*;
use crate::is_lambda_closure_type;

pub(crate) fn gen_member_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    if let Expr::Member { object, member, ty, .. } = expr {
        gen.gen_member_addr(object, member, &loc);
        // Lambda by-reference capture: need to load the captured pointer first
        if object.ty().is_pointer() {
            if let Type::Pointer { pointee, .. } = object.ty() {
                if let Type::Class { name, .. } = pointee.as_ref() {
                    if let Some(by_ref_fields) = gen.lambda_by_ref_fields.get(name) {
                        if by_ref_fields.contains(member) {
                            gen.emit(OpCode::LoadMem, 0, &loc);
                        }
                    }
                }
            }
        }
        if !ty.is_array() {
            if ty.kind() == TypeKind::Char {
                gen.emit(OpCode::LoadMemByte, 0, &loc);
            } else if ty.kind() == TypeKind::Double {
                gen.emit(OpCode::LoadMemD, 0, &loc);
            } else if ty.kind() == TypeKind::LongLong {
                gen.emit(OpCode::LoadMemQ, 0, &loc);
            } else {
                gen.emit(OpCode::LoadMem, 0, &loc);
            }
        }
    }
}

pub(crate) fn gen_member_call_expr(gen: &mut BytecodeGen, expr: &mut Expr) {
    let loc = *expr.loc();
    gen.gen_member_call(expr, &loc);
}

impl BytecodeGen {
    pub(crate) fn gen_member_call(&mut self, expr: &mut Expr, loc: &SourceLoc) {
        let Expr::MemberCall {
            object,
            method,
            args,
            is_virtual,
            resolved_mangled,
            ty,
            ..
        } = expr
        else {
            self.report_error("gen_member_call 期望 MemberCall 表达式", loc);
            self.emit(OpCode::PushConst, 0, loc);
            return;
        };

        let obj_type = object.ty().clone();
        let class_name = match self.extract_class_name(&obj_type) {
            Some(n) => n,
            None => {
                self.report_error("MemberCall 需要类类型对象", loc);
                self.emit(OpCode::PushConst, 0, loc);
                return;
            }
        };
        let is_struct_ret = ty.is_struct() || ty.is_class();
        let ret_temp_offset = if is_struct_ret {
            let sz = (self.type_size(ty) + 3) & !3;
            let offset = self.next_local_offset;
            self.next_local_offset += sz;
            Some(offset)
        } else {
            None
        };
        // Generate args in reverse order (excluding this)
        for arg in args.iter_mut().rev() {
            let arg_ty = arg.ty().clone();
            if arg_ty.is_struct() || arg_ty.is_class() {
                let is_lambda = is_lambda_closure_type(&arg_ty);
                let sz = self.type_size(&arg_ty);
                // lambda 以闭包对象地址（1 word）传参，与闭包类字段大小无关（Issue B2）
                let words = if is_lambda { 1 } else { (sz + 3) / 4 };
                if let Expr::Identifier { name: arg_name, .. } = arg {
                    if let Some(&offset) = self.local_indices.get(arg_name) {
                        if is_lambda {
                            self.emit(OpCode::LoadLocal, offset, loc);
                        } else {
                            for i in 0..words {
                                self.emit(OpCode::LoadLocal, offset + i * 4, loc);
                            }
                        }
                    } else if let Some(&offset) = self.static_local_indices.get(arg_name) {
                        if is_lambda {
                            self.emit(OpCode::LoadGlobal, offset, loc);
                        } else {
                            for i in 0..words {
                                self.emit(OpCode::LoadGlobal, offset + i * 4, loc);
                            }
                        }
                    } else if let Some(&offset) = self.global_indices.get(arg_name) {
                        if is_lambda {
                            self.emit(OpCode::LoadGlobal, offset, loc);
                        } else {
                            for i in 0..words {
                                self.emit(OpCode::LoadGlobal, offset + i * 4, loc);
                            }
                        }
                    } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                        self.gen_expr(arg);
                        let addr_temp = self.get_temp_slot(0);
                        self.emit(OpCode::StoreLocal, addr_temp, loc);
                        for i in 0..words {
                            self.emit(OpCode::LoadLocal, addr_temp, loc);
                            if i > 0 {
                                self.emit(OpCode::PushConst, i * 4, loc);
                                self.emit(OpCode::Add, 0, loc);
                            }
                            self.emit(OpCode::LoadMem, 0, loc);
                        }
                    } else {
                        self.gen_expr(arg);
                        for _ in 1..words {
                            self.emit(OpCode::PushConst, 0, loc);
                        }
                    }
                } else if matches!(arg, Expr::Call { .. } | Expr::CallPtr { .. }) {
                    self.gen_expr(arg);
                    let addr_temp = self.get_temp_slot(0);
                    self.emit(OpCode::StoreLocal, addr_temp, loc);
                    for i in 0..words {
                        self.emit(OpCode::LoadLocal, addr_temp, loc);
                        if i > 0 {
                            self.emit(OpCode::PushConst, i * 4, loc);
                            self.emit(OpCode::Add, 0, loc);
                        }
                        self.emit(OpCode::LoadMem, 0, loc);
                    }
                } else {
                    self.gen_expr(arg);
                    for _ in 1..words {
                        self.emit(OpCode::PushConst, 0, loc);
                    }
                }
            } else if arg_ty.kind() == TypeKind::Double {
                self.gen_expr(arg);
                self.emit(OpCode::SplitD, 0, loc);
            } else if arg_ty.kind() == TypeKind::LongLong {
                self.gen_expr(arg);
                self.emit(OpCode::SplitQ, 0, loc);
            } else {
                self.gen_expr(arg);
            }
        }
        // Generate this pointer
        if obj_type.is_pointer() {
            self.gen_expr(object);
        } else if obj_type.kind() == TypeKind::Class {
            if let Expr::Identifier { name, .. } = object.as_ref() {
                if let Some(&offset) = self.local_indices.get(name) {
                    self.emit(OpCode::GetFrameBase, 0, loc);
                    self.emit(OpCode::PushConst, offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                } else if let Some(&offset) = self.global_indices.get(name) {
                    self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + offset, loc);
                } else {
                    self.report_error("未声明的类对象", loc);
                    self.emit(OpCode::PushConst, 0, loc);
                }
            } else {
                self.gen_addr(object, loc);
            }
        } else if obj_type.is_reference() || obj_type.is_rvalue_ref() {
            // 引用对象：gen_addr 返回其存储的地址
            self.gen_addr(object, loc);
        } else {
            self.report_error("不支持的类对象表达式", loc);
            self.emit(OpCode::PushConst, 0, loc);
        }
        if *is_virtual {
            let this_temp = self.get_temp_slot(0);
            self.emit(OpCode::StoreLocal, this_temp, loc);
            if let Some(offset) = ret_temp_offset {
                self.emit(OpCode::GetFrameBase, 0, loc);
                self.emit(OpCode::PushConst, offset, loc);
                self.emit(OpCode::Add, 0, loc);
            }
            // Push this as argument
            self.emit(OpCode::LoadLocal, this_temp, loc);
            // Virtual lookup
            let class = match self.class_defs.get(&class_name) {
                Some(c) => c,
                None => {
                    self.report_error(&format!("未知类 '{}'", class_name), loc);
                    return;
                }
            };
            let vtable = match &class.vtable {
                Some(vt) => vt,
                None => {
                    self.report_error(&format!("类 '{}' 没有虚表", class_name), loc);
                    return;
                }
            };
            let vindex = match vtable.entries.iter().position(|(n, _)| n == method) {
                Some(i) => i,
                None => {
                    self.report_error(&format!("方法 '{}' 不在 '{}' 的虚表中", method, class_name), loc);
                    return;
                }
            };
            self.emit(OpCode::LoadLocal, this_temp, loc);
            self.emit(OpCode::LoadMem, 0, loc);
            self.emit(OpCode::PushConst, (vindex * 4) as i32, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::LoadMem, 0, loc);
            self.emit(OpCode::CallPtr, (args.len() + 1) as i32, loc);
        } else {
            if let Some(offset) = ret_temp_offset {
                self.emit(OpCode::GetFrameBase, 0, loc);
                self.emit(OpCode::PushConst, offset, loc);
                self.emit(OpCode::Add, 0, loc);
            }
            let mangled = resolved_mangled
                .clone()
                .unwrap_or_else(|| format!("{}__{}", class_name, method));
            if let Some(&idx) = self.func_index.get(&mangled) {
                self.emit(OpCode::Call, idx, loc);
            } else {
                self.report_error(&format!("未定义的类方法 '{}'", mangled), loc);
                self.emit(OpCode::PushConst, 0, loc);
            }
        }
        if let Some(offset) = ret_temp_offset {
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, offset, loc);
            self.emit(OpCode::Add, 0, loc);
        }
    }
}
