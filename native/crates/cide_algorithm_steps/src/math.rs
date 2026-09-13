use crate::*;

// ============================================================================
// 最大公约数
// ============================================================================

pub(crate) fn infer_gcd(source_line: &str, vars: &VarMap, algorithm: &AlgorithmMatch) -> Option<AlgorithmStepSnapshot> {
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
    if line_lower.contains('%') {
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
            return Some(build_step(algorithm, "recursive", &format!("递归移动 {} 个盘子", n - 1)));
        }
        return Some(build_step(algorithm, "recursive", &format!("移动 {} 个盘子的汉诺塔问题", n)));
    }

    if line_lower.contains("move") && line_lower.contains("disk") {
        return Some(build_step(algorithm, "move", &format!("移动第 {} 个盘子", n)));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "汉诺塔移动完成"));
    }

    None
}
