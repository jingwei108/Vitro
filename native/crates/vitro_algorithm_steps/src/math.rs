use crate::*;

// ============================================================================
// 最大公约数
// ============================================================================

pub(crate) fn infer_gcd(source_line: &str, vars: &VarMap, algorithm: &AlgorithmMatch, env: &crate::InferEnv<'_>) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let a = vars.get_int_any(&["a"]).unwrap_or(-1);
    let b = vars.get_int_any(&["b"]).unwrap_or(-1);

    if line_lower.starts_with("while ") {
        return Some(build_step(algorithm, "loop", &format!("辗转相除：a={}, b={}", a, b)));
    }

    // U1#1 P0-4（用户审阅）：mod 是"展示运算过程"的 phase——行末帧的 b 已
    // 被本语句赋值，用当前 a/b 拼算式会得到 48 % 12 = 0（真值 48 % 18 = 12）。
    // 正解需要 prev_vars 快照（语句执行前操作数），管道改动登记下一批；
    // 本批安全降级为不展示操作数的文案，避免锚定错误算式。
    // 二审 P0-C：`contains('%')` 过宽——printf 格式串的百分号命中，
    // 命中，mod 首现漂到 main 的打印行。排除 IO 行 + 要求 % 出现在表达式
    // 语境（a % b / %= / x % y 形态，而非 "..." 字符串内）。
    let is_io_line = line_lower.contains("printf") || line_lower.contains("scanf")
        || line_lower.contains("sprintf") || line_lower.contains("fprintf")
        || line_lower.contains("snprintf");
    let has_mod_expr = line_lower.contains(" % ") || line_lower.contains("%=");
    if has_mod_expr && !is_io_line {
        // U1#1 管道批（P0-4 收口）：运算过程类 phase 用**行入口操作数**拼
        // 算式——行末帧的 b 已被 `b = a % b` 赋值（48 % 12 = 0 是错的），
        // prev 是进入本行前一刻（48 % 18），结果恰是行末的 b（余数 12）。
        let pa = env.prev_vars.iter().find(|v| v.name == "a").and_then(|v| v.value.parse::<i32>().ok());
        let pb = env.prev_vars.iter().find(|v| v.name == "b").and_then(|v| v.value.parse::<i32>().ok());
        let b_now = vars.get_int("b").unwrap_or(-1);
        if let (Some(x), Some(y)) = (pa, pb) {
            if x > 0 && y > 0 && b_now >= 0 {
                return Some(build_step(
                    algorithm,
                    "mod",
                    &format!("计算 {} % {} = {}（余数作为新的 b）", x, y, b_now),
                ));
            }
        }
        return Some(build_step(algorithm, "mod", "求余并更新 b（辗转相除一步）"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", &format!("最大公约数为 {}", a)));
    }

    None
}
// ============================================================================
// 素数判断
// ============================================================================

pub(crate) fn infer_is_prime(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let n = vars.get_int_any(&["n"]).unwrap_or(-1);
    let i = vars.get_int("i").unwrap_or(-1);

    if line_lower.starts_with("if ") && line_lower.contains("<=") && line_lower.contains("1") {
        return Some(build_step(algorithm, "check_small", "排除小于等于 1 的数"));
    }

    if line_lower.starts_with("for ") {
        return Some(build_step(
            algorithm,
            "test_divisor",
            &format!("试除 i={}，检查 {} % {} == 0", i, n, i),
        ));
    }

    if line_lower.contains("n % i") && line_lower.contains("== 0") {
        return Some(build_step(
            algorithm,
            "found_factor",
            &format!("发现因子 {}，{} 不是素数", i, n),
        ));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", &format!("{} 是素数", n)));
    }

    None
}
// ============================================================================
// 汉诺塔
// ============================================================================

pub(crate) fn infer_hanoi(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
    func_name: &str,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let n = vars.get_int_any(&["n"]).unwrap_or(-1);

    if line_lower.contains("n == 1") || line_lower.contains("n==1") {
        return Some(build_step(algorithm, "base", "基准情况：直接把盘子从起始柱移到目标柱"));
    }

    if source_line.contains(&format!("{}(", func_name)) {
        // U1#1 P1-50（用户审阅）：main 调用点（n=3）曾被文案恒减一报"2 个
        // 盘子"。区分：函数体内的递归调用行（hanoi( 出现且非定义）才减一，
        // 顶层调用按 n 原值表述。
        // 递归实参含 n-1 形态才是函数体内的自递归（减一正确）；
        // main 顶层的 hanoi(3, …) 原值表述。
        if line_lower.contains("n - 1") || line_lower.contains("n-1") {
            // 二审 §7.5：n=1 时 hanoi(n-1) 是 0 盘递归（基准分支前的形态），
            // 不产出"递归移动 0 个盘子"。
            if n > 1 {
                return Some(build_step(algorithm, "recursive", &format!("递归移动 {} 个盘子", n - 1)));
            }
            return None;
        }
        // v5 ✗3：顶层调用帧的"问题陈述"（"移动 N 个盘子的汉诺塔问题"）改
        // 入口语义——从 from 柱移动到 to 柱（char 形参按 i32 存，还原为字符；
        // 取不到时降级不带柱名）。
        let from_c = vars.get_int_any(&["from"]).and_then(|v| char::from_u32(v as u32));
        let to_c = vars.get_int_any(&["to"]).and_then(|v| char::from_u32(v as u32));
        return Some(match (from_c, to_c) {
            (Some(f), Some(t)) => build_step(algorithm, "recursive", &format!("从 {} 柱移动 {} 个盘子到 {} 柱", f, n, t)),
            _ => build_step(algorithm, "recursive", &format!("启动汉诺塔：移动 {} 个盘子", n)),
        });
    }

    if line_lower.contains("move") && line_lower.contains("disk") {
        return Some(build_step(algorithm, "move", &format!("移动第 {} 个盘子", n)));
    }

    if line_lower.starts_with("return") {
        // §6-5（v4 #68）：hanoi 体内的 return（含基准分支 L6）是**该层递归
        // 结束**，不是整体完成——原文案"汉诺塔移动完成"与所挂语句不符
        //（真正完成在 main 末尾的 return）。
        if func_name == "main" {
            return Some(build_step(algorithm, "finish", "汉诺塔移动完成"));
        }
        return Some(build_step(algorithm, "finish", "该层递归结束，返回上一层"));
    }

    None
}
