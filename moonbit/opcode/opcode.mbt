///|
/// Vitro 字节码操作码（132 条，编号照搬 Rust oracle `vitro_runtime/src/opcode.rs`，不重排）。
///
/// 编号契约：0..137 区间，空号 44–49（历史浮点段位预留）；`from_u8` 对空号与
/// >137 越界值返回 None（0..255 全空间断言见 opcode_wbtest）。
/// `Strlen` / `TrapBoundsVla` 为零发射死码——保留编号位（产物对拍依赖编号冻结），
/// 不新增使用。
///
/// operand 语义校验：**未在本版落地**——Rust 侧无先验语义表，将待 S5（codegen 片）
/// 以发射侧为真值源补齐（诚实登记，不脑测发明语义）。
pub(all) enum OpCode {
  Nop
  PushConst
  LoadLocal
  StoreLocal
  LoadGlobal
  StoreGlobal
  GetFrameBase
  Pop
  Dup
  Swap
  LoadMem
  StoreMem
  LoadMemByte
  StoreMemByte
  Add
  Sub
  Mul
  Div
  Mod
  Neg
  Eq
  Ne
  Lt
  Le
  Gt
  Ge
  And
  Or
  Not
  Jump
  JumpIfZero
  JumpIfNotZero
  Call
  CallHost
  Ret
  RetVoid
  StepEvent
  TrapBounds
  BitAnd
  BitOr
  BitXor
  BitNot
  Shl
  Shr
  PushConstF
  AddF
  SubF
  MulF
  DivF
  NegF
  CastI2F
  CastF2I
  EqF
  NeF
  LtF
  LeF
  GtF
  GeF
  PushConstD
  AddD
  SubD
  MulD
  DivD
  NegD
  CastI2D
  CastF2D
  CastD2I
  CastD2F
  EqD
  NeD
  LtD
  LeD
  GtD
  GeD
  LoadLocalD
  StoreLocalD
  LoadGlobalD
  StoreGlobalD
  LoadMemD
  StoreMemD
  SplitD
  PushConstQ
  AddQ
  SubQ
  MulQ
  DivQ
  ModQ
  NegQ
  CastI2Q
  CastQ2I
  CastQ2D
  CastD2Q
  EqQ
  NeQ
  LtQ
  LeQ
  GtQ
  GeQ
  LoadLocalQ
  StoreLocalQ
  LoadGlobalQ
  StoreGlobalQ
  LoadMemQ
  StoreMemQ
  SplitQ
  CallPtr
  ULt
  ULe
  UGt
  UGe
  UDiv
  UMod
  LShr
  StackAlloc
  USub
  UNeg
  UAdd
  UMul
  Memcpy
  Memset
  Strlen
  PushArgc
  PushArgv
  TrapBoundsVla
  BitAndQ
  BitOrQ
  BitXorQ
  CallVar
  BitNotQ
  ShlQ
  ShrQ
  LShrQ
} derive(Debug, Eq, Hash, Compare)

///|
/// opcode 编号（穷尽 match，编号手工显式——照搬 Rust，不重排）。
pub fn OpCode::code(self : OpCode) -> Int {
  match self {
    Nop => 0
    PushConst => 1
    LoadLocal => 2
    StoreLocal => 3
    LoadGlobal => 4
    StoreGlobal => 5
    GetFrameBase => 6
    Pop => 7
    Dup => 8
    Swap => 9
    LoadMem => 10
    StoreMem => 11
    LoadMemByte => 12
    StoreMemByte => 13
    Add => 14
    Sub => 15
    Mul => 16
    Div => 17
    Mod => 18
    Neg => 19
    Eq => 20
    Ne => 21
    Lt => 22
    Le => 23
    Gt => 24
    Ge => 25
    And => 26
    Or => 27
    Not => 28
    Jump => 29
    JumpIfZero => 30
    JumpIfNotZero => 31
    Call => 32
    CallHost => 33
    Ret => 34
    RetVoid => 35
    StepEvent => 36
    TrapBounds => 37
    BitAnd => 38
    BitOr => 39
    BitXor => 40
    BitNot => 41
    Shl => 42
    Shr => 43
    PushConstF => 50
    AddF => 51
    SubF => 52
    MulF => 53
    DivF => 54
    NegF => 55
    CastI2F => 56
    CastF2I => 57
    EqF => 58
    NeF => 59
    LtF => 60
    LeF => 61
    GtF => 62
    GeF => 63
    PushConstD => 64
    AddD => 65
    SubD => 66
    MulD => 67
    DivD => 68
    NegD => 69
    CastI2D => 70
    CastF2D => 71
    CastD2I => 72
    CastD2F => 73
    EqD => 74
    NeD => 75
    LtD => 76
    LeD => 77
    GtD => 78
    GeD => 79
    LoadLocalD => 80
    StoreLocalD => 81
    LoadGlobalD => 82
    StoreGlobalD => 83
    LoadMemD => 84
    StoreMemD => 85
    SplitD => 86
    PushConstQ => 87
    AddQ => 88
    SubQ => 89
    MulQ => 90
    DivQ => 91
    ModQ => 92
    NegQ => 93
    CastI2Q => 94
    CastQ2I => 95
    CastQ2D => 96
    CastD2Q => 97
    EqQ => 98
    NeQ => 99
    LtQ => 100
    LeQ => 101
    GtQ => 102
    GeQ => 103
    LoadLocalQ => 104
    StoreLocalQ => 105
    LoadGlobalQ => 106
    StoreGlobalQ => 107
    LoadMemQ => 108
    StoreMemQ => 109
    SplitQ => 110
    CallPtr => 111
    ULt => 112
    ULe => 113
    UGt => 114
    UGe => 115
    UDiv => 116
    UMod => 117
    LShr => 118
    StackAlloc => 119
    USub => 120
    UNeg => 121
    UAdd => 122
    UMul => 123
    Memcpy => 124
    Memset => 125
    Strlen => 126
    PushArgc => 127
    PushArgv => 128
    TrapBoundsVla => 129
    BitAndQ => 130
    BitOrQ => 131
    BitXorQ => 132
    CallVar => 133
    BitNotQ => 134
    ShlQ => 135
    ShrQ => 136
    LShrQ => 137
  }
}

///|
/// 编号 → opcode；空号（44–49）与越界（>137 / <0）返回 None。
pub fn OpCode::from_u8(value : Int) -> OpCode? {
  match value {
    0 => Some(Nop)
    1 => Some(PushConst)
    2 => Some(LoadLocal)
    3 => Some(StoreLocal)
    4 => Some(LoadGlobal)
    5 => Some(StoreGlobal)
    6 => Some(GetFrameBase)
    7 => Some(Pop)
    8 => Some(Dup)
    9 => Some(Swap)
    10 => Some(LoadMem)
    11 => Some(StoreMem)
    12 => Some(LoadMemByte)
    13 => Some(StoreMemByte)
    14 => Some(Add)
    15 => Some(Sub)
    16 => Some(Mul)
    17 => Some(Div)
    18 => Some(Mod)
    19 => Some(Neg)
    20 => Some(Eq)
    21 => Some(Ne)
    22 => Some(Lt)
    23 => Some(Le)
    24 => Some(Gt)
    25 => Some(Ge)
    26 => Some(And)
    27 => Some(Or)
    28 => Some(Not)
    29 => Some(Jump)
    30 => Some(JumpIfZero)
    31 => Some(JumpIfNotZero)
    32 => Some(Call)
    33 => Some(CallHost)
    34 => Some(Ret)
    35 => Some(RetVoid)
    36 => Some(StepEvent)
    37 => Some(TrapBounds)
    38 => Some(BitAnd)
    39 => Some(BitOr)
    40 => Some(BitXor)
    41 => Some(BitNot)
    42 => Some(Shl)
    43 => Some(Shr)
    50 => Some(PushConstF)
    51 => Some(AddF)
    52 => Some(SubF)
    53 => Some(MulF)
    54 => Some(DivF)
    55 => Some(NegF)
    56 => Some(CastI2F)
    57 => Some(CastF2I)
    58 => Some(EqF)
    59 => Some(NeF)
    60 => Some(LtF)
    61 => Some(LeF)
    62 => Some(GtF)
    63 => Some(GeF)
    64 => Some(PushConstD)
    65 => Some(AddD)
    66 => Some(SubD)
    67 => Some(MulD)
    68 => Some(DivD)
    69 => Some(NegD)
    70 => Some(CastI2D)
    71 => Some(CastF2D)
    72 => Some(CastD2I)
    73 => Some(CastD2F)
    74 => Some(EqD)
    75 => Some(NeD)
    76 => Some(LtD)
    77 => Some(LeD)
    78 => Some(GtD)
    79 => Some(GeD)
    80 => Some(LoadLocalD)
    81 => Some(StoreLocalD)
    82 => Some(LoadGlobalD)
    83 => Some(StoreGlobalD)
    84 => Some(LoadMemD)
    85 => Some(StoreMemD)
    86 => Some(SplitD)
    87 => Some(PushConstQ)
    88 => Some(AddQ)
    89 => Some(SubQ)
    90 => Some(MulQ)
    91 => Some(DivQ)
    92 => Some(ModQ)
    93 => Some(NegQ)
    94 => Some(CastI2Q)
    95 => Some(CastQ2I)
    96 => Some(CastQ2D)
    97 => Some(CastD2Q)
    98 => Some(EqQ)
    99 => Some(NeQ)
    100 => Some(LtQ)
    101 => Some(LeQ)
    102 => Some(GtQ)
    103 => Some(GeQ)
    104 => Some(LoadLocalQ)
    105 => Some(StoreLocalQ)
    106 => Some(LoadGlobalQ)
    107 => Some(StoreGlobalQ)
    108 => Some(LoadMemQ)
    109 => Some(StoreMemQ)
    110 => Some(SplitQ)
    111 => Some(CallPtr)
    112 => Some(ULt)
    113 => Some(ULe)
    114 => Some(UGt)
    115 => Some(UGe)
    116 => Some(UDiv)
    117 => Some(UMod)
    118 => Some(LShr)
    119 => Some(StackAlloc)
    120 => Some(USub)
    121 => Some(UNeg)
    122 => Some(UAdd)
    123 => Some(UMul)
    124 => Some(Memcpy)
    125 => Some(Memset)
    126 => Some(Strlen)
    127 => Some(PushArgc)
    128 => Some(PushArgv)
    129 => Some(TrapBoundsVla)
    130 => Some(BitAndQ)
    131 => Some(BitOrQ)
    132 => Some(BitXorQ)
    133 => Some(CallVar)
    134 => Some(BitNotQ)
    135 => Some(ShlQ)
    136 => Some(ShrQ)
    137 => Some(LShrQ)
    _ => None
  }
}

///|
/// 变体名（与 Rust 变体名逐字一致——诊断显示与差分锚的稳定标识）。
pub fn OpCode::name(self : OpCode) -> String {
  match self {
    Nop => "Nop"
    PushConst => "PushConst"
    LoadLocal => "LoadLocal"
    StoreLocal => "StoreLocal"
    LoadGlobal => "LoadGlobal"
    StoreGlobal => "StoreGlobal"
    GetFrameBase => "GetFrameBase"
    Pop => "Pop"
    Dup => "Dup"
    Swap => "Swap"
    LoadMem => "LoadMem"
    StoreMem => "StoreMem"
    LoadMemByte => "LoadMemByte"
    StoreMemByte => "StoreMemByte"
    Add => "Add"
    Sub => "Sub"
    Mul => "Mul"
    Div => "Div"
    Mod => "Mod"
    Neg => "Neg"
    Eq => "Eq"
    Ne => "Ne"
    Lt => "Lt"
    Le => "Le"
    Gt => "Gt"
    Ge => "Ge"
    And => "And"
    Or => "Or"
    Not => "Not"
    Jump => "Jump"
    JumpIfZero => "JumpIfZero"
    JumpIfNotZero => "JumpIfNotZero"
    Call => "Call"
    CallHost => "CallHost"
    Ret => "Ret"
    RetVoid => "RetVoid"
    StepEvent => "StepEvent"
    TrapBounds => "TrapBounds"
    BitAnd => "BitAnd"
    BitOr => "BitOr"
    BitXor => "BitXor"
    BitNot => "BitNot"
    Shl => "Shl"
    Shr => "Shr"
    PushConstF => "PushConstF"
    AddF => "AddF"
    SubF => "SubF"
    MulF => "MulF"
    DivF => "DivF"
    NegF => "NegF"
    CastI2F => "CastI2F"
    CastF2I => "CastF2I"
    EqF => "EqF"
    NeF => "NeF"
    LtF => "LtF"
    LeF => "LeF"
    GtF => "GtF"
    GeF => "GeF"
    PushConstD => "PushConstD"
    AddD => "AddD"
    SubD => "SubD"
    MulD => "MulD"
    DivD => "DivD"
    NegD => "NegD"
    CastI2D => "CastI2D"
    CastF2D => "CastF2D"
    CastD2I => "CastD2I"
    CastD2F => "CastD2F"
    EqD => "EqD"
    NeD => "NeD"
    LtD => "LtD"
    LeD => "LeD"
    GtD => "GtD"
    GeD => "GeD"
    LoadLocalD => "LoadLocalD"
    StoreLocalD => "StoreLocalD"
    LoadGlobalD => "LoadGlobalD"
    StoreGlobalD => "StoreGlobalD"
    LoadMemD => "LoadMemD"
    StoreMemD => "StoreMemD"
    SplitD => "SplitD"
    PushConstQ => "PushConstQ"
    AddQ => "AddQ"
    SubQ => "SubQ"
    MulQ => "MulQ"
    DivQ => "DivQ"
    ModQ => "ModQ"
    NegQ => "NegQ"
    CastI2Q => "CastI2Q"
    CastQ2I => "CastQ2I"
    CastQ2D => "CastQ2D"
    CastD2Q => "CastD2Q"
    EqQ => "EqQ"
    NeQ => "NeQ"
    LtQ => "LtQ"
    LeQ => "LeQ"
    GtQ => "GtQ"
    GeQ => "GeQ"
    LoadLocalQ => "LoadLocalQ"
    StoreLocalQ => "StoreLocalQ"
    LoadGlobalQ => "LoadGlobalQ"
    StoreGlobalQ => "StoreGlobalQ"
    LoadMemQ => "LoadMemQ"
    StoreMemQ => "StoreMemQ"
    SplitQ => "SplitQ"
    CallPtr => "CallPtr"
    ULt => "ULt"
    ULe => "ULe"
    UGt => "UGt"
    UGe => "UGe"
    UDiv => "UDiv"
    UMod => "UMod"
    LShr => "LShr"
    StackAlloc => "StackAlloc"
    USub => "USub"
    UNeg => "UNeg"
    UAdd => "UAdd"
    UMul => "UMul"
    Memcpy => "Memcpy"
    Memset => "Memset"
    // 零发射死码（总计划 §5 亲证），保留编号位
    Strlen => "Strlen"
    PushArgc => "PushArgc"
    PushArgv => "PushArgv"
    // 零发射死码（总计划 §5 亲证），保留编号位
    TrapBoundsVla => "TrapBoundsVla"
    BitAndQ => "BitAndQ"
    BitOrQ => "BitOrQ"
    BitXorQ => "BitXorQ"
    CallVar => "CallVar"
    BitNotQ => "BitNotQ"
    ShlQ => "ShlQ"
    ShrQ => "ShrQ"
    LShrQ => "LShrQ"
  }
}

///|
/// 名字 → opcode（双向映射的逆向；未知名返回 None）。
pub fn OpCode::from_name(s : String) -> OpCode? {
  match s {
    "Nop" => Some(Nop)
    "PushConst" => Some(PushConst)
    "LoadLocal" => Some(LoadLocal)
    "StoreLocal" => Some(StoreLocal)
    "LoadGlobal" => Some(LoadGlobal)
    "StoreGlobal" => Some(StoreGlobal)
    "GetFrameBase" => Some(GetFrameBase)
    "Pop" => Some(Pop)
    "Dup" => Some(Dup)
    "Swap" => Some(Swap)
    "LoadMem" => Some(LoadMem)
    "StoreMem" => Some(StoreMem)
    "LoadMemByte" => Some(LoadMemByte)
    "StoreMemByte" => Some(StoreMemByte)
    "Add" => Some(Add)
    "Sub" => Some(Sub)
    "Mul" => Some(Mul)
    "Div" => Some(Div)
    "Mod" => Some(Mod)
    "Neg" => Some(Neg)
    "Eq" => Some(Eq)
    "Ne" => Some(Ne)
    "Lt" => Some(Lt)
    "Le" => Some(Le)
    "Gt" => Some(Gt)
    "Ge" => Some(Ge)
    "And" => Some(And)
    "Or" => Some(Or)
    "Not" => Some(Not)
    "Jump" => Some(Jump)
    "JumpIfZero" => Some(JumpIfZero)
    "JumpIfNotZero" => Some(JumpIfNotZero)
    "Call" => Some(Call)
    "CallHost" => Some(CallHost)
    "Ret" => Some(Ret)
    "RetVoid" => Some(RetVoid)
    "StepEvent" => Some(StepEvent)
    "TrapBounds" => Some(TrapBounds)
    "BitAnd" => Some(BitAnd)
    "BitOr" => Some(BitOr)
    "BitXor" => Some(BitXor)
    "BitNot" => Some(BitNot)
    "Shl" => Some(Shl)
    "Shr" => Some(Shr)
    "PushConstF" => Some(PushConstF)
    "AddF" => Some(AddF)
    "SubF" => Some(SubF)
    "MulF" => Some(MulF)
    "DivF" => Some(DivF)
    "NegF" => Some(NegF)
    "CastI2F" => Some(CastI2F)
    "CastF2I" => Some(CastF2I)
    "EqF" => Some(EqF)
    "NeF" => Some(NeF)
    "LtF" => Some(LtF)
    "LeF" => Some(LeF)
    "GtF" => Some(GtF)
    "GeF" => Some(GeF)
    "PushConstD" => Some(PushConstD)
    "AddD" => Some(AddD)
    "SubD" => Some(SubD)
    "MulD" => Some(MulD)
    "DivD" => Some(DivD)
    "NegD" => Some(NegD)
    "CastI2D" => Some(CastI2D)
    "CastF2D" => Some(CastF2D)
    "CastD2I" => Some(CastD2I)
    "CastD2F" => Some(CastD2F)
    "EqD" => Some(EqD)
    "NeD" => Some(NeD)
    "LtD" => Some(LtD)
    "LeD" => Some(LeD)
    "GtD" => Some(GtD)
    "GeD" => Some(GeD)
    "LoadLocalD" => Some(LoadLocalD)
    "StoreLocalD" => Some(StoreLocalD)
    "LoadGlobalD" => Some(LoadGlobalD)
    "StoreGlobalD" => Some(StoreGlobalD)
    "LoadMemD" => Some(LoadMemD)
    "StoreMemD" => Some(StoreMemD)
    "SplitD" => Some(SplitD)
    "PushConstQ" => Some(PushConstQ)
    "AddQ" => Some(AddQ)
    "SubQ" => Some(SubQ)
    "MulQ" => Some(MulQ)
    "DivQ" => Some(DivQ)
    "ModQ" => Some(ModQ)
    "NegQ" => Some(NegQ)
    "CastI2Q" => Some(CastI2Q)
    "CastQ2I" => Some(CastQ2I)
    "CastQ2D" => Some(CastQ2D)
    "CastD2Q" => Some(CastD2Q)
    "EqQ" => Some(EqQ)
    "NeQ" => Some(NeQ)
    "LtQ" => Some(LtQ)
    "LeQ" => Some(LeQ)
    "GtQ" => Some(GtQ)
    "GeQ" => Some(GeQ)
    "LoadLocalQ" => Some(LoadLocalQ)
    "StoreLocalQ" => Some(StoreLocalQ)
    "LoadGlobalQ" => Some(LoadGlobalQ)
    "StoreGlobalQ" => Some(StoreGlobalQ)
    "LoadMemQ" => Some(LoadMemQ)
    "StoreMemQ" => Some(StoreMemQ)
    "SplitQ" => Some(SplitQ)
    "CallPtr" => Some(CallPtr)
    "ULt" => Some(ULt)
    "ULe" => Some(ULe)
    "UGt" => Some(UGt)
    "UGe" => Some(UGe)
    "UDiv" => Some(UDiv)
    "UMod" => Some(UMod)
    "LShr" => Some(LShr)
    "StackAlloc" => Some(StackAlloc)
    "USub" => Some(USub)
    "UNeg" => Some(UNeg)
    "UAdd" => Some(UAdd)
    "UMul" => Some(UMul)
    "Memcpy" => Some(Memcpy)
    "Memset" => Some(Memset)
    "Strlen" => Some(Strlen)
    "PushArgc" => Some(PushArgc)
    "PushArgv" => Some(PushArgv)
    "TrapBoundsVla" => Some(TrapBoundsVla)
    "BitAndQ" => Some(BitAndQ)
    "BitOrQ" => Some(BitOrQ)
    "BitXorQ" => Some(BitXorQ)
    "CallVar" => Some(CallVar)
    "BitNotQ" => Some(BitNotQ)
    "ShlQ" => Some(ShlQ)
    "ShrQ" => Some(ShrQ)
    "LShrQ" => Some(LShrQ)
    _ => None
  }
}
