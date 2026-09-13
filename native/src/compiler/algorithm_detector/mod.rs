//! 算法模式检测器
//!
//! 基于 AST 进行启发式算法识别，支持排序、图、树、搜索、字符串、数学及常见数据结构。

use crate::compiler::ast::ProgramNode;
use crate::session::AlgorithmMatch;

pub(crate) mod features;
pub(crate) mod graph;
pub(crate) mod math;
pub(crate) mod search;
pub(crate) mod sorting;
pub(crate) mod string;
pub(crate) mod structures;
pub(crate) mod tree;

/// 检测程序中的所有算法模式
pub fn detect_algorithms(program: &ProgramNode) -> Vec<AlgorithmMatch> {
    let mut matches = Vec::new();
    for func in &program.funcs {
        matches.extend(detect_in_func(func));
    }
    matches
}

fn detect_in_func(func: &crate::compiler::ast::FuncDecl) -> Vec<AlgorithmMatch> {
    let body = match func.body.as_ref() {
        Some(b) => b,
        None => return Vec::new(),
    };
    let features = features::extract_features(func, body);

    let name_lower = func.name.to_lowercase();
    let mut matches = Vec::new();
    matches.extend(sorting::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(search::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(graph::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(tree::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(structures::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(string::detect(&name_lower, &features, &func.name, func.loc.line));
    matches.extend(math::detect(&name_lower, &features, &func.name, func.loc.line));

    // U1#1 P1-a：dp 检测接线——`vitro_algorithm_steps::dp::infer_dp` 早已实现
    // 但检测器无 dp 分支（孤儿，41 个 build_match 无一产出 dp）。dpFib/
    // dpKnapsack/dpLCS/dpLIS 的状态机都在 main 或独立函数里，以 `dp[`
    // 状态表访问 + 循环为结构特征；命名含 dp/lcs/lis/knapsack 亦命中。
    if features.has_dp_array
        && (features.has_single_loop || features.has_nested_loops)
    {
        matches.push(crate::session::AlgorithmMatch {
            name: "dp".to_string(),
            display_name: "动态规划".to_string(),
            func_name: func.name.clone(),
            confidence: 85,
            suggestion: String::new(),
            line: func.loc.line,
            vis_events: Vec::new(),
        });
    }

    matches
}
