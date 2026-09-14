use crate::*;

// ============================================================================
// BST 插入
// ============================================================================

pub(crate) fn infer_bst_insert(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
    func_name: &str,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if source_line.contains(&format!("{}(", func_name)) {
        return Some(build_step(algorithm, "recursive", "递归查找插入位置"));
    }

    if line_lower.contains("null") && line_lower.contains("return") {
        return Some(build_step(algorithm, "create", "找到空位，创建新节点"));
    }

    if line_lower.contains("val") && line_lower.contains("root->val") && line_lower.contains('<') {
        return Some(build_step(algorithm, "compare", "比较插入值与当前节点值，决定向左或向右"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "BST 插入完成"));
    }

    None
}
// ============================================================================
// BST 合法性校验（二审 P0-B 方案②，2026-09-13）
// 挂载点锚定 binarySearchTreeValidation 模板 isValidBST 的三行：
//   L20 `if (root == NULL) return 1;`                       → empty_valid
//   L21 `if (root->val <= min || root->val >= max) return 0;` → range_check
//   L22 `return isValidBST(root->left, min, root->val) &&`  → recursive
// ============================================================================

pub(crate) fn infer_bst_validate(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
    func_name: &str,
    env: &crate::InferEnv<'_>,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    // 递归校验行（判据置顶：该行同时含 min/max，会被区间判据误吃）。
    if source_line.contains(&format!("{}(", func_name)) {
        // v5 ✗2：顶层调用帧（main 调用行，at_callee_entry + caller_is_main——
        // 与 quick/merge 同款区分）给入口语义——原"递归校验子树：…区间收窄"
        // 描述的是函数体内两分支，与所挂的 main 调用行不符（人审 ✗）。
        if env.at_callee_entry && env.caller_is_main {
            return Some(build_step(algorithm, "recursive", "启动校验：从根节点开始，检查整棵树是否满足 BST 性质"));
        }
        return Some(build_step(algorithm, "recursive", "递归校验子树：左子树区间收窄为 (min, val)，右子树为 (val, max)"));
    }

    // 空树合法判定：if (root == NULL) return 1;
    if line_lower.contains("null") && line_lower.contains("return") {
        return Some(build_step(algorithm, "empty_valid", "空子树不违反 BST 性质，判定合法"));
    }

    // 区间校验：if (root->val <= min || root->val >= max) return 0;
    let mentions_bound = line_lower.contains("min") || line_lower.contains("max");
    if mentions_bound && (line_lower.contains("<=") || line_lower.contains(">=")) {
        return Some(build_step(algorithm, "range_check", "校验当前节点值：必须严格落在开区间 (min, max) 内，越界即非法"));
    }

    None
}

// ============================================================================
// BST 删除（人审档 A：bstDelete 模板裸命名 deleteNode 漏检的补齐侧）
// ============================================================================

pub(crate) fn infer_bst_delete(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
    func_name: &str,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    // 找中序后继（判据置顶：findMin(root->right) 赋值行含 temp，会被替换判据误吃）。
    let successor_call = line_lower.contains("findmin") || line_lower.contains("find_min")
        || line_lower.contains("successor");
    if successor_call && line_lower.contains('(') {
        return Some(build_step(algorithm, "find_successor", "双孩子情况：找右子树最小节点作为中序后继"));
    }

    // 递归删除：root->left = deleteNode(root->left, key);
    if source_line.contains(&format!("{}(", func_name)) {
        return Some(build_step(algorithm, "recursive", "递归在子树中定位并删除目标节点"));
    }

    // 释放被删节点：free(root); return temp;
    if line_lower.contains("free(") {
        return Some(build_step(algorithm, "free", "摘除并释放被删节点"));
    }

    // 后继值替换：root->val = temp->val;
    if line_lower.contains("val") && line_lower.contains("temp") && line_lower.contains('=') && !line_lower.contains("==") {
        return Some(build_step(algorithm, "replace", "用中序后继的值替换被删节点的值"));
    }

    // 空/未找到：if (root == NULL) return NULL;
    if line_lower.contains("null") && line_lower.contains("return") {
        return Some(build_step(algorithm, "not_found", "搜索到空节点，目标不存在"));
    }

    // 单孩子/叶子判定：if (root->left == NULL) / else if (root->right == NULL)
    if (line_lower.contains("left") || line_lower.contains("right")) && line_lower.contains("null") {
        return Some(build_step(algorithm, "single_child", "判断孩子情况：叶子或单孩子节点可直接摘除"));
    }

    // 比较：if (key < root->val) / else if (key > root->val)
    if line_lower.contains("key") && line_lower.contains("val") && (line_lower.contains('<') || line_lower.contains('>')) {
        return Some(build_step(algorithm, "compare", "比较目标关键字与当前节点值，决定递归方向"));
    }

    None
}

// 层序遍历
// ============================================================================

pub(crate) fn infer_level_order(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("queue") && line_lower.contains("[") && line_lower.contains("root") {
        return Some(build_step(algorithm, "enqueue", "根节点入队"));
    }

    if line_lower.contains("node") && line_lower.contains("queue") && line_lower.contains("front") {
        return Some(build_step(algorithm, "dequeue", "取出队头节点访问"));
    }

    if line_lower.contains("left") && line_lower.contains("queue") && line_lower.contains("[") {
        return Some(build_step(algorithm, "enqueue_left", "左子节点入队"));
    }

    if line_lower.contains("right") && line_lower.contains("queue") && line_lower.contains("[") {
        return Some(build_step(algorithm, "enqueue_right", "右子节点入队"));
    }

    None
}
// ============================================================================
// BST 查找
// ============================================================================

pub(crate) fn infer_bst_search(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
    func_name: &str,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if source_line.contains(&format!("{}(", func_name)) {
        return Some(build_step(algorithm, "recursive", "递归进入子树查找"));
    }

    if line_lower.contains("val") && line_lower.contains("key") && line_lower.contains("==") {
        return Some(build_step(algorithm, "hit", "找到目标节点"));
    }

    if line_lower.contains("null") && line_lower.contains("return") {
        return Some(build_step(algorithm, "miss", "到达空节点，查找失败"));
    }

    if line_lower.contains("key") && line_lower.contains("val") && line_lower.contains('<') {
        return Some(build_step(algorithm, "compare", "比较关键字与当前节点值"));
    }

    None
}
// ============================================================================
// 线索二叉树
// ============================================================================

pub(crate) fn infer_threaded_binary_tree(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("ltag") && line_lower.contains("thread") {
        return Some(build_step(algorithm, "thread_left", "建立左线索指向前驱"));
    }

    if line_lower.contains("rtag") && line_lower.contains("thread") {
        return Some(build_step(algorithm, "thread_right", "建立右线索指向后继"));
    }

    if line_lower.contains("ltag") && line_lower.contains("link") {
        return Some(build_step(algorithm, "find_leftmost", "沿左孩子找到最左节点"));
    }

    if line_lower.contains("rtag") && line_lower.contains("thread") && line_lower.contains("while") {
        return Some(build_step(algorithm, "follow_thread", "沿后继线索访问节点"));
    }

    None
}
// ============================================================================
// 哈夫曼树
// ============================================================================

pub(crate) fn infer_huffman_tree(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("parent") && line_lower.contains("-1") && line_lower.contains("weight") {
        return Some(build_step(algorithm, "select", "在森林中选择两个最小权值节点"));
    }

    if line_lower.contains("parent") && line_lower.contains('=') && !line_lower.contains("-1") {
        return Some(build_step(algorithm, "merge", "合并两个节点为新树"));
    }

    None
}
// ============================================================================
// AVL 树
// ============================================================================

pub(crate) fn infer_avl_tree(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("r_rotate") || line_lower.contains("l_rotate") {
        return Some(build_step(algorithm, "rotate", "旋转调整平衡"));
    }

    if line_lower.contains("leftbalance") || line_lower.contains("rightbalance") {
        return Some(build_step(algorithm, "balance", "平衡因子失衡，进行平衡调整"));
    }

    if line_lower.contains("bf") && line_lower.contains('=') && !line_lower.contains("null") {
        return Some(build_step(algorithm, "update_bf", "更新平衡因子"));
    }

    None
}

#[cfg(test)]
mod tests {
    // 测试断言失败即 panic 是合理语义（同 crash_regression_tests 先例）
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn validate_match() -> AlgorithmMatch {
        AlgorithmMatch {
            name: "bst_validate".to_string(),
            display_name: "BST 合法性校验".to_string(),
            func_name: "isValidBST".to_string(),
            confidence: 85,
            suggestion: String::new(),
            line: 19,
        }
    }

    fn delete_match() -> AlgorithmMatch {
        AlgorithmMatch {
            name: "bst_delete".to_string(),
            display_name: "BST 删除".to_string(),
            func_name: "deleteNode".to_string(),
            confidence: 85,
            suggestion: String::new(),
            line: 32,
        }
    }

    fn no_vars() -> VarMap<'static> {
        VarMap::new(&[])
    }

    /// 二审 P0-B 方案②回归锚：bst_validate 三 phase 必须落在
    /// binarySearchTreeValidation 模板 isValidBST 的真实源码行上
    /// （二审表格的 L20/L21/L22——旧判据在这三行上教"插入"）。
    #[test]
    fn bst_validate_phases_match_template_lines() {
        let a = validate_match();
        let env = crate::InferEnv { prev_vars: &[], at_callee_entry: false, caller_is_main: false, lookahead: "" };
        let empty = infer_bst_validate(
            "if (root == NULL) return 1;",
            &no_vars(),
            &a,
            "isValidBST",
            &env,
        )
        .expect("空树判定行应标注");
        assert_eq!(empty.phase, "empty_valid");

        let range = infer_bst_validate(
            "if (root->val <= min || root->val >= max) return 0;",
            &no_vars(),
            &a,
            "isValidBST",
            &env,
        )
        .expect("区间校验行应标注");
        assert_eq!(range.phase, "range_check");

        let rec = infer_bst_validate(
            "return isValidBST(root->left, min, root->val) &&",
            &no_vars(),
            &a,
            "isValidBST",
            &env,
        )
        .expect("递归校验行应标注");
        assert_eq!(rec.phase, "recursive");
    }

    /// bst_validate 反向锚：校验判据不得吃掉普通比较行
    /// （旧 bst_insert 的 compare 文案"决定向左或向右"曾挂在区间校验上）。
    #[test]
    fn bst_validate_rejects_plain_compare() {
        let a = validate_match();
        assert!(
            infer_bst_validate("if (val < root->val)", &no_vars(), &a, "isValidBST", &crate::InferEnv { prev_vars: &[], at_callee_entry: false, caller_is_main: false, lookahead: "" }).is_none(),
            "无 min/max 界的普通比较不应判区间校验"
        );
    }

    /// bstDelete 模板 deleteNode 的关键行 → 各 phase（人审档 A 补齐侧）。
    #[test]
    fn bst_delete_phases_match_template_lines() {
        let a = delete_match();
        let lines: &[(&str, &str)] = &[
            ("if (root == NULL) return NULL;", "not_found"),
            ("if (key < root->val)", "compare"),
            ("root->left = deleteNode(root->left, key);", "recursive"),
            ("if (root->left == NULL) {", "single_child"),
            ("free(root); return temp;", "free"),
            ("struct TreeNode* temp = findMin(root->right);", "find_successor"),
            ("root->val = temp->val;", "replace"),
            ("root->right = deleteNode(root->right, temp->val);", "recursive"),
        ];
        for (src, want_phase) in lines {
            let step = infer_bst_delete(src, &no_vars(), &a, "deleteNode")
                .unwrap_or_else(|| panic!("该行应标注: {src}"));
            assert_eq!(&step.phase, want_phase, "行 `{src}` 应为 {want_phase}");
        }
    }

    /// bst_delete 反向锚：与删除无关的行不标注。
    #[test]
    fn bst_delete_rejects_unrelated_lines() {
        let a = delete_match();
        for src in ["struct TreeNode* temp = root->right;", "return root;", "printf(\"%d \", root->val);"] {
            assert!(
                infer_bst_delete(src, &no_vars(), &a, "deleteNode").is_none(),
                "无关行不应标注: {src}"
            );
        }
    }
}
