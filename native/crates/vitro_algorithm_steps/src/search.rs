use crate::*;

// ============================================================================
// 二分查找
// ============================================================================

pub(crate) fn infer_binary_search(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let left = vars.get_int_any(&["left", "low", "l"]).unwrap_or(-1);
    let right = vars.get_int_any(&["right", "high", "r"]).unwrap_or(-1);
    let mid = vars.get_int_any(&["mid", "m"]).unwrap_or(-1);
    let target = vars
        .get_str("target")
        .or_else(|| vars.get_str("key"))
        .unwrap_or_else(|| "?".to_string());

    if line_lower.starts_with("while ") {
        let l = if left >= 0 { left.to_string() } else { "?".to_string() };
        let r = if right >= 0 { right.to_string() } else { "?".to_string() };
        return Some(build_step(algorithm, "loop", &format!("搜索范围 [{}, {}]", l, r)));
    }

    // U1#1 修复（审查实锤：三分支不可达）：mid_calc 的旧条件
    // `contains("mid") && (contains('=') || contains('/'))` 过宽——
    // `if (a[mid] == target)` 含 ==、`left = mid + 1` 含 =，compare /
    // narrow_left / narrow_right 三个分支全部被短路，循环体里学生只看得到
    // "计算中点"（机器取证：`if (a[mid] == target)` 被标"计算中点 mid=2"）。
    // 收紧为声明/赋值形态（行首 int mid / mid = 且含除法特征）。
    if (line_lower.starts_with("int mid") || line_lower.starts_with("mid ="))
        && line_lower.contains('/')
    {
        let m = if mid >= 0 { mid.to_string() } else { "?".to_string() };
        return Some(build_step(algorithm, "mid_calc", &format!("计算中点 mid={}", m)));
    }

    if line_lower.starts_with("if ")
        && is_comparison_line(&line_lower)
        && (line_lower.contains("arr[") || line_lower.contains("a["))
    {
        let m = if mid >= 0 { mid.to_string() } else { "?".to_string() };
        return Some(build_step(
            algorithm,
            "compare",
            &format!("arr[{}] 与目标值 {} 比较", m, target),
        ));
    }

    if line_lower.contains("right") && line_lower.contains('=') && line_lower.contains("mid") {
        // U1#1 修复（审查实锤：硬编码差一）：旧文案打印 `right={mid}`，但
        // 标准二分是 `right = mid - 1`（对照 narrow_right 的 left=mid+1 打印
        // 恰好正确）。改打印 right 变量的**实际值**——对 `right = mid - 1`
        // 与 `right = mid`（左闭右开约定）两种实现都正确。
        let r_now = if right >= 0 { right.to_string() } else { "?".to_string() };
        return Some(build_step(
            algorithm,
            "narrow_left",
            &format!("目标值在左半区，调整右边界 right={}", r_now),
        ));
    }

    if line_lower.contains("left") && line_lower.contains('=') && line_lower.contains("mid") {
        // 同上：打印 left 变量实际值，而非硬编码 mid+1。
        let l_now = if left >= 0 { left.to_string() } else { "?".to_string() };
        return Some(build_step(
            algorithm,
            "narrow_right",
            &format!("目标值在右半区，调整左边界 left={}", l_now),
        ));
    }

    if line_lower.starts_with("return ") && line_lower.contains("mid") {
        let m = if mid >= 0 { mid.to_string() } else { "?".to_string() };
        return Some(build_step(algorithm, "found", &format!("找到目标值，返回索引 {}", m)));
    }

    if line_lower.starts_with("return ") && (line_lower.contains("-1") || line_lower.contains("0")) {
        return Some(build_step(algorithm, "not_found", "搜索结束，未找到目标值"));
    }

    None
}
// ============================================================================
// 字符串反转
// ============================================================================

pub(crate) fn infer_string_reverse(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let len = vars.get_int_any(&["len", "length"]).unwrap_or(-1);
    let i = vars.get_int("i").unwrap_or(-1);

    if line_lower.starts_with("while ") && line_lower.contains("\\0") {
        return Some(build_step(algorithm, "measure", "扫描字符串，计算长度"));
    }

    if line_lower.starts_with("for ") && line_lower.contains("len") {
        return Some(build_step(
            algorithm,
            "swap",
            &format!("交换位置 {} 和 {}", i, if len >= 0 { len - i - 1 } else { -1 }),
        ));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "字符串反转完成"));
    }

    None
}
// ============================================================================
// 朴素模式匹配
// ============================================================================

pub(crate) fn infer_string_match_bf(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let i = vars.get_int_any(&["i"]).unwrap_or(-1);
    let j = vars.get_int_any(&["j"]).unwrap_or(-1);

    if line_lower.contains("s[") && line_lower.contains("t[") && line_lower.contains("==") {
        return Some(build_step(algorithm, "compare", &format!("比较 S[{}] 与 T[{}]", i, j)));
    }

    if line_lower.contains("i - j + 1") {
        return Some(build_step(algorithm, "backtrack", "字符不匹配，主串回溯"));
    }

    if line_lower.starts_with("return") && line_lower.contains("-1") {
        return Some(build_step(algorithm, "not_found", "模式匹配失败"));
    }

    None
}
// ============================================================================
// KMP 模式匹配
// ============================================================================

pub(crate) fn infer_string_match_kmp(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let j = vars.get_int_any(&["j"]).unwrap_or(-1);
    let k = vars.get_int_any(&["k"]).unwrap_or(-1);

    // §6-7（v4 #35）：next 与 nextval 是两张表（nextval 是 next 的加速修正表），
    // 原判据把 nextval 构建行/调用行都标成"构建 next 数组"（computeNextVal
    // 模板 22 条挂错，人审 ✗）。词汇只增：新增 build_nextval。必须放在
    // next 分支之前（`nextval[next[j]]` 这类行同时含 "next["）。
    if line_lower.contains("nextval[") && line_lower.contains('=') {
        // v5 ✗1（复判新发现）：下标/值从**行文本**解析——此前用变量 j 填下标位，
        // 对 `nextval[0] = -1;`（下标是字面量）产出 "nextval[-1]"（j 取到的是
        // 右侧的**值**，下标位被填成值）。下标：字面量直取，标识符用变量值；
        // 值：右侧为纯整数字面量时附带（复杂右式省略），与 build_next 的
        // `next[N]=V` 格式对齐。
        let desc = match parse_array_assign(source_line, "nextval", vars) {
            Some((idx, Some(val))) => format!("构建 nextval 数组，nextval[{}]={}", idx, val),
            Some((idx, None)) => format!("构建 nextval 数组，nextval[{}]", idx),
            None => "构建 nextval 数组".to_string(),
        };
        return Some(build_step(algorithm, "build_nextval", &desc));
    }
    if line_lower.contains("getnextval") {
        return Some(build_step(algorithm, "build_nextval", "调用构建 nextval 数组"));
    }
    if line_lower.contains("getnext") || (line_lower.contains("next[") && line_lower.contains('=')) {
        // U1#1 P1-19/96（用户审阅）：main 里的 getNext(T, next) 调用行命中
        // contains("getnext")，数值取的是外层作用域 j/k（巧合非构建帧）。
        // 调用点产出无数值的启动描述；函数体内（next[..] = 赋值形态）才带数值。
        if !line_lower.contains("next[") {
            return Some(build_step(algorithm, "build_next", "调用构建 next 数组"));
        }
        return Some(build_step(
            algorithm,
            "build_next",
            &format!("构建 next 数组，next[{}]={}", j, k),
        ));
    }

    if line_lower.contains("s[") && line_lower.contains("t[") && line_lower.contains("==") {
        return Some(build_step(algorithm, "compare", "比较主串与模式串字符"));
    }

    if line_lower.contains("next[j]") {
        return Some(build_step(algorithm, "skip", &format!("j 回溯到 next[{}]={}", j, k)));
    }

    None
}

/// v5 ✗1：解析 `<name>[<idx>] = <rhs>;` 形态——返回 (下标显示, 右侧纯字面量值)。
/// 下标为字面量时直取；为标识符时取该变量的当前值；右侧仅当是纯整数字面量
///（可负）时返回值，复杂右式返回 None。
fn parse_array_assign(line: &str, name: &str, vars: &VarMap) -> Option<(String, Option<String>)> {
    let lower = line.to_lowercase();
    let open = format!("{}[", name);
    let start = lower.find(&open)? + open.len();
    let end = lower[start..].find(']')? + start;
    let idx_text = line[start..end].trim();
    let idx = if idx_text.chars().all(|c| c.is_ascii_digit()) && !idx_text.is_empty() {
        idx_text.to_string()
    } else {
        vars.get_int_any(&[idx_text]).map(|v| v.to_string()).unwrap_or_else(|| idx_text.to_string())
    };
    let rhs = line[end + 1..].split('=').next_back()?.trim().trim_end_matches(';').trim();
    let value = if !rhs.is_empty()
        && rhs.chars().enumerate().all(|(i, c)| c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')))
    {
        Some(rhs.to_string())
    } else {
        None
    };
    Some((idx, value))
}

#[cfg(test)]
mod tests {
    // 测试断言失败即 panic 是合理语义（同 crash_regression_tests 先例）
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn algo() -> AlgorithmMatch {
        AlgorithmMatch {
            name: "binary_search".to_string(),
            display_name: "二分查找".to_string(),
            func_name: "binary_search".to_string(),
            confidence: 90,
            suggestion: String::new(),
            line: 1,
        }
    }

    fn vars(pairs: &[(&str, &str)]) -> Vec<crate::VariableSnapshot> {
        pairs
            .iter()
            .map(|(n, v)| crate::VariableSnapshot { name: n.to_string(), value: v.to_string() })
            .collect()
    }

    /// U1#1 红锚①：比较行不得被 mid_calc 短路。旧条件
    /// `contains("mid") && (contains('=') || contains('/'))` 过宽——
    /// `if (a[mid] == target)` 含 ==，compare/narrow_left/narrow_right
    /// 三分支全部不可达（机器取证：比较行被标"计算中点 mid=2"，
    /// 学生在循环体里只看得到"计算中点"——概念教反级）。
    #[test]
    fn compare_line_not_swallowed_by_mid_calc() {
        let v = vars(&[("left", "0"), ("right", "4"), ("mid", "2"), ("target", "7")]);
        let step = infer_binary_search("if (a[mid] == target) return mid;", &VarMap::new(&v), &algo())
            .expect("比较行应有算法步骤标注");
        assert_eq!(step.phase, "compare", "比较行应命中 compare 分支，实际 {:?}", step);
        assert!(step.description.contains("arr[2]"), "描述应含比较下标: {}", step.description);
    }

    /// U1#1 红锚②：narrow 打印**实际边界值**而非硬编码 mid（差一修复）。
    /// `right = mid - 1` 执行后 right=1（mid=2）——旧文案打印 right=2。
    /// 用实际值对 `right = mid - 1`（闭区间）与 `right = mid`（左闭右开）
    /// 两种约定都正确。
    #[test]
    fn narrow_uses_actual_boundary_value() {
        let v = vars(&[("left", "0"), ("right", "1"), ("mid", "2"), ("target", "7")]);
        let step = infer_binary_search("right = mid - 1;", &VarMap::new(&v), &algo())
            .expect("边界更新行应有算法步骤标注");
        assert_eq!(step.phase, "narrow_left");
        assert!(
            step.description.contains("right=1"),
            "应打印 right 实际值 1（mid=2 减一），而非 mid 本身（差一）: {}",
            step.description
        );
    }

    /// 反向锚：mid 计算行（声明/赋值形态 + 除法）仍命中 mid_calc。
    #[test]
    fn mid_calc_line_still_detected() {
        let v = vars(&[("left", "0"), ("right", "4"), ("mid", "2"), ("target", "7")]);
        let step = infer_binary_search("int mid = (left + right) / 2;", &VarMap::new(&v), &algo())
            .expect("mid 计算行应有算法步骤标注");
        assert_eq!(step.phase, "mid_calc");
        assert!(step.description.contains("mid=2"));
    }
}
