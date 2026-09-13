use crate::*;

// ============================================================================
// 链表删除
// ============================================================================

pub(crate) fn infer_linked_list_delete(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("head") && line_lower.contains("next") && line_lower.contains("free") {
        return Some(build_step(algorithm, "delete_head", "删除头节点并释放内存"));
    }

    if line_lower.starts_with("while ") && line_lower.contains("data") && line_lower.contains("key") {
        return Some(build_step(algorithm, "search", "遍历链表查找目标节点"));
    }

    if line_lower.contains("prev") && line_lower.contains("next") && line_lower.contains('=') {
        return Some(build_step(algorithm, "unlink", "调整前驱指针，跳过待删除节点"));
    }

    if line_lower.contains("free") && line_lower.contains("temp") {
        return Some(build_step(algorithm, "free", "释放被删除节点的堆内存"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "链表删除完成"));
    }

    None
}
// 顺序表
// ============================================================================

pub(crate) fn infer_seq_list(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("length") && line_lower.contains("maxsize") && line_lower.contains("||") {
        return Some(build_step(algorithm, "check", "检查插入/删除位置的合法性"));
    }

    if line_lower.contains("data[i]") && line_lower.contains("data[i - 1]") && line_lower.contains('=') {
        return Some(build_step(algorithm, "shift", "移动元素，腾出或填补位置"));
    }

    if line_lower.contains("data[pos]") && line_lower.contains('=') && !line_lower.contains("||") {
        return Some(build_step(algorithm, "place", "在目标位置放入元素"));
    }

    // U1#1 P1-89（用户审阅）：旧判据命中后移循环，真值在 L->length--。
    if source_line.contains("length--") || source_line.contains("length++") {
        return Some(build_step(algorithm, "update_len", "更新表长度"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "顺序表操作完成"));
    }

    None
}
// ============================================================================
// 链表尾插
// ============================================================================

pub(crate) fn infer_linked_list_append(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("next") && line_lower.contains("null") && line_lower.contains("while") {
        return Some(build_step(algorithm, "find_tail", "遍历链表寻找尾节点"));
    }

    if line_lower.contains("next") && line_lower.contains('=') && !line_lower.contains("null") {
        return Some(build_step(algorithm, "link", "将新节点链接到链表尾部"));
    }

    None
}
// ============================================================================
// 循环队列
// ============================================================================

pub(crate) fn infer_circular_queue(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("rear") && line_lower.contains("front") && line_lower.contains('%') {
        return Some(build_step(algorithm, "check", "利用取模判断队列空或满"));
    }

    if line_lower.contains("data") && line_lower.contains("rear") && line_lower.contains('=') {
        return Some(build_step(algorithm, "enqueue", "元素入队"));
    }

    if line_lower.contains("data") && line_lower.contains("front") && !line_lower.contains("==") {
        return Some(build_step(algorithm, "dequeue", "元素出队"));
    }

    if line_lower.contains("rear") && line_lower.contains('%') {
        return Some(build_step(algorithm, "wrap", "指针循环绕回数组开头"));
    }

    None
}
// 链栈
// ============================================================================

pub(crate) fn infer_linked_stack(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("next")
        && line_lower.contains("top")
        && line_lower.contains('=')
        && line_lower.contains("malloc")
    {
        return Some(build_step(algorithm, "push", "新节点入栈"));
    }

    if line_lower.contains("free") && line_lower.contains("temp") {
        return Some(build_step(algorithm, "pop", "释放栈顶节点"));
    }

    None
}
// ============================================================================
// 链队列
// ============================================================================

pub(crate) fn infer_linked_queue(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("rear") && line_lower.contains("next") && line_lower.contains('=') {
        return Some(build_step(algorithm, "enqueue", "新节点入队并更新尾指针"));
    }

    if line_lower.contains("front") && line_lower.contains("next") && !line_lower.contains("rear") {
        return Some(build_step(algorithm, "dequeue", "队头出队"));
    }

    if line_lower.contains("free") {
        return Some(build_step(algorithm, "free", "释放被删除节点"));
    }

    None
}

pub(crate) fn infer_hash_table(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("key") && line_lower.contains('%') {
        return Some(build_step(algorithm, "hash", "计算哈希值"));
    }

    if line_lower.contains("occupied") && line_lower.contains("while") {
        return Some(build_step(algorithm, "probe", "线性探测寻找空位或目标"));
    }

    if line_lower.contains("key") && line_lower.contains("==") && line_lower.contains("return") {
        return Some(build_step(algorithm, "hit", "找到目标关键字"));
    }

    None
}
// ============================================================================
// 约瑟夫环
// ============================================================================

pub(crate) fn infer_josephus(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let m = vars.get_int_any(&["m"]).unwrap_or(-1);
    let remain = vars.get_int_any(&["remain"]).unwrap_or(-1);

    if line_lower.contains("alive") && line_lower.contains("1") && line_lower.contains('=') {
        return Some(build_step(algorithm, "init", "初始化所有人为存活状态"));
    }

    if line_lower.contains("alive") && line_lower.contains("0") && line_lower.contains('=') {
        return Some(build_step(
            algorithm,
            "eliminate",
            &format!("报到 {} 的人被淘汰，剩余 {} 人", m, remain),
        ));
    }

    if line_lower.contains('%') && line_lower.contains("+ 1") {
        return Some(build_step(algorithm, "rotate", "下标循环绕回，模拟圆圈"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "约瑟夫环淘汰完成"));
    }

    None
}
// ============================================================================
// 循环链表
// ============================================================================

pub(crate) fn infer_circular_linked_list(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("next") && line_lower.contains("head") && line_lower.contains('=') {
        return Some(build_step(algorithm, "link", "尾节点回指头节点，形成循环"));
    }

    if line_lower.contains("do") {
        return Some(build_step(algorithm, "traverse", "do-while 遍历循环链表"));
    }

    if line_lower.starts_with("return") {
        return Some(build_step(algorithm, "finish", "循环链表操作完成"));
    }

    None
}
// ============================================================================
// 静态链表
// ============================================================================

pub(crate) fn infer_static_linked_list(
    source_line: &str,
    _vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();

    if line_lower.contains("cur") && line_lower.contains("i + 1") {
        return Some(build_step(algorithm, "init", "初始化备用链表"));
    }

    if line_lower.contains("space[0].cur") && line_lower.contains("space[i].cur") {
        return Some(build_step(algorithm, "malloc", "从备用链表分配节点"));
    }

    if line_lower.contains("space[k].cur") && line_lower.contains("space[0].cur") {
        return Some(build_step(algorithm, "free", "回收节点到备用链表"));
    }

    None
}
// ============================================================================
// 并查集
// ============================================================================

pub(crate) fn infer_union_find(
    source_line: &str,
    vars: &VarMap,
    algorithm: &AlgorithmMatch,
) -> Option<AlgorithmStepSnapshot> {
    let line_lower = source_line.to_lowercase();
    let x = vars.get_int_any(&["x"]).unwrap_or(-1);

    if line_lower.contains("parent[x]") && line_lower.contains("< 0") {
        return Some(build_step(algorithm, "find", &format!("查找 {} 的根节点", x)));
    }

    if line_lower.contains("parent[x]") && line_lower.contains("find") && line_lower.contains('=') {
        return Some(build_step(algorithm, "compress", "路径压缩，直接指向根"));
    }

    if line_lower.contains("union") && line_lower.contains("root") {
        return Some(build_step(algorithm, "union", "合并两个集合"));
    }

    None
}
