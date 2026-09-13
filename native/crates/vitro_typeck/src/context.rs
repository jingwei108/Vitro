use super::{TypeChecker, VarSymbol};
use vitro_ast::{compute_type_size, SourceLoc, StructField, Type};
use vitro_shared::ErrorCode;
use std::collections::HashMap;

impl TypeChecker {
    /// Compute the byte size of a type using the registered struct/union definitions.
    pub fn compute_type_size(&self, ty: &Type) -> i32 {
        let struct_defs: HashMap<String, Vec<StructField>> = self
            .structs
            .iter()
            .map(|(name, sym)| {
                let fields: Vec<StructField> = sym
                    .fields
                    .iter()
                    .map(|(ty, name)| StructField {
                        ty: ty.clone(),
                        name: name.clone(),
                    })
                    .collect();
                (name.clone(), fields)
            })
            .collect();
        let union_defs: HashMap<String, Vec<StructField>> = self
            .unions
            .iter()
            .map(|(name, sym)| {
                let fields: Vec<StructField> = sym
                    .fields
                    .iter()
                    .map(|(ty, name)| StructField {
                        ty: ty.clone(),
                        name: name.clone(),
                    })
                    .collect();
                (name.clone(), fields)
            })
            .collect();
        let class_size_map: HashMap<String, i32> =
            self.classes.iter().map(|(name, sym)| (name.clone(), sym.size)).collect();
        compute_type_size(ty, &struct_defs, &union_defs, &class_size_map)
    }

    // =========================================================================
    // Scope management
    // =========================================================================

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub(crate) fn declare_var(&mut self, name: &str, ty: &Type, is_global: bool, is_extern: bool, is_static: bool) {
        if self.scopes.is_empty() {
            self.scopes.push(HashMap::new());
        }
        let scope = match self.scopes.last_mut() {
            Some(s) => s,
            None => return,
        };
        if let Some(existing) = scope.get(name) {
            if is_extern {
                // extern declaration of an existing symbol is allowed
                return;
            }
            // Non-extern definition can replace an extern declaration
            if existing.is_extern {
                scope.insert(
                    name.to_string(),
                    VarSymbol {
                        ty: ty.clone(),
                        is_global,
                        is_extern,
                        is_static,
                    },
                );
                return;
            }
            // Multiple static globals with the same name in different files are allowed
            // (internal linkage). We keep the latest one; access check is done at use site.
            if existing.is_static && is_static && is_global {
                scope.insert(
                    name.to_string(),
                    VarSymbol {
                        ty: ty.clone(),
                        is_global,
                        is_extern,
                        is_static,
                    },
                );
                return;
            }
            self.report_error(
                &format!("变量 '{}' 已在此作用域中声明", name),
                &SourceLoc { line: 0, column: 0, file_id: 0 },
                ErrorCode::E3001_VarRedeclared,
            );
            return;
        }
        scope.insert(
            name.to_string(),
            VarSymbol {
                ty: ty.clone(),
                is_global,
                is_extern,
                is_static,
            },
        );
    }

    pub(crate) fn lookup_var(&self, name: &str) -> Option<VarSymbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym.clone());
            }
        }
        None
    }
}
