//! 图算法检测

use super::features::{build_match, FuncFeatures};
use crate::session::AlgorithmMatch;

pub(crate) fn detect(name_lower: &str, features: &FuncFeatures, func_name: &str, line: i32) -> Vec<AlgorithmMatch> {
    let mut matches = Vec::new();

    // BFS 广度优先搜索
    // U1#1 P0-2（用户复审实锤）：旧结构分支 `contains("search") &&
    // has_single_loop && cfg_has_back_edge` 把 linearSearch / hashTable /
    // stringBasicOps 等一切"带回边的单循环查找"全判成 BFS——结构特征
    // 缺队列/visited 判据，误判面比命名子串更大。收紧为命名主导；
    // 漏检（未叫 bfs 名的 BFS 实现）由模板命名规范兜底。
    if name_lower.contains("bfs") || name_lower.contains("breadth") {
        matches.push(build_match("bfs", "BFS 广度优先搜索", func_name, line, &features.compare_lines));
    }

    // DFS 深度优先搜索
    // U1#1 P0-2：同 BFS——`is_recursive && contains("search")` 结构分支
    // 会把递归的 binarySearch / interpolationSearch 等误判 DFS。命名主导。
    if name_lower.contains("dfs") || name_lower.contains("depth") {
        matches.push(build_match("dfs", "DFS 深度优先搜索", func_name, line, &features.compare_lines));
    }

    // Prim 最小生成树
    if name_lower.contains("prim") {
        matches.push(build_match(
            "prim_mst",
            "Prim 最小生成树",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // Kruskal 最小生成树
    if name_lower.contains("kruskal") {
        matches.push(build_match(
            "kruskal_mst",
            "Kruskal 最小生成树",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // Dijkstra 最短路径
    if name_lower.contains("dijkstra") {
        matches.push(build_match(
            "dijkstra",
            "Dijkstra 最短路径",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // Floyd 最短路径
    if name_lower.contains("floyd") {
        matches.push(build_match("floyd", "Floyd 最短路径", func_name, line, &features.compare_lines));
    }

    // 拓扑排序
    if name_lower.contains("topolog") {
        matches.push(build_match(
            "topological_sort",
            "拓扑排序",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U1#1 P0-2（用户复审实锤）：linearSearch（单循环 + 回边）曾被
    /// BFS 结构分支误判——"linear(linearSearch) → BFS 遍历完成"。
    #[test]
    fn linear_search_not_misdetected_as_bfs() {
        let f = FuncFeatures { has_single_loop: true, cfg_has_back_edge: true, ..Default::default() };
        let m = detect("linearsearch", &f, "linearSearch", 1);
        assert!(m.is_empty(), "linearSearch 不应被判 BFS: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>());
    }

    /// U1#1 P0-2：递归 binarySearch 曾被 DFS 结构分支误判。
    #[test]
    fn recursive_binary_search_not_misdetected_as_dfs() {
        let f = FuncFeatures { is_recursive: true, ..Default::default() };
        let m = detect("binarysearch", &f, "binarySearch", 1);
        assert!(m.is_empty(), "递归 binarySearch 不应被判 DFS");
    }

    /// 反向锚：bfs/dfs 命名仍识别。
    #[test]
    fn bfs_dfs_names_still_detected() {
        let f = FuncFeatures::default();
        assert!(detect("bfs", &f, "bfs", 1).iter().any(|x| x.name == "bfs"));
        assert!(detect("dfs_graph", &f, "dfs", 1).iter().any(|x| x.name == "dfs"));
    }
}
