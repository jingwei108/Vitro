use super::*;

pub(crate) fn gen_literal(gen: &mut BytecodeGen, value: i32, loc: &SourceLoc) {
    gen.emit(OpCode::PushConst, value, loc);
}

pub(crate) fn gen_float_literal(gen: &mut BytecodeGen, value: f64, ty: &Type, loc: &SourceLoc) {
    if ty.kind() == TypeKind::Double {
        let idx = gen.push_f64_constant(value);
        gen.emit(OpCode::PushConstD, idx, loc);
    } else {
        let bits = (value as f32).to_bits() as i32;
        gen.emit(OpCode::PushConstF, bits, loc);
    }
}

pub(crate) fn gen_long_literal(gen: &mut BytecodeGen, value: i64, loc: &SourceLoc) {
    let idx = gen.push_i64_constant(value);
    gen.emit(OpCode::PushConstQ, idx, loc);
}

pub(crate) fn gen_string_literal(gen: &mut BytecodeGen, value: &str, loc: &SourceLoc) {
    let aligned = ((vitro_shared::cstring_len(value) + 1) as u32 + 3) & !3;
    let addr = vitro_runtime::GLOBAL_START + gen.next_global_offset as u32;
    // R1 ②：越界判据改走 bump_global_offset 的 GLOBAL_REGION_LIMIT 单源口径
    //（原为 MEM_SIZE / 16 魔数，与 VM 侧 HEAP_START 判据分裂）。
    if gen.bump_global_offset(aligned as i32, "字符串字面量", loc).is_some() {
        gen.string_data.push((addr, value.to_string()));
    }
    gen.emit(OpCode::PushConst, addr as i32, loc);
}
