//! 树算法检测

use super::features::{build_match, has_word, FuncFeatures};
use crate::session::AlgorithmMatch;

pub(crate) fn detect(name_lower: &str, features: &FuncFeatures, func_name: &str, line: i32) -> Vec<AlgorithmMatch> {
    let mut matches = Vec::new();

    // 二叉搜索树插入
    // U1#1 P0-2（用户复审实锤）：旧结构分支 `contains("insert") &&
    // is_recursive && cfg_has_back_edge` 把 hashTable / stringBasicOps 等
    // 递归插入形态误判 BST 插入。收紧为 bst 命名语境。
    // U1#1 复审补丁：`contains("bst")` **自身仍是子串匹配**——`subString`
    // 小写为 `substring`，内含 "bst"，会把字符串基本操作整份文件误判 BST
    // 插入（实测复现）。改用词边界/驼峰整词匹配。
    if has_word(func_name, "bst") {
        matches.push(build_match("bst_insert", "BST 插入", func_name, line, &features.compare_lines));
    }

    // BST 查找
    // U1#1 P0-2：`contains("search") && is_recursive` 会把递归 binarySearch
    // 误判 BST 查找。收紧为 bst 命名语境（同上，用整词匹配避免子串陷阱）。
    if has_word(func_name, "bst") && features.is_recursive {
        matches.push(build_match("bst_search", "BST 查找", func_name, line, &features.compare_lines));
    }

    // 层序遍历
    if name_lower.contains("levelorder") || name_lower.contains("level_order") {
        matches.push(build_match("level_order", "层序遍历", func_name, line, &features.compare_lines));
    }

    // AVL 树
    if name_lower.contains("avl") {
        matches.push(build_match("avl_tree", "AVL 树", func_name, line, &features.compare_lines));
    }

    // 哈夫曼树
    if name_lower.contains("huffman") {
        matches.push(build_match(
            "huffman_tree",
            "哈夫曼树",
            func_name,
            line,
            &features.compare_lines,
        ));
    }

    // 线索二叉树
    if name_lower.contains("thread") && name_lower.contains("tree") {
        matches.push(build_match(
            "threaded_binary_tree",
            "线索二叉树",
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

    /// U1#1 P0-2（用户复审实锤）：hashTable 的递归插入形态曾被
    /// `contains("insert") && is_recursive && cfg_has_back_edge` 误判 BST 插入。
    #[test]
    fn recursive_insert_not_misdetected_as_bst() {
        let f = FuncFeatures { is_recursive: true, cfg_has_back_edge: true, ..Default::default() };
        let m = detect("hashtable_insert", &f, "hashTable", 1);
        assert!(m.is_empty(), "hashTable 插入不应被判 BST 插入: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>());
    }

    /// 反向锚：bst 命名仍识别。
    #[test]
    fn bst_names_still_detected() {
        let f = FuncFeatures { is_recursive: true, ..Default::default() };
        assert!(detect("bst_insert", &f, "bst_insert", 1).iter().any(|x| x.name == "bst_insert"));
    }

    /// U1#1 复审补丁红锚：`contains("bst")` 的子串陷阱——`subString` 小写为
    /// `substring`，内含 "bst"。收紧为"bst 命名语境"后仍曾把 stringBasicOps
    /// 整份文件误判 BST 插入（实测只输出"递归查找插入位置"）。
    #[test]
    fn substring_not_misdetected_as_bst() {
        let f = FuncFeatures::default();
        let m = detect("substring", &f, "subString", 1);
        assert!(
            m.iter().all(|x| x.name != "bst_insert" && x.name != "bst_search"),
            "subString 不应被判 BST: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );
    }

    /// 反向锚：驼峰形态仍识别——整词匹配不能因小写化而丢掉驼峰边界。
    #[test]
    fn camel_case_bst_still_detected() {
        let f = FuncFeatures { is_recursive: true, ..Default::default() };
        let m = detect("bstinsert", &f, "bstInsert", 1);
        assert!(
            m.iter().any(|x| x.name == "bst_insert"),
            "驼峰 bstInsert 应识别: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );
    }
}
