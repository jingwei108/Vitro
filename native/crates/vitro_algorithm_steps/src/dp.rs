use crate::*;

// ============================================================================
// 动态规划
// ============================================================================

pub(crate) fn infer_dp(source_line: &str, vars: &VarMap, algorithm: &AlgorithmMatch, env: &crate::InferEnv<'_>) -> Option<AlgorithmStepSnapshot> {
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
        // 二审 P1-1：4/6 模板的 outer_loop 首现挂在初始化循环
        //（for (…) dp[i] = 1000; / dp[i] = 1; / dp[i][0] = 0; 等）——
        // 循环体为 dp[..] = 纯字面量时不算子问题遍历。
        // U1#1 管道批（P1-1 边界收口）：多行/嵌套 for 的体不在 for 行内
        //（dpLCS L10 for i / L11 for j / L12 体）——lookahead 里找首个
        // 非循环头/非花括号的行作为体。
        let body_line = if line_lower.contains("dp[") {
            line_lower.clone()
        } else {
            env.lookahead
                .lines()
                .map(|l| l.trim().to_lowercase())
                .find(|l| {
                    !l.is_empty()
                        && !l.starts_with("for ")
                        && !l.starts_with("while ")
                        && !l.starts_with('{')
                })
                .unwrap_or_default()
        };
        if i_head && !dp_loop_body_is_init(&body_line) {
            return Some(build_step(algorithm, "outer_loop", &format!("遍历{} i={}", dp_outer_subject(&line_lower, env), i)));
        }
        // §6-2（v4 清单，2026-09-14）：j 分支补同款初始化体排除——多行双层
        // 初始化循环（dpKnapsack L14 / dpLCS L11 / matrixChain L6 的内层 j，
        // 体 `dp[i][j] = 0;` 在下一行）此前漏判，inner_loop 首现挂初始化循环
        // （人审 ✗ ×3）。红锚：inner_loop_init_multiline_body_excluded。
        if (line_lower.contains("int j") || line_lower.contains(" j <") || line_lower.contains("(j <"))
            && !dp_loop_body_is_init(&body_line)
        {
            return Some(build_step(algorithm, "inner_loop", &format!("遍历{} j={}", dp_inner_subject(&line_lower, env), w)));
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

/// 二审 P1-1：循环行是否为 dp 状态表初始化（体为 `dp[..] = <纯字面量>`）。
/// 真正的子问题遍历循环体要么为空（`{`），要么引用其他变量/dp 依赖。
fn dp_loop_body_is_init(line_lower: &str) -> bool {
    if !line_lower.contains("dp[") || !line_lower.contains('=') {
        return false;
    }
    // 取最后一个 '=' 的右侧，trim 后是纯整数字面量即初始化
    if let Some(eq) = line_lower.rfind('=') {
        let rhs = line_lower[eq + 1..].trim().trim_end_matches(';').trim();
        return !rhs.is_empty() && rhs.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// §6-3（v4 清单 #43/#51）：外层循环主语——i 不总是"子问题"下标：
/// 币种循环（dpCoinChange：`for (i < coinCount)`）与物品循环（背包：行或
/// lookahead 含 `wt[`/`weight[`）。检测到特征词才具名，否则保持泛化
/// "子问题"（dpFib/dpLIS/dpLCS/matrixChain 的 i 确是子问题下标）。
fn dp_outer_subject(line_lower: &str, env: &crate::InferEnv<'_>) -> &'static str {
    let ctx = format!("{} {}", line_lower, env.lookahead.to_lowercase());
    if ctx.contains("coin") {
        "币种"
    } else if ctx.contains("wt[") || ctx.contains("weight[") {
        "物品"
    } else {
        "子问题"
    }
}

/// §6-3：内层循环主语——dpCoinChange 的 j 索引金额（`for (j = coins[i]; j <= amount)`）。
fn dp_inner_subject(line_lower: &str, env: &crate::InferEnv<'_>) -> &'static str {
    let ctx = format!("{} {}", line_lower, env.lookahead.to_lowercase());
    if ctx.contains("coin") {
        "金额"
    } else {
        "子问题维度"
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn algo() -> AlgorithmMatch {
        AlgorithmMatch {
            name: "dp".to_string(),
            display_name: "动态规划".to_string(),
            func_name: "main".to_string(),
            confidence: 90,
            suggestion: String::new(),
            line: 1,
        }
    }

    fn env(lookahead: &str) -> crate::InferEnv<'_> {
        crate::InferEnv {
            prev_vars: &[],
            at_callee_entry: false,
            caller_is_main: false,
            lookahead,
        }
    }

    /// §6-2（v4 清单，2026-09-14）：初始化双层循环的**内层 j 行**不得产出
    /// inner_loop——多行 for 体不在 for 行内（dpLCS L11 for j / L12 体
    /// `dp[i][j] = 0;`），outer_loop 已有排除而 j 分支漏掉，致
    /// dpKnapsack/dpLCS/matrixChain 三键 inner_loop 首现挂初始化循环（人审 ✗）。
    /// 修复前：返回 Some(inner_loop) → 本测试红。
    #[test]
    fn inner_loop_init_multiline_body_excluded() {
        let vars = VarMap::new(&[]);
        // dpLCS 形态：L10 for i / L11 for j / L12 体
        let r = infer_dp(
            "for (j = 0; j <= n; j++) {",
            &vars,
            &algo(),
            &env("        dp[i][j] = 0;\n    }\n"),
        );
        assert!(r.is_none(), "初始化内层 j 循环不得产出 inner_loop，实际 {:?}", r);

        // matrixChain 形态：体为 dp[j][j] = 0;
        let r = infer_dp(
            "for (j = 0; j < n; j++)",
            &vars,
            &algo(),
            &env("            dp[j][j] = 0;\n        }\n"),
        );
        assert!(r.is_none(), "dp[j][j] = 0 初始化体同上，实际 {:?}", r);
    }

    /// 反向锚：算法体的内层 j 循环（体引用 dp 依赖/变量）仍产出 inner_loop。
    #[test]
    fn inner_loop_real_body_still_annotated() {
        let vars = VarMap::new(&[]);
        // dpCoinChange 形态：j 从 coins[i] 起、体是转移语句
        let r = infer_dp(
            "for (j = coins[i]; j <= amount; j++) {",
            &vars,
            &algo(),
            &env("            dp[j] = min(dp[j], dp[j - coins[i]] + 1);\n        }\n"),
        );
        assert!(r.is_some(), "真实子问题维度循环应产出 inner_loop");
        assert_eq!(r.unwrap().phase, "inner_loop");
    }
}
