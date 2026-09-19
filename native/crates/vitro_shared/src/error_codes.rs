//! Vitro 诊断错误码枚举。
//!
//! 下沉到 `vitro_shared` 供 lexer、parser、typeck、codegen、vm 等各 crate 共享。

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ErrorCode {
    Unknown = 0,
    E1001_UnknownChar = 1001,
    E1002_UnterminatedString = 1002,
    E1003_StringCrossLine = 1003,
    E1004_UnsupportedOp = 1004,
    E1005_InvalidDefine = 1005,
    E1006_UnsupportedFeature = 1006,
    E1007_ComplexDeclarator = 1007,
    E1010_UnterminatedComment = 1010,
    E1011_UnmatchedConditional = 1011,
    E1012_DuplicateElse = 1012,
    E1013_UnclosedConditional = 1013,
    // E2（模块化预处理器）
    E1014_CondExprError = 1014,
    E1015_IncludeCycle = 1015,
    E1016_TokenPasteInvalid = 1016,
    E1017_ExpandDepthExceeded = 1017,
    W1018_MacroShadowing = 1018,
    W1019_MacroArgSideEffect = 1019,
    E1020_StaticAssertFailed = 1020,
    /// include 目标不存在（U1#11 H-1：此前静默跳过、错误错位到使用点）。
    E1021_IncludeNotFound = 1021,
    /// U3#4：函数/类模板实例化轮数超上限（对齐 clang -ftemplate-depth 语义）。
    E1022_TemplateInstantiationLimit = 1022,
    E2001_ExpectedType = 2001,
    E2002_ExpectedArraySize = 2002,
    E2003_ExpectedExpr = 2003,
    E2004_ExpectedCaseOrDefault = 2004,
    E2005_ExpectedSemicolon = 2005,
    E2006_ExpectedClosingBrace = 2006,
    E2007_ExpectedClosingParen = 2007,
    E2008_ExpectedClosingBracket = 2008,
    E3001_VarRedeclared = 3001,
    E3002_StructRedeclared = 3002,
    E3003_FuncRedeclared = 3003,
    E3004_TypeMismatch = 3004,
    E3005_ArrayInitTooMany = 3005,
    E3006_ArrayInitTypeMismatch = 3006,
    E3007_StringInitNonCharArray = 3007,
    E3008_StringTooLong = 3008,
    E3009_InvalidArrayInit = 3009,
    E3010_BreakOutsideLoop = 3010,
    E3011_ContinueOutsideLoop = 3011,
    E3012_VoidFuncReturnValue = 3012,
    E3013_MissingReturnValue = 3013,
    E3014_ReturnTypeMismatch = 3014,
    E3015_InvalidCondition = 3015,
    E3016_ArithmeticTypeError = 3016,
    E3017_ComparisonTypeError = 3017,
    E3018_RelationTypeError = 3018,
    E3019_LogicTypeError = 3019,
    E3020_UnaryTypeError = 3020,
    E3021_DerefNonPointer = 3021,
    E3022_IncDecTypeError = 3022,
    E3023_UndeclaredVar = 3023,
    E3024_MallocArgCount = 3024,
    E3025_MallocArgType = 3025,
    E3026_FreeArgCount = 3026,
    E3027_FreeArgType = 3027,
    E3028_BuiltInArgCount = 3028,
    E3029_BuiltInArgType = 3029,
    E3030_PrintfArgCount = 3030,
    E3031_PrintfFirstArg = 3031,
    E3032_PrintfArgType = 3032,
    E3033_ScanfArgCount = 3033,
    E3034_ScanfFirstArg = 3034,
    E3035_ScanfArgType = 3035,
    E3036_UndefinedFunc = 3036,
    E3037_FuncArgCount = 3037,
    E3038_FuncArgType = 3038,
    E3039_ArrayIndexType = 3039,
    E3040_IndexNonArray = 3040,
    E3041_MemberNonStruct = 3041,
    E3042_UnknownMember = 3042,
    E3043_AssignToRValue = 3043,
    E3044_AssignTypeMismatch = 3044,
    E3045_CompoundAssignType = 3045,
    E3046_SwitchCondType = 3046,
    E3047_CaseNotConstant = 3047,
    E3048_BitOpTypeError = 3048,
    E3049_AssignToConst = 3049,
    W3050_AssignInCondition = 3050,
    W3051_ArrayBoundOffByOne = 3051,
    W3052_ArrayToPointerDecay = 3052,
    W3053_ImplicitScalarConversion = 3053,
    W3054_IntToPointerCast = 3054,
    W3055_VoidPointerCast = 3055,
    W3056_UnsignedToInt = 3056,
    H3057_ImplicitConversionHint = 3057,
    E3058_StaticFuncAccess = 3058,
    E3059_StaticGlobalAccess = 3059,
    E3060_UseAfterFree = 3060,
    E3061_DoubleFree = 3061,
    E3062_PrintfFormatMismatch = 3062,
    E3063_ScanfFormatMismatch = 3063,
    W3064_DoublePointerCast = 3064,
    E3065_ConstViolation = 3065,
    E3066_CallNonFunction = 3066,
    /// 指针类型不兼容的隐式赋值（P1-6）。
    ///
    /// 与 `W3053_ImplicitScalarConversion` 区分：那是**标量**隐式转换（int → char 截断等，
    /// "可能导致数据截断"的建议成立）；指针不兼容是另一类问题——C++ 向上转型
    /// （`Derived* → Base*`）本就允许且不需要转换，向下转型/无关类型才需要显式转换。
    /// 此前共用 W3053，把多态基础建议成了"数据截断"，属教学误导。
    W3067_PointerTypeMismatch = 3067,
    E3070_BufferOverflow = 3070,
    E3071_UndefinedLabel = 3071,
    E3072_StructSelfContain = 3072,
    // C++ 扩展错误码预留 (Phase 1)
    E4001_ExceptionNotSupported = 4001,
    E4002_OperatorOverloadNotSupported = 4002,
    E4003_TemplateSpecializationNotSupported = 4003,
    E4004_ThreadNotSupported = 4004,
    E4005_MultipleInheritanceNotSupported = 4005,
    E4006_NamespaceNotSupported = 4006,
    E4007_VirtualInheritanceNotSupported = 4007,
    E4008_ExplicitNotSupported = 4008,
    E4009_MutableNotSupported = 4009,
    E4010_FriendNotSupported = 4010,
    E4011_UsingDirectiveNotSupported = 4011,
    E4012_TypenameContextError = 4012,
    E4013_NewDeleteNotSupported = 4013,
    E4014_RefQualifierNotSupported = 4014,
    E4015_NoexceptNotSupported = 4015,
    E4016_StaticAssertNotSupported = 4016,
    E4017_ConstexprNotSupported = 4017,
    E4018_DeletingDestructorNotSupported = 4018,
    E4019_EnumClassNotSupported = 4019,
    E4020_RangeForNotSupported = 4020,
    E4021_BaseClassNotFound = 4021,
    E4022_TemplateRecursionDepthExceeded = 4022,
    E4023_ThisOutsideClass = 4023,
    E4024_PrivateMemberAccess = 4024,
    E4025_AutoRequiresInitializer = 4025,
    E4026_AmbiguousMethodCall = 4026,
    E4027_InvalidNewType = 4027,
    E4028_InvalidDeleteType = 4028,
    E4029_ReferenceBindLvalueRequired = 4029,
    E4030_ConstructorNotFound = 4030,
    E4031_ConstructorOverloadAmbiguous = 4031,
    // C++ 教学知识卡片（运行时/设计缺陷提示）
    E4100_CppMemoryLeak = 4100,
    E4101_CppDanglingReference = 4101,
    E4102_CppObjectSlicing = 4102,
    E4103_CppUniquePtrDoubleFree = 4103,
    E4104_CppUseAfterMove = 4104,
    E4105_CppShallowCopyDoubleFree = 4105,
    E4106_CppReferenceToTemporary = 4106,
}

/// W 级码白名单（P3）。**单点维护**：与枚举的 W 变体集合必须双向一致
/// ——由 `test_whitelist_matches_source_variants` 对账本源文件（新增 W 码
/// 漏登记时源提取集 ≠ 白名单集，先红）。此前的"计数锚"断言测试内手工
/// 清单自身长度（自指，与枚举零联动）——审阅埋雷实证（2026-09-19）
/// `W4999` 变体三测全绿，护栏声明不成立，本修复以源文件对账替代。
pub const WARN_CODES: &[i32] = &[1018, 1019, 3050, 3051, 3052, 3053, 3054, 3055, 3056, 3064, 3067];
/// H 级码白名单（同上，与 H 变体集合双向一致）。
pub const HINT_CODES: &[i32] = &[3057];

/// P3（2026-09-18）：码值 → 静态显示前缀（E/W/H）。
/// 前缀是变体名首字母的段位语义；W/H 数值区间与 E 交织（E3060 与
/// W3064 相邻），无法按数值段判定——显式白名单。此前四处显示点
/// 硬编码 `"E{}"`，W/H 级码被伪造为 E 前缀（`[警告] … (E3053)`、
/// serve 帧 `code=E3053`+`severity=warning` 自相矛盾）。
pub fn code_prefix(code: i32) -> &'static str {
    if WARN_CODES.contains(&code) {
        "W"
    } else if HINT_CODES.contains(&code) {
        "H"
    } else {
        "E"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ErrorCode::*;

    /// P3 红→绿锚：W/H 级码的静态前缀。修复前显示点硬编码 "E{}"，
    /// `[警告] … (E3053)` 与 serve 帧 `code=E3053`+`severity=warning`
    /// 自相矛盾——本断言锁定白名单按段位输出。
    #[test]
    fn test_code_prefix_warn_and_hint() {
        assert_eq!(code_prefix(W3053_ImplicitScalarConversion as i32), "W");
        assert_eq!(code_prefix(W1018_MacroShadowing as i32), "W");
        assert_eq!(code_prefix(W3067_PointerTypeMismatch as i32), "W");
        assert_eq!(code_prefix(H3057_ImplicitConversionHint as i32), "H");
    }

    /// E 级码（含 C++ 4xxx 段）与未知码默认 E 前缀。
    #[test]
    fn test_code_prefix_error_and_unknown() {
        assert_eq!(code_prefix(E1006_UnsupportedFeature as i32), "E");
        assert_eq!(code_prefix(E3060_UseAfterFree as i32), "E");
        assert_eq!(code_prefix(E4022_TemplateRecursionDepthExceeded as i32), "E");
        assert_eq!(code_prefix(0), "E");
        assert_eq!(code_prefix(9999), "E");
    }

    /// **真护栏**（审阅埋雷后的修复，2026-09-19）：从本源文件提取全部
    /// W/H 变体码，与 `WARN_CODES` / `HINT_CODES` 双向对账——新增 W/H 码
    /// 漏登记白名单（源提取集多出成员）或白名单登记了不存在的码（集合
    /// 腐化）均红。逐行手扫变体声明（`W3050_Name = 3050,` 形态），零依赖
    /// 不用 regex；`include_str!` 编译期绑定本文件——改枚举不改白名单
    /// 必然失配。J9 埋雷已验：加 `W4999_TestProbe` 不登记 → 本测试红
    /// （"漏登记的新 W 码会静默显示 E 前缀"），恢复 → 绿。
    #[test]
    fn test_whitelist_matches_source_variants() {
        let src = include_str!("error_codes.rs");
        let mut source_w: Vec<i32> = Vec::new();
        let mut source_h: Vec<i32> = Vec::new();
        for line in src.lines() {
            let t = line.trim_start();
            for (prefix, out) in [("W", &mut source_w), ("H", &mut source_h)] {
                if let Some(rest) = t.strip_prefix(prefix) {
                    if let Some(eq) = rest.find(" = ") {
                        let name = &rest[..eq];
                        if !name.is_empty() && name.starts_with(|c: char| c.is_ascii_digit()) {
                            let num: String = rest[eq + 3..]
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            if let Ok(v) = num.parse::<i32>() {
                                out.push(v);
                            }
                        }
                    }
                }
            }
        }
        assert!(
            !source_w.is_empty() && !source_h.is_empty(),
            "源提取失败（变体声明形态变了？提取到 W={} H={})",
            source_w.len(),
            source_h.len()
        );
        let mut wl = WARN_CODES.to_vec();
        let mut hl = HINT_CODES.to_vec();
        source_w.sort_unstable();
        source_h.sort_unstable();
        wl.sort_unstable();
        hl.sort_unstable();
        assert_eq!(
            source_w, wl,
            "W 白名单与枚举源不一致——漏登记的新 W 码会静默显示 E 前缀（伪造复发）"
        );
        assert_eq!(
            source_h, hl,
            "H 白名单与枚举源不一致——漏登记的新 H 码会静默显示 E 前缀（伪造复发）"
        );
        for &c in WARN_CODES {
            assert_eq!(code_prefix(c), "W", "码 {c} 应为 W 前缀");
        }
        for &c in HINT_CODES {
            assert_eq!(code_prefix(c), "H", "码 {c} 应为 H 前缀");
        }
    }
}