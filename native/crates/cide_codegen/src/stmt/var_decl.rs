//! 变量声明语句代码生成。

use crate::expr::ExprGen;
use crate::flatten_init_list;
use crate::is_lambda_closure_type;
use cide_ast::base_element_type;
use cide_ast::{Designator, Expr, InitElement, SourceLoc, Type, TypeKind};
use cide_runtime::opcode::OpCode;
use cide_runtime::Symbol;
use cide_runtime::GLOBAL_START;

use super::super::BytecodeGen;

impl BytecodeGen {
    pub(crate) fn gen_var_decl(
        &mut self,
        var_type: &Type,
        name: &str,
        init: &mut Option<Expr>,
        extra_vars: &mut [(Type, String, Option<Expr>)],
        is_static: bool,
        loc: &SourceLoc,
    ) {
        self.emit_single_var(var_type, name, init, loc, is_static);
        for (ety, ename, einit) in extra_vars.iter_mut() {
            self.emit_single_var(ety, ename, einit, loc, is_static);
        }
    }

    fn emit_single_var(&mut self, vty: &Type, n: &str, init: &mut Option<Expr>, loc: &SourceLoc, is_static: bool) {
        if is_static {
            self.emit_static_var(vty, n, init, loc);
            return;
        }

        // Issue B2：lambda 变量的槽里存的是**闭包对象地址**（见
        // `cpp/var_decl.rs::try_gen_cpp_class_init` 的 Lambda 分支：gen_lambda
        // 压入地址后直接 StoreLocal），而不是闭包对象本身。因此槽位必须按
        // 指针大小（4 字节）分配——闭包类无捕获时 size 为 0，旧实现按
        // `type_size` 得出 aligned_sz = 0，帧内根本没有 f 的槽位，
        // StoreLocal 写到帧外并冲出 1MB 线性内存（"StoreLocal: 地址越界"）。
        let sz = if is_lambda_closure_type(vty) {
            4
        } else {
            self.type_size(vty)
        };
        let aligned_sz = (sz + 3) & !3;
        let local_offset = self.next_local_offset;
        self.next_local_offset += aligned_sz;
        self.record_scope_var(n);
        // V-P1-6：登记栈上数组缓冲区，供 strcpy/strcat/scanf("%s") 做容量校验
        // （参数区数组已退化为指针，不登记；VLA 运行时分配，不登记）
        if vty.is_array() && !vty.is_vla() && local_offset >= self.current_func_arg_bytes {
            if let Some(meta) = self.func_table.get_mut(&self.current_func) {
                meta.local_buffers.push(cide_runtime::LocalBuffer {
                    offset: local_offset,
                    size: sz,
                    name: n.to_string(),
                });
            }
        }
        self.local_indices.insert(n.to_string(), local_offset);
        self.local_types.insert(n.to_string(), vty.clone());
        self.sym_index.insert(n.to_string(), self.symbols.len() as i32);
        self.symbols.push(Symbol {
            name: n.to_string(),
            addr: local_offset as u32,
            is_local: true,
            ty: vty.clone(),
            scope_depth: 1,
            func_name: self.current_func.clone(),
            decl_line: loc.line,
        });

        if vty.is_vla() {
            self.emit_vla_alloc(vty, local_offset, loc);
            if let Some(ref mut e) = init {
                if matches!(e, Expr::InitList { .. }) {
                    self.report_error("变长数组(VLA)不支持初始化列表", loc);
                }
            }
        } else if let Some(ref mut e) = init {
            self.emit_local_init(vty, e, local_offset, loc);
        } else {
            self.emit_zero_init(vty, local_offset, loc);
            // C++ 栈对象 RAII：对未初始化的 class 类型调用默认构造函数
            self.try_gen_cpp_class_default_ctor(vty, local_offset, loc);
        }
    }

    fn emit_static_var(&mut self, vty: &Type, n: &str, init: &mut Option<Expr>, loc: &SourceLoc) {
        if vty.is_vla() {
            self.report_error("static 变量不支持变长数组(VLA)", loc);
            return;
        }
        if self.static_local_indices.contains_key(n) {
            return;
        }
        // Issue B2 同源分支：lambda 静态变量槽里存的同样是闭包对象地址（见
        // emit_static_init 兜底分支的 StoreGlobal），必须按 4 字节占地。旧实现
        // 按闭包类字段大小分配，无捕获闭包 size=0 → 多个 static 闭包地址重叠，
        // printf 读到非法数据（Clang 输出 "s=8 6"，Cide 输出乱码）。
        let sz = if is_lambda_closure_type(vty) {
            4
        } else {
            self.type_size(vty)
        };
        let aligned_sz = (sz + 3) & !3;
        let Some(global_offset) =
            self.bump_global_offset(aligned_sz, &format!("静态局部变量 '{}'", n), loc)
        else {
            return;
        };
        self.static_local_indices.insert(n.to_string(), global_offset);
        self.static_local_types.insert(n.to_string(), vty.clone());
        self.sym_index.insert(n.to_string(), self.symbols.len() as i32);
        self.symbols.push(Symbol {
            name: n.to_string(),
            addr: (GLOBAL_START as i32 + global_offset) as u32,
            is_local: false,
            ty: vty.clone(),
            scope_depth: 0,
            func_name: self.current_func.clone(),
            decl_line: loc.line,
        });
        if let Some(e) = init {
            self.emit_static_init(vty, e, global_offset, loc);
        }
    }

    fn emit_static_init(&mut self, vty: &Type, init: &mut Expr, global_offset: i32, loc: &SourceLoc) {
        match init {
            Expr::InitList { elements, .. } => {
                let has_designators = elements.iter().any(|e| !e.designators.is_empty());
                if has_designators {
                    self.report_error("静态局部变量的 designated initializer 暂不支持", loc);
                    return;
                }
                let is_char_array = if let Type::Array { element, .. } = vty {
                    element.kind() == TypeKind::Char
                } else {
                    false
                };
                if is_char_array {
                    let values = flatten_init_list(elements, &mut self.errors);
                    for i in 0..self.type_size(vty) as usize {
                        self.globals_init_32
                            .push((global_offset as u32 + i as u32, values.get(i).copied().unwrap_or(0)));
                    }
                } else {
                    self.emit_static_scalar_array_init(vty, elements, global_offset, loc);
                }
            }
            Expr::StringLiteral { value, .. } => {
                for i in 0..self.type_size(vty) as usize {
                    let byte = if i < value.len() { value.as_bytes()[i] as i32 } else { 0 };
                    self.globals_init_32.push((global_offset as u32 + i as u32, byte));
                }
            }
            Expr::Literal { .. } | Expr::LongLiteral { .. } | Expr::FloatLiteral { .. } => {
                // T-P0-1：static 局部标量初始化按目标类型编码位模式（与全局路径同构）
                let _ = self.push_literal_init(global_offset as u32, init, vty);
            }
            Expr::Unary {
                op: cide_ast::UnaryOp::Neg,
                operand,
                ..
            } if matches!(
                operand.as_ref(),
                Expr::Literal { .. } | Expr::LongLiteral { .. } | Expr::FloatLiteral { .. }
            ) =>
            {
                // 负数字面量（如 static double d = -2;）与全局路径同构处理
                let _ = self.push_literal_init(global_offset as u32, init, vty);
            }
            Expr::Identifier { name: id_name, .. } => {
                if let Some(&idx) = self.func_index.get(id_name) {
                    self.globals_init_32.push((global_offset as u32, idx));
                }
            }
            _ => {
                self.gen_expr(init);
                if vty.kind() == TypeKind::Double {
                    self.emit(OpCode::StoreGlobalD, global_offset, loc);
                } else if vty.kind() == TypeKind::LongLong {
                    self.emit(OpCode::StoreGlobalQ, global_offset, loc);
                } else {
                    self.emit(OpCode::StoreGlobal, global_offset, loc);
                }
            }
        }
    }

    fn emit_static_scalar_array_init(
        &mut self,
        vty: &Type,
        elements: &mut [InitElement],
        global_offset: i32,
        _loc: &SourceLoc,
    ) {
        let elem_size = self.elem_type_size(vty);
        let count = vty.total_elements();
        if elem_size == 8 {
            // T-P0-2 同构修复：按元素类型编码 8 字节位模式
            // （此前 int 字面量初始化 long long 数组元素被写成 32 位截断值）
            let elem_ty = base_element_type(vty).clone();
            for i in 0..count as usize {
                let addr = global_offset as u32 + (i as u32) * elem_size as u32;
                let val64 = elements
                    .get(i)
                    .and_then(|elem| crate::init::literal_init_bits(&elem.value, elem_ty.kind()))
                    .map(|(bits, _)| bits)
                    .unwrap_or(0);
                self.globals_init_64.push((addr, val64));
            }
        } else {
            for (i, elem) in elements.iter().enumerate() {
                let addr = global_offset as u32 + (i as u32) * elem_size as u32;
                match &elem.value {
                    Expr::StringLiteral { value, .. } => {
                        let aligned = ((value.len() + 1) as u32 + 3) & !3;
                        let Some(str_offset) =
                            self.bump_global_offset(aligned as i32, "静态初始化字符串", _loc)
                        else {
                            continue;
                        };
                        let str_addr = GLOBAL_START + str_offset as u32;
                        self.string_data.push((str_addr, value.clone()));
                        self.globals_init_32.push((addr, str_addr as i32));
                    }
                    Expr::FloatLiteral { value, .. } => {
                        self.globals_init_32.push((addr, (*value as f32).to_bits() as i32));
                    }
                    Expr::LongLiteral { value, .. } => {
                        self.globals_init_32.push((addr, *value as i32));
                    }
                    Expr::Literal { value, .. } => {
                        self.globals_init_32.push((addr, *value));
                    }
                    Expr::Unary {
                        op: cide_ast::UnaryOp::Neg,
                        operand,
                        ..
                    } => {
                        if let Expr::Literal { value, .. } = operand.as_ref() {
                            self.globals_init_32.push((addr, -(*value)));
                        } else {
                            self.globals_init_32.push((addr, 0));
                        }
                    }
                    _ => {
                        let val = flatten_init_list(std::slice::from_ref(elem), &mut self.errors)
                            .first()
                            .copied()
                            .unwrap_or(0);
                        self.globals_init_32.push((addr, val));
                    }
                }
            }
            for i in elements.len()..count as usize {
                let addr = global_offset as u32 + (i as u32) * elem_size as u32;
                self.globals_init_32.push((addr, 0));
            }
        }
    }

    fn emit_vla_alloc(&mut self, vty: &Type, local_offset: i32, loc: &SourceLoc) {
        if let Type::Array { dims, vla_dims, .. } = vty {
            let mut vla_idx = 0;
            let mut vla_dims_clone = vla_dims.clone();
            let elem_size = self.elem_type_size(vty);
            if dims.is_empty() {
                self.emit(OpCode::PushConst, 0, loc);
            } else {
                for &dim in dims.iter() {
                    if dim > 0 {
                        self.emit(OpCode::PushConst, dim, loc);
                    } else if let Some(dim_expr) = vla_dims_clone.get_mut(vla_idx) {
                        self.gen_expr(dim_expr);
                        vla_idx += 1;
                    } else {
                        self.emit(OpCode::PushConst, 0, loc);
                    }
                }
                for _ in 1..dims.len() {
                    self.emit(OpCode::Mul, 0, loc);
                }
                if elem_size > 1 {
                    self.emit(OpCode::PushConst, elem_size, loc);
                    self.emit(OpCode::Mul, 0, loc);
                }
            }
            self.emit(OpCode::StackAlloc, 0, loc);
            self.emit(OpCode::StoreLocal, local_offset, loc);
        }
    }

    fn emit_local_init(&mut self, vty: &Type, init: &mut Expr, local_offset: i32, loc: &SourceLoc) {
        if vty.is_array() && matches!(init, Expr::InitList { .. }) {
            self.emit_local_array_init(vty, init, local_offset, loc);
        } else if vty.is_array() && matches!(init, Expr::CompoundLiteral { .. }) {
            if let Expr::CompoundLiteral { init: inner, .. } = init {
                self.emit_local_array_init(vty, inner, local_offset, loc);
            }
        } else if (vty.is_struct() || vty.is_class()) && matches!(init, Expr::InitList { .. }) {
            self.emit_local_struct_init(vty, init, local_offset, loc);
        } else if (vty.is_struct() || vty.is_class()) && matches!(init, Expr::CompoundLiteral { .. }) {
            self.gen_struct_copy_to_local(local_offset, init, loc);
        } else if vty.is_struct() || vty.is_class() {
            if !self.try_gen_cpp_class_init(vty, init, local_offset, loc) {
                self.gen_struct_copy_to_local(local_offset, init, loc);
            }
        } else if vty.is_array() && matches!(init, Expr::StringLiteral { .. }) {
            self.emit_local_string_array_init(vty, init, local_offset, loc);
        } else if vty.is_reference() || vty.is_rvalue_ref() {
            self.gen_cpp_reference_init(vty, init, local_offset, loc);
        } else if let Expr::CompoundLiteral { target_type, .. } = init {
            // 标量复合字面量：gen_expr 留下临时对象地址，加载值后存入变量槽位
            let scalar_kind = target_type.kind();
            self.gen_expr(init);
            match scalar_kind {
                TypeKind::Double => {
                    self.emit(OpCode::LoadMemD, 0, loc);
                    self.emit(OpCode::StoreLocalD, local_offset, loc);
                }
                TypeKind::LongLong => {
                    self.emit(OpCode::LoadMemQ, 0, loc);
                    self.emit(OpCode::StoreLocalQ, local_offset, loc);
                }
                _ => {
                    self.emit(OpCode::LoadMem, 0, loc);
                    self.emit(OpCode::StoreLocal, local_offset, loc);
                }
            }
        } else {
            self.gen_expr(init);
            self.emit_scalar_cast_store(vty, init, local_offset, loc);
        }
    }

    pub(crate) fn emit_scalar_cast_store(&mut self, vty: &Type, init: &Expr, local_offset: i32, loc: &SourceLoc) {
        if vty.kind() == TypeKind::Float
            && init.ty().kind() != TypeKind::Float
            && init.ty().kind() != TypeKind::Double
            && init.ty().kind() != TypeKind::LongLong
        {
            self.emit(OpCode::CastI2F, 0, loc);
        }
        if vty.kind() == TypeKind::Double
            && init.ty().kind() != TypeKind::Float
            && init.ty().kind() != TypeKind::Double
            && init.ty().kind() != TypeKind::LongLong
        {
            self.emit(OpCode::CastI2D, 0, loc);
        }
        if vty.kind() == TypeKind::LongLong
            && init.ty().kind() != TypeKind::LongLong
            && init.ty().kind() != TypeKind::Double
            && init.ty().kind() != TypeKind::Float
        {
            self.emit(OpCode::CastI2Q, 0, loc);
        }
        if vty.kind() == TypeKind::Double {
            self.emit(OpCode::StoreLocalD, local_offset, loc);
        } else if vty.kind() == TypeKind::LongLong {
            self.emit(OpCode::StoreLocalQ, local_offset, loc);
        } else {
            self.emit(OpCode::StoreLocal, local_offset, loc);
        }
    }

    pub(crate) fn emit_local_string_array_init(&mut self, vty: &Type, init: &Expr, local_offset: i32, loc: &SourceLoc) {
        if let Expr::StringLiteral { value, .. } = init {
            let base_temp = self.get_temp_slot(0);
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, local_offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::StoreLocal, base_temp, loc);
            let byte_count = vty.array_size() as usize;
            for i in 0..byte_count {
                self.emit(OpCode::LoadLocal, base_temp, loc);
                self.emit(OpCode::PushConst, i as i32, loc);
                self.emit(OpCode::Add, 0, loc);
                let byte = if i < value.len() { value.as_bytes()[i] as i32 } else { 0 };
                self.emit(OpCode::PushConst, byte, loc);
                self.emit(OpCode::StoreMemByte, 0, loc);
            }
        }
    }

    pub(crate) fn emit_local_array_init(&mut self, vty: &Type, init: &mut Expr, local_offset: i32, loc: &SourceLoc) {
        if let Expr::InitList { ref mut elements, .. } = init {
            let base_temp = self.get_temp_slot(0);
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, local_offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::StoreLocal, base_temp, loc);

            let is_char_array = if let Type::Array { element, .. } = vty {
                element.kind() == TypeKind::Char
            } else {
                false
            };

            if is_char_array {
                let values = flatten_init_list(elements, &mut self.errors);
                let byte_count = vty.array_size() as usize;
                for i in 0..byte_count {
                    self.emit(OpCode::LoadLocal, base_temp, loc);
                    self.emit(OpCode::PushConst, i as i32, loc);
                    self.emit(OpCode::Add, 0, loc);
                    let byte = values.get(i).copied().unwrap_or(0);
                    self.emit(OpCode::PushConst, byte, loc);
                    self.emit(OpCode::StoreMemByte, 0, loc);
                }
                return;
            }

            let inner_ty = vty.subscript_type();
            let has_nested_init = elements.iter().any(|e| matches!(&e.value, Expr::InitList { .. }));
            if has_nested_init && (inner_ty.is_struct() || inner_ty.is_class() || inner_ty.is_array()) {
                let elem_stride = self.type_size(&inner_ty);
                for (i, elem) in elements.iter_mut().enumerate() {
                    let addr_offset = (i as i32) * elem_stride;
                    self.gen_nested_init(base_temp, addr_offset, &inner_ty, &mut elem.value, loc);
                }
                let expected_count = if !vty.dims().is_empty() && vty.dims()[0] > 0 {
                    vty.dims()[0] as usize
                } else {
                    elements.len()
                };
                for (i, _) in (elements.len()..expected_count).enumerate() {
                    let idx = elements.len() + i;
                    let addr_offset = (idx as i32) * elem_stride;
                    self.emit(OpCode::LoadLocal, base_temp, loc);
                    if addr_offset > 0 {
                        self.emit(OpCode::PushConst, addr_offset, loc);
                        self.emit(OpCode::Add, 0, loc);
                    }
                    self.emit(OpCode::PushConst, 0, loc);
                    if inner_ty.kind() == TypeKind::Double {
                        self.emit(OpCode::StoreMemD, 0, loc);
                    } else if inner_ty.kind() == TypeKind::LongLong {
                        self.emit(OpCode::StoreMemQ, 0, loc);
                    } else {
                        self.emit(OpCode::StoreMem, 0, loc);
                    }
                }
            } else {
                self.emit_flat_array_init(vty, elements, base_temp, loc);
            }
        }
    }

    fn emit_flat_array_init(&mut self, vty: &Type, elements: &mut [InitElement], base_temp: i32, loc: &SourceLoc) {
        let elem_size = self.elem_type_size(vty);
        let count = vty.total_elements();
        let has_designators = elements.iter().any(|e| !e.designators.is_empty());
        if has_designators {
            // Zero-fill entire array before designated init
            self.emit(OpCode::PushConst, count * elem_size, loc);
            self.emit(OpCode::PushConst, 0, loc);
            self.emit(OpCode::LoadLocal, base_temp, loc);
            self.emit(OpCode::Memset, 0, loc);
            for elem in elements.iter_mut() {
                if elem.designators.is_empty() {
                    continue;
                }
                let idx = match &elem.designators[0] {
                    Designator::Index(idx_expr) => {
                        if let Expr::Literal { value, .. } = idx_expr.as_ref() {
                            *value
                        } else {
                            self.report_error("数组 designated initializer 的索引必须是编译期常量", loc);
                            continue;
                        }
                    }
                    _ => {
                        self.report_error("数组初始化只能使用 [index] 形式的 designator", loc);
                        continue;
                    }
                };
                if idx < 0 || idx >= count {
                    self.report_error("数组 designated initializer 索引超出范围", loc);
                    continue;
                }
                let addr_offset = idx * elem_size;
                self.emit(OpCode::LoadLocal, base_temp, loc);
                if addr_offset > 0 {
                    self.emit(OpCode::PushConst, addr_offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                }
                if elem_size == 8 {
                    self.gen_expr(&mut elem.value);
                    self.emit(OpCode::StoreMemD, 0, loc);
                } else {
                    self.gen_expr(&mut elem.value);
                    self.emit(OpCode::StoreMem, 0, loc);
                }
            }
        } else {
            // U1#2（2026-09-13，评估 R1 实锤修复）：flatten 调用保留——其副作用
            // 仍上报 designated initializer 的上下文错误——但**不再用它的值**。
            // 此前 4 字节元素只对 Identifier/StringLiteral 走 gen_expr，其余
            //（Call/算术等）落入 flatten 值（非字面量返回 None）→ unwrap_or(0)
            // → PushConst 0：`int a[2]={f(),3}` 的 a[0] 静默变成 0。现在与
            // 8 字节分支一致，全部走 gen_expr（通用表达式求值）。
            flatten_init_list(elements, &mut self.errors);
            for i in 0..count as usize {
                let addr_offset = (i as i32) * elem_size;
                self.emit(OpCode::LoadLocal, base_temp, loc);
                if addr_offset > 0 {
                    self.emit(OpCode::PushConst, addr_offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                }
                if let Some(elem) = elements.get_mut(i) {
                    if elem_size == 8 {
                        self.gen_expr(&mut elem.value);
                        self.emit(OpCode::StoreMemD, 0, loc);
                    } else {
                        self.gen_expr(&mut elem.value);
                        self.emit(OpCode::StoreMem, 0, loc);
                    }
                } else {
                    self.emit_zero_elem(elem_size, loc);
                }
            }
        }
    }

    fn emit_zero_elem(&mut self, elem_size: i32, loc: &SourceLoc) {
        if elem_size == 8 {
            self.emit(OpCode::PushConst, 0, loc);
            self.emit(OpCode::CastI2D, 0, loc);
            self.emit(OpCode::StoreMemD, 0, loc);
        } else {
            self.emit(OpCode::PushConst, 0, loc);
            self.emit(OpCode::StoreMem, 0, loc);
        }
    }

    pub(crate) fn emit_local_struct_init(&mut self, vty: &Type, init: &mut Expr, local_offset: i32, loc: &SourceLoc) {
        if let Expr::InitList { ref mut elements, .. } = init {
            let base_temp = self.get_temp_slot(0);
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, local_offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::StoreLocal, base_temp, loc);
            let fields = self.struct_defs.get(vty.name()).cloned().unwrap_or_default();
            let has_designators = elements.iter().any(|e| !e.designators.is_empty());
            if has_designators {
                // Zero-fill entire struct before designated init
                self.emit(OpCode::PushConst, self.type_size(vty), loc);
                self.emit(OpCode::PushConst, 0, loc);
                self.emit(OpCode::LoadLocal, base_temp, loc);
                self.emit(OpCode::Memset, 0, loc);
                for elem in elements.iter_mut() {
                    if elem.designators.is_empty() {
                        continue;
                    }
                    let field_idx = match &elem.designators[0] {
                        Designator::Field(name) => fields.iter().position(|f| &f.name == name),
                        _ => {
                            self.report_error("结构体初始化只能使用 .field 形式的 designator", loc);
                            None
                        }
                    };
                    let field_idx = match field_idx {
                        Some(i) if i < fields.len() => i,
                        _ => continue,
                    };
                    let offset = fields.iter().take(field_idx).map(|f| self.type_size(&f.ty)).sum::<i32>();
                    self.emit_field_init(base_temp, offset, &fields[field_idx].ty, &mut elem.value, loc);
                }
            } else {
                for (i, elem) in elements.iter_mut().enumerate() {
                    if i >= fields.len() {
                        break;
                    }
                    let offset = fields.iter().take(i).map(|f| self.type_size(&f.ty)).sum::<i32>();
                    self.emit_field_init(base_temp, offset, &fields[i].ty, &mut elem.value, loc);
                }
            }
        }
    }

    fn emit_field_init(&mut self, base_temp: i32, offset: i32, field_ty: &Type, value: &mut Expr, loc: &SourceLoc) {
        if matches!(value, Expr::InitList { .. })
            && (field_ty.is_struct() || field_ty.is_class() || field_ty.is_array())
        {
            self.gen_nested_init(base_temp, offset, field_ty, value, loc);
        } else {
            self.emit(OpCode::LoadLocal, base_temp, loc);
            if offset > 0 {
                self.emit(OpCode::PushConst, offset, loc);
                self.emit(OpCode::Add, 0, loc);
            }
            self.gen_expr(value);
            // T-P0-4：char 字段按 1 字节写，避免 4 字节 StoreMem 破坏相邻字段
            if field_ty.kind() == TypeKind::Char {
                self.emit(OpCode::StoreMemByte, 0, loc);
            } else if field_ty.kind() == TypeKind::Double {
                self.emit(OpCode::StoreMemD, 0, loc);
            } else if field_ty.kind() == TypeKind::LongLong {
                self.emit(OpCode::StoreMemQ, 0, loc);
            } else {
                self.emit(OpCode::StoreMem, 0, loc);
            }
        }
    }

    fn emit_zero_init(&mut self, vty: &Type, local_offset: i32, loc: &SourceLoc) {
        let sz = self.type_size(vty);
        if sz == 0 {
            // Nothing to do for zero-size types
        } else if sz == 8 && vty.kind() == TypeKind::Double {
            self.emit(OpCode::PushConst, 0, loc);
            self.emit(OpCode::CastI2D, 0, loc);
            self.emit(OpCode::StoreLocalD, local_offset, loc);
        } else if sz <= 4 {
            self.emit(OpCode::PushConst, 0, loc);
            self.emit(OpCode::StoreLocal, local_offset, loc);
        } else {
            let base_temp = self.get_temp_slot(0);
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, local_offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::StoreLocal, base_temp, loc);
            for i in 0..sz {
                self.emit(OpCode::LoadLocal, base_temp, loc);
                self.emit(OpCode::PushConst, i, loc);
                self.emit(OpCode::Add, 0, loc);
                self.emit(OpCode::PushConst, 0, loc);
                self.emit(OpCode::StoreMemByte, 0, loc);
            }
        }
    }
}
