# Golden 生成失败记录

> 原则：Golden 生成失败 ≠ 必须修复模板代码。如果失败暴露的是模板代码本身的问题（教材代码缺陷、不完整的算法移植），应记录问题而非扭曲代码来迎合测试。测试的目的是发现问题。

## KNOWN_DIVERGENCE（Golden 生成阶段发现的模板代码缺陷 / 工具链差异）

### merge_default

- **模板**: `templates/merge/source.c`
- **失败原因**: `error: conflicting types for 'merge'` — `mergeSort` 先调用 `merge()`，后定义 `merge()`。Clang（C99/C11）不允许隐式函数声明，编译器在调用点推断返回类型为 `int`，但后续定义返回 `void`，导致类型冲突。
- **是否模板代码问题**: ⚠️ 待评估。C89 允许隐式函数声明，Vitro 编译器当前行为未知。若 Vitro 支持隐式声明且运行正常，则此为 Clang 与 Vitro 的语义差异，不应修改模板。
- **是否 Vitro 限制**: 待验证。需确认 Vitro 编译器对前向声明/隐式声明的处理策略。
- **建议**: 暂不修改 `source.c`。在 Vitro e2e 测试中单独验证该用例；若 Vitro 本身编译通过，记录为"Clang-Vitro 语义差异"。

### threadedBinaryTree_default

- **模板**: `templates/threadedBinaryTree/source.c`
- **失败原因**: 运行超时（10s）。`InOrderTraverse_Thr` 中 `while (p->RTag == Thread && p->rchild != T)` 配合 `main` 中没有哑头节点的树结构，导致遍历完最后一个节点后通过左指针回到根节点，陷入无限循环。
- **是否模板代码问题**: ✅ **是**。教材标准线索二叉树遍历通常依赖哑头节点（`T` 为哑头），或终止条件为 `p != NULL`。本模板移除了哑头节点但保留了 `p->rchild != T` 的终止条件，当 `p->rchild` 恰好等于根节点 `T` 时条件为假，随后 `p = p->rchild` 回到根，再通过左链再次遍历，形成死循环。
- **是否 Vitro 限制**: 否。这是模板算法本身的逻辑缺陷，任何标准 C 编译器执行都会死循环。
- **建议**: 保留原始代码以暴露问题。记录为"模板算法逻辑缺陷 — 线索二叉树遍历在无哑头节点场景下死循环"。

## 已解决的历史失败

### dpKnapsack / dpLCS / dpLIS / dpCoinChange

- **解决方式**: `scripts/sync_templates.py` 的 `run_with_clang()` 中增加 `#undef min` / `#undef max`（Windows `stdlib.h` 宏冲突）。
- **性质**: 测试基础设施适配（Windows 平台头文件污染），未修改任何模板 `source.c`。
- **状态**: ✅ Golden 已生成并锁定。

---

## 记录规范

新增失败时按以下格式追加：

```markdown
### <case_name>

- **模板**: `templates/<key>/source.c`
- **失败原因**: 
- **是否模板代码问题**: 
- **是否 Vitro 限制**: 
- **建议**: 
```
