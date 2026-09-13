//! 算法检测器·真实管线锚（二审 P0-B，2026-09-13）。
//!
//! 检测器单测全部手工构造 `FuncFeatures`，覆盖不到 `extract_features` 的
//! 真实提取路径——本文件用完整 Lexer→Parser→detect_algorithms 管线补锚。
//! 两处只在真实管线上暴露的既有缺陷由此锚定：
//!
//! 1. `extract_features` 曾给 `walk_stmt` 传空 func_name——自调用比较
//!    `name == func_name` 永假，`is_recursive` 恒 false；
//! 2. 本前端把函数调用统一表示为 `Expr::CallPtr { callee }`（非
//!    `Expr::Call { name }`），walk 只匹配后者——同上恒 false。
//!
//! 修复前实测（bstSearch 模板）：`match: bst_insert on insert` 一条，
//! search 零 match（47 帧全部无标注）。

use vitro_native::compiler::algorithm_detector::detect_algorithms;
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;

const BST_SEARCH_TEMPLATE: &str = r#"
#include <stdio.h>
#include <stdlib.h>

struct TreeNode {
    int val;
    struct TreeNode* left;
    struct TreeNode* right;
};

struct TreeNode* createNode(int val) {
    struct TreeNode* node = (struct TreeNode*)malloc(sizeof(struct TreeNode));
    node->val = val;
    node->left = NULL;
    node->right = NULL;
    return node;
}

struct TreeNode* insert(struct TreeNode* root, int val) {
    if (root == NULL) return createNode(val);
    if (val < root->val)
        root->left = insert(root->left, val);
    else
        root->right = insert(root->right, val);
    return root;
}

struct TreeNode* search(struct TreeNode* root, int key) {
    if (root == NULL || root->val == key) return root;
    if (key < root->val)
        return search(root->left, key);
    else
        return search(root->right, key);
}

int main() {
    struct TreeNode* root = NULL;
    root = insert(root, 5);
    struct TreeNode* res = search(root, 5);
    printf("%d\n", res ? res->val : -1);
    return 0;
}
"#;

fn detect(source: &str) -> Vec<(String, String)> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "lexer: {lex_errors:?}");
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    let program = maybe_program.unwrap_or_else(|| panic!("parser 返回 None"));
    assert!(parse_errors.is_empty(), "parser: {parse_errors:?}");
    detect_algorithms(&program)
        .into_iter()
        .map(|m| (m.name, m.func_name))
        .collect()
}

/// 真实管线：bstSearch 模板的裸命名 insert/search(TreeNode*) 各自命中
/// bst_insert / bst_search——后者依赖 is_recursive 真实提取（CallPtr 形态
/// 自调用 + 非 空 func_name），修复前 search 零 match。
#[test]
fn pipeline_detects_bst_insert_and_search() {
    let matches = detect(BST_SEARCH_TEMPLATE);
    assert!(
        matches.iter().any(|(n, f)| n == "bst_insert" && f == "insert"),
        "insert(TreeNode*) 应判 bst_insert: {matches:?}"
    );
    assert!(
        matches.iter().any(|(n, f)| n == "bst_search" && f == "search"),
        "递归 search(TreeNode*) 应判 bst_search（is_recursive 真实提取）: {matches:?}"
    );
}

/// 真实管线：isValidBST 命中 bst_validate 而非 bst_insert（二审 P0-B
/// 核心红锚的管线级复现——旧判据在模板上教错内容）。
#[test]
fn pipeline_detects_bst_validate_for_isvalidbst() {
    let source = r#"
struct TreeNode {
    int val;
    struct TreeNode* left;
    struct TreeNode* right;
};

int isValidBST(struct TreeNode* root, long long min, long long max) {
    if (root == NULL) return 1;
    if (root->val <= min || root->val >= max) return 0;
    return isValidBST(root->left, min, root->val) &&
           isValidBST(root->right, root->val, max);
}
"#;
    let matches = detect(source);
    assert!(
        matches.iter().any(|(n, _)| n == "bst_validate"),
        "isValidBST 应判 bst_validate: {matches:?}"
    );
    assert!(
        !matches.iter().any(|(n, _)| n == "bst_insert" || n == "bst_search"),
        "isValidBST 不得被判插入/查找: {matches:?}"
    );
}
