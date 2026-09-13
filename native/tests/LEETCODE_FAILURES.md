# LeetCode 失败记录

> 记录原则：诚实记录，不隐藏失败。
> 格式参见 `docs/current/PHASE_KR_LEETCODE_TEST_PLAN.md` 附录 B。

## 当前状态

**LeetCode 防线已全面实施（阶段 4 + 阶段 5），并于 2026-06-18 启动阶段 6 中等题填充。**

- `native/tests/cases/leetcode/` 已创建
- `native/tests/cases_golden/leetcode/` 已创建
- 当前已填充 **48** 道 LeetCode 简单题 + **20** 道中等题源码
- 当前通过 **68** 道，已知失败 **0** 道
- 在填充过程中发现 1 处 Vitro 与 Clang 行为差异，已通过改写源码规避，详见下方"实施过程发现"章节

## 已覆盖用例

### 阶段 4：数组与字符串（27 道）

| 用例 | 题目 | 类别 | 状态 | 备注 |
|:-----|:-----|:-----|:-----|:-----|
| lc_1 | Two Sum | 数组 | 通过 | 返回动态分配数组，驱动中释放 |
| lc_26 | Remove Duplicates from Sorted Array | 数组 | 通过 | |
| lc_27 | Remove Element | 数组 | 通过 | |
| lc_35 | Search Insert Position | 数组/二分 | 通过 | |
| lc_53 | Maximum Subarray | 数组/DP | 通过 | |
| lc_66 | Plus One | 数组/数学 | 通过 | 返回动态分配数组 |
| lc_88 | Merge Sorted Array | 数组/双指针 | 通过 | |
| lc_121 | Best Time to Buy and Sell Stock | 数组/贪心 | 通过 | |
| lc_136 | Single Number | 数组/位运算 | 通过 | |
| lc_169 | Majority Element | 数组 | 通过 | |
| lc_189 | Rotate Array | 数组 | 通过 | |
| lc_217 | Contains Duplicate | 数组 | 通过 | 使用 `qsort` + 回调比较函数 |
| lc_238 | Product of Array Except Self | 数组 | 通过 | 返回动态分配数组 |
| lc_268 | Missing Number | 数组/数学 | 通过 | |
| lc_283 | Move Zeroes | 数组/双指针 | 通过 | |
| lc_14 | Longest Common Prefix | 字符串 | 通过 | 返回动态分配字符串 |
| lc_28 | Implement strStr() | 字符串 | 通过 | |
| lc_58 | Length of Last Word | 字符串 | 通过 | |
| lc_125 | Valid Palindrome | 字符串 | 通过 | 使用 `ctype.h` 的 `isalnum` |
| lc_344 | Reverse String | 字符串 | 通过 | |
| lc_387 | First Unique Character | 字符串 | 通过 | |
| lc_9 | Palindrome Number | 数学 | 通过 | |
| lc_13 | Roman to Integer | 数学 | 通过 | |
| lc_69 | Sqrt(x) | 数学/二分 | 通过 | |
| lc_70 | Climbing Stairs | DP | 通过 | |
| lc_118 | Pascal's Triangle | 数组 | 通过 | 返回动态分配二维数组 |
| lc_119 | Pascal's Triangle II | 数组 | 通过 | 返回动态分配数组 |

### 阶段 5：链表、树、栈与队列（21 道）

| 用例 | 题目 | 类别 | 状态 | 备注 |
|:-----|:-----|:-----|:-----|:-----|
| lc_20 | Valid Parentheses | 栈 | 通过 | 数组模拟栈 |
| lc_155 | Min Stack | 栈 | 通过 | 结构体封装 |
| lc_225 | Implement Stack using Queues | 栈 | 通过 | 数组模拟队列 |
| lc_232 | Implement Queue using Stacks | 队列 | 通过 | 原始复合副作用写法触发 Vitro 差异，已改写规避 |
| lc_206 | Reverse Linked List | 链表 | 通过 | |
| lc_21 | Merge Two Sorted Lists | 链表 | 通过 | |
| lc_141 | Linked List Cycle | 链表 | 通过 | 构造环状链表 |
| lc_160 | Intersection of Two Linked Lists | 链表 | 通过 | |
| lc_203 | Remove Linked List Elements | 链表 | 通过 | |
| lc_234 | Palindrome Linked List | 链表 | 通过 | |
| lc_237 | Delete Node in a Linked List | 链表 | 通过 | |
| lc_876 | Middle of the Linked List | 链表 | 通过 | |
| lc_104 | Maximum Depth of Binary Tree | 树 | 通过 | |
| lc_100 | Same Tree | 树 | 通过 | |
| lc_101 | Symmetric Tree | 树 | 通过 | |
| lc_108 | Convert Sorted Array to BST | 树 | 通过 | 中序遍历验证 |
| lc_110 | Balanced Binary Tree | 树 | 通过 | |
| lc_111 | Minimum Depth of Binary Tree | 树 | 通过 | |
| lc_112 | Path Sum | 树 | 通过 | |
| lc_226 | Invert Binary Tree | 树 | 通过 | 中序遍历验证 |
| lc_572 | Subtree of Another Tree | 树 | 通过 | |

### 阶段 6：中等题初步填充（5 道）

| 用例 | 题目 | 类别 | 状态 | 备注 |
|:-----|:-----|:-----|:-----|:-----|
| lc_3 | Longest Substring Without Repeating Characters | 字符串/滑动窗口 | 通过 | 使用 256 长度数组记录字符最后出现位置 |
| lc_33 | Search in Rotated Sorted Array | 数组/二分 | 通过 | 旋转有序数组二分查找 |
| lc_48 | Rotate Image | 矩阵 | 通过 | 一维数组模拟二维矩阵，原地转置 + 行反转 |
| lc_62 | Unique Paths | DP | 通过 | 一维滚动数组求组合路径数 |
| lc_64 | Minimum Path Sum | DP | 通过 | 一维滚动数组求最小路径和 |
| lc_2 | Add Two Numbers | 链表 | 通过 | 逐位相加，注意进位与内存释放 |
| lc_11 | Container With Most Water | 数组/双指针 | 通过 | 两端向中间移动计算最大面积 |
| lc_19 | Remove Nth Node From End of List | 链表 | 通过 | 快慢指针定位倒数第 n 个节点 |
| lc_31 | Next Permutation | 数组 | 通过 | 从右向左找拐点，交换后反转后缀 |
| lc_34 | Find First and Last Position of Element in Sorted Array | 数组/二分 | 通过 | 两次二分分别找左右边界 |
| lc_15 | 3Sum | 数组/双指针 | 通过 | 排序后双指针去重找三元组 |
| lc_39 | Combination Sum | 回溯 | 通过 | 全局递归回溯，避免 Vitro 不支持嵌套函数 |
| lc_46 | Permutations | 回溯 | 通过 | 全局递归回溯，used 数组标记访问 |
| lc_75 | Sort Colors | 数组 | 通过 | 荷兰国旗三指针原地分类 |
| lc_198 | House Robber | DP | 通过 | 滚动变量保存前两个状态最大值 |
| lc_55 | Jump Game | 贪心 | 通过 | 维护最远可达位置 |
| lc_142 | Linked List Cycle II | 链表 | 通过 | 快慢指针相遇后再同步寻找入环点 |
| lc_152 | Maximum Product Subarray | DP | 通过 | 同时维护最大/最小乘积处理负数 |
| lc_200 | Number of Islands | 图/DFS | 通过 | 一维数组模拟二维网格，DFS 沉没岛屿 |
| lc_221 | Maximal Square | DP | 通过 | 滚动一维 DP，注意数组大小匹配 |

## 统计摘要

| 阶段 | 总数 | 通过 | 失败 | 记录时间 |
|:-----|:-----|:-----|:-----|:---------|
| LeetCode 数组/字符串 | 27 | 27 | 0 | 2026-06-14 |
| LeetCode 链表/树/栈 | 21 | 21 | 0 | 2026-06-14 |
| LeetCode 混合难度扩展 | 15 | 15 | 0 | 2026-06-18 |
| LeetCode 中等题 | 30 | 30 | 0 | 2026-06-18 |
| **合计** | **92** | **92** | **0** | 2026-06-18 |

## 实施过程发现

### lc_232：复合副作用数组索引表达式触发 Vitro 差异

- **来源**: LeetCode 232 — Implement Queue using Queues
- **发现时间**: 2026-06-14
- **现象**: 原始实现使用 `obj->out[++obj->outTop] = obj->in[obj->inTop--];` 时，Vitro 运行时报告"访问了 NULL 指针区域（地址 0x0000）。NULL 指针不能解引用"；同一份代码在 Clang 下正常执行。
- **是否 Vitro 限制**: 是
- **是否代码本身问题**: 否（代码在 Clang/GCC 下行为正确）
- **是否环境差异**: 否
- **涉及语法特性**: 数组索引表达式中同时包含对两个不同对象的 `++`/`--` 副作用
- **学生影响评级**: P1
- **修复时间**: 2026-06-25
- **是否 Vitro 限制**: 是（已修复）
- **根因**: `gen_mem_inc_dec`（`++`/`--` 内存操作）使用 `temp_slot0` 保存新值，而 `gen_assign` 的 Index 赋值也使用 `temp_slot0` 保存左侧地址；右侧索引表达式的副作用在赋值完成前覆盖了左侧地址临时变量，导致最后读取赋值表达式返回值时访问错误地址。
- **修复方案**: `gen_mem_inc_dec` 改用 `temp_slot3` 保存新值；新增 `baseline/side_effect_index.c` 回归用例。
- **当前处理**: 原 `lc_232.c` 已保留拆分写法以兼容旧版本；新增 `baseline/side_effect_index.c` 专门验证复合副作用数组索引修复。

### lc_4：函数返回 `double` 值在 Vitro VM 下异常（已修复）

- **来源**: LeetCode 4 — Median of Two Sorted Arrays
- **发现时间**: 2026-06-18
- **修复时间**: 2026-06-24
- **现象**: 原始实现使用 `double findMedianSortedArrays(...)` 返回值，在 Clang 下正确输出 `2.00000`、`2.50000`、`1.00000`；在 Vitro VM 下调用该函数后 `printf("%.5f", ...)` 输出全为 `0.00000`。进一步简化测试表明：`double x = 2.5; printf(...)` 正常，但 `printf(..., f())`（`f` 返回 `double`）输出 `0.0`，说明问题集中在函数 double 返回路径。
- **是否 Vitro 限制**: 是（已修复）
- **是否代码本身问题**: 否（代码在 Clang/GCC 下行为正确）
- **是否环境差异**: 否
- **涉及语法特性**: 函数返回值类型为 `double` 时的传值语义
- **学生影响评级**: P1
- **根因**: `return` 语句未对返回值表达式插入隐式类型转换。`return 2.5;` 中的 `2.5` 被解析为 `float` 字面量，在函数返回类型为 `double` 时生成 `PushConstF` 而非 `PushConstD`。
- **修复方案**: TypeChecker 在 `return` 语句的 `check_assignable` 成功后调用 `insert_implicit_cast`，并新增 `baseline/float_func_return.c` 回归用例。
- **当前处理**: `lc_4.c` 已恢复为原始 `double` 返回实现，golden 同步更新，Shadow Verification 通过。

## 已知失败详情

当前无已知失败。

<!-- 待新增失败时按以下模板追加：
### <case_name>

- **来源**: LeetCode XXX
- **失败原因**: <编译错误 / 运行时错误 / 输出不匹配 / 超时>
- **最小复现**: <关键代码片段>
- **是否 Vitro 限制**: 是/否
- **是否代码本身问题**: 是/否
- **是否环境差异**: 是/否
- **涉及语法特性**: <如 long long, 递归深度, qsort 回调>
- **学生影响评级**: P0 / P1 / P2
- **建议**: <修复方向 / 记录为已知限制 / 待进一步分析>
-->

## 阶段 6 完成情况

LeetCode 用例已纳入 `shadow_verify.py` 扫描范围，生成专项报告 `native/tests/shadow_verification/reports/kr_leetcode_report.json`。

| 来源 | 总数 | 匹配 | 编译缺口 | 运行时缺口 | 输出差异 |
|:-----|:-----|:-----|:---------|:-----------|:---------|
| K&R | 69 | 69 | 0 | 0 | 0 |
| LeetCode | 92 | 92 | 0 | 0 | 0 |

## 后续计划

1. 持续观察新增用例是否暴露其他 Vitro 与 Clang 行为差异。
2. LeetCode 中等题已填充至 30 道，并继续 all in 扩展 15 道混合难度题，当前 LeetCode 用例总数 92 道；后续可评估困难题或 K&R 进阶覆盖。
3. 将 shadow 报告路径纳入 CI artifact 上传。
