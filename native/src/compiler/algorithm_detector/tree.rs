//! 树算法检测

use super::features::{build_match, has_word, FuncFeatures};
use crate::session::AlgorithmMatch;

pub(crate) fn detect(name_lower: &str, features: &FuncFeatures, func_name: &str, line: i32) -> Vec<AlgorithmMatch> {
    let mut matches = Vec::new();

    // —— bst 家族判据（二审 P0-B 方案②，2026-09-13）——
    // 名字 + 语义双条件：bst 整词命中后还须带对应语义词；裸命名
    // （bstInsert/bstSearch/bstDelete 模板的 insert/search/deleteNode，
    // 人审清单档 A 漏检）由"语义词 + TreeNode 参数语境"补齐。
    //
    // 旧判据 `has_word(func_name, "bst")` 单条件（U1#1 P0-2 收紧后的形态）
    // 把 isValidBST 判成"BST 插入"——校验函数被教成建节点/插入决策/插入
    // 位置，三条文案全部与所挂语句语义不符（二审实锤）；bstHeight/
    // bstTraverse 等无语义词名字同被讲成插入。
    //
    // 语境特征 `is_treenode_ctx` 排除同形裸命名：hashTable 的
    // insert/search(HashEntry[])、redBlackTree 的 insert(RBNode*)、
    // bTree 的 search(BTreeNode*)（整名匹配，BTreeNode 不算 TreeNode）。
    let has_bst = has_word(func_name, "bst");
    let in_tree_ctx = has_bst || features.is_treenode_ctx;
    let valid_word = has_word(func_name, "valid") || has_word(func_name, "validate");
    let insert_word = has_word(func_name, "insert") || has_word(func_name, "add");
    let search_word = has_word(func_name, "search") || has_word(func_name, "find");
    let delete_word = has_word(func_name, "delete") || has_word(func_name, "remove");

    // 校验优先于插入：isValidBST 含 valid 词，不得落入插入分支。
    if valid_word && in_tree_ctx {
        matches.push(build_match("bst_validate", "BST 合法性校验", func_name, line, &features.compare_lines));
    } else if insert_word && in_tree_ctx {
        matches.push(build_match("bst_insert", "BST 插入", func_name, line, &features.compare_lines));
    } else if search_word && in_tree_ctx && features.is_recursive {
        // 查找要求递归：bstDelete 的迭代 findMin(TreeNode*) 不算 BST 查找。
        matches.push(build_match("bst_search", "BST 查找", func_name, line, &features.compare_lines));
    } else if delete_word && in_tree_ctx {
        matches.push(build_match("bst_delete", "BST 删除", func_name, line, &features.compare_lines));
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

    /// 二审 P0-B 红锚（2026-09-13）：`has_word(func_name, "bst")` 单条件把
    /// isValidBST 判成 bst_insert——校验函数被按"BST 插入"教学，三条文案
    /// 全部教错（"找到空位，创建新节点"挂在 `return 1;` 上等）。
    /// 修复方向（方案②）：valid 语义词优先，判 bst_validate。
    #[test]
    fn isvalidbst_detected_as_bst_validate_not_insert() {
        let f = FuncFeatures { is_recursive: true, ..Default::default() };
        let m = detect("isvalidbst", &f, "isValidBST", 1);
        let names = || m.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
        assert!(
            m.iter().any(|x| x.name == "bst_validate"),
            "isValidBST 应判 bst_validate（方案②新增算法）: {:?}",
            names()
        );
        assert!(
            m.iter().all(|x| x.name != "bst_insert" && x.name != "bst_search"),
            "isValidBST 不得被判插入/查找: {:?}",
            names()
        );
    }

    /// 二审 P0-B 中段反锚：含 bst 但无语义词的名字（bstHeight/bstTraverse）
    /// 曾一律被讲成"插入"——双条件后应零 bst 家族匹配。
    #[test]
    fn bst_names_without_semantic_word_not_insert() {
        let f = FuncFeatures { is_recursive: true, ..Default::default() };
        for name in ["bstHeight", "bstTraverse"] {
            let m = detect(&name.to_lowercase(), &f, name, 1);
            assert!(
                m.is_empty(),
                "{name} 无插入/查找/校验语义词，不应命中 bst 家族: {:?}",
                m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
            );
        }
    }

    /// 人审清单档 A 漏检补齐：bstInsert/bstSearch/bstDelete 模板的裸命名
    /// `insert`/`search`/`deleteNode`（函数名不含 bst）靠 TreeNode 参数
    /// 语境各自命中。findMin(TreeNode*) 迭代求最小不判 BST 查找。
    #[test]
    fn bare_names_with_treenode_ctx_detected() {
        let insert_f = FuncFeatures { is_treenode_ctx: true, ..Default::default() };
        let m = detect("insert", &insert_f, "insert", 1);
        assert!(
            m.iter().any(|x| x.name == "bst_insert"),
            "裸 insert(TreeNode*) 应判 bst_insert: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );

        let search_f = FuncFeatures { is_recursive: true, is_treenode_ctx: true, ..Default::default() };
        let m = detect("search", &search_f, "search", 1);
        assert!(
            m.iter().any(|x| x.name == "bst_search"),
            "裸 search(TreeNode*) 递归应判 bst_search: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );

        let find_min_f = FuncFeatures { is_treenode_ctx: true, ..Default::default() };
        let m = detect("findmin", &find_min_f, "findMin", 1);
        assert!(
            m.is_empty(),
            "迭代 findMin 不应判 bst_search（递归条件）: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );

        let delete_f = FuncFeatures { is_treenode_ctx: true, ..Default::default() };
        let m = detect("deletenode", &delete_f, "deleteNode", 1);
        assert!(
            m.iter().any(|x| x.name == "bst_delete"),
            "裸 deleteNode(TreeNode*) 应判 bst_delete: {:?}",
            m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        );
    }

    /// 语境反锚：同形裸命名但非 TreeNode 语境——hashTable 的
    /// insert/search(HashEntry[])、redBlackTree 的 insert(RBNode*)、
    /// bTree 的 search(BTreeNode*)、linkedDelete 的 deleteNode(Node*)。
    #[test]
    fn bare_names_without_treenode_ctx_rejected() {
        for name in ["insert", "search", "deleteNode"] {
            let f = FuncFeatures { is_recursive: true, ..Default::default() };
            let m = detect(&name.to_lowercase(), &f, name, 1);
            assert!(
                m.is_empty(),
                "{name} 无 TreeNode 语境不应命中 bst 家族: {:?}",
                m.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
            );
        }
    }
}
