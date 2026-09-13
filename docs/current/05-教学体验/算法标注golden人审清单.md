# 算法标注 golden 人审清单（防线 6 · U1#1 ①，v2 2026-09-13）

> **v3 修复批头注（2026-09-13，用户机器复核驱动）**：本清单 §2 主表为
> 修复前提取——用户复核（见 `算法标注golden审阅意见.md`，100 行逐行判定
> 62✅/25✗/13⛔）驱动的第三批修复已落地：P0-1 has_word 缩写、P0-2 四算法
> 误判、P0-3 dp 判据、P1 判据批 10 项（详见 CHANGELOG [Unreleased]）。
> **主表待重提取对账后方可勾选**；P0-4（运算过程类 phase 的 prev 操作数
> 管道）登记下一批。
>
> **三审修复批头注（2026-09-13 同日，二审 P0-C/P1 批）**：P0-C gcd mod
> 排除 IO 行（首现回真语句 L6）；P1-1 dp 初始化循环排除（单行 for 形态
> 修复 2/4——dpLCS/matrixChain 的多行 for 体不在行内、单行判定不可达，
> 登记边界）；P1-2 radix count 收紧 `]++` 自增形态；P1-5 签名行统一排除
> （infer_algorithm_step 总入口，类型关键字开头 + `{` 结尾）；§7.5 hanoi
> 零盘递归不产出；§7.3 step_next 改 append 语义防 batch>1 丢帧。
> **登记未修**：P1-3/P1-4 顶层调用区分——带标注帧是 callee entry
>（func_name 已是被调函数、code_line 仍是 main 调用行），需
> at_callee_entry 传入 inferrer（与 P0-4 prev_vars 同批管道改动）。
> **P1-6 缺口登记**：activitySelection / externalSort / mergeSortedLists
> 三模板因误判消除转零标注（外部排序/置换选择/链表归并是真实教学内容，
> 属代价非成果）；huffmanTree 的 select 分支零触发。清单 §3/§4 的数量口径以审阅意见 §4.4-4.6 修正为准
> （实测 36+46；bTree/criticalPath 归档 C 非"截断"；stringBasicOps 与
> binarySearchTreeValidation 归收紧漏检——后者已随 has_word 修复复亮）。

> **v2 修订（用户审阅反馈驱动）**：v1 清单被否决——"表格里的数值列系统性不可信，
> 勾选会把错误固化成 golden"。两项 P0（首帧旧值 / 结构特征误判）已在本轮修复，
> 本清单为修复后的重提取（serve 会话 × 4000 步，phase 键已修正）。
> **审阅方法不变**：逐行勾选 ☐ 通过 / ✗ 改写。注意：description 中的数值
> 已是**行末帧（语句完成态）**的值（P0-1 修复后），仍需人工判断语义正确性。

## 1. 二轮修复记录（请复核，均含单测锚定 + 模板实测）

1. **P0-1 首帧旧值**（用户实测三例）：同一语句的多帧中首帧在赋值发生前，
   产出错误数值标注（binary 首帧"计算中点 mid=0"实际 mid=2、shellSort
   "取增量 gap=0"、dijkstra"顶点 -1"）。修复：**行末帧标注保留**双层——
   ① run_batch 返回数组 + frame_cache 同步去重（同行非末帧标注清除）；
   ② serve step.next 一帧发布缓冲（流式协议下行末判定需要未来信息，
   当前帧暂存、下一帧到来时回改上一帧后再发布）。
   **实测三例首帧错误值全部消失**（2026-09-13 复审后补测：`binary` 的
   `step.next` 路径只剩 `计算中点 mid=2`；`shellSort` 为 `gap=2`；
   `dijkstra` 为 `顶点 1`）。
   > **复审更正（2026-09-13）**：② 的首版实现**实际从未执行**——
   > `session.unified_pending` 唯一赋值点被写在 `if let Some(pending)` 块内，
   > 而该字段初始为 `None`，分支永不进入（Option 死锁）。故当时
   > `step.next` 路径仍下发 `mid=0`，仅 `payload.get`（引擎层）有效。
   > 已修复：赋值移出 `if` 块；全局首帧因无下一帧可对比，**保守清除标注**
   > （宁可少报一条，不可把赋值前旧值当正确值呈现）。`serve_smoke` 51 PASS
   > 未受影响。副作用：`step.next` 现为真·延迟一帧（帧序 +1）。
2. **P0-2 结构特征误判**（用户实测：linear→BFS / hashTable→队列非空 /
   stringBasicOps→递归查找插入位置）：检测器四个结构分支收紧为命名主导
   ——BFS 删 `search+单循环+回边` 分支、DFS 删 `递归+search` 分支、
   BST 插入/查找收紧 bst 语境、链表删除收紧 linked/list 语境。
   > **复审更正（2026-09-13）**：首版"实测 linear/hashTable/stringBasicOps
   > 零误判"**不成立**——`stringBasicOps` 实测仍判 `bst_insert`（与 §2 主表
   > 该行自相矛盾）。根因：收紧后的新判据 `name_lower.contains("bst")`
   > **自身仍是子串匹配**，而 `subString` 小写为 `substring`、内含 `bst`。
   > 已修复：新增 `features::has_word`（按 `_` / `-` / 数字 + 驼峰切分后整词
   > 比对），`tree.rs` 两处 bst 判据改用它；补单测
   > `substring_not_misdetected_as_bst` + `camel_case_bst_still_detected`。
   > 实测 `stringBasicOps` 现**零标注**（可归 §3 档 C）。
   > **教训**：命名判据一律用 `has_word`，禁止裸 `contains()`——
   > `contains("insert")` 命 `insert_node`、`contains("bst")` 命 `substring`，
   > 同一个坑已踩两次。
3. **dijkstra confirm 误匹配**（P0-1 顺带）：`visited[v0] = 1;` 初始化行
   被判"确认顶点 -1"——收紧为 `visited[u] =` 形态。
4. **P1-a dp 接线**：`infer_dp` 孤儿接通（检测器补 `dp[` 状态表特征
   分支）——dpFib 实测出现 transition/outer_loop/finish。
5. **v1 口径错误修正**（用户指出）：v1 §1.1 把 target=7 的序列写成
   "模板实测"（模板默认 target=5 一次命中仅 4 条）。
   > **复审更正（2026-09-13）**："本轮描述均注明探针来源与输入"一句
   > **未兑现**——主表列仅 `模板 | 算法 | phase | description | 审阅`，
   > 无来源/输入列。现改为在 §2 口径统一声明（取帧路径 + 参数来源），
   > 不再逐行重复。**单条标注的可复现输入 = 该模板的默认参数**（见
   > `templates/<模板>/meta.yaml` 的 `params`）；本轮未做非默认参数的
   > 分支提取（例如 binary 的 `target=7` 序列**不在本表内**，其
   > `narrow_left` / `found` 分支因此未出现——这属覆盖面而非缺陷）。

**已知代价（待裁定）**：bstInsert / bstSearch / bstDelete 模板的函数名是
裸 `insert` / `search` / `deleteNode`，收紧后漏检（零标注）。两条路：
① 模板函数名加 `bst_` 前缀（对 golden 无影响——函数名不进 stdout，但需
sync_templates 重生成 + 防线复跑）；② 检测器加树语境特征（参数/返回
TreeNode* 判据，成本高）。**建议 ①**。

**dp 文案泛化问题**：infer_dp 的 outer_loop 文案是"遍历物品 i=N"（背包
语境），dpFib（斐波那契）也输出"物品"——phase 正确但文案需按算法族
泛化（"遍历子问题"类），列入下方主表审阅项。

## 2. 主审阅表（38 模板 × 100 条标注）

**口径（2026-09-13 修订，消除 v2 的两处歧义）**：

- description 取「**行末帧序列中，该 (算法, phase) 组合的首次出现**」。
  既不是"原始首帧"（首帧在赋值发生前，数值是旧值/哨兵——曾产出
  `计算中点 mid=0`、`取增量 gap=0`、`顶点 -1` 一类错误标注），也不是
  "该 phase 的末次出现"。v2 中两个口径混用（§2 标题写"行末帧形态"、
  §4 写"首次出现形态"），现统一为本句。
- **取帧路径 = `serve` 的 `step.next`**（学生端同款，含一帧延迟发布）。
  与 `payload.get`（引擎帧缓存、无延迟）结果可能不同：修复前实测
  `binary` 的 `mid_calc` 在 `step.next` 下为 `mid=0`+`mid=2`、
  在 `payload.get` 下只有 `mid=2`。**勿混用两条路径做提取/比对。**
- 覆盖不全的模板见 §4（截断），其行**不代表该 phase 全集**。

判定要点：① phase 分类是否符合算法教学阶段划分；② description 语义
是否准确（数值已为完成态，但术语/语境可能错——如 dp 的"物品"）；
③ 有无明显缺 phase。

### 2.1 质检结果（机器可判，先于人工审阅）

下列行含**未解析值（`?`）**或**无效值（`-1` / 反区间）**，属"匹配错行 /
变量取值失败"缺陷，**不应进入 golden**——已从正常审阅项中摘出，单独定位。

| 模板 | phase | 异常形态 | 性质 |
|------|-------|----------|------|
| activitySelection | compare / inner_loop | `arr[?]` / `min_idx=?` | 变量取不到值 |
| selection | compare / inner_loop | `arr[?]` / `min_idx=?` | 同上 |
| externalSort | compare | `arr[?]` | 同上 |
| huffmanTree | compare | `arr[?]` | 同上 |
| insertion | insert | 行尾孤立 `?` | 插入位置变量缺失 |
| primMST | add_vertex | `顶点 -1 加入生成树` | 匹配错行（初始化行） |
| mergeSortedLists | recursive_split | `区间 [-1, -1]` | 同上 |

其余 **93 条**无 `?` / `-1` 异常，可正常逐条审阅。

| 模板 | 算法 | phase | description（行末帧 / 每 phase 首现） | 审阅 |
|------|------|-------|--------------------------------------|------|
| activitySelection | selection_sort | compare | 比较 arr[1] 与当前最小值 arr[?] | ☐ |
| activitySelection | selection_sort | inner_loop | 扫描 j=1，当前最小值在 min_idx=? | ☐ |
| avlTree | avl_tree | balance | 平衡因子失衡，进行平衡调整 | ☐ |
| avlTree | avl_tree | update_bf | 更新平衡因子 | ☐ |
| bfs | bfs | dequeue | 出队节点 u=0 | ☐ |
| bfs | bfs | enqueue | 邻居节点入队 | ☐ |
| bfs | bfs | loop | 队列非空，继续广度优先搜索 [front=0, rear=1] | ☐ |
| bfs | bfs | visit | 标记节点 0 为已访问 | ☐ |
| binary | binary_search | compare | arr[2] 与目标值 5 比较 | ☐ |
| binary | binary_search | loop | 搜索范围 [0, 4] | ☐ |
| binary | binary_search | mid_calc | 计算中点 mid=2 | ☐ |
| binarySearchTreeValidation | bst_insert | compare | 比较插入值与当前节点值，决定向左或向右 | ☐ |
| binarySearchTreeValidation | bst_insert | create | 找到空位，创建新节点 | ☐ |
| binarySearchTreeValidation | bst_insert | recursive | 递归查找插入位置 | ☐ |
| bubble | bubble_sort | compare | 比较 arr[0] 与 arr[1] | ☐ |
| bubble | bubble_sort | inner_loop | 内层循环 j=0，比较相邻元素 | ☐ |
| bubble | bubble_sort | outer_loop | 第 1 趟：将第 1 大的元素放到正确位置 | ☐ |
| bubble | bubble_sort | swap | 交换 arr[0]↔arr[1]，较大的元素向右移动 | ☐ |
| computeNextVal | string_match_kmp | build_next | 构建 next 数组，next[0]=-1 | ☐ |
| countingSort | counting_sort | collect | 按数值从小到大收集元素 | ☐ |
| countingSort | counting_sort | place | 将数值放回原数组的正确位置 | ☐ |
| dfs | dfs | recursive | 递归深入：从节点 0 继续深度优先搜索 | ☐ |
| dfs | dfs | scan | 扫描节点 0 的邻居 | ☐ |
| dfs | dfs | visit | 标记节点 0 为已访问 | ☐ |
| dijkstra | dijkstra | confirm | 顶点 1 的最短距离已确定 | ☐ |
| dijkstra | dijkstra | relax | 松弛操作，更新邻接顶点距离 | ☐ |
| dpCoinChange | dp | finish | 动态规划计算完成 | ☐ |
| dpCoinChange | dp | outer_loop | 遍历物品 i=12 | ☐ |
| dpCoinChange | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| dpFib | dp | finish | 动态规划计算完成 | ☐ |
| dpFib | dp | outer_loop | 遍历物品 i=2 | ☐ |
| dpFib | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| dpKnapsack | dp | outer_loop | 遍历物品 i=0 | ☐ |
| dpKnapsack | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| dpLCS | dp | outer_loop | 遍历物品 i=0 | ☐ |
| dpLCS | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| dpLIS | dp | finish | 动态规划计算完成 | ☐ |
| dpLIS | dp | outer_loop | 遍历物品 i=8 | ☐ |
| dpLIS | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| externalSort | selection_sort | compare | 比较 arr[?] 与当前最小值 arr[?] | ☐ |
| externalSort | selection_sort | outer_loop | 第 4 趟：从第 4 个位置开始找最小值 | ☐ |
| floyd | floyd | outer_loop | 枚举中间顶点 k=0 | ☐ |
| floyd | floyd | relax | 检查经 k 中转是否更短 | ☐ |
| gcd | gcd | finish | 最大公约数为 6 | ☐ |
| gcd | gcd | loop | 辗转相除：a=48, b=18 | ☐ |
| gcd | gcd | mod | 计算 48 % 12 = 0 | ☐ |
| hanoi | hanoi | base | 基准情况：直接把盘子从起始柱移到目标柱 | ☐ |
| hanoi | hanoi | finish | 汉诺塔移动完成 | ☐ |
| hanoi | hanoi | move | 移动第 1 个盘子 | ☐ |
| hanoi | hanoi | recursive | 递归移动 2 个盘子 | ☐ |
| hashTable | hash_table | hash | 计算哈希值 | ☐ |
| heapSort | heap_sort | build_heap | 建堆：自底向上将数组调整为最大堆 | ☐ |
| heapSort | heap_sort | extract | 取出堆顶元素并重新堆化 | ☐ |
| heapSort | heap_sort | heapify | 递归堆化：确保子树满足堆性质 | ☐ |
| heapSort | heap_sort | swap | 交换元素，调整堆结构 | ☐ |
| huffmanTree | selection_sort | compare | 比较 arr[?] 与当前最小值 arr[?] | ☐ |
| huffmanTree | selection_sort | outer_loop | 第 0 趟：从第 0 个位置开始找最小值 | ☐ |
| huffmanTree | huffman_tree | merge | 合并两个节点为新树 | ☐ |
| insertion | insertion_sort | inner_loop | 元素后移 j=0，为插入腾出位置 | ☐ |
| insertion | insertion_sort | insert | 将 key=11 插入到正确位置 ? | ☐ |
| insertion | insertion_sort | outer_loop | 第 1 个元素：准备插入到已排序部分 | ☐ |
| kruskalMST | kruskal_mst | add_edge | 加入生成树并合并集合 | ☐ |
| kruskalMST | kruskal_mst | check_cycle | 并查集判环 | ☐ |
| kruskalMST | kruskal_mst | sort | 按边权排序 | ☐ |
| levelOrder | level_order | dequeue | 取出队头节点访问 | ☐ |
| levelOrder | level_order | enqueue | 根节点入队 | ☐ |
| levelOrder | level_order | enqueue_left | 左子节点入队 | ☐ |
| levelOrder | level_order | enqueue_right | 右子节点入队 | ☐ |
| matrixChain | dp | outer_loop | 遍历物品 i=0 | ☐ |
| matrixChain | dp | transition | 状态转移：计算当前子问题的最优解 | ☐ |
| merge | merge_sort | merge | 合并两个有序子数组 | ☐ |
| merge | merge_sort | recursive_split | 将数组区间 [0, 4] 递归分成两半 | ☐ |
| mergeSortedLists | merge_sort | finish | 归并排序完成 | ☐ |
| mergeSortedLists | merge_sort | merge | 合并两个有序子数组 | ☐ |
| mergeSortedLists | merge_sort | recursive_split | 将数组区间 [-1, -1] 递归分成两半 | ☐ |
| primMST | prim_mst | add_vertex | 顶点 -1 加入生成树 | ☐ |
| primMST | prim_mst | update | 更新邻接顶点的最小边权 | ☐ |
| quick | quick_sort | partition_init | 分区：选取枢轴 pivot=0 | ☐ |
| quick | quick_sort | recursive | 递归调用 quickSort，处理子子数组 [left=0, right=4] | ☐ |
| radixSort | radix_sort | count | 统计当前位各数字出现次数 | ☐ |
| radixSort | radix_sort | digit_loop | 按第 1 位进行分配-收集 | ☐ |
| radixSort | radix_sort | place | 按前缀和放置元素 | ☐ |
| radixSort | radix_sort | prefix | 计算前缀和，确定位置 | ☐ |
| selection | selection_sort | compare | 比较 arr[1] 与当前最小值 arr[?] | ☐ |
| selection | selection_sort | inner_loop | 扫描 j=1，当前最小值在 min_idx=? | ☐ |
| selection | selection_sort | outer_loop | 第 0 趟：从第 0 个位置开始找最小值 | ☐ |
| selection | selection_sort | swap | 将最小元素交换到位置 0 | ☐ |
| seqList | seq_list | finish | 顺序表操作完成 | ☐ |
| seqList | seq_list | update_len | 更新表长度 | ☐ |
| shellSort | shell_sort | inner_loop | 同组内元素后移，腾出插入位置 | ☐ |
| shellSort | shell_sort | insert | 保存当前元素，准备在同组内插入 | ☐ |
| shellSort | shell_sort | outer_loop | 取增量 gap=2，分组进行插入排序 | ☐ |
| stringBasicOps | bst_insert | recursive | 递归查找插入位置 | ☐ |
| stringMatchBF | string_match_bf | backtrack | 字符不匹配，主串回溯 | ☐ |
| stringMatchBF | string_match_bf | compare | 比较 S[0] 与 T[0] | ☐ |
| stringMatchKMP | string_match_kmp | build_next | 构建 next 数组，next[0]=0 | ☐ |
| stringMatchKMP | string_match_kmp | compare | 比较主串与模式串字符 | ☐ |
| topologicalSort | topological_sort | decrease | 删边，邻接点入度减 1 | ☐ |
| topologicalSort | topological_sort | enqueue | 入度为 0 的顶点入队 | ☐ |
| topologicalSort | topological_sort | output | 输出顶点 0 | ☐ |

## 3. 覆盖缺口（44 个零标注模板，分三档）

**数量口径**（v2 未写明，读者算不出 44 的来历）：
`88（全部模板）= 38（§2 主表已覆盖）+ 44（本节零标注）+ 6（cpp 模板，见 §4）`；
44 = 档 A 4 个 + 档 C 40 个（档 B 描述算法而非模板，不计入）。

**档 A · 检测已收紧致漏检（4 个，修复成本最低——改模板函数名即可）**：
`bstInsert`, `bstSearch`, `bstDelete`, **`linkedDelete`**

> `linkedDelete` 为 2026-09-13 复审补入：检测器**有** `linked_list_delete`
> 分支（`native/src/compiler/algorithm_detector/structures.rs`）且单测
> `linked_list_delete_node_still_detected` 为绿，v1 主表也曾出现其标注
> （`linked_list_delete | 遍历链表查找目标节点`）——收紧 linked/list 语境后
> 该模板函数名不再命中，属**收紧致漏检**，与三个 bst 同因，不应归入档 C
> 的"完全未实现"（后者需检测分支 + 步骤模板双写，成本高一个量级）。

**档 B · 步骤模板已实现但检测分支缺失（对接成本）**：
`bst_insert` / `bst_search` 的步骤模板（`vitro_algorithm_steps/src/tree.rs`）已实现
——与档 A 同一根因。**本档描述的是算法而非模板**，故不计入 44 的模板计数。

**档 C · 完全未实现（需检测分支 + 步骤模板双写）**：
`array`, `bTree`, `bellmanFord`, `bucketSort`, `circularLinkedList`, `circularQueue`, `criticalPath`, `deque`, `doublyLinkedList`, `factorial`, `fib`, `fibonacciSearch`, `infixEvaluation`, `inorder`, `interpolationSearch`, `isPrime`, `josephus`, `linear`, `linked`, `linkedInsert`, `linkedListTail`, `linkedQueue`, `linkedStack`, `linkedTraverse`, `parenthesesMatch`, `pointer`, `polynomialAdd`, `postorder`, `queueArray`, `redBlackTree`, `sieveOfEratosthenes`, `spfa`, `stackArray`, `staticLinkedList`, `stringReverse`, `threadedBinaryTree`, `treeNode`, `treePreorder`, `treeToForest`, `unionFind`

## 4. 提取口径限制（非缺陷，登记备查）

- **截断 7 个**（4000 步仍不足）：`bTree`, `criticalPath`, `dpKnapsack`, `dpLCS`, `floyd`, `matrixChain`, `radixSort`
  ——需要提高步数或分段提取后补审。
  **其中 4 个已在 §2 主表出现行**：`floyd`（2 条，缺 `finish` 类）、
  `matrixChain` / `dpKnapsack` / `dpLCS`（各 2 条，均缺 `finish`）。
  **这些行只代表"跑到截断点为止已出现的 phase"，不代表全集**——人工审阅时
  请勿据此判定"该算法缺 phase"，更勿据此固化 golden（golden 需全集）。
  实测复核（4200 帧仍截断）：`floyd` 只到 `outer_loop k=3`，
  `matrixChain` 的 `outer_loop i` 才到 7。
- **cpp 模板 6 个**（无 source.c，需 C++ 编译口径）：
  `cpp_class_basic`, `cpp_hello`, `cpp_range_for`, `cpp_unique_ptr`, `cpp_vector_int`, `cpp_vector_struct`
- 提取环境：本轮修复后的 release 构建（serve 会话，phase 键自
  `AlgorithmStepSnapshot.phase`）；标注为该 (算法, phase) 组合的**首次
  出现形态**；golden 固化 phase + description 模板（数值位占位符），
  行末帧语义已由引擎保证。

## 5. 审阅后的固化路径（不变，待确认）

勾选 → golden JSON（模板 → phase → description 期望集）→ serve 会话
收集比对进 CI（U1#1① 收口）→ ② property 层（标注-行为一致性抽样）。

## 6. 同族风险（v1 §6 扩充：用户指出结构特征是误判大头）

- **命名子串 5 项**（未修，待裁定）：merge / binary / quick / heap /
  select——收紧时**必须改用 `features::has_word` 整词匹配**，不得再用裸
  `contains()`：`contains("insert")` 命 `insert_node`、
  `contains("bst")` 命 `subString`（小写 `substring` 内含 bst）——
  同类误判已两次实锤，后者为 2026-09-13 复审发现（见 §1.2）。
- **结构特征分支已清查收紧 4 处**（本轮已修，见 §1.2）；其余结构分支
  （如快排的 `is_recursive && has_partition_pattern && has_nested_loops`）
  理论上仍有误判面（递归分治的非排序算法）——建议防线 6 golden 落地后
  由回归测试暴露，不再预先猜测收紧。
