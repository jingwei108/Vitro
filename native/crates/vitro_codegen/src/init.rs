//! 全局初始化扁平化与数组 stride 计算。

use crate::BytecodeGen;
use vitro_ast::*;
use vitro_shared::SourceLoc;

/// 字面量的数值形态：整数走 i64（避免 f64 只有 53 位尾数丢精度），浮点走 f64。
#[derive(Clone, Copy)]
enum LitNum {
    I(i64),
    F(f64),
}

fn lit_num(expr: &Expr) -> Option<LitNum> {
    match expr {
        // typeck 的 insert_implicit_cast 会把初始化元素包装为 Cast{Literal}，
        // 剥开后按内层字面量取值（Cast 语义 = 转换为目标类型，与本函数的编码一致）
        Expr::Cast { expr: inner, .. } => lit_num(inner),
        Expr::Literal { value, .. } => Some(LitNum::I(*value as i64)),
        Expr::LongLiteral { value, .. } => Some(LitNum::I(*value)),
        Expr::FloatLiteral { value, .. } => Some(LitNum::F(*value)),
        Expr::Unary { op: UnaryOp::Neg, operand, .. } => match lit_num(operand)? {
            LitNum::I(v) => Some(LitNum::I(v.wrapping_neg())),
            LitNum::F(v) => Some(LitNum::F(-v)),
        },
        _ => None,
    }
}

/// T-P0-1/T-P0-2：把初始化字面量按**目标类型**编码为全局初始化位模式。
/// 返回 `(位模式, 是否 8 字节槽)`；非字面量返回 None。
///
/// 此前各初始化路径按**字面量自身类型**写入：`double g = 1` 把 int 位模式
/// 写进 double 槽（得 0）；`long long ga[2] = {1,2}` 把 i64 写成 f64 位模式。
pub(crate) fn literal_init_bits(expr: &Expr, target_kind: TypeKind) -> Option<(u64, bool)> {
    let num = lit_num(expr)?;
    Some(match (num, target_kind) {
        (LitNum::I(v), TypeKind::Double) => ((v as f64).to_bits(), true),
        (LitNum::F(v), TypeKind::Double) => (v.to_bits(), true),
        (LitNum::I(v), TypeKind::Float) => ((v as f32).to_bits() as u64, false),
        (LitNum::F(v), TypeKind::Float) => ((v as f32).to_bits() as u64, false),
        // T-P0-2 根因：long long 目标必须写 i64 位模式，而非 f64 位模式
        (LitNum::I(v), TypeKind::LongLong) => (v as u64, true),
        (LitNum::F(v), TypeKind::LongLong) => (v as i64 as u64, true),
        (LitNum::I(v), _) => (v as i32 as u32 as u64, false),
        (LitNum::F(v), _) => (v as i32 as u32 as u64, false),
    })
}

impl BytecodeGen {
    /// 按目标类型写入一个字面量初始化（辅助收敛各初始化路径的位模式决策）。
    pub(crate) fn push_literal_init(&mut self, offset: u32, expr: &Expr, target_ty: &Type) -> bool {
        match literal_init_bits(expr, target_ty.kind()) {
            Some((bits, is64)) => {
                if is64 {
                    self.globals_init_64.push((offset, bits));
                } else {
                    self.globals_init_32.push((offset, bits as i32));
                }
                true
            }
            None => false,
        }
    }

    /// Flatten a nested InitList into globals_init_32 / globals_init_64 for global variable initialization.
    pub(crate) fn flatten_global_init(&mut self, target_ty: &Type, init: &Expr, base_offset: u32) {
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
                    for (i, elem) in elements.iter().enumerate() {
                        if i >= fields.len() {
                            break;
                        }
                        let field_offset = fields.iter().take(i).map(|f| self.type_size(&f.ty)).sum::<i32>() as u32;
                        self.flatten_global_init(&fields[i].ty, &elem.value, base_offset + field_offset);
                    }
                } else if target_ty.is_array() {
                    let inner_ty = target_ty.subscript_type();
                    let elem_size = self.type_size(&inner_ty) as u32;
                    for (i, elem) in elements.iter().enumerate() {
                        let elem_offset = (i as u32) * elem_size;
                        self.flatten_global_init(&inner_ty, &elem.value, base_offset + elem_offset);
                    }
                } else {
                    if let Some(first) = elements.first() {
                        self.flatten_global_init(target_ty, &first.value, base_offset);
                    }
                }
            }
            // T-P0-1/T-P0-2：标量字面量（含负字面量）统一按目标类型编码位模式
            _ => {
                if !self.push_literal_init(base_offset, init, target_ty) {
                    match init {
                        Expr::Identifier { name, .. } => {
                            if let Some(&idx) = self.func_index.get(name) {
                                self.globals_init_32.push((base_offset, idx));
                            }
                        }
                        Expr::StringLiteral { value, .. } => {
                            // 延迟到 Pass 1 结束后分配地址，确保字符串区位于全局变量区之后。
                            self.pending_string_inits.push((base_offset, value.clone()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub(crate) fn stmt_loc(stmt: &Stmt) -> SourceLoc {
    match stmt {
        Stmt::Block { loc, .. } => *loc,
        Stmt::VarDecl { loc, .. } => *loc,
        Stmt::Expr { loc, .. } => *loc,
        Stmt::If { loc, .. } => *loc,
        Stmt::While { loc, .. } => *loc,
        Stmt::DoWhile { loc, .. } => *loc,
        Stmt::For { loc, .. } => *loc,
        Stmt::Return { loc, .. } => *loc,
        Stmt::Break { loc, .. } => *loc,
        Stmt::Continue { loc, .. } => *loc,
        Stmt::Switch { loc, .. } => *loc,
        Stmt::Case { loc, .. } => *loc,
        Stmt::Goto { loc, .. } => *loc,
        Stmt::Label { loc, .. } => *loc,
        // === C++ 新增 (Phase 31 占位) ===
        _ => SourceLoc::default(),
    }
}

pub(crate) fn flatten_init_list(elements: &[InitElement], errors: &mut Vec<String>) -> Vec<i32> {
    let mut result = Vec::new();
    for elem in elements {
        // Designated initializer in flatten context: we can only handle simple sequential init
        // For designated init, we report an error since flatten is used for char arrays / simple arrays
        if !elem.designators.is_empty() {
            errors.push("Designated initializer 不支持在此上下文中扁平化".to_string());
        }
        match elem.value {
            Expr::Literal { value, .. } => result.push(value),
            Expr::LongLiteral { value, .. } => {
                if value < i32::MIN as i64 || value > i32::MAX as i64 {
                    errors.push(format!(
                        "初始化列表中的 long long 常量 {} 超出 int 范围，无法用于此上下文",
                        value
                    ));
                    result.push(0);
                } else {
                    result.push(value as i32);
                }
            }
            Expr::FloatLiteral { value, .. } => result.push((value as f32).to_bits() as i32),
            Expr::InitList { ref elements, .. } => result.extend(flatten_init_list(elements, errors)),
            Expr::Unary {
                op: UnaryOp::Neg, ref operand, ..
            } => {
                if let Expr::Literal { value, .. } = operand.as_ref() {
                    result.push(-*value);
                } else if let Expr::LongLiteral { value, .. } = operand.as_ref() {
                    if *value < i32::MIN as i64 || *value > i32::MAX as i64 {
                        errors.push(format!(
                            "初始化列表中的 long long 常量 {} 超出 int 范围，无法用于此上下文",
                            value
                        ));
                        result.push(0);
                    } else {
                        result.push(-*value as i32);
                    }
                } else {
                    result.push(0);
                }
            }
            _ => result.push(0),
        }
    }
    result
}

pub(crate) fn compute_stride(arr_type: &Type, elem_size: i32) -> i32 {
    if arr_type.is_array() && !arr_type.dims().is_empty() {
        let mut stride = elem_size;
        for i in 1..arr_type.dims().len() {
            let dim = arr_type.dims()[i];
            if dim <= 0 {
                // Guard: non-positive dimension indicates VLA (handled at runtime)
                // or corrupted AST. Return 0 as sentinel.
                return 0;
            }
            stride = stride.checked_mul(dim).unwrap_or(0);
            if stride == 0 {
                // Overflow: dimension product exceeds i32 range.
                return 0;
            }
        }
        stride
    } else if let Type::Pointer { pointee, .. } = arr_type {
        if pointee.is_array() && !pointee.dims().is_empty() {
            let mut stride = elem_size;
            for i in 0..pointee.dims().len() {
                let dim = pointee.dims()[i];
                if dim <= 0 {
                    return 0;
                }
                stride = stride.checked_mul(dim).unwrap_or(0);
                if stride == 0 {
                    return 0;
                }
            }
            stride
        } else {
            elem_size
        }
    } else {
        elem_size
    }
}
