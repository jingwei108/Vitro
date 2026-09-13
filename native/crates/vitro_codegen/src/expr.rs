// TODO(#D08): codegen/expr.rs 仍保留部分表达式分发逻辑，未来可进一步下沉到各子模块。
use super::*;

mod array;
mod assign;
mod binary;
mod call;
mod cast;
mod compound_literal;
mod generic;
mod literal;
mod new_delete;
mod struct_;
mod unary;

/// Returns true if `expr` denotes an object with storage (an lvalue in C++ terms).
pub(crate) fn is_lvalue_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Identifier { .. }
            | Expr::Index { .. }
            | Expr::Member { .. }
            | Expr::Unary { op: UnaryOp::Deref, .. }
            | Expr::This { .. }
            | Expr::CompoundLiteral { .. }
    )
}

pub(crate) trait ExprGen {
    fn gen_expr(&mut self, expr: &mut Expr);
    fn gen_nested_init(&mut self, base_temp: i32, offset: i32, target_ty: &Type, init: &mut Expr, loc: &SourceLoc);
    fn gen_member_addr(&mut self, object: &mut Expr, member: &str, loc: &SourceLoc);
    fn gen_index(&mut self, array: &mut Expr, index: &mut Expr, result_ty: &Type, loc: &SourceLoc, is_assign: bool);
    fn gen_vla_stride(&mut self, arr_type: &Type, loc: &SourceLoc);
    fn gen_addr(&mut self, expr: &mut Expr, loc: &SourceLoc);
    fn gen_struct_copy_common<F: FnMut(&mut Self, &SourceLoc, i32)>(
        &mut self,
        size: i32,
        src_expr: &mut Expr,
        dst_emit: F,
        loc: &SourceLoc,
    );
    fn gen_struct_copy(&mut self, left: &mut Expr, right: &mut Expr, loc: &SourceLoc);
    fn gen_struct_copy_to_local(&mut self, local_offset: i32, right: &mut Expr, loc: &SourceLoc);
}

impl ExprGen for BytecodeGen {
    fn gen_expr(&mut self, expr: &mut Expr) {
        let loc = *expr.loc();
        match expr {
            Expr::Literal { value, .. } => literal::gen_literal(self, *value, &loc),
            Expr::FloatLiteral { value, ty, .. } => literal::gen_float_literal(self, *value, ty, &loc),
            Expr::LongLiteral { value, .. } => literal::gen_long_literal(self, *value, &loc),
            Expr::StringLiteral { value, .. } => literal::gen_string_literal(self, value, &loc),
            Expr::Identifier { name, .. } => {
                // C99 __func__（E1 B 档）：预定义标识符 → 当前函数名字符串字面量
                if name == "__func__" {
                    literal::gen_string_literal(self, &self.current_func.clone(), &loc);
                    return;
                }
                // Function name used as value (function pointer)
                if let Some(&idx) = self.func_index.get(name) {
                    self.emit(OpCode::PushConst, idx, &loc);
                    return;
                }
                // C++ reference auto-dereference
                let base_ty = self
                    .local_types
                    .get(name)
                    .or_else(|| self.global_types.get(name))
                    .or_else(|| self.static_local_types.get(name))
                    .and_then(|t| t.reference_base().cloned());
                if let Some(base_ty) = base_ty {
                    self.gen_addr(expr, &loc);
                    match base_ty.kind() {
                        TypeKind::Char => self.emit(OpCode::LoadMemByte, 0, &loc),
                        TypeKind::Double => self.emit(OpCode::LoadMemD, 0, &loc),
                        TypeKind::LongLong => self.emit(OpCode::LoadMemQ, 0, &loc),
                        _ => self.emit(OpCode::LoadMem, 0, &loc),
                    }
                    return;
                }
                // static local variable
                if let Some(&static_offset) = self.static_local_indices.get(name) {
                    if let Some(ty) = self.static_local_types.get(name) {
                        if ty.is_array() {
                            self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + static_offset, &loc);
                        } else if ty.kind() == TypeKind::Double {
                            self.emit(OpCode::LoadGlobalD, static_offset, &loc);
                        } else if ty.kind() == TypeKind::LongLong {
                            self.emit(OpCode::LoadGlobalQ, static_offset, &loc);
                        } else {
                            self.emit(OpCode::LoadGlobal, static_offset, &loc);
                        }
                    } else {
                        self.emit(OpCode::LoadGlobal, static_offset, &loc);
                    }
                    return;
                }
                let local_offset = self.resolve_local(name);
                if local_offset >= 0 {
                    if let Some(ty) = self.local_types.get(name) {
                        if ty.is_array() {
                            if ty.is_vla() {
                                // VLA is stored as a pointer on stack
                                self.emit(OpCode::LoadLocal, local_offset, &loc);
                            } else if local_offset < self.current_func_arg_bytes {
                                // Array parameter decayed to pointer
                                self.emit(OpCode::LoadLocal, local_offset, &loc);
                            } else {
                                // Local array: compute base address
                                self.emit(OpCode::GetFrameBase, 0, &loc);
                                self.emit(OpCode::PushConst, local_offset, &loc);
                                self.emit(OpCode::Add, 0, &loc);
                            }
                        } else if ty.kind() == TypeKind::Double {
                            self.emit(OpCode::LoadLocalD, local_offset, &loc);
                        } else if ty.kind() == TypeKind::LongLong {
                            self.emit(OpCode::LoadLocalQ, local_offset, &loc);
                        } else {
                            self.emit(OpCode::LoadLocal, local_offset, &loc);
                        }
                    } else {
                        self.emit(OpCode::LoadLocal, local_offset, &loc);
                    }
                } else {
                    let global_offset = self.resolve_global(name);
                    if global_offset >= 0 {
                        if let Some(ty) = self.global_types.get(name) {
                            if ty.is_array() {
                                self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + global_offset, &loc);
                            } else if ty.kind() == TypeKind::Double {
                                self.emit(OpCode::LoadGlobalD, global_offset, &loc);
                            } else if ty.kind() == TypeKind::LongLong {
                                self.emit(OpCode::LoadGlobalQ, global_offset, &loc);
                            } else {
                                self.emit(OpCode::LoadGlobal, global_offset, &loc);
                            }
                        } else {
                            self.emit(OpCode::LoadGlobal, global_offset, &loc);
                        }
                    } else {
                        self.report_error(&format!("未声明的标识符 '{}'", name), &loc);
                        self.emit(OpCode::PushConst, 0, &loc);
                    }
                }
            }
            Expr::Binary { op, left, right, ty, .. } => binary::gen_binary(self, op, left, right, ty, &loc),
            Expr::Unary { .. } => unary::gen_unary(self, expr),
            Expr::Call { .. } => call::gen_call(self, expr),
            Expr::CallPtr { .. } => call::gen_call_ptr(self, expr),
            Expr::Index { .. } => array::gen_index_expr(self, expr),
            Expr::Member { .. } => struct_::gen_member_expr(self, expr),
            Expr::Ternary { .. } => assign::gen_ternary_expr(self, expr),
            Expr::Assign { .. } => assign::gen_assign_expr(self, expr),
            Expr::Sizeof { .. } => cast::gen_sizeof_expr(self, expr),
            Expr::Alignof { .. } => cast::gen_alignof_expr(self, expr),
            Expr::Cast { .. } => cast::gen_cast_expr(self, expr),
            Expr::InitList { .. } => {
                self.report_error("初始化列表只能在变量声明中使用", &loc);
                self.emit(OpCode::PushConst, 0, &loc);
            }
            Expr::Generic { .. } => generic::gen_generic_expr(self, expr),
            Expr::CompoundLiteral { .. } => compound_literal::gen_compound_literal_expr(self, expr),
            Expr::Offsetof { .. } => cast::gen_offsetof_expr(self, expr),
            // === C++ 新增 (Phase 33) ===
            Expr::This { .. } => new_delete::gen_this_expr(self, expr),
            Expr::MemberCall { .. } => struct_::gen_member_call_expr(self, expr),
            Expr::New { .. } => new_delete::gen_new_expr(self, expr),
            Expr::Delete { .. } => new_delete::gen_delete_expr(self, expr),
            Expr::Move { .. } => new_delete::gen_move_expr(self, expr),
            Expr::Lambda { .. } => new_delete::gen_lambda_expr(self, expr),
        }
    }

    /// Generate initialization code for a nested InitList at a base address stored in a local temp slot.
    fn gen_nested_init(&mut self, base_temp: i32, offset: i32, target_ty: &Type, init: &mut Expr, loc: &SourceLoc) {
        match init {
            Expr::InitList { elements, .. } => {
                if target_ty.is_struct() || target_ty.is_class() {
                    let fields = if target_ty.is_struct() {
                        self.struct_defs.get(target_ty.name()).cloned().unwrap_or_default()
                    } else {
                        self.class_defs
                            .get(target_ty.name())
                            .map(|c| {
                                c.members
                                    .iter()
                                    .filter_map(|m| match m {
                                        ClassMember::Field { name, ty, .. } => Some(StructField {
                                            name: name.clone(),
                                            ty: ty.clone(),
                                        }),
                                        _ => None,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    for (i, elem) in elements.iter_mut().enumerate() {
                        if i >= fields.len() {
                            break;
                        }
                        let field_offset = fields.iter().take(i).map(|f| self.type_size(&f.ty)).sum::<i32>();
                        self.gen_nested_init(base_temp, offset + field_offset, &fields[i].ty, &mut elem.value, loc);
                    }
                } else if target_ty.is_array() {
                    let elem_size = self.elem_type_size(target_ty);
                    let inner_ty = target_ty.subscript_type();
                    for (i, elem) in elements.iter_mut().enumerate() {
                        let elem_offset = offset + (i as i32) * elem_size;
                        self.gen_nested_init(base_temp, elem_offset, &inner_ty, &mut elem.value, loc);
                    }
                } else {
                    if let Some(first) = elements.first_mut() {
                        self.gen_nested_init(base_temp, offset, target_ty, &mut first.value, loc);
                    }
                }
            }
            _ => {
                self.emit(OpCode::LoadLocal, base_temp, loc);
                if offset != 0 {
                    self.emit(OpCode::PushConst, offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                }
                self.gen_expr(init);
                match target_ty.kind() {
                    TypeKind::Double => self.emit(OpCode::StoreMemD, 0, loc),
                    TypeKind::LongLong => self.emit(OpCode::StoreMemQ, 0, loc),
                    TypeKind::Char => self.emit(OpCode::StoreMemByte, 0, loc),
                    _ => self.emit(OpCode::StoreMem, 0, loc),
                }
            }
        }
    }

    fn gen_member_addr(&mut self, object: &mut Expr, member: &str, loc: &SourceLoc) {
        if object.ty().is_pointer() {
            self.gen_expr(object);
        } else if object.ty().is_reference() || object.ty().is_rvalue_ref() {
            // Reference/RValueRef: gen_addr yields the address it stores,
            // which is the object address we need for member access.
            self.gen_addr(object, loc);
        } else if let Expr::Index { array, index, ty, .. } = object {
            self.gen_index(array, index, ty, loc, true);
        } else if let Expr::Member { object: inner, member: m, .. } = object {
            self.gen_member_addr(inner, m, loc);
        } else if let Expr::Identifier { name, .. } = object {
            // TypeChecker 会对引用变量做 auto-dereference，导致 object.ty() 变成 Class。
            // 这里检查实际变量类型，如果是引用则加载其存储的地址。
            let actual_ty = self
                .local_types
                .get(name)
                .or_else(|| self.global_types.get(name))
                .or_else(|| self.static_local_types.get(name));
            if actual_ty.map(|t| t.is_reference() || t.is_rvalue_ref()).unwrap_or(false) {
                self.gen_addr(object, loc);
            } else if let Some(&offset) = self.local_indices.get(name) {
                self.emit(OpCode::GetFrameBase, 0, loc);
                self.emit(OpCode::PushConst, offset, loc);
                self.emit(OpCode::Add, 0, loc);
            } else if let Some(&offset) = self.global_indices.get(name) {
                self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + offset, loc);
            } else {
                self.report_error("未声明的结构体变量", loc);
                self.emit(OpCode::PushConst, 0, loc);
            }
        } else if object.ty().is_struct() || object.ty().is_class() {
            // 函数按值返回结构体/类等复杂表达式，gen_expr 会留下地址
            self.gen_expr(object);
        } else {
            self.report_error("复杂结构体表达式暂不支持", loc);
            self.emit(OpCode::PushConst, 0, loc);
        }
        let offset = self.get_member_offset(object.ty(), member);
        if offset > 0 {
            self.emit(OpCode::PushConst, offset, loc);
            self.emit(OpCode::Add, 0, loc);
        }
    }

    fn gen_index(&mut self, array: &mut Expr, index: &mut Expr, result_ty: &Type, loc: &SourceLoc, is_assign: bool) {
        let mut bound_size = -1;
        let mut sym_idx = -1;
        if let Expr::Identifier { name, .. } = array {
            if let Some(ty) = self.local_types.get(name) {
                if ty.is_array() {
                    bound_size = if ty.dims().is_empty() {
                        ty.array_size()
                    } else {
                        ty.dims()[0]
                    };
                    sym_idx = self.resolve_symbol_index(name);
                }
            } else if let Some(ty) = self.global_types.get(name) {
                if ty.is_array() {
                    bound_size = if ty.dims().is_empty() {
                        ty.array_size()
                    } else {
                        ty.dims()[0]
                    };
                    sym_idx = self.resolve_symbol_index(name);
                }
            }
        } else if let Expr::Index { .. } = array {
            if array.ty().is_array() && !array.ty().dims().is_empty() {
                bound_size = array.ty().dims()[0];
            }
        }
        let stride = super::compute_stride(array.ty(), self.elem_type_size(array.ty()));
        if stride == 0 && !array.ty().is_vla() {
            self.report_error("数组索引步长计算失败：存在无效维度", loc);
        }
        self.gen_expr(array);
        self.gen_expr(index);

        if bound_size > 0 {
            if sym_idx >= 0 {
                self.emit(OpCode::TrapBounds, sym_idx, loc);
            } else {
                self.emit(OpCode::TrapBounds, -bound_size, loc);
            }
        }

        if stride > 0 {
            self.emit(OpCode::PushConst, stride, loc);
        } else if array.ty().is_vla() {
            self.gen_vla_stride(array.ty(), loc);
        } else {
            self.emit(OpCode::PushConst, self.elem_type_size(array.ty()), loc);
        }
        self.emit(OpCode::Mul, 0, loc);
        self.emit(OpCode::Add, 0, loc);
        if !is_assign && !result_ty.is_array() {
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
    }

    fn gen_vla_stride(&mut self, arr_type: &Type, loc: &SourceLoc) {
        if let Type::Array { dims, vla_dims, .. } = arr_type {
            let mut vla_idx = 0;
            for dim in dims.iter().take(1.min(dims.len())) {
                if *dim == 0 {
                    vla_idx += 1;
                }
            }
            let mut vla_dims_clone = vla_dims.clone();
            let mut first = true;
            for dim in dims.iter().skip(1) {
                if !first {
                    self.emit(OpCode::Mul, 0, loc);
                }
                if *dim > 0 {
                    self.emit(OpCode::PushConst, *dim, loc);
                } else if let Some(dim_expr) = vla_dims_clone.get_mut(vla_idx) {
                    self.gen_expr(dim_expr);
                    vla_idx += 1;
                } else {
                    self.emit(OpCode::PushConst, 0, loc);
                }
                first = false;
            }
            let elem_size = self.elem_type_size(arr_type);
            if first {
                self.emit(OpCode::PushConst, elem_size, loc);
            } else {
                self.emit(OpCode::PushConst, elem_size, loc);
                self.emit(OpCode::Mul, 0, loc);
            }
        } else {
            self.emit(OpCode::PushConst, self.elem_type_size(arr_type), loc);
        }
    }

    fn gen_addr(&mut self, expr: &mut Expr, loc: &SourceLoc) {
        match expr {
            Expr::Identifier { name, .. } => {
                if let Some(&offset) = self.local_indices.get(name) {
                    if let Some(ty) = self.local_types.get(name) {
                        if ty.is_reference() || ty.is_rvalue_ref() {
                            self.emit(OpCode::LoadLocal, offset, loc);
                            return;
                        }
                    }
                    self.emit(OpCode::GetFrameBase, 0, loc);
                    self.emit(OpCode::PushConst, offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                } else if let Some(&offset) = self.global_indices.get(name) {
                    if let Some(ty) = self.global_types.get(name) {
                        if ty.is_reference() || ty.is_rvalue_ref() {
                            self.emit(OpCode::LoadGlobal, offset, loc);
                            return;
                        }
                    }
                    self.emit(OpCode::PushConst, vitro_runtime::GLOBAL_START as i32 + offset, loc);
                } else {
                    self.report_error("未声明的变量", loc);
                    // 错误已被记录，提前返回以避免生成无意义指令。
                    // 编译管线末端的 has_errors() 守卫会拦截并丢弃错误字节码。
                }
            }
            Expr::Index { array, index, ty, .. } => {
                self.gen_index(array, index, ty, loc, true);
            }
            Expr::Member { object, member, .. } => {
                self.gen_member_addr(object, member, loc);
            }
            Expr::Unary {
                op: UnaryOp::Deref, operand, ..
            } => {
                self.gen_expr(operand);
            }
            Expr::Call { ty, .. } | Expr::CallPtr { ty, .. }
                if ty.is_struct() || ty.is_class() || ty.is_reference() || ty.is_rvalue_ref() =>
            {
                // For std::move(x), gen_addr should yield the address of x,
                // not the value that gen_expr would leave.
                if let Expr::CallPtr { callee, args, .. } = expr {
                    if let Expr::Identifier { name, .. } = callee.as_ref() {
                        if name == "std__move" && args.len() == 1 {
                            self.gen_addr(&mut args[0], loc);
                            return;
                        }
                    }
                }
                // 函数按值返回结构体 / 返回引用，gen_expr 已在栈顶留下地址
                self.gen_expr(expr);
            }
            Expr::MemberCall { ty, .. } if ty.is_reference() || ty.is_rvalue_ref() => {
                // 成员函数返回引用：gen_expr 已在栈顶留下目标地址
                self.gen_expr(expr);
            }
            Expr::MemberCall { ty, .. } if ty.is_struct() || ty.is_class() => {
                // 成员函数按值返回结构体/类：gen_expr 已在栈顶留下临时对象地址
                self.gen_expr(expr);
            }
            Expr::Lambda { .. } => {
                // Lambda 表达式已在栈顶留下临时闭包对象的地址
                self.gen_expr(expr);
            }
            Expr::Move { expr: inner, .. } => {
                // std::move(x) — address of the moved-from object
                self.gen_addr(inner, loc);
            }
            Expr::CompoundLiteral { .. } => {
                // 复合字面量 gen_expr 已在栈顶留下临时对象地址
                self.gen_expr(expr);
            }
            _ => {
                self.report_error("不支持的地址生成", loc);
            }
        }
    }

    /// 通用结构体/union 拷贝循环：通过闭包生成目标地址加载指令。
    fn gen_struct_copy_common<F: FnMut(&mut Self, &SourceLoc, i32)>(
        &mut self,
        size: i32,
        src_expr: &mut Expr,
        mut dst_emit: F,
        loc: &SourceLoc,
    ) {
        if size <= 0 {
            return;
        }
        let src_temp = self.get_temp_slot(0);
        self.gen_addr(src_expr, loc);
        self.emit(OpCode::StoreLocal, src_temp, loc);
        for i in 0..size / 4 {
            dst_emit(self, loc, i);
            self.emit(OpCode::LoadLocal, src_temp, loc);
            if i > 0 {
                self.emit(OpCode::PushConst, i * 4, loc);
                self.emit(OpCode::Add, 0, loc);
            }
            self.emit(OpCode::LoadMem, 0, loc);
            self.emit(OpCode::StoreMem, 0, loc);
        }
    }

    fn gen_struct_copy(&mut self, left: &mut Expr, right: &mut Expr, loc: &SourceLoc) {
        let size = self.type_size(left.ty());
        let dst_temp = self.get_temp_slot(1);
        self.gen_addr(left, loc);
        self.emit(OpCode::StoreLocal, dst_temp, loc);
        self.gen_struct_copy_common(
            size,
            right,
            |gen, loc, i| {
                gen.emit(OpCode::LoadLocal, dst_temp, loc);
                if i > 0 {
                    gen.emit(OpCode::PushConst, i * 4, loc);
                    gen.emit(OpCode::Add, 0, loc);
                }
            },
            loc,
        );
    }

    fn gen_struct_copy_to_local(&mut self, local_offset: i32, right: &mut Expr, loc: &SourceLoc) {
        let size = self.type_size(right.ty());
        if size <= 0 {
            return;
        }

        // 如果右端是函数按值返回的类临时对象，在拷贝后调用其析构函数，
        // 避免临时对象内的资源泄漏。
        let is_temporary_return = matches!(right, Expr::Call { .. } | Expr::CallPtr { .. }) && right.ty().is_class();
        let dtor_name = if is_temporary_return {
            format!("__dtor__{}", right.ty().name())
        } else {
            String::new()
        };
        let has_dtor = is_temporary_return && self.func_index.contains_key(&dtor_name);

        if has_dtor {
            // 函数返回值已在栈顶留下临时对象地址。
            self.gen_expr(right);
            let src_temp = self.get_temp_slot(0);
            self.emit(OpCode::StoreLocal, src_temp, loc);
            for i in 0..size / 4 {
                self.emit(OpCode::GetFrameBase, 0, loc);
                self.emit(OpCode::PushConst, local_offset + i * 4, loc);
                self.emit(OpCode::Add, 0, loc);
                self.emit(OpCode::LoadLocal, src_temp, loc);
                if i > 0 {
                    self.emit(OpCode::PushConst, i * 4, loc);
                    self.emit(OpCode::Add, 0, loc);
                }
                self.emit(OpCode::LoadMem, 0, loc);
                self.emit(OpCode::StoreMem, 0, loc);
            }
            self.emit(OpCode::LoadLocal, src_temp, loc);
            if let Some(&idx) = self.func_index.get(&dtor_name) {
                self.emit(OpCode::Call, idx, loc);
            }
            return;
        }

        self.gen_struct_copy_common(
            size,
            right,
            |gen, loc, i| {
                gen.emit(OpCode::GetFrameBase, 0, loc);
                gen.emit(OpCode::PushConst, local_offset + i * 4, loc);
                gen.emit(OpCode::Add, 0, loc);
            },
            loc,
        );
    }
}
