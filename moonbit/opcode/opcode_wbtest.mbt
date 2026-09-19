///|
/// 0..255 全空间断言（T3 验收锚）：from_u8 恰好命中 132 个编号，
/// 且命中的编号与 code() 互逆；其余全部 None。
test "from_u8_full_space_0_to_255" {
  let mut hit = 0
  for v in 0..<=255 {
    match OpCode::from_u8(v) {
      Some(op) => {
        hit += 1
        assert_eq(op.code(), v)
      }
      None => ()
    }
  }
  assert_eq(hit, 132)
}

///|
/// 空号断言：44–49 六个历史预留空号全部 None（编号照搬不重排的证据锚）。
test "from_u8_holes_44_to_49" {
  for v in 44..<=49 {
    assert_true(OpCode::from_u8(v) is None)
  }
}

///|
/// 越界：>137 与负值 None（u8 表示域外的负值防御）。
test "from_u8_out_of_range" {
  assert_true(OpCode::from_u8(138) is None)
  assert_true(OpCode::from_u8(255) is None)
  assert_true(OpCode::from_u8(-1) is None)
}

///|
/// 双向映射互逆：每个 opcode 的 name → from_name 恢复原值，
/// 且名字唯一（from_name 不会两义）。
test "name_from_name_roundtrip_all_132" {
  let mut seen = 0
  for v in 0..<=137 {
    match OpCode::from_u8(v) {
      Some(op) => {
        seen += 1
        match OpCode::from_name(op.name()) {
          Some(back) => assert_eq(back.code(), op.code())
          None => fail("from_name 未识别 \{op.name()}")
        }
      }
      None => ()
    }
  }
  assert_eq(seen, 132)
}

///|
/// 关键编号抽查（防整体漂移的定点锚，值亲验自 Rust 源）：
/// Nop=0 / PushConst=1 / Jump=29 / Call=32 / Strlen=126（死码）/
/// TrapBoundsVla=129（死码）/ LShrQ=137（末位）。
test "landmark_codes_frozen" {
  assert_eq(Nop.code(), 0)
  assert_eq(PushConst.code(), 1)
  assert_eq(Jump.code(), 29)
  assert_eq(Call.code(), 32)
  assert_eq(Strlen.code(), 126)
  assert_eq(TrapBoundsVla.code(), 129)
  assert_eq(LShrQ.code(), 137)
}

///|
/// 编号连续性形状：浮点段 50 起（44–49 空号后的重排历史），
/// PushConstF=50 是浮点段首位。
test "float_segment_starts_at_50" {
  assert_eq(PushConstF.code(), 50)
  assert_true(OpCode::from_u8(49) is None)
}

///|
/// 死码编号位保留（零发射但编号冻结——产物对拍依赖）。
test "dead_opcodes_keep_number_slots" {
  assert_eq(Strlen.name(), "Strlen")
  assert_eq(TrapBoundsVla.name(), "TrapBoundsVla")
}

///|
/// Instruction 三元组构造与相等（含 loc 全字段参与判等）。
test "instruction_construct_and_equality" {
  let a = Instruction::new(PushConst, 7, @source.SourceLoc::new(1, 1))
  let b = Instruction::new(PushConst, 7, @source.SourceLoc::new(1, 1))
  let c = Instruction::new(PushConst, 8, @source.SourceLoc::new(1, 1))
  let d = Instruction::new(PushConst, 7, @source.SourceLoc::new(2, 1))
  assert_eq(a, b)
  assert_true(a != c)
  assert_true(a != d)
}

///|
/// OpCode 可作 Map 键（Hash）且全序可用（Compare——编号序即声明序）。
test "opcode_hash_and_compare" {
  let m : Map[OpCode, Int] = Map([])
  m[Nop] = 1
  m[PushConst] = 2
  assert_eq(m.length(), 2)
  assert_true(Nop < LShrQ)
  assert_true(Jump < Call)
}
