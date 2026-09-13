//! 算法步骤语义标注
//!
//! 为已检测到的算法生成教学友好的步骤描述，结合当前源码行特征和运行时变量值。
//! 本 crate 不依赖 FRB，通过 `AlgorithmContext` trait 与调用方解耦。

pub mod dp;
pub mod graph;
pub mod math;
pub mod search;
pub mod sorting;
pub mod structures;
pub mod tree;

/// 算法匹配信息（非 FRB 基础版本）。
#[derive(Debug, Clone)]
pub struct AlgorithmMatch {
    pub name: String,
    pub display_name: String,
    pub func_name: String,
    pub confidence: i32,
    pub suggestion: String,
    pub line: i32,
}

/// 变量快照（非 FRB 基础版本，value 已格式化为字符串）。
#[derive(Debug, Clone)]
pub struct VariableSnapshot {
    pub name: String,
    pub value: String,
}

/// 算法步骤语义快照（用于前端步骤标注）。
#[derive(Debug, Clone)]
pub struct AlgorithmStepSnapshot {
    pub algorithm_name: String,
    pub display_name: String,
    pub phase: String,
    pub description: String,
}

/// 调用方上下文：提供源码行查询与算法匹配查询能力。
pub trait AlgorithmContext {
    fn source_line(&self, line: i32) -> Option<String>;
    fn find_algorithm(&self, func_name: &str) -> Option<AlgorithmMatch>;
}

/// 推断当前执行步骤对应的算法语义描述。
pub fn infer_algorithm_step(
    code_line: i32,
    local_vars: &[VariableSnapshot],
    func_name: &str,
    ctx: &dyn AlgorithmContext,
) -> Option<AlgorithmStepSnapshot> {
    if code_line <= 0 || func_name.is_empty() {
        return None;
    }

    let source_line = get_source_line(ctx, code_line).unwrap_or_default();
    let algorithm = find_algorithm_for_func(ctx, func_name)?;
    let vars = VarMap::new(local_vars);

    match algorithm.name.as_str() {
        "bubble_sort" => sorting::infer_bubble_sort(&source_line, &vars, &algorithm),
        "selection_sort" => sorting::infer_selection_sort(&source_line, &vars, &algorithm),
        "insertion_sort" => sorting::infer_insertion_sort(&source_line, &vars, &algorithm),
        "quick_sort" => sorting::infer_quick_sort(&source_line, &vars, &algorithm, func_name),
        "merge_sort" => sorting::infer_merge_sort(&source_line, &vars, &algorithm, func_name),
        "binary_search" => search::infer_binary_search(&source_line, &vars, &algorithm),
        "heap_sort" => sorting::infer_heap_sort(&source_line, &vars, &algorithm, func_name),
        "bfs" => graph::infer_bfs(&source_line, &vars, &algorithm),
        "dfs" => graph::infer_dfs(&source_line, &vars, &algorithm, func_name),
        "dp" => dp::infer_dp(&source_line, &vars, &algorithm),
        "shell_sort" => sorting::infer_shell_sort(&source_line, &vars, &algorithm),
        "counting_sort" => sorting::infer_counting_sort(&source_line, &vars, &algorithm),
        "linked_list_delete" => structures::infer_linked_list_delete(&source_line, &vars, &algorithm),
        "bst_insert" => tree::infer_bst_insert(&source_line, &vars, &algorithm, func_name),
        "bst_search" => tree::infer_bst_search(&source_line, &vars, &algorithm, func_name),
        "bst_delete" => tree::infer_bst_delete(&source_line, &vars, &algorithm, func_name),
        "bst_validate" => tree::infer_bst_validate(&source_line, &vars, &algorithm, func_name),
        "string_reverse" => search::infer_string_reverse(&source_line, &vars, &algorithm),
        "gcd" => math::infer_gcd(&source_line, &vars, &algorithm),
        "is_prime" => math::infer_is_prime(&source_line, &vars, &algorithm),
        "hanoi" => math::infer_hanoi(&source_line, &vars, &algorithm, func_name),
        "seq_list" => structures::infer_seq_list(&source_line, &vars, &algorithm),
        "linked_list_append" => structures::infer_linked_list_append(&source_line, &vars, &algorithm),
        "circular_queue" => structures::infer_circular_queue(&source_line, &vars, &algorithm),
        "linked_stack" => structures::infer_linked_stack(&source_line, &vars, &algorithm),
        "linked_queue" => structures::infer_linked_queue(&source_line, &vars, &algorithm),
        "level_order" => tree::infer_level_order(&source_line, &vars, &algorithm),
        "hash_table" => structures::infer_hash_table(&source_line, &vars, &algorithm),
        "josephus" => structures::infer_josephus(&source_line, &vars, &algorithm),
        "circular_linked_list" => structures::infer_circular_linked_list(&source_line, &vars, &algorithm),
        "static_linked_list" => structures::infer_static_linked_list(&source_line, &vars, &algorithm),
        "string_match_bf" => search::infer_string_match_bf(&source_line, &vars, &algorithm),
        "string_match_kmp" => search::infer_string_match_kmp(&source_line, &vars, &algorithm),
        "threaded_binary_tree" => tree::infer_threaded_binary_tree(&source_line, &vars, &algorithm),
        "huffman_tree" => tree::infer_huffman_tree(&source_line, &vars, &algorithm),
        "union_find" => structures::infer_union_find(&source_line, &vars, &algorithm),
        "avl_tree" => tree::infer_avl_tree(&source_line, &vars, &algorithm),
        "prim_mst" => graph::infer_prim_mst(&source_line, &vars, &algorithm),
        "kruskal_mst" => graph::infer_kruskal_mst(&source_line, &vars, &algorithm),
        "dijkstra" => graph::infer_dijkstra(&source_line, &vars, &algorithm),
        "floyd" => graph::infer_floyd(&source_line, &vars, &algorithm),
        "topological_sort" => graph::infer_topological_sort(&source_line, &vars, &algorithm),
        "radix_sort" => sorting::infer_radix_sort(&source_line, &vars, &algorithm),
        _ => None,
    }
}

// ============================================================================
// 工具函数
// ============================================================================

fn get_source_line(ctx: &dyn AlgorithmContext, code_line: i32) -> Option<String> {
    ctx.source_line(code_line)
}

fn find_algorithm_for_func(ctx: &dyn AlgorithmContext, func_name: &str) -> Option<AlgorithmMatch> {
    ctx.find_algorithm(func_name)
}

pub(crate) struct VarMap<'a> {
    vars: &'a [VariableSnapshot],
}

impl<'a> VarMap<'a> {
    pub(crate) fn new(vars: &'a [VariableSnapshot]) -> Self {
        Self { vars }
    }

    pub(crate) fn get_int(&self, name: &str) -> Option<i32> {
        self.vars
            .iter()
            .find(|v| v.name == name)
            .and_then(|v| v.value.parse::<i32>().ok())
    }

    pub(crate) fn get_int_any(&self, names: &[&str]) -> Option<i32> {
        for name in names {
            if let Some(v) = self.get_int(name) {
                return Some(v);
            }
        }
        None
    }

    pub(crate) fn get_str(&self, name: &str) -> Option<String> {
        self.vars.iter().find(|v| v.name == name).map(|v| v.value.clone())
    }
}

pub(crate) fn is_comparison_line(line: &str) -> bool {
    line.contains('>') || line.contains('<') || line.contains("==") || line.contains("!=")
}

pub(crate) fn build_step(algorithm: &AlgorithmMatch, phase: &str, description: &str) -> AlgorithmStepSnapshot {
    AlgorithmStepSnapshot {
        algorithm_name: algorithm.name.clone(),
        display_name: algorithm.display_name.clone(),
        phase: phase.to_string(),
        description: description.to_string(),
    }
}
