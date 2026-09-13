//! 排序算法检测

use super::features::{build_match, has_word, FuncFeatures};
use crate::session::AlgorithmMatch;

pub(crate) fn detect(name_lower: &str, features: &FuncFeatures, func_name: &str, line: i32) -> Vec<AlgorithmMatch> {
    let mut matches = Vec::new();

    // 冒泡排序
    if name_lower.contains("bubble")
        || (features.has_nested_loops
            && features.has_array_compare
            && features.has_swap
            && features.loop_depth >= 2
            && features.has_adjacent_index_compare)
    {
        matches.push(build_match("bubble_sort", "冒泡排序", func_name, line, &features.compare_lines));
    }

    // 选择排序
    // U1#1 P0-2（用户审阅实锤四处误判）：旧条件 `contains("select")` 把
    // huffman 的 Select / activitySelection 的 selectActivities /
    // externalSort 的 replacementSelection 全判成选择排序（套用其步骤
    // 模板产出"arr[?] 与当前最小值比较"等错误教学内容）。收紧：
    // 命名需 select 整词 + sort 语境；结构分支排除已知贪心/置换语境。
    let greedy_ctx = name_lower.contains("huffman")
        || name_lower.contains("activity")
        || name_lower.contains("replacement");
    let has_sort_word = has_word(func_name, "sort");
    if ((has_word(func_name, "select") || has_word(func_name, "selection")) && has_sort_word)
        || (!greedy_ctx
            && features.has_nested_loops
            && features.has_array_compare
            && features.has_min_max_track
            && features.loop_depth >= 2
            && !features.has_swap_in_inner_loop)
    {
        matches.push(build_match(
            "selection_sort",
            "选择排序",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // 插入排序
    // U1#1 修复（审查实锤：子串误判）：旧条件 `contains("insert")` 会把
    // `insert_node`（链表插入）等含 insert 子串的函数误判为插入排序。
    // 收紧为 insertion / insert_sort / insertsort（驼峰折叠）命名形态；
    // insert_node 等不含这些形态；结构特征分支不变。
    if name_lower.contains("insertion")
        || name_lower.contains("insert_sort")
        || name_lower.contains("insertsort")
        || (features.has_nested_loops && features.has_shift_pattern && features.loop_depth >= 2 && !features.has_swap)
    {
        matches.push(build_match(
            "insertion_sort",
            "插入排序",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // 快速排序
    if name_lower.contains("quick")
        || (features.is_recursive && features.has_partition_pattern && features.has_nested_loops)
    {
        matches.push(build_match("quick_sort", "快速排序", func_name, line, &features.compare_lines));
    }

    // 归并排序
    // U1#1 P0-2：旧条件 `contains("merge")` 把 mergeSortedLists 的链表
    // 归并函数（就叫 merge）误判归并排序（"将数组区间 [-1,-1] 递归分成
    // 两半"挂在 main 调用行）。收紧：命名需 merge 整词 + sort 语境；
    // 结构分支不变（迭代链表归并非递归，不命中）。
    if (has_word(func_name, "merge") && has_word(func_name, "sort"))
        || (features.is_recursive && features.has_merge_pattern && !features.has_partition_pattern)
    {
        matches.push(build_match("merge_sort", "归并排序", func_name, line, &features.compare_lines));
    }

    // 堆排序
    if name_lower.contains("heap")
        || (features.is_recursive && features.has_array_compare && features.has_swap && name_lower.contains("sort"))
    {
        matches.push(build_match("heap_sort", "堆排序", func_name, line, &features.compare_lines));
    }

    // 希尔排序
    if name_lower.contains("shell") {
        matches.push(build_match("shell_sort", "希尔排序", func_name, line, &features.compare_lines));
    }

    // 计数排序
    if name_lower.contains("counting") {
        matches.push(build_match(
            "counting_sort",
            "计数排序",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // 基数排序
    if name_lower.contains("radix") {
        matches.push(build_match("radix_sort", "基数排序", func_name, line, &features.compare_lines));
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U1#1 红锚③：insert 子串误判（审查实锤）——`insert_node`（链表插入）
    /// 含 "insert" 子串曾被判插入排序。收紧为 insertion / insert_sort 形态。
    #[test]
    fn insert_node_not_misdetected_as_insertion_sort() {
        let f = FuncFeatures::default();
        let m = detect("insert_node", &f, "insert_node", 1);
        assert!(
            m.is_empty(),
            "insert_node 不应被判插入排序，实际: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );
    }

    /// 反向锚：标准命名仍识别。
    #[test]
    fn insertion_sort_names_still_detected() {
        let f = FuncFeatures::default();
        for name in ["insertion_sort", "insertSort", "insert_sort"] {
            let lower = name.to_lowercase();
            let m = detect(&lower, &f, name, 1);
            assert!(
                m.iter().any(|x| x.name == "insertion_sort"),
                "{name} 应识别为插入排序"
            );
        }
    }
}
