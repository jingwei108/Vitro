use crate::*;

// ============================================================================
// 动态规划
// ============================================================================

pub(crate) fn infer_dp(source_line: &str, vars: &VarMap, algorithm: &AlgorithmMatch) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let i = vars.get_int_any(&["i", "n"]).unwrap_or(-1);
    let w = vars.get_int_any(&["w", "j", "capacity"]).unwrap_or(-1);

    // U1#1 P0-3（用户审阅实锤）：① 旧外循环判据 `contains("i") &&
    // contains("n")` 把 `for (int j = coins[i]; j <= amount; j++)` 的
    // **amount 含 n** 误判外循环——收紧为 i 的形态判据（int i / i < / i <=）；
    // ② 文案"遍历物品"按算法族泛化（dpFib/dpLCS/dpLIS/matrixChain 均非
    // 背包），标注展示的是子问题下标。
    if line_lower.starts_with("for ") || line_lower.starts_with("while ") {
        let i_head = line_lower.contains("int i")
            || line_lower.contains(" i <")
            || line_lower.contains(" i <=")
            || line_lower.contains("(i <")
            || line_lower.contains("(i <=");
        if i_head {
            return Some(build_step(algorithm, "outer_loop", &format!("遍历子问题 i={}", i)));
        }
        if line_lower.contains("int j") || line_lower.contains(" j <") || line_lower.contains("(j <") {
            return Some(build_step(algorithm, "inner_loop", &format!("遍历子问题维度 j={}", w)));
        }
    }

    // U1#1 P0-3：旧判据 `contains("dp[") && contains('=')` 使**全部 5 个
    // dp 模板的 transition 首现挂在初始化行**（dp[0] = 0; 等）。真正的
    // 状态转移右侧引用 dp[（dp[i] = f(dp[..])）——要求同语句两侧都有 dp[。
    let has_dp = line_lower.matches("dp[").count();
    if has_dp >= 2 && line_lower.contains('=') {
        return Some(build_step(algorithm, "transition", "状态转移：计算当前子问题的最优解"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "动态规划计算完成"));
    }

    None
}
