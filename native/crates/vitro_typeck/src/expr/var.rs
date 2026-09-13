use super::*;

impl TypeChecker {
    pub(crate) fn resolve_identifier(&mut self, expr: &mut Expr) -> Type {
        let (name, loc) = if let Expr::Identifier { name, loc, .. } = expr {
            (name.clone(), *loc)
        } else {
            unreachable!()
        };

        let result = if let Some(sym) = self.lookup_var(&name) {
            if sym.is_static && sym.is_global {
                if let Some(files) = self.static_global_files.get(&name) {
                    if !files.contains(&self.current_file) {
                        self.report_error(
                            &format!("static 全局变量 '{}' 在其他文件中不可见", name),
                            &loc,
                            ErrorCode::E3059_StaticGlobalAccess,
                        );
                    }
                }
            }
            let mut result_ty = sym.ty.clone();
            // Auto-dereference reference types for expression value
            if let Type::Reference { base, .. } | Type::RValueRef { base, .. } = &sym.ty {
                result_ty = *base.clone();
            }
            result_ty
        } else if let Some(sym) = self.funcs.get(&name).cloned() {
            // Function name used as value (function pointer)
            Type::function_pointer(sym.return_type, sym.param_types)
        } else {
            // Check if it's a class field (implicit this->field)
            if let Some(class_name) = self.current_class.clone() {
                if let Some(class_sym) = self.classes.get(&class_name) {
                    if let Some((field_ty, _, _)) = class_sym.fields.iter().find(|(_, n, _)| n == &name) {
                        let this_ty = Type::Pointer {
                            pointee: Box::new(Type::Class {
                                name: class_name.clone(),
                                is_const: self.current_method_is_const,
                            }),
                            is_const: self.current_method_is_const,
                        };
                        *expr = Expr::Member {
                            object: Box::new(Expr::Identifier {
                                name: "this".to_string(),
                                ty: this_ty.clone(),
                                loc,
                            }),
                            member: name.clone(),
                            ty: field_ty.clone(),
                            loc,
                        };
                        return self.resolve_expr_type(expr);
                    }
                }
            }
            if name == "__func__" {
                // C99 __func__（E1 B 档）：预定义标识符，按 const char 数组定型
                //（退化语义与字符串字面量一致）；值由 codegen 发射当前函数名
                return Type::Array {
                    element: Box::new(Type::char()),
                    array_size: name.len() as i32, // 名字长度运行期才定，占位即可
                    dims: vec![name.len() as i32],
                    is_const: true,
                    is_vla: false,
                    vla_dims: Vec::new(),
                };
            }
            self.report_error(&format!("未声明的变量 '{}'", name), &loc, ErrorCode::E3023_UndeclaredVar);
            Type::int()
        };

        if let Expr::Identifier { ty, .. } = expr {
            *ty = result.clone();
        }
        result
    }

    pub(crate) fn resolve_index(&mut self, array: &mut Expr, index: &mut Expr, loc: &SourceLoc, ty: &mut Type) -> Type {
        let arr_type = self.resolve_expr_type(array);
        let idx_type = self.resolve_expr_type(index);
        if !self.is_int(&idx_type) {
            self.report_error("数组索引必须是 int 类型", loc, ErrorCode::E3039_ArrayIndexType);
            *ty = Type::int();
        } else if !arr_type.is_array() && !arr_type.is_pointer() {
            self.report_error("不能对非数组/指针类型进行索引", loc, ErrorCode::E3040_IndexNonArray);
            *ty = Type::int();
        } else if arr_type.is_array() {
            *ty = arr_type.subscript_type();
        } else if let Type::Pointer { pointee, .. } = arr_type {
            *ty = *pointee.clone();
        } else {
            *ty = Type::int();
        }
        ty.clone()
    }

    pub(crate) fn resolve_member(&mut self, object: &mut Expr, member: &str, loc: &SourceLoc, ty: &mut Type) -> Type {
        let obj_type = self.resolve_expr_type(object);
        let (type_name, kind) = if obj_type.is_struct() {
            (obj_type.name().to_string(), "struct")
        } else if obj_type.is_union() {
            (obj_type.name().to_string(), "union")
        } else if obj_type.is_class() {
            (obj_type.name().to_string(), "class")
        } else if let Type::Pointer { pointee, .. } = &obj_type {
            if let Type::Struct { name, .. } = pointee.as_ref() {
                (name.clone(), "struct")
            } else if let Type::Union { name, .. } = pointee.as_ref() {
                (name.clone(), "union")
            } else if let Type::Class { name, .. } = pointee.as_ref() {
                (name.clone(), "class")
            } else {
                self.report_error(
                    "'.' 和 '->' 只能用于结构体、联合体或类类型",
                    loc,
                    ErrorCode::E3041_MemberNonStruct,
                );
                *ty = Type::int();
                return ty.clone();
            }
        } else if let Type::Reference { base, .. } | Type::RValueRef { base } = &obj_type {
            if let Type::Struct { name, .. } = base.as_ref() {
                (name.clone(), "struct")
            } else if let Type::Union { name, .. } = base.as_ref() {
                (name.clone(), "union")
            } else if let Type::Class { name, .. } = base.as_ref() {
                (name.clone(), "class")
            } else {
                self.report_error(
                    "'.' 和 '->' 只能用于结构体、联合体或类类型",
                    loc,
                    ErrorCode::E3041_MemberNonStruct,
                );
                *ty = Type::int();
                return ty.clone();
            }
        } else {
            self.report_error(
                "'.' 和 '->' 只能用于结构体、联合体或类类型",
                loc,
                ErrorCode::E3041_MemberNonStruct,
            );
            *ty = Type::int();
            return ty.clone();
        };
        let (field_type, access) = if kind == "union" {
            (self.get_union_field_type(&type_name, member), None)
        } else if kind == "struct" {
            (self.get_struct_field_type(&type_name, member), None)
        } else {
            self.get_class_field_type_with_access(&type_name, member)
        };
        if let Some(ft) = field_type {
            // Access control check for class members
            if let Some(acc) = access {
                if matches!(acc, AccessSpec::Private) && self.current_class.as_ref() != Some(&type_name) {
                    self.report_error(
                        &format!("无法访问类 '{}' 的私有成员 '{}'", type_name, member),
                        loc,
                        ErrorCode::E4024_PrivateMemberAccess,
                    );
                }
            }
            *ty = ft;
        } else {
            let kind_str = if kind == "union" {
                "联合体"
            } else if kind == "struct" {
                "结构体"
            } else {
                "类"
            };
            self.report_error(
                &format!("{} '{}' 没有成员 '{}'", kind_str, type_name, member),
                loc,
                ErrorCode::E3042_UnknownMember,
            );
            *ty = Type::int();
        }
        ty.clone()
    }

    pub(crate) fn resolve_this(&mut self, loc: &SourceLoc, ty: &mut Type) -> Type {
        if let Some(ref class_name) = self.current_class {
            *ty = Type::Pointer {
                pointee: Box::new(Type::Class {
                    name: class_name.clone(),
                    is_const: self.current_method_is_const,
                }),
                is_const: self.current_method_is_const,
            };
        } else {
            self.report_error("'this' 只能在类成员函数中使用", loc, ErrorCode::E4023_ThisOutsideClass);
            *ty = Type::int();
        }
        ty.clone()
    }
}
