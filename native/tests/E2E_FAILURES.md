# E2E 端到端失败记录

> 记录原则：诚实记录，不隐藏失败。
> 覆盖范围：Template Generated、Baseline 补充失败记录。
> 分类约定：
> - `KNOWN_FAILURE`：Vitro 编译器/VM 的真实缺陷，需要修复。
> - `KNOWN_DIVERGENCE`：模板代码本身存在未定义行为（如未初始化指针、越界、死循环），Vitro 的安全检测机制使其比 Clang 更早暴露问题；属于设计决策差异，不视为 Vitro 缺陷。

---

## 统计摘要

| 类别 | 总数 | 通过 | 已知偏差 | 记录时间 |
|------|------|------|----------|----------|
| Template Generated | 82 | 79 | 3 | 2026-09-06 |

---

## KNOWN_DIVERGENCE（已知偏差 / 模板代码缺陷）

以下用例均因模板代码自身存在未定义行为或逻辑缺陷，在 Clang 下也可能崩溃或产生非预期输出；Vitro 因启用边界检查、NULL 指针检测、无限循环检测等教学安全机制而更早暴露问题。这些差异是 Vitro 教学安全特性的预期行为，不作为编译器缺陷修复。

### bTree_default

- **来源**: 算法模板批量生成（B 树）
- **现象**: Runtime error — 访问 NULL 指针区域（地址 0x0010）。Clang 运行时因未初始化子节点指针访问未定义行为，可能 segfault（exit 139）或继续执行并输出垃圾值，表现非确定性。
- **根因**: B 树插入/分裂逻辑访问了未初始化的子节点指针。模板代码未在创建节点时将 `children[]` 数组全部初始化为 NULL。
- **分类**: 模板代码缺陷（未初始化指针 / 未定义行为）
- **是否 Vitro 限制**: 否
- **涉及语法特性**: 复杂数据结构
- **学生影响评级**: P2
- **建议**: 向学生强调 `malloc` 后必须初始化指针字段；该用例保留作为安全检测教学示例

### ~~infixEvaluation_default~~（已修复 2026-09-06）

- **来源**: 算法模板批量生成（中缀表达式求值）
- **原记录**: Runtime error — 数组越界：访问了 `valStack[-1]`，曾归因为模板栈下溢缺陷。
- **实际根因**: Vitro 编译器 bug —— 自增/自减作为数组索引（`opStack[++opTop]` / `valStack[valTop--]`）的代码生成缺陷，2026-09-06 第三批 codegen soundness 修复（T-P0-3/T-P0-5 类型化自增路径）后输出与 Clang golden 一致（`11`）。
- **状态**: 已从 KNOWN_TEMPLATE_FAILURES 与 shadow KNOWN_FAILURE_CASES 同步移除。

## 历史已修复条目

以下条目曾因失败被记录，现已修复并从当前偏差列表移除：

| 用例 | 修复时间 | 修复说明 |
|---|---|---|
| `bfs_default` | 2026-06-06 | 修复全局二维数组嵌套初始化子元素大小计算错误 |
| `spfa_default` | 2026-09-13 | SPFA 模板队列溢出修复（普通队列容量上界 = 每点最多入队 n 次 → `MAXV*MAXV`；U0#1③，shadow 转 match，双向对账同步移除 KNOWN_TEMPLATE_FAILURES） |
| `dfs_default` | 2026-06-06 | 同上 |
| `binarySearchTreeValidation_default` | 2026-06-07 | 新增 `<limits.h>` 支持，`INT_MIN`/`INT_MAX` 已预定义 |
| `bellmanFord_default` | 2026-06-15 | Parser 对 `int u, v, w;` 结构体后多字段声明已支持 |
| `polynomialAdd_default` | 2026-06-15 | 指针参与逻辑运算 `&&` 已可通过其他方式绕过，当前测试通过 |
| `redBlackTree_default` | 2026-06-15 | 同上 |
| `threadedBinaryTree_default` | 2026-06-27 | 模板源码修复：线索化遍历引入标准头节点法，终止条件正确 |

---

## 记录规范

新增失败时按以下格式追加到本文件顶部（保持时间倒序）：

```markdown
### <case_name>
- **来源**: 
- **现象**: 
- **根因**: 
- **分类**: 模板代码缺陷 / Vitro 已知限制 / Vitro 语义差异 / Vitro 语法限制
- **是否 Vitro 限制**: 是 / 否 / 待确认
```

若测试已通过，应将该条目移入“历史已修复条目”表格，不得直接删除记录。
