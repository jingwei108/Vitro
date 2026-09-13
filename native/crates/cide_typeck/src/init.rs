use super::{insert_implicit_cast, TypeChecker};
use cide_ast::{Designator, Expr, SourceLoc, Type, TypeKind};
use cide_shared::ErrorCode;

impl TypeChecker {
    // =========================================================================
    // Initializer checks
    // =========================================================================

    pub(crate) fn check_struct_initializer(&mut self, struct_type: &Type, init: &mut Expr, loc: &SourceLoc) {
        // 复合字面量 (struct S){...} 等价于其初始化列表
        if let Expr::CompoundLiteral { target_type, init: inner, .. } = init {
            if *target_type != *struct_type {
                self.report_error(
                    &format!("复合字面量类型 '{}' 与目标结构体类型 '{}' 不匹配", target_type, struct_type),
                    loc,
                    ErrorCode::E3006_ArrayInitTypeMismatch,
                );
                return;
            }
            return self.check_struct_initializer(struct_type, inner, loc);
        }
        if !matches!(init, Expr::InitList { .. }) {
            let init_type = self.resolve_expr_type(init);
            if !self.check_assignable(struct_type, &init_type, loc) {
                self.report_error(
                    &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, struct_type),
                    loc,
                    ErrorCode::E3004_TypeMismatch,
                );
            }
            return;
        }
        let elements = match init {
            Expr::InitList { elements, .. } => elements.as_mut_slice(),
            _ => return,
        };
        let fields = match self.structs.get(struct_type.name()) {
            Some(s) => s.fields.clone(),
            None => {
                self.report_error(
                    &format!("未知的结构体类型 '{}'", struct_type.name()),
                    loc,
                    ErrorCode::E3004_TypeMismatch,
                );
                return;
            }
        };
        let has_designators = elements.iter().any(|e| !e.designators.is_empty());
        if has_designators {
            for elem in elements.iter_mut() {
                if elem.designators.is_empty() {
                    self.report_error(
                        "初始化列表中不能混合使用指定初始化和非指定初始化",
                        loc,
                        ErrorCode::E3005_ArrayInitTooMany,
                    );
                    continue;
                }
                if elem.designators.len() != 1 {
                    self.report_error("暂不支持多级 designated initializer", loc, ErrorCode::E3005_ArrayInitTooMany);
                    continue;
                }
                match &elem.designators[0] {
                    Designator::Field(field_name) => {
                        if let Some(field_idx) = fields.iter().position(|f| &f.1 == field_name) {
                            let field_ty = &fields[field_idx].0;
                            if field_ty.is_struct() && matches!(&elem.value, Expr::InitList { .. }) {
                                self.check_struct_initializer(field_ty, &mut elem.value, loc);
                            } else if field_ty.is_array() && matches!(&elem.value, Expr::InitList { .. }) {
                                let mut sub_ty = field_ty.clone();
                                self.check_array_initializer(&mut sub_ty, &mut elem.value, loc);
                            } else {
                                let e_type = self.resolve_expr_type(&mut elem.value);
                                if !self.check_assignable(field_ty, &e_type, loc) {
                                    self.report_error(
                                        &format!(
                                            "结构体初始化类型不匹配：字段 '{}' 期望 '{}'，实际 '{}'",
                                            field_name, field_ty, e_type
                                        ),
                                        loc,
                                        ErrorCode::E3006_ArrayInitTypeMismatch,
                                    );
                                } else {
                                    insert_implicit_cast(&mut elem.value, field_ty);
                                }
                            }
                        } else {
                            self.report_error(
                                &format!("结构体 '{}' 没有字段 '{}'", struct_type.name(), field_name),
                                loc,
                                ErrorCode::E3042_UnknownMember,
                            );
                        }
                    }
                    _ => {
                        self.report_error(
                            "结构体初始化只能使用 .field 形式的 designator",
                            loc,
                            ErrorCode::E3005_ArrayInitTooMany,
                        );
                    }
                }
            }
            return;
        }
        if elements.len() > fields.len() {
            self.report_error("初始化列表元素数量超过结构体字段数", loc, ErrorCode::E3005_ArrayInitTooMany);
        }
        for (i, elem) in elements.iter_mut().enumerate() {
            if i >= fields.len() {
                break;
            }
            if fields[i].0.is_struct() && matches!(&elem.value, Expr::InitList { .. }) {
                self.check_struct_initializer(&fields[i].0, &mut elem.value, loc);
            } else if fields[i].0.is_array() && matches!(&elem.value, Expr::InitList { .. }) {
                let mut sub_ty = fields[i].0.clone();
                self.check_array_initializer(&mut sub_ty, &mut elem.value, loc);
            } else {
                let e_type = self.resolve_expr_type(&mut elem.value);
                if !self.check_assignable(&fields[i].0, &e_type, loc) {
                    self.report_error(
                        &format!(
                            "结构体初始化类型不匹配：字段 '{}' 期望 '{}'，实际 '{}'",
                            fields[i].1, fields[i].0, e_type
                        ),
                        loc,
                        ErrorCode::E3006_ArrayInitTypeMismatch,
                    );
                }
            }
        }
    }

    fn validate_nested_init_list(
        &mut self,
        dims: &[i32],
        init: &mut Expr,
        loc: &SourceLoc,
        element_type: &Type,
    ) -> bool {
        if dims.is_empty() {
            if element_type.is_struct() && matches!(init, Expr::InitList { .. }) {
                self.check_struct_initializer(element_type, init, loc);
                return true;
            }
            if element_type.is_array() && matches!(init, Expr::InitList { .. }) {
                let mut sub_ty = element_type.clone();
                self.check_array_initializer(&mut sub_ty, init, loc);
                return true;
            }
            let e_type = self.resolve_expr_type(init);
            if !self.check_assignable(element_type, &e_type, loc) {
                self.report_error(
                    &format!("数组初始化元素类型不匹配：期望 '{}'，实际 '{}'", element_type, e_type),
                    loc,
                    ErrorCode::E3006_ArrayInitTypeMismatch,
                );
                return false;
            }
            insert_implicit_cast(init, element_type);
            return true;
        }
        if !matches!(init, Expr::InitList { .. }) {
            // C/C++ 允许括号省略：标量可用于初始化当前子数组的第一个元素，其余零填充。
            // 例如 int a[5][5] = {0}; int b[2][3] = {1, 2}; 都是合法的。
            let e_type = self.resolve_expr_type(init);
            if self.check_assignable(element_type, &e_type, loc) {
                insert_implicit_cast(init, element_type);
                return true;
            }
            self.report_error("多维数组初始化需要嵌套初始化列表", loc, ErrorCode::E3009_InvalidArrayInit);
            return false;
        }
        let elements = match init {
            Expr::InitList { elements, .. } => elements.as_mut_slice(),
            _ => return false,
        };
        // 若当前层级遇到纯扁平标量列表（如 int a[2][3] = {1,2,3,4,5,6};），
        // 允许元素数量达到当前子数组最深层元素总数，按行主序填充。
        let is_flat_scalar = elements.iter().all(|e| !matches!(e.value, Expr::InitList { .. }));
        let expected_count = if dims[0] > 0 {
            if is_flat_scalar && dims.len() > 1 {
                dims.iter().map(|&d| d as usize).product()
            } else {
                dims[0] as usize
            }
        } else {
            elements.len()
        };
        if elements.len() > expected_count {
            self.report_error("初始化列表元素数量超过数组维度大小", loc, ErrorCode::E3005_ArrayInitTooMany);
        }
        for elem in elements {
            if !self.validate_nested_init_list(&dims[1..], &mut elem.value, loc, element_type) {
                return false;
            }
        }
        true
    }

    pub(crate) fn check_array_initializer(&mut self, arr_type: &mut Type, init: &mut Expr, loc: &SourceLoc) {
        // 复合字面量 (T[]){...} 等价于其初始化列表
        if let Expr::CompoundLiteral { target_type, init: inner, .. } = init {
            if target_type.kind() != TypeKind::Array {
                self.report_error(
                    &format!("复合字面量类型 '{}' 不是数组类型", target_type),
                    loc,
                    ErrorCode::E3006_ArrayInitTypeMismatch,
                );
                return;
            }
            // 先递归检查内部初始化列表，让未指定大小的数组（如 int[]）推断出长度
            self.check_array_initializer(target_type, inner, loc);
            if *target_type != *arr_type {
                self.report_error(
                    &format!("复合字面量类型 '{}' 与目标数组类型 '{}' 不匹配", target_type, arr_type),
                    loc,
                    ErrorCode::E3006_ArrayInitTypeMismatch,
                );
                return;
            }
            return;
        }
        let elem_type = arr_type.innermost_element_type();

        if !arr_type.dims().is_empty() && arr_type.dims().len() > 1 {
            if let Expr::InitList { elements, .. } = init {
                let has_designators = elements.iter().any(|e| !e.designators.is_empty());
                if has_designators {
                    self.report_error(
                        "多维数组暂不支持 designated initializer",
                        loc,
                        ErrorCode::E3009_InvalidArrayInit,
                    );
                    return;
                }
                let total_elems = arr_type.total_elements();
                if let Type::Array { dims, array_size, .. } = arr_type {
                    // U1#12：仅"未指定尺寸"（哨兵 -1）可由初始化器推断——
                    // 此前 `<= 0` 会把显式 `int c[0]={1,2}` 静默推断成 2 元素
                    // 数组（clang 拒绝该写法）；显式 0/负尺寸由
                    // check_array_dims_legality 报错，这里不再兜底吞掉。
                    if dims[0] == -1 {
                        dims[0] = elements.len() as i32;
                        *array_size = total_elems;
                    }
                    let dims_copy = dims.clone();
                    self.validate_nested_init_list(&dims_copy, init, loc, &elem_type);
                }
            } else {
                let init_type = self.resolve_expr_type(init);
                self.report_error(
                    &format!("多维数组初始化必须使用嵌套初始化列表，不能是 '{}'", init_type),
                    loc,
                    ErrorCode::E3009_InvalidArrayInit,
                );
            }
            return;
        }

        if let Expr::InitList { elements, .. } = init {
            let has_designators = elements.iter().any(|e| !e.designators.is_empty());
            if has_designators {
                for elem in elements.iter_mut() {
                    if elem.designators.is_empty() {
                        self.report_error(
                            "初始化列表中不能混合使用指定初始化和非指定初始化",
                            loc,
                            ErrorCode::E3005_ArrayInitTooMany,
                        );
                        continue;
                    }
                    if elem.designators.len() != 1 {
                        self.report_error(
                            "暂不支持多级 designated initializer",
                            loc,
                            ErrorCode::E3005_ArrayInitTooMany,
                        );
                        continue;
                    }
                    match &mut elem.designators[0] {
                        Designator::Index(idx_expr) => {
                            let idx_ty = self.resolve_expr_type(idx_expr);
                            if !self.is_int(&idx_ty) {
                                self.report_error("数组索引必须是 int 类型", loc, ErrorCode::E3039_ArrayIndexType);
                            }
                            let e_type = self.resolve_expr_type(&mut elem.value);
                            // W0-4：同普通列表路径的 char 窄化豁免
                            let suppress = elem_type.kind() == TypeKind::Char
                                && Self::is_char_safe_initializer(&elem.value);
                            if suppress {
                                self.char_narrow_suppress = true;
                            }
                            let ok = self.check_assignable(&elem_type, &e_type, loc);
                            if suppress {
                                self.char_narrow_suppress = false;
                            }
                            if !ok {
                                self.report_error(
                                    &format!("数组初始化元素类型不匹配：期望 '{}'，实际 '{}'", elem_type, e_type),
                                    loc,
                                    ErrorCode::E3006_ArrayInitTypeMismatch,
                                );
                            } else {
                                insert_implicit_cast(&mut elem.value, &elem_type);
                            }
                        }
                        _ => {
                            self.report_error(
                                "数组初始化只能使用 [index] 形式的 designator",
                                loc,
                                ErrorCode::E3005_ArrayInitTooMany,
                            );
                        }
                    }
                }
                return;
            }
            let mut expected_size = arr_type.array_size();
            if expected_size <= 0 {
                expected_size = elements.len() as i32;
                if let Type::Array { array_size, dims, .. } = arr_type {
                    *array_size = expected_size;
                    if !dims.is_empty() {
                        dims[0] = expected_size;
                    }
                }
            }
            if elements.len() > expected_size as usize {
                self.report_error("初始化列表元素数量超过数组大小", loc, ErrorCode::E3005_ArrayInitTooMany);
            }
            for elem in elements.iter_mut() {
                if elem_type.is_struct() && matches!(&elem.value, Expr::InitList { .. }) {
                    self.check_struct_initializer(&elem_type, &mut elem.value, loc);
                } else if elem_type.is_array() && matches!(&elem.value, Expr::InitList { .. }) {
                    let mut sub_ty = elem_type.clone();
                    self.check_array_initializer(&mut sub_ty, &mut elem.value, loc);
                } else {
                    let e_type = self.resolve_expr_type(&mut elem.value);
                    // W0-4：char 元素的字符常量/值域内整常量初始化是 C 惯用写法，豁免窄化警告
                    let suppress = elem_type.kind() == TypeKind::Char
                        && Self::is_char_safe_initializer(&elem.value);
                    if suppress {
                        self.char_narrow_suppress = true;
                    }
                    let ok = self.check_assignable(&elem_type, &e_type, loc);
                    if suppress {
                        self.char_narrow_suppress = false;
                    }
                    if !ok {
                        self.report_error(
                            &format!("数组初始化元素类型不匹配：期望 '{}'，实际 '{}'", elem_type, e_type),
                            loc,
                            ErrorCode::E3006_ArrayInitTypeMismatch,
                        );
                    } else {
                        insert_implicit_cast(&mut elem.value, &elem_type);
                    }
                }
            }
        } else if let Expr::StringLiteral { value, .. } = init {
            if elem_type.kind() != TypeKind::Char {
                self.report_error(
                    "字符串字面量只能用于初始化 char 数组",
                    loc,
                    ErrorCode::E3007_StringInitNonCharArray,
                );
                return;
            }
            let str_len = value.len() as i32;
            if arr_type.array_size() <= 0 {
                if let Type::Array { array_size, .. } = arr_type {
                    *array_size = str_len + 1;
                }
            } else if str_len + 1 > arr_type.array_size() {
                self.report_error("字符串字面量长度超过数组大小", loc, ErrorCode::E3008_StringTooLong);
            }
        } else {
            let init_type = self.resolve_expr_type(init);
            self.report_error(
                &format!("数组初始化必须使用初始化列表或字符串字面量，不能是 '{}'", init_type),
                loc,
                ErrorCode::E3009_InvalidArrayInit,
            );
        }
    }
}
