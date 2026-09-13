# 算法标注 golden 人审清单 · 机器复核（2026-09-13）

> **复核对象**：`docs/current/05-教学体验/算法标注golden人审清单.md`（v2，文件 mtime 21:31）
> **复核方式**：在工作树当前状态下重建二进制 → `serve` + `step.next`（学生端同款路径）→ 82 个含
> `source.c` 的模板逐模板 4000 步全量提取 → 与清单 §2 主表 100 行做逐字符对账 + 逐行语义判定。
> **定位约定**：下文「第 N 行」= 清单 §2 主表自上而下第 N 行（含表头后第一行 activitySelection/compare）。
> **边界**：本文件为项目持有人的初审，不保证完全正确，进行修复后会进行二审。

---

## 0. 结论摘要

1. **清单的忠实度没有问题**。100 行中 **96 行与实测逐字符一致**，描述不符 **0 行**，实测有而清单缺的 phase **0 条**。
   也就是说 v2 确实做到了"表的每一格都是引擎输出的转录"，v1 被否决的"表格数值系统性不可信"已基本消除。
2. **但语义层不能直接勾选**。逐行判定结果：**✅ 可进 golden 62 行 / ✗ 必须改写 25 行 / ⛔ 必须作废 13 行**。
3. **三个阻断项**（未修完不得固化 golden）：
   - **P0-1** `binarySearchTreeValidation` 因 `has_word` 的驼峰缩写切分缺陷**整体静默失效**（实测 4001 帧 / 0 标注），
     而清单仍留着它的 3 行（第 12–14 行）；同类形态（`isValidBST`、`BSTInsert`）今后一律漏检。
   - **P0-2** 4 个模板被误判成 `selection_sort` / `merge_sort`，牵连 **13 行**：
     `huffmanTree`、`activitySelection`、`externalSort`（→ selection_sort）、`mergeSortedLists`（→ merge_sort）。
   - **P0-3** **全部 5 个 dp 模板的 `transition` 首现都挂在初始化行**（`dp[...] = 0;`）上，"状态转移"文案与所挂语句不符。
4. 清单 §2.1 的机器质检（只扫 `?` / `-1`）覆盖到 38 处问题中的 **9 处**；
   **另外 29 处问题躺在"其余 93 条可正常逐条审阅"里，没有任何提示**——这正是 v1 被否决的同一个风险点，只是从"数值不可信"换成了"语义不可信"。
5. §3 的数量口径已经过期（清单 38+44，实测 **36+46**），且 §3 档 C 与 §4 截断名单**互相矛盾**。

---

## 1. 复核可复现性

```bash
# 1) 全量提取（口径 = 清单 §2：serve step.next，取 (算法, phase) 首次出现）
python tmp/annot_extract.py --next 4000        # 82 模板 / 36 个有标注 / 用时 33.4s
#    -> tmp/annot_extract.json（已归档为 reports/annot_extract_20260913.json）

# 2) 与清单主表对账
python tmp/annot_diff.py                       # -> tmp/annot_diff.md

# 3) 单模板取证（带 code_line + 源码行 + 该帧局部变量）
python tmp/annot_frames.py gcd --next 120
python tmp/annot_raw.py  gcd --next 6
```

对账输出（原文）：

```
文档主表行数 100；实测有标注模板 36
=== A. 完全一致 96 ===
=== B. 描述不一致 0 ===
=== C. 文档有、实测无（行已失效） 4 ===
  binarySearchTreeValidation compare   -> 实测该模板零标注（整份文件）
  binarySearchTreeValidation create    -> 实测该模板零标注（整份文件）
  binarySearchTreeValidation recursive -> 实测该模板零标注（整份文件）
  stringBasicOps             recursive -> 实测该模板零标注（整份文件）
=== D. 实测有、文档缺（漏登 phase） 0 ===
```

---

## 2. P0 阻断项

### P0-1 `has_word` 驼峰缩写缺陷 → `binarySearchTreeValidation` 静默零标注

**证据链**（三重一致）：

| 环节 | 事实 |
|------|------|
| 代码改动 | `git diff tree.rs`：旧 `if name_lower.contains("bst") \|\| …` → 新 `if has_word(func_name, "bst")` |
| 输入语义 | `algorithm_detector/mod.rs:33` `let name_lower = func.name.to_lowercase();`（两者同源，只是切分方式变了） |
| 模板函数名 | `templates/binarySearchTreeValidation/source.c:20` `int isValidBST(struct TreeNode* root, long long min, long long max)` |
| 切分结果 | `has_word("isValidBST","bst")`：连续大写被逐字母切开 → `["is","valid","b","s","t"]` → **不含 `bst`** → false |
| 实测 | 该模板 4001 帧、**0 条 algorithm_step**；`payload.get` 同 |

**性质**：这是本轮"修 `subString` 里的 bst 子串陷阱"时引入的**反向漏报**，而且漏报是**静默**的——
零标注不会触发任何断言，前端只是"什么都不显示"。清单 §2 仍留着它的 3 行（第 12–14 行），
§3 档 A"收紧致漏检"也**没有**把它算进去（档 A 只有 `bstInsert/bstSearch/bstDelete/linkedDelete` 四个）。

**建议修法**：
1. `has_word` 增加"连续大写缩写"合并档：全大写串（`BST`）作为一个整词保留，再把缩写**小写整词**与其他词一起比对；
2. 单测补两向锚：`isValidBST` / `allCapsAcronymBFS` **应识别**，`subString` **不应识别**（现有两个锚只覆盖驼峰 `bstInsert` 与 `subString`）；
3. golden 里为**零标注模板**加一条负向断言（"要么有期望集，要么显式登记为已知零标注"），否则这类回归永远只能靠人眼发现。

### P0-2 四处算法误判（13 行必须作废）

| 模板 | 函数名（模板源码） | 误判为 | 真算法 | 典型错标 | 实测首现条数 |
|------|--------------------|--------|--------|----------|--------------|
| `huffmanTree` | `Select(HTNode HT[], int n, int* s1, int* s2)` | `selection_sort` | 哈夫曼"选两个最小权值节点" | `比较 arr[?] 与当前最小值 arr[?]`（数组实为 `HT`，且无 `min_idx`）<br>`第 0 趟：从第 0 个位置开始找最小值` | 10 条中 9 条错、仅 `merge` 1 条对 |
| `activitySelection` | `selectActivities(int start[], int finish[], int n)` | `selection_sort` | 贪心活动安排 | `扫描 j=1，当前最小值在 min_idx=?`<br>`比较 arr[6] 与当前最小值 arr[?]`（6 元素数组，下标 6 **越界**） | 9 条全错 |
| `externalSort` | `replacementSelection(int arr[], int n)` | `selection_sort` | 置换选择（外部排序） | 在**装填堆的循环**（L7）上标"第 4 趟：从第 4 个位置开始找最小值" | 4 条全错 |
| `mergeSortedLists` | `merge(struct Node* L1, struct Node* L2)` | `merge_sort` | 有序链表归并 | `将数组区间 [-1, -1] 递归分成两半`（挂在 `main` 的调用行 L51）<br>`归并排序完成`（挂在 `return dummy.next;`） | 3 条全错 |

**这不是"变量取不到值"，是判定本身错**——清单 §2.1 把 `arr[?]` 一律归因"变量取不到值"，
会让修复方向跑偏（去补变量名别名是**修不好**这四处的）。

**建议修法**（按成本）：
- 短期：给 `selection_sort` 判据加"证据要求"——命中需同时具备 `min_idx` 变量 + 数组名（或含 `sort/select` 语义的函数名）且**排除**已知语境词（`huffman/activity/replacement`）；
- 中期：引入**模板级算法白名单**（模板 meta.yaml 声明期望算法），检测器作为兜底而非唯一来源——与 Phase 41"内置容器布局以 `.cpp` 为单一真相来源"同一思路；
- 长期：`arr` 不应硬编码在文案模板里（`sorting.rs` 多处 `format!("… arr[{}] …")`），应取实际数组名。

### P0-3 全部 dp 模板的 `transition` 首现挂着初始化行

| 行号 | 模板 | 清单文案 | 实测首现挂载行 | 该行实际语义 |
|------|------|----------|----------------|--------------|
| 29 | dpCoinChange | 状态转移：计算当前子问题的最优解 | L8 `dp[0] = 0;` | 初始化 |
| 32 | dpFib | 同上 | L6 `dp[0] = 0;` | 初始化 |
| 34 | dpKnapsack | 同上 | L15 `dp[i][j] = 0;` | 初始化 |
| 36 | dpLCS | 同上 | L12 `dp[i][j] = 0;` | 初始化 |
| 39 | dpLIS | 同上 | L14 `result = max(result, dp[i]);` | 结果更新（真正转移在 L12，本轮**未产生** transition 帧） |
| 70 | matrixChain | 同上 | L7 `dp[i][j] = 0;` | 初始化 |

**根因**：`vitro_algorithm_steps/src/dp.rs:21` 的判据只有 `line_lower.contains("dp[") && line_lower.contains('=')`，
初始化行天然满足。**建议**：排除右侧为字面量的形态（`dp[...] = <int literal>;`），
或要求同一语句内两侧都有 `dp[`（真正的转移是 `dp[i][j] = f(dp[...])`）。

附带（同一文件的判据问题）：`dp.rs:13` 用 `line_lower.contains("i") && line_lower.contains("n")` 判外循环——
`for (int j = coins[i]; j <= amount; j++)` 里的 **`amount` 含字母 n**，使 dpCoinChange 的**内层币种循环被当成 outer_loop**
（实测"遍历物品 i=0 / i=1 / i=2"三条都来自这一行）。这与用户已经踩过两次的"裸 `contains` 做命名/模式判据"是同一类错误。

### P0-4 `gcd / mod` 数值全错 → "行末帧 = 正确数值"这个前提不成立

**实测原始帧（`tmp/annot_frames.py gcd`）**：

```
[ 20] L4  loop  辗转相除：a=48, b=18          src: while (b != 0) {        vars: a=48, b=18
[ 30] L6  mod   计算 48 % 12 = 0               src: b = a % b;              vars: a=48, b=12, temp=18
[ 41] L4  loop  辗转相除：a=18, b=12                                     vars: a=18, b=12
[ 51] L6  mod   计算 18 % 6 = 0                src: b = a % b;              vars: a=18, b=6, temp=12
[ 62] L4  loop  辗转相除：a=12, b=6                                       vars: a=12, b=6
[ 72] L6  mod   计算 12 % 0 = 0                src: b = a % b;              vars: a=12, b=0, temp=6
[ 84] L9  finish 最大公约数为 6                 src: return a;               vars: a=6, b=0
```

`gcd(48,18)` 的真实首个取模是 **48 % 18 = 12**；清单第 46 行的 `计算 48 % 12 = 0` 里，
**两个操作数与结果没有一个是对的**。成因不是修帧没修干净，而是**文案需要的不是同一批变量**：
`math.rs:16-22` 用"当前帧的 a、b"拼 `a % b = a % b`，而行末帧的 `b` **已被该语句赋值**（48→12）。
即 P0-1 的"行末帧"修复对 `loop/visit/finish` 这类**展示结果值**的 phase 是对的，
对 `mod` 这类**展示运算过程**的 phase 反而引入了系统性错误。

**建议**：把口径从"行末帧"细化为「**展示结果值的 phase 用行末帧；展示运算过程的 phase 用语句执行前的操作数**」，
并在 `build_step` 侧显式区分（例如 `infer_*` 需要 `prev_vars` 快照）。这一条不修，golden 里就会锚定一个错误算式。

### P0-5 `stringBasicOps` 行已在 §1.2 承认失效，但主表未删

- 清单 §1.2 复审更正写明："实测 `stringBasicOps` 现**零标注**（可归 §3 档 C）"；
- 但第 93 行仍在主表（`stringBasicOps | bst_insert | recursive | 递归查找插入位置`），
  §3 档 C 的 40 项清单里也**没有** `stringBasicOps`；
- 实测（本轮）确认：零标注。

→ 同一份文档里三处口径不一致。**建议**：删第 93 行，并把 `stringBasicOps` 显式登记（或归入档 A：`subString` 命名不命中，属收紧致漏检）。

---

## 3. P1 逐项缺陷（✗ 必须改写，附建议文本）

| 行号 | 模板/phase | 清单文案 | 缺陷 | 建议 |
|------|-----------|----------|------|------|
| 84 | selection/compare | 比较 arr[1] 与当前最小值 arr[?] | `min_idx=?`：模板变量名是 **`minIdx`**（`templates/selection/source.c:5`），`sorting.rs:90` 的别名表 `["min_idx","minIndex","min","minindex"]` 漏了它；且家族内含 `比较 arr[5]`（n=5，**越界**） | 别名表补 `minIdx`；给 selection_sort 的 compare 加 `j_in_inner_range` 守卫（bubble 已有，`sorting.rs:20/59`，属同类未闭环） |
| 85 | selection/inner_loop | 扫描 j=1，当前最小值在 min_idx=? | 同上 | 同上 |
| 1 | activitySelection/compare | 比较 arr[1] 与当前最小值 arr[?] | 算法误判 + 数组名错（实为 `start`/`finish`）+ `arr[6]` 越界 | 见 P0-2 |
| 2 | activitySelection/inner_loop | 扫描 j=1，当前最小值在 min_idx=? | 同上 | 见 P0-2 |
| 60 | insertion/insert | 将 key=11 插入到正确位置 ? | 分支条件过宽（`sorting.rs:180-191`：`(arr[`\|`a[`) && '=' && !for/while`）→ 命中 **L5 `int key = arr[i];`**（读操作，j 未定义 → `?`），并让后移行 `arr[j+1] = arr[j];` 反复报"插入到位置 3/2/1" | 收紧为 `arr[j + 1] = key` 形态；position 取 `j+1` 前先确认 j 有效 |
| 76 | primMST/add_vertex | 顶点 -1 加入生成树 | 命中**初始化行** `lowcost[0] = 0;`（L8，`k` 未初始化 → -1）；真实加入生成树在 L24 `lowcost[k] = 0;` | 收紧为 `lowcost[k] = 0` 形态——与 dijkstra 的 `visited[u]` 同款修法（`graph.rs:149` 已示范） |
| 21 | countingSort/place | 将数值放回原数组的正确位置 | 命中 `int index = 0;`（L8）；真实写入在 L11 `arr[index++] = i;` | 排除"右侧为字面量"的赋值；或要求出现 `index++` |
| 80 | radixSort/count | 统计当前位各数字出现次数 | 命中**清零循环** L12 `for (i<10) count[i] = 0;`；真实统计在 L13 | 排除右侧为字面量 0 的 `count[...] =` 形态 |
| 81 | radixSort/digit_loop | 按第 1 位进行分配-收集 | `exp` 是**位权**（1 / 10 / 100），文案当"第几位"打印 → 实测出现 `按第 10 位` / `按第 100 位` | 改为位序：`第 log10(exp)+1 位` 或个位/十位/百位 |
| 79 | quick/recursive | 递归调用 quickSort，处理子子数组 [left=0, right=4] | ① 首现落在 **main 的顶层调用** L31，却称"递归调用…处理子子数组"；② "子子数组"是错字；③ 家族内含 `[left=0, right=-1]`、`[left=3, right=2]`、`[left=5, right=4]` 等**空区间/反区间** | 顶层调用与函数体内递归分开表述；屏蔽 `left > right` 的帧 |
| 50 | hanoi/recursive | 递归移动 2 个盘子 | 首现落在 **main 调用点** L15（n=3），文案 `math.rs:87` 恒减一 → "2 个盘子"（应 3）；函数体内（L8）减一才正确 | 调用点用 `n`，函数体内用 `n-1` |
| 19 | computeNextVal/build_next | 构建 next 数组，next[0]=-1 | 首现是 **main 的调用行** L35 `getNextVal(T, nextval);`（命中 `contains("getnext")`，`search.rs:157`），数值取自**外层作用域**的 j/k → 是巧合不是"构建首帧" | 区分调用点与 `getNext` 函数体；函数入口 `k` 未初始化时应输出不带数值的"开始构建 next 数组" |
| 96 | stringMatchKMP/build_next | 构建 next 数组，next[0]=0 | 同上（L22 `getNext(T, next);`）；另同族含 `next[-1]=-1`（L4 函数入口） | 同上 |
| 28 | dpCoinChange/outer_loop | 遍历物品 i=12 | 挂在**初始化循环** L7（i 是金额，非物品）；且内层币种循环被误判为 outer_loop（见 P0-3 附注） | 见 P0-3 |
| 31 | dpFib/outer_loop | 遍历物品 i=2 | 斐波那契无"物品"概念 | 文案按算法族泛化：`遍历子问题 i=` |
| 35 | dpLCS/outer_loop | 遍历物品 i=0 | LCS 是字符/子序列，非物品 | 同上 |
| 38 | dpLIS/outer_loop | 遍历物品 i=8 | ① "物品"错；② 挂在**初始化循环** L7（i 收尾值 8） | 同上 + 排除初始化循环 |
| 69 | matrixChain/outer_loop | 遍历物品 i=0 | 矩阵链乘无物品 | 文案改为"枚举链长 i=" |
| 6 | bfs/enqueue | 邻居节点入队 | 首帧实为**起点入队**（L16 `queue[rear++] = start;`） | 首帧用"起点入队"，循环内用"邻居节点入队" |
| 89 | seqList/update_len | 更新表长度 | 挂在**后移循环** L24 `for (i = pos; i < L->length - 1; i++)`；真正的 `L->length--` 在 L26（且整个模板只有 2 条标注，插入/查找部分零标注） | 判据改为 `L->length--` / `L->length++` 形态 |

---

## 4. 清单自身的口径问题（不影响 golden，影响可信度）

| # | 位置 | 问题 | 证据 |
|---|------|------|------|
| 4.1 | §2.1 | **"其余 93 条"的算术不成立**。表内 7 行实际覆盖 **9 条标注**（activitySelection 与 selection 各占 compare+inner_loop 两条），100 − 9 = **91**，不是 93（93 = 100 − 7 行）。 | §2.1 表 vs 主表 |
| 4.2 | §2.1 | **"已从正常审阅项中摘出"未执行**：那 9 条**仍全部留在主表**（第 1、2、40、56、60、75、76、84、85 行）。 | 主表逐行核对 |
| 4.3 | §2.1 | **三类根因被混为"变量取不到值"**。实测至少三种：① 真实变量名不匹配（selection 的 `minIdx`）；② **算法误判**（huffmanTree / activitySelection / externalSort / mergeSortedLists）；③ **初始化行误命中**（primMST `lowcost[0]=0`、countingSort `int index=0`）。②③ 补变量别名是修不好的。 | §2 P0-2、§3 表 |
| 4.4 | §3 | **数量口径过期**：写 `38 覆盖 + 44 零标注`，实测 **36 + 46**；差值恰为 `binarySearchTreeValidation`（P0-1）与 `stringBasicOps`（P0-5）。 | 全量提取：36 个模板有标注 |
| 4.5 | §3 档 A | 漏登 `binarySearchTreeValidation`（同因"收紧致漏检"，且它比 bstInsert 更严重——bst 模板函数名是裸 `insert` 尚属已知代价，`isValidBST` 是**修出来的新漏报**）。 | P0-1 |
| 4.6 | §3 档 C ↔ §4 | **自相矛盾**：`bTree`、`criticalPath` 同时出现在档 C"完全未实现（零标注）"与 §4"截断 7 个"。实测二者 4000 帧零标注，**支持档 C** → §4 的截断名单应删掉这两个。 | 提取结果 |
| 4.7 | §2 口径 | "该 (算法, phase) 首次出现"会稳定抓到三类**非算法体位点**：**函数调用点**（computeNextVal/KMP/dfs/hanoi/quick/merge 的 recursive/build_next/recursive_split 首现全在 `main`）、**初始化行**（primMST/countingSort/radixSort/dp×5）、**顶层调用**（quick/merge）。建议口径改为「该 phase 在**算法函数体内、非调用点、非初始化行**的首次出现」，否则 golden 锚定的第一条就是错位的。 | §3 表逐行 |

---

## 5. 逐行审阅结果（100 行）

判定口径：**✅** = phase 与所挂语句一致且文案语义正确，可进 golden；**✗** = 必须改写（文案/数值/挂载）；
**⛔** = 必须作废（算法判定错 或 该行已失效）。合计 **✅ 62 / ✗ 25 / ⛔ 13**。

| # | 模板 | phase | 判定 | 实测首现挂载 / 依据 |
|---|------|-------|------|---------------------|
| 1 | activitySelection | compare | ⛔ | L7 贪心比较被套 selection_sort 文案；数组实为 start/finish；`arr[6]` 越界 |
| 2 | activitySelection | inner_loop | ⛔ | L6 同上 |
| 3 | avlTree | balance | ✅ | L87 `case 1: LeftBalance(T); break;` |
| 4 | avlTree | update_bf | ✅ | L88 `case 0: (*T)->bf = 1;` |
| 5 | bfs | dequeue | ✅ | L18 `int u = queue[front++];` |
| 6 | bfs | enqueue | ✗ | L16 首帧是**起点**入队，文案说"邻居" |
| 7 | bfs | loop | ✅ | L17 `while (front < rear)` |
| 8 | bfs | visit | ✅ | L15 `visited[start] = 1;` |
| 9 | binary | compare | ✅ | L7 `if (arr[mid] == target)`；target=5 与默认参数一致 |
| 10 | binary | loop | ✅ | L5 `while (left <= right)` |
| 11 | binary | mid_calc | ✅ | L6 `int mid = left + (right - left) / 2;`（P0-1 修复后首现即 mid=2） |
| 12 | binarySearchTreeValidation | compare | ⛔ | **零标注**（P0-1） |
| 13 | binarySearchTreeValidation | create | ⛔ | 同上 |
| 14 | binarySearchTreeValidation | recursive | ⛔ | 同上 |
| 15 | bubble | compare | ✅ | L6 |
| 16 | bubble | inner_loop | ✅ | L5 |
| 17 | bubble | outer_loop | ✅ | L4；"第 1 趟→第 1 大"修正确认有效 |
| 18 | bubble | swap | ✅ | L7 |
| 19 | computeNextVal | build_next | ✗ | L35 调用点，数值系外层变量巧合 |
| 20 | countingSort | collect | ✅ | L9 `for (i<10)` |
| 21 | countingSort | place | ✗ | L8 `int index = 0;`（初始化） |
| 22 | dfs | recursive | ✅ | L24 调用点起手（可接受） |
| 23 | dfs | scan | ✅ | L15 |
| 24 | dfs | visit | ✅ | L13 `visited[u] = 1;` |
| 25 | dijkstra | confirm | ✅ | L23 `visited[u] = 1;`，u=1 与 G[0] 首跳 → dist[1]=2 一致 |
| 26 | dijkstra | relax | ✅ | L25 |
| 27 | dpCoinChange | finish | ✅ | L15 `return dp[amount];` |
| 28 | dpCoinChange | outer_loop | ✗ | L7 初始化循环 + "物品" + 内层币种循环误判为 outer |
| 29 | dpCoinChange | transition | ✗ | L8 `dp[0] = 0;`（初始化） |
| 30 | dpFib | finish | ✅ | L12 `return 0;`（程序末尾，可接受） |
| 31 | dpFib | outer_loop | ✗ | L8；"物品"不成立 |
| 32 | dpFib | transition | ✗ | L6 `dp[0] = 0;`（初始化） |
| 33 | dpKnapsack | outer_loop | ✅ | L13 `for (i<5)` 确为物品循环 |
| 34 | dpKnapsack | transition | ✗ | L15 `dp[i][j] = 0;`（初始化） |
| 35 | dpLCS | outer_loop | ✗ | L10 `for (i<=m)` 是字符循环；"物品"错 |
| 36 | dpLCS | transition | ✗ | L12 `dp[i][j] = 0;`（初始化） |
| 37 | dpLIS | finish | ✅ | L16 `return result;` |
| 38 | dpLIS | outer_loop | ✗ | L7 初始化循环（i 收尾值 8）+ "物品"错 |
| 39 | dpLIS | transition | ✗ | L14 `result = max(result, dp[i]);`；真正转移 L12 未出帧 |
| 40 | externalSort | compare | ⛔ | L14 置换选择被套 selection_sort；`arr[?]` |
| 41 | externalSort | outer_loop | ⛔ | L7 装填堆循环被标"第 4 趟找最小值" |
| 42 | floyd | outer_loop | ✅ | L6 `for (k<n)` |
| 43 | floyd | relax | ✅ | L9 |
| 44 | gcd | finish | ✅ | L9 `return a;` → 6 正确 |
| 45 | gcd | loop | ✅ | L4 `while (b != 0)` |
| 46 | gcd | mod | ✗ | L6；应为 `48 % 18 = 12`（P0-4） |
| 47 | hanoi | base | ✅ | L4 `if (n == 1)` |
| 48 | hanoi | finish | ✅ | L6 `return;` |
| 49 | hanoi | move | ✅ | L5 printf |
| 50 | hanoi | recursive | ✗ | L15 调用点 n=3 却报"2 个盘子" |
| 51 | hashTable | hash | ✅ | L10 `return key % TABLE_SIZE;` |
| 52 | heapSort | build_heap | ✅ | L20 `for (i = n/2 - 1; …)` |
| 53 | heapSort | extract | ✅ | L22 `for (i = n-1; …)` |
| 54 | heapSort | heapify | ✅ | L21 `heapify(arr,n,i);` |
| 55 | heapSort | swap | ✅ | L12 `int temp = arr[i];` |
| 56 | huffmanTree | compare | ⛔ | L16 `Select()` 被套 selection_sort；`arr[?]` |
| 57 | huffmanTree | outer_loop | ⛔ | L15 同上 |
| 58 | huffmanTree | merge | ✅ | L37 `HT[s1].parent = i;`（huffman_tree 判定正确） |
| 59 | insertion | inner_loop | ✅ | L7 `while (j >= 0 && arr[j] > key)` |
| 60 | insertion | insert | ✗ | L5 `int key = arr[i];`（读操作）→ `?` |
| 61 | insertion | outer_loop | ✅ | L4 |
| 62 | kruskalMST | add_edge | ✅ | L39 `Union(parent,u,v);` |
| 63 | kruskalMST | check_cycle | ✅ | L37 `Find(parent,u) != Find(parent,v)` |
| 64 | kruskalMST | sort | ✅ | L26 `if (edges[j].w < edges[min].w)` |
| 65 | levelOrder | dequeue | ✅ | L26 |
| 66 | levelOrder | enqueue | ✅ | L24 `queue[rear++] = root;` |
| 67 | levelOrder | enqueue_left | ✅ | L28 |
| 68 | levelOrder | enqueue_right | ✅ | L29 |
| 69 | matrixChain | outer_loop | ✗ | L5 `for (i<n)`；"物品"错，应"枚举链长" |
| 70 | matrixChain | transition | ✗ | L7 `dp[i][j] = 0;`（初始化） |
| 71 | merge | merge | ✅ | L7（拷贝左半循环，文案通用可接受） |
| 72 | merge | recursive_split | ✅ | L30 main 顶层调用 [0,4] |
| 73 | mergeSortedLists | finish | ⛔ | L32 `return dummy.next;` 挂"归并排序完成" |
| 74 | mergeSortedLists | merge | ⛔ | L20 链表归并被判 merge_sort（"子数组"应为"子链表"） |
| 75 | mergeSortedLists | recursive_split | ⛔ | L51 main 调用行；`left/right` 取不到 → `[-1,-1]` |
| 76 | primMST | add_vertex | ✗ | L8 `lowcost[0] = 0;`（初始化）→ 顶点 -1 |
| 77 | primMST | update | ✅ | L26 |
| 78 | quick | partition_init | ✅ | L5 `int pivot = partition(arr, low, high);`（"选取枢轴"用分区返回值，低危） |
| 79 | quick | recursive | ✗ | L31 顶层调用被称"递归…子子数组"；家族含反区间 |
| 80 | radixSort | count | ✗ | L12 清零循环 |
| 81 | radixSort | digit_loop | ✗ | L11 `exp` 当位数 → `第 10 位` / `第 100 位` |
| 82 | radixSort | place | ✅ | L16 |
| 83 | radixSort | prefix | ✅ | L14 |
| 84 | selection | compare | ✗ | L7；`minIdx` 未识别 → `arr[?]`；含 `arr[5]` 越界 |
| 85 | selection | inner_loop | ✗ | L6；`min_idx=?` |
| 86 | selection | outer_loop | ✅ | L4 |
| 87 | selection | swap | ✅ | L10 `int temp = arr[i];` |
| 88 | seqList | finish | ✅ | L27 `return 1;` |
| 89 | seqList | update_len | ✗ | L24 后移循环；真值在 L26 `L->length--` |
| 90 | shellSort | inner_loop | ✅ | L9 `arr[j] = arr[j - gap];` |
| 91 | shellSort | insert | ✅ | L6 `int temp = arr[i];` |
| 92 | shellSort | outer_loop | ✅ | L4 |
| 93 | stringBasicOps | recursive | ⛔ | **零标注**（§1.2 已承认，行未删） |
| 94 | stringMatchBF | backtrack | ✅ | L14 `i = i - j + 1;` |
| 95 | stringMatchBF | compare | ✅ | L10 |
| 96 | stringMatchKMP | build_next | ✗ | L22 调用点；`next[0]=0` 系外层变量巧合 |
| 97 | stringMatchKMP | compare | ✅ | L26 |
| 98 | topologicalSort | decrease | ✅ | L21 `indegree[v]--;` |
| 99 | topologicalSort | enqueue | ✅ | L14 |
| 100 | topologicalSort | output | ✅ | L17 `int u = queue[front++];` |

**另有 2 处覆盖缺口（不是"行错"，是"该有的 phase 没有"），建议登记**：

- `huffman_tree` 的 `select` 分支（`vitro_algorithm_steps/src/tree.rs:131` "在森林中选择两个最小权值节点"）**从未触发**——因为 `Select()` 被判定成 `selection_sort`，走的是 selection_sort 的模板；
- `seqList` 全程只有 2 条标注：`listInsert` 的后移/写值、`listFind` 的查找零标注（模板演示的三类操作只覆盖了删除一半）。

---

## 6. 建议的修复顺序（golden 固化前置）

1. **P0-1**（改 `has_word` + 补单测）→ 让 `binarySearchTreeValidation` 复亮；顺带给"零标注模板"加负向断言防再回归。
2. **P0-2**（四处误判）→ 至少让选择排序判据带上"证据要求"，把这 13 行从 golden 里剔除。
3. **P0-3**（dp transition 挂初始化行 + `contains("n")` 误判）→ 5 个 dp 模板全体重取。
4. **P0-4**（`mod` 型 phase 需要"语句前操作数"）→ 这是唯一一个**改口径**而非改判据的修复，建议先定规则再改代码。
5. **P1 工具层小改**（成本低、收益明确）：`minIdx` 别名、selection_sort 的越界守卫、insertion `insert` 收紧、primMST `lowcost[k]`、countingSort/radixSort 排除初始化行、radixSort 位序文案、hanoi 调用点不减一、`arr` 改用实际数组名。
6. **重提取 → 再对账**（本轮脚本可直接复用），然后才进入 §5 "勾选 → golden JSON → CI 收集比对"。
   golden 建议按 `(模板, phase) → 期望描述集合（数值位占位符）` 组织，并把"零标注"也作为可断言状态。

---

## 7. 本次复核未覆盖的部分

- 只有 20 余个模板逐条读过 `source.c` 并核对到具体语句；其余 ✅ 判定基于"phase 与所挂语句是否对应 + 文案与算法语义是否自洽"，**未做 Clang 行为对照**。
- §4 的"截断 7 个"本轮**无法验证**：4000 步内所有模板都能持续返回帧且 `err_count = 0`，从帧数无法区分"程序未跑完"与"程序已结束"。
  要验证该名单，需要 serve 侧补一个明确的"程序结束"信号（或让 `step.next` 在结束后返回终止帧）。
