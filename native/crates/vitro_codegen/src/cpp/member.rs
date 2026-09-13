use vitro_ast::*;

use super::super::BytecodeGen;

impl BytecodeGen {
    pub(crate) fn get_class_member_offset(&self, class_name: &str, member_name: &str) -> i32 {
        let class = match self.class_defs.get(class_name) {
            Some(c) => c,
            None => return 0,
        };
        let mut offset = if class.vtable.is_some() { 4 } else { 0 };
        // Check base class fields first
        if let Some(ref base_name) = class.base {
            let base_offset = self.get_class_member_offset(base_name, member_name);
            if base_offset > 0 {
                return base_offset; // base_offset already includes vptr of base
            }
            let base_size =
                self.class_sizes
                    .get(base_name)
                    .copied()
                    .unwrap_or(if class.vtable.is_some() { 4 } else { 0 });
            offset = base_size;
        }
        // Search in current class fields
        for member in &class.members {
            if let ClassMember::Field { name, ty, .. } = member {
                if name == member_name {
                    return offset;
                }
                offset += self.type_size(ty);
            }
        }
        0
    }

    pub(crate) fn extract_class_name(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Class { name, .. } => Some(name.clone()),
            Type::Pointer { pointee, .. }
            | Type::Reference { base: pointee, .. }
            | Type::RValueRef { base: pointee } => self.extract_class_name(pointee),
            _ => None,
        }
    }
}
