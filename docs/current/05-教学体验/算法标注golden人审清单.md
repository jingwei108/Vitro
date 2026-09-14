# 算法标注 golden 人审清单（防线 6 · U1#1 ①，**v5 2026-09-14**）

## 0. 当前状态

- **基线 = v3 golden**（`native/tests/golden/algorithm_annotations_v3.json`，已接 CI）：
  **82 个 C 模板 = 37 有标注（310 条首现 / 113 个 (模板, phase) 键）+ 45 零标注**，0 错误帧。
- **v4 → v5 两轮**：
  1. **v4**：按《算法标注golden审阅意见三审20260914.md》§5 完成 8 项文档修复，并在重建后的
     表上**以审阅人身份逐行判定**——首判 **✅ 94 / ✗ 11 / ⛔ 8**。
  2. **v5（本版）**：§6 代码侧两批落地后（提交 `a528a4a` / `9cd1aa8`，含 golden 增 `algorithm` 归属），
     **对 8 个 ⛔ 键按算法复判**，并复判 11 个 ✗ 键与 2 个新词条——结果
     **✅ 110 / ✗ 3 / ⛔ 0**（113 键 / 310 条）。
- **golden 定位（不变）**：三审修复链后的**行为基线快照，不是语义认证**。本表的人审结论
  与 golden 的差异即"基线已知偏差点"清单（§2），更新 golden 须附红→绿锚。
- **口径（钉死，消除历史歧义）**：
  1. 提取单位 = **每 `(phase, description)` 首现**（golden 的存储单位，本表「条」列之和 = 310）；
     本表**按 `(模板, phase)` 折叠成一行**，「首现 description」列 = 该键下最先出现的那条。
  2. 取帧路径 = **serve 的 `step.next`**（学生端同款，含一帧延迟发布）；
     与 `payload.get`（引擎帧缓存）结果可能不同，**勿混作一谈**。
  3. 「行末帧」= 同一语句多帧中语句完成态那一帧，由引擎（run_batch 批内去重 +
     `demote_row_annotations`）与协议层（step_next 一帧发布缓冲）双层保证。
- **判定规则**（审阅人自定，供复核）：
  ① phase 是否符合算法教学阶段划分；
  ② description 与**所挂语句**是否一致（数值取行末帧，但术语/语境仍可能错）；
  ③ 挂载点必须落在**算法体真语句**上。**顶层调用帧**（caller=main、`func_name`=被调函数、
  `code_line` 仍为 main 调用行）仅在文案为**入口语义**（启动／调用／从 X 开始）时可接受；
  若文案描述被调函数的**内部分支或多步行为**，或写成**问题陈述**，判 ✗；
  ④ 有无明显缺 phase（单独登记于 §5，不阻断）。

### 0.1 本版修复动作（对照三审意见 §5）

| # | 三审意见 | 本版处置 |
|---|----------|----------|
| 1 | §2 主表与 v3 不同源（38×100 vs 37×113），照表勾选会固化已修缺陷 | **按 v4 提取全量重生成**（§2）；列含 算法归属 / 条数 / 行号 / 质检 |
| 2 | §2.1 质检与主表分裂（"表说能勾、质检说不能"） | **质检并入同表「质检」列**，旧 §2.1 表删除 |
| 3 | §3 计数与档位过期（44→45、档 A 4→1、档 C 40→44） | §3 全量改写，算术 `88 = 37 + 45 + 6` |
| 4 | §4 截断 7 个（含非截断的 bTree）、matrixChain i 值、CI 覆盖面未声明 | §5 改写：截断 6 个，补「golden 与 4000 步预算绑定」「CI 仅覆盖 82 C 模板」 |
| 5 | 四层头注数字互斥（37+45 / 36+46 / 38+44 / 34+48） | 头注收敛为本节，历史三批移入**附录 A（冻结·只读）** |
| 6 | §3 档 A 建议①"改函数名对 golden 无影响"错误 | §3 改为「须同步更新 golden 并附红→绿锚」（golden 存 `src`） |
| 7 | §6 引用不存在的「§1.2」 | §7 改为「附录 A.2」 |
| 8 | golden 的 `algorithm` 归属不可见（bstSearch/bstDelete 混流无从判断） | v4 先跑 `tmp/annot_extract_v4_20260914.json` 取回 `algorithm_name / display_name / func_name`（表内「算法」列与 M4 标记由此而来）；**v5 复核时该字段已由 `a528a4a` 正式写入 golden**（§0.2） |

> **v4 只动文档**（+ 临时探针 `tmp/annot_extract_v4.py`）；**v5 不含任何代码改动**——
> §6 代码侧由提交 `a528a4a`（golden 增归属 + 终态末帧重放根治 + dp 初始化内层排除）、
> `9cd1aa8`（dp 子族具名 / insertion / hanoi-finish / topo-output / quick-partition / KMP nextval）落地，
> 本轮只看结果、不碰代码。

### 0.2 复判（v5，2026-09-14）

| 复判对象 | 依据 | 结果 |
|----------|------|------|
| **8 个 ⛔ 键**（`bstSearch`/`bstDelete` × `recursive`/`create`/`compare`/`finish`） | golden 已增 `algorithm`/`display_name`（`a528a4a`），可按算法分行看 | **全部降为 ✅**——每条文案与其 `algorithm_name` 自洽（§1.2） |
| 11 个 ✗ 键 | `9cd1aa8` 自报"人审 ✗ 键全数处置" | **7 键转 ✅**、**2 键整体消失**（`dpKnapsack`/`matrixChain` 的 `inner_loop` 缺陷键被移除）、**2 键仍 ✗**（`9cd1aa8` 的处置清单未覆盖） |
| 2 个新词条 | 本轮改动新引入 | `topologicalSort/dequeue` ✅；`computeNextVal/build_nextval` **✗（复判新发现，见 §1.1）** |

**基线一致性复核**：`tmp/annot_extract_v5_20260914.json`（当前 release 构建 × 4000 步）
与当前 golden **逐条 0 差异**——310 条 / 113 键 / 37 模板，CI 基线可复现。

## 1. 审阅统计与结论

| 判定 | 键数 | 含义 |
|------|------|------|
| ✅ 可进 golden | **110** | phase 与所挂语句一致、文案语义正确（含可接受的顶层调用帧） |
| ✗ 必须改写 | **3** | 文案/挂载行/相位有误——**不阻断 golden 存在，但学生所见文案应修** |
| ⛔ 必须作废或移出 | **0** | （v4 曾为 8 键 bst 归属不可表达；v5 复判后**归零**） |
| 合计 | **113** | 与当前 golden 的 (模板, phase) 键数一致 |

**一句话结论**：基线可信且可复现（逐条 0 差异）；**⛔ 归零**——bst 归属已由 golden 的
`algorithm` 字段表达清楚，那 8 键是"建树准备阶段"的帧，内容与归属都正确。

> **§1.1 的 3 个 ✗ 键已由代码侧第三批全部修复（2026-09-14 同日，红→绿：
> golden 漂移 4 处 → 重提取更新 311 条 → CI 复绿）**：① `build_nextval`
> 下标/值改从**行文本**解析（`nextval[0]=-1`——此前用变量 j 填下标位，
> j 取到的是右侧的值）；② `bst_validate/recursive` 顶层调用帧（传 env，
> at_callee_entry + caller_is_main 与 quick/merge 同款区分）给入口语义
> "启动校验：从根节点开始…"，体内行保持原递归文案（键内分离为两条变体，
> 310→311 条）；③ `hanoi/recursive` 顶层调用帧改"从 A 柱移动 3 个盘子到
> C 柱"（char 形参按 i32 存、还原为字符；取不到降级不带柱名）。
> **113 键至此全部 ✅（人审 + 代码侧闭环）**。

### 1.1 ✗ 必须改写（3 键）

- **`computeNextVal/build_nextval`（复判新发现）**：「构建 nextval 数组，**nextval[-1]**」挂在
  L23 `nextval[0] = -1;`——**下标位被填成了值**（-1 是值，下标应为 0）；且该词条只给下标不给值
  （`nextval[2]`/`nextval[3]`…），与 `构建 next 数组，next[N]=V` 的格式不一致。
  建议统一为 `构建 nextval 数组，nextval[0]=-1`。
- **`binarySearchTreeValidation/recursive`（旧判未处置）**：首现仍挂 L32 顶层调用行（M3），
  文案仍描述递归体内两分支（L22/L23 区间收窄）——文案与所挂语句不符。
  建议改入口语义，或把首现挂到 L22 的递归调用行。
- **`hanoi/recursive`（旧判未处置）**：#5 只改了 `finish`；首现仍是 L15 顶层调用帧（M3）上的
  **问题陈述**「移动 3 个盘子的汉诺塔问题」。建议改入口语义「从 A 柱移动 3 个盘子到 C 柱」。

**已由 §6 代码侧消除的旧 ✗（9 键：7 转 ✅ + 2 键消失，留痕备查）**：dp 初始化循环挂载 ×3
（`dpKnapsack`/`matrixChain` 的 `inner_loop` **缺陷键整体消失**，`dpLCS/inner_loop` 首现移回真内层 L14）、
`topologicalSort/output` 挂载点、`hanoi/finish` 文案、`insertion/insert` 两态、
`quick/partition_init` 值/下标、`dpCoinChange/outer_loop` 具名、`computeNextVal/build_next` nextval 混入。
**仍 ✗ 的 2 键**（`binarySearchTreeValidation/recursive`、`hanoi/recursive`）不在 `9cd1aa8` 的处置范围内。

### 1.2 ⛔ 已消除（8 键 → ✅）

v4 判 ⛔ 的唯一理由是**归属不可表达**：`bstSearch`(4) + `bstDelete`(4) 的
`recursive / create / compare / finish` 首现落在 **main 里 `insert()` 建树阶段**的帧上
（`func_name=insert`、`algorithm_name=bst_insert`），内容本身没错——学生的程序确实在建树。

v5 复判（golden 已有 `algorithm` 字段）逐条核对：

| 键 | 条目 | 判定 |
|----|------|------|
| `bstSearch/recursive` | [bst_insert] @L37 建树 ／ [bst_search]「递归进入子树查找」@L42 | ✅ 两条各自自洽 |
| `bstSearch/compare` | [bst_insert] @L20 ／ [bst_search]「比较关键字与当前节点值」@L29 | ✅ |
| `bstDelete/recursive` | [bst_insert] @L64 ／ [bst_delete]「递归在子树中定位并删除目标节点」@L71 | ✅ |
| `bstDelete/compare` | [bst_insert] @L20 ／ [bst_delete]「比较目标关键字与当前节点值，决定递归方向」@L34 | ✅ |
| `bstSearch`/`bstDelete` × `create` / `finish` | 仅 [bst_insert]（建树准备阶段） | ✅ 内容与归属一致 |

> **残留观察（不判缺陷，供消费方考虑）**：4 个单归属键（`bstSearch`/`bstDelete` 的
> `create`/`finish`）只承载 bst_insert 的建树语义，且**键级首现仍是建树帧**——因此按
> `(模板, phase)` 排序展示时，bstSearch/bstDelete 的前几条会是"插入"。golden 已能表达
> 归属，**消费方应按 `algorithm` 分组**（或把准备阶段单独呈现）；`bst_search`/`bst_delete`
> 自身无 `create`/`finish` phase 已登记在 §4。

## 2. 主审表（113 键 / 310 条）

列说明：「算法」= 该键下所有帧的 `algorithm_name`（多个即"一键多算法"，见 M4 注，非缺陷）；「条」= 该键下不同
`(phase, description)` 数；「首现 description」= 该键最先出现的那条文案；「行」= 首现挂载行号；
「质检」= 机器可判标记（M1 未解析值 `?` / M2 结构位点哨兵 `-1` / M3 顶层调用帧 /
M4 一键多算法（v5 起按"**非缺陷**"读：同一模板的不同阶段由不同算法产出标注，
见下 M5 注）/ M5 同键文案多态）；「审阅」= 本次判定。

| # | 模板 | 算法 | phase | 条 | 首现 description | 行 | 质检 | 审阅 | 备注 |
|---|------|------|-------|----|------------------|----|------|------|------|
| 1 | `avlTree` | avl_tree | `balance` | 1 | 平衡因子失衡，进行平衡调整 | 87 | — | ✅ | L87 `case 1: LeftBalance(T);` 左失衡分支 ✓；缺 LR/RL 双旋分型 phase |
| 2 | `avlTree` | avl_tree | `update_bf` | 1 | 更新平衡因子 | 88 | — | ✅ | L88 `case 0: (*T)->bf = 1;` ✓ |
| 3 | `bfs` | bfs | `visit` | 5 | 标记节点 0 为已访问 | 15 | — | ✅ | L15 `visited[start] = 1;` ✓ |
| 4 | `bfs` | bfs | `enqueue` | 2 | 起点入队 | 16 | M5 文案多态 | ✅ | L16 `queue[rear++] = start;`，首现「起点入队」与行一致；变体「邻居节点入队」挂 L20 ✓（M5 良性） |
| 5 | `bfs` | bfs | `loop` | 5 | 队列非空，继续广度优先搜索 [front=0, rear=1] | 17 | — | ✅ | L17 `while (front < rear)` ✓ |
| 6 | `bfs` | bfs | `dequeue` | 5 | 出队节点 u=0 | 18 | — | ✅ | L18 `int u = queue[front++];` ✓ |
| 7 | `binary` | binary_search | `loop` | 1 | 搜索范围 [0, 4] | 5 | — | ✅ | L5 `while (left <= right)` ✓ |
| 8 | `binary` | binary_search | `mid_calc` | 1 | 计算中点 mid=2 | 6 | — | ✅ | L6 `int mid = left + (right - left) / 2;`，首现 mid=2 ✓（P0-1 已修） |
| 9 | `binary` | binary_search | `compare` | 1 | arr[2] 与目标值 5 比较 | 7 | — | ✅ | L7 `if (arr[mid] == target)`，target=5 与默认参数一致 ✓；默认参数一次命中，found/narrow_left 未出现（覆盖面） |
| 10 | `binarySearchTreeValidation` | bst_validate | `recursive` | 1 | 递归校验子树：左子树区间收窄为 (min, val)，右子树为 (val, max) | 32 | M3 顶层调用帧 | ✗ | **复判仍 ✗（未处置）**：首现仍挂 L32 顶层调用行（M3），文案仍描述递归体内两分支（L22/L23 区间收窄），与所挂语句不符。建议改入口语义「校验整棵树：以 (INT_MIN-1, INT_MAX+1) 开区间开始」，或把首现挂到 L22 递归调用行 |
| 11 | `binarySearchTreeValidation` | bst_validate | `empty_valid` | 1 | 空子树不违反 BST 性质，判定合法 | 20 | — | ✅ | L20 `if (root == NULL) return 1;` ✓ |
| 12 | `binarySearchTreeValidation` | bst_validate | `range_check` | 1 | 校验当前节点值：必须严格落在开区间 (min, max) 内，越界即非法 | 21 | — | ✅ | L21 `if (root->val <= min \|\| root->val >= max) return 0;` ✓；缺「校验通过/非法」结论 phase |
| 13 | `bstDelete` | bst_delete/bst_insert | `recursive` | 2 | 递归查找插入位置 | 64 | M3 顶层调用帧、M4 一键多算法、M5 文案多态 | ✅ | **复判（golden 已带归属）**：2 条各自归属——[bst_insert]「递归查找插入位置」@L64（建树）、[bst_delete]「递归在子树中定位并删除目标节点」@L71 ✓。原 ⛔ 的根因是归属不可见，现降为 ✅ |
| 14 | `bstDelete` | bst_insert | `create` | 1 | 找到空位，创建新节点 | 19 | — | ✅ | **复判**：唯一条目归属 bst_insert（建树准备阶段），内容与归属一致 ✓。注：bst_delete 自身无 create phase（§4） |
| 15 | `bstDelete` | bst_delete/bst_insert | `compare` | 2 | 比较插入值与当前节点值，决定向左或向右 | 20 | M4 一键多算法、M5 文案多态 | ✅ | **复判**：2 条各自归属——[bst_insert] @L20、[bst_delete]「比较目标关键字与当前节点值，决定递归方向」@L34 ✓ |
| 16 | `bstDelete` | bst_insert | `finish` | 1 | BST 插入完成 | 24 | — | ✅ | **复判**：唯一条目归属 bst_insert（「BST 插入完成」@L24）✓。注：bst_delete 自身无 finish phase（§4） |
| 17 | `bstDelete` | bst_delete | `not_found` | 1 | 搜索到空节点，目标不存在 | 33 | — | ✅ | L33 `if (root == NULL) return NULL;` ✓ |
| 18 | `bstDelete` | bst_delete | `single_child` | 1 | 判断孩子情况：叶子或单孩子节点可直接摘除 | 39 | — | ✅ | L39 `if (root->left == NULL) {` ✓ |
| 19 | `bstDelete` | bst_delete | `find_successor` | 1 | 双孩子情况：找右子树最小节点作为中序后继 | 48 | — | ✅ | L48 `struct TreeNode* temp = findMin(root->right);` ✓ |
| 20 | `bstDelete` | bst_delete | `replace` | 1 | 用中序后继的值替换被删节点的值 | 49 | — | ✅ | L49 `root->val = temp->val;` ✓ |
| 21 | `bstDelete` | bst_delete | `free` | 1 | 摘除并释放被删节点 | 41 | — | ✅ | L41 `free(root);`（单孩子/叶子分支内）✓ |
| 22 | `bstInsert` | bst_insert | `recursive` | 1 | 递归查找插入位置 | 36 | M3 顶层调用帧 | ✅ | L36 `root = insert(root, 5);` 顶层调用帧，文案为入口语义 → 可接受（M3）；建议改「顶层调用：向树插入 5」更贴行 |
| 23 | `bstInsert` | bst_insert | `create` | 1 | 找到空位，创建新节点 | 19 | — | ✅ | L19 `if (root == NULL) return createNode(val);` ✓ |
| 24 | `bstInsert` | bst_insert | `compare` | 1 | 比较插入值与当前节点值，决定向左或向右 | 20 | — | ✅ | L20 `if (val < root->val)` ✓ |
| 25 | `bstInsert` | bst_insert | `finish` | 1 | BST 插入完成 | 24 | — | ✅ | L24 `return root;` ✓（每次递归返回都触发，文案「BST 插入完成」偏强，轻微） |
| 26 | `bstSearch` | bst_insert/bst_search | `recursive` | 2 | 递归查找插入位置 | 37 | M3 顶层调用帧、M4 一键多算法、M5 文案多态 | ✅ | **复判（golden 已带归属）**：2 条各自归属——[bst_insert] @L37（建树）、[bst_search]「递归进入子树查找」@L42 ✓。原 ⛔ 降为 ✅ |
| 27 | `bstSearch` | bst_insert | `create` | 1 | 找到空位，创建新节点 | 19 | — | ✅ | **复判**：唯一条目归属 bst_insert（建树准备阶段）✓——搜索本身不创建节点，故该 phase 只出现在准备阶段。注：bst_search 自身无 create/finish（§4） |
| 28 | `bstSearch` | bst_insert/bst_search | `compare` | 2 | 比较插入值与当前节点值，决定向左或向右 | 20 | M4 一键多算法、M5 文案多态 | ✅ | **复判**：2 条各自归属——[bst_insert] @L20、[bst_search]「比较关键字与当前节点值」@L29 ✓ |
| 29 | `bstSearch` | bst_insert | `finish` | 1 | BST 插入完成 | 24 | — | ✅ | **复判**：唯一条目归属 bst_insert（@L24）✓ |
| 30 | `bstSearch` | bst_search | `hit` | 1 | 找到目标节点 | 28 | — | ✅ | L28 `if (root == NULL \|\| root->val == key) return root;` ✓；该行同时含「未命中」，缺 miss/not_found phase |
| 31 | `bubble` | bubble_sort | `outer_loop` | 4 | 第 1 趟：将第 1 大的元素放到正确位置 | 4 | — | ✅ | L4 `for (i < n - 1)` ✓（4 变体：第 1–4 趟） |
| 32 | `bubble` | bubble_sort | `inner_loop` | 4 | 内层循环 j=0，比较相邻元素 | 5 | — | ✅ | L5 `for (j < n - i - 1)` ✓ |
| 33 | `bubble` | bubble_sort | `compare` | 4 | 比较 arr[0] 与 arr[1] | 6 | — | ✅ | L6 `if (arr[j] > arr[j + 1])` ✓ |
| 34 | `bubble` | bubble_sort | `swap` | 4 | 交换 arr[0]↔arr[1]，较大的元素向右移动 | 7 | — | ✅ | L7 `int temp = arr[j];` 文案「较大的元素向右移动」与冒泡方向一致 ✓ |
| 35 | `computeNextVal` | string_match_kmp | `build_nextval` | 10 | 调用构建 nextval 数组 | 35 | M3 顶层调用帧、M5 文案多态 | ✗ | **#7 新引入的词条，但文案有值/下标错位**：10 条中 `构建 nextval 数组，nextval[-1]` 挂在 L23 `nextval[0] = -1;`——**下标位被填成了值 -1**（应为 0）；且本词条只给下标不给值（`nextval[2]`/`nextval[3]`…），与 `构建 next 数组，next[N]=V` 的格式不一致。建议统一为 `构建 nextval 数组，nextval[0]=-1`。首现 L35 `getNextVal(T, nextval);` 为调用点起手（入口语义）✓ |
| 36 | `computeNextVal` | string_match_kmp | `build_next` | 21 | 调用构建 next 数组 | 21 | M5 文案多态 | ✅ | **复判**：nextval 段已分离到新词条 `build_nextval`（#7），本键 21 条全部落在 getNext 内真语句（L6/L12/L14）或调用点起手（首现 L21 `getNext(T, next);`，入口语义）✓，原 ✗ 消除。残留观察：L25 `if (T[j] == T[next[j]])` 上的「构建 next 数组，next[#]=-1」展示的是**读到的 next 值**，建议文案改「读取 next[j]=-1」 |
| 37 | `countingSort` | counting_sort | `collect` | 1 | 按数值从小到大收集元素 | 9 | — | ✅ | L9 `for (i < 10)` 值域遍历 ✓；缺 count（L5-6 统计频次）phase |
| 38 | `countingSort` | counting_sort | `place` | 1 | 将数值放回原数组的正确位置 | 11 | — | ✅ | L11 `arr[index++] = i;` ✓（一审指出的 L8 初始化行已修） |
| 39 | `dfs` | dfs | `recursive` | 5 | 递归深入：从节点 0 继续深度优先搜索 | 24 | M3 顶层调用帧 | ✅ | L24 `dfs(0, n);` 顶层调用帧，文案为入口语义 → 可接受（承一审） |
| 40 | `dfs` | dfs | `visit` | 5 | 标记节点 0 为已访问 | 13 | — | ✅ | L13 `visited[u] = 1;` ✓ |
| 41 | `dfs` | dfs | `scan` | 5 | 扫描节点 0 的邻居 | 15 | — | ✅ | L15 `for (v < n)` ✓ |
| 42 | `dijkstra` | dijkstra | `confirm` | 4 | 顶点 1 的最短距离已确定 | 23 | — | ✅ | L23 `visited[u] = 1;`，首现 u=1 ✓ |
| 43 | `dijkstra` | dijkstra | `relax` | 1 | 松弛操作，更新邻接顶点距离 | 25 | — | ✅ | L25 松弛条件行 ✓；缺 select_min（L16-20 找最近点）phase |
| 44 | `dpCoinChange` | dp | `outer_loop` | 3 | 遍历币种 i=0 | 9 | — | ✅ | **复判**：文案已具名为「遍历币种 i=0/1/2」✓（#3 dp 子族具名，`dp_outer_subject` 特征词），原 ✗ 消除 |
| 45 | `dpCoinChange` | dp | `inner_loop` | 11 | 遍历金额 j=1 | 10 | — | ✅ | **复判**：文案已具名为「遍历金额 j=1..11」✓（#3）。j 索引 dp[j]，语义正确且信息量提升 |
| 46 | `dpCoinChange` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 11 | — | ✅ | L11 `dp[j] = min(dp[j], dp[j - coins[i]] + 1);` ✓ |
| 47 | `dpCoinChange` | dp | `finish` | 1 | 动态规划计算完成 | 15 | — | ✅ | L15 `return dp[amount];` ✓；缺 init（L7-8）与无解分支（L14）phase |
| 48 | `dpFib` | dp | `outer_loop` | 9 | 遍历子问题 i=2 | 8 | — | ✅ | L8 `for (i = 2; i <= n; i++)`，i 索引 dp[i] ✓ |
| 49 | `dpFib` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 9 | — | ✅ | L9 `dp[i] = dp[i - 1] + dp[i - 2];` ✓ |
| 50 | `dpFib` | dp | `finish` | 1 | 动态规划计算完成 | 12 | — | ✅ | L12 `return 0;`（算法内联在 main，程序末尾）✓ |
| 51 | `dpKnapsack` | dp | `outer_loop` | 2 | 遍历物品 i=1 | 18 | — | ✅ | **复判**：文案已具名为「遍历物品 i=1/2」✓（#3，`wt[` 特征词）。原 `inner_loop` 键（挂 dp 初始化双层循环）已随 #2 初始化内层排除**整体消失**——该 ✗ 属「缺陷键消失」而非改写；dpKnapsack 现为 2 键 |
| 52 | `dpKnapsack` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 23 | — | ✅ | 首现落 L23 `dp[i][w] = dp[i - 1][w];`（放不下的退化转移）；主转移 L21 未作首现 |
| 53 | `dpLCS` | dp | `outer_loop` | 4 | 遍历子问题 i=1 | 13 | — | ✅ | L13 `for (i = 1; i <= m; i++)` ✓ |
| 54 | `dpLCS` | dp | `inner_loop` | 6 | 遍历子问题维度 j=1 | 14 | — | ✅ | **复判**：首现已从初始化内层 L11 移到真内层 L14 `for (j = 1; j <= n; j++)`，「遍历子问题维度 j」成立 ✓（#2） |
| 55 | `dpLCS` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 18 | — | ✅ | L18 `dp[i][j] = max(dp[i-1][j], dp[i][j-1]);`（else 分支）；相等分支 L16 未作首现 |
| 56 | `dpLIS` | dp | `outer_loop` | 7 | 遍历子问题 i=1 | 9 | — | ✅ | L9 `for (i = 1; i < n; i++)` ✓ |
| 57 | `dpLIS` | dp | `inner_loop` | 7 | 遍历子问题维度 j=0 | 10 | — | ✅ | L10 `for (j = 0; j < i; j++)` ✓ |
| 58 | `dpLIS` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 12 | — | ✅ | L12 `dp[i] = max(dp[i], dp[j] + 1);` ✓ |
| 59 | `dpLIS` | dp | `finish` | 1 | 动态规划计算完成 | 16 | — | ✅ | L16 `return result;` ✓ |
| 60 | `floyd` | floyd | `outer_loop` | 4 | 枚举中间顶点 k=0 | 6 | — | ✅ | L6 `for (k < n)` ✓ |
| 61 | `floyd` | floyd | `relax` | 1 | 检查经 k 中转是否更短 | 9 | — | ✅ | L9 `if (G[i][k] + G[k][j] < G[i][j])` ✓；截断，缺 finish |
| 62 | `gcd` | gcd | `loop` | 3 | 辗转相除：a=48, b=18 | 4 | — | ✅ | L4 `while (b != 0)` ✓ |
| 63 | `gcd` | gcd | `mod` | 3 | 计算 48 % 18 = 12（余数作为新的 b） | 6 | — | ✅ | L6 `b = a % b;`，首现「计算 48 % 18 = 12」✓（P0-4 行入口操作数修复生效） |
| 64 | `gcd` | gcd | `finish` | 1 | 最大公约数为 6 | 9 | — | ✅ | L9 `return a;` ✓ |
| 65 | `hanoi` | hanoi | `recursive` | 3 | 移动 3 个盘子的汉诺塔问题 | 15 | M3 顶层调用帧、M5 文案多态 | ✗ | **复判仍 ✗（未处置）**：#5 只改了 `finish`；首现仍是 L15 顶层调用帧（M3）上的**问题陈述**「移动 3 个盘子的汉诺塔问题」。变体「递归移动 N 个盘子」@L8 正确。建议首现改入口语义「从 A 柱移动 3 个盘子到 C 柱」 |
| 66 | `hanoi` | hanoi | `base` | 1 | 基准情况：直接把盘子从起始柱移到目标柱 | 4 | — | ✅ | L4 `if (n == 1) {` ✓ |
| 67 | `hanoi` | hanoi | `move` | 3 | 移动第 1 个盘子 | 5 | — | ✅ | L5 `printf("Move disk 1 ...")` ✓（变体 L9 为第 2/3 个盘子） |
| 68 | `hanoi` | hanoi | `finish` | 1 | 该层递归结束，返回上一层 | 6 | — | ✅ | **复判**：文案已按 `func_name` 区分——体内 return 报「该层递归结束，返回上一层」@L6 ✓（#5），原 ✗ 消除 |
| 69 | `hashTable` | hash_table | `hash` | 1 | 计算哈希值 | 10 | — | ✅ | L10 `return key % TABLE_SIZE;` ✓；该模板仅 1 个 phase，覆盖面极窄 |
| 70 | `heapSort` | heap_sort | `build_heap` | 1 | 建堆：自底向上将数组调整为最大堆 | 20 | — | ✅ | L20 `for (i = n/2 - 1; i >= 0; i--)` ✓ |
| 71 | `heapSort` | heap_sort | `heapify` | 1 | 递归堆化：确保子树满足堆性质 | 21 | — | ✅ | L21 `heapify(arr, n, i);` 调用帧，入口语义 ✓ |
| 72 | `heapSort` | heap_sort | `swap` | 1 | 交换元素，调整堆结构 | 12 | — | ✅ | L12 `int temp = arr[i];`（L12-14 三行交换）✓ |
| 73 | `heapSort` | heap_sort | `extract` | 1 | 取出堆顶元素并重新堆化 | 22 | — | ✅ | L22 `for (i = n - 1; i > 0; i--)` ✓ |
| 74 | `huffmanTree` | huffman_tree | `merge` | 1 | 合并两个节点为新树 | 37 | — | ✅ | L37 `HT[s1].parent = i;` ✓；select（选两棵最小树）零触发，属覆盖缺口 |
| 75 | `insertion` | insertion_sort | `outer_loop` | 4 | 第 1 个元素：准备插入到已排序部分 | 4 | — | ✅ | L4 `for (i = 1; i < n; i++)` ✓ |
| 76 | `insertion` | insertion_sort | `inner_loop` | 4 | 元素后移 j=0，为插入腾出位置 | 7 | — | ✅ | L7 `while (j >= 0 && arr[j] > key)` ✓（后移动作在 L8；文案描述循环目的，可接受） |
| 77 | `insertion` | insertion_sort | `insert` | 4 | 将 key=11 插入到正确位置 0 | 11 | — | ✅ | **复判**：4 变体文案已统一为「将 key=K 插入到正确位置 P」（含 P=0）✓（#4 用行入口快照 `prev_vars` 的 j），原 ✗ 消除 |
| 78 | `kruskalMST` | kruskal_mst | `sort` | 1 | 按边权排序 | 26 | — | ✅ | L26 `if (edges[j].w < edges[min].w) min = j;`（选择排序找最小边）✓ |
| 79 | `kruskalMST` | kruskal_mst | `check_cycle` | 1 | 并查集判环 | 37 | — | ✅ | L37 `if (Find(parent, u) != Find(parent, v))` ✓ |
| 80 | `kruskalMST` | kruskal_mst | `add_edge` | 1 | 加入生成树并合并集合 | 39 | — | ✅ | L39 `Union(parent, u, v);` ✓（「加入生成树」的可见动作在 L38 printf，未标注） |
| 81 | `levelOrder` | level_order | `enqueue` | 1 | 根节点入队 | 24 | — | ✅ | L24 `queue[rear++] = root;` ✓ |
| 82 | `levelOrder` | level_order | `dequeue` | 1 | 取出队头节点访问 | 26 | — | ✅ | L26 `struct TreeNode* node = queue[front++];` ✓ |
| 83 | `levelOrder` | level_order | `enqueue_left` | 1 | 左子节点入队 | 28 | — | ✅ | L28 ✓ |
| 84 | `levelOrder` | level_order | `enqueue_right` | 1 | 右子节点入队 | 29 | — | ✅ | L29 ✓；缺「层分界/finish」phase |
| 85 | `matrixChain` | dp | `outer_loop` | 5 | 遍历子问题 i=1 | 9 | — | ✅ | L9 `for (i = 1; i < n - len + 1; i++)` ✓；缺「枚举区间长度 len」（L8）phase。原 `inner_loop` 键（挂 dp 初始化内层）已随 #2 **整体消失** |
| 86 | `matrixChain` | dp | `transition` | 1 | 状态转移：计算当前子问题的最优解 | 13 | — | ✅ | L13 `int cost = dp[i][k] + dp[k+1][j] + p[i-1]*p[k]*p[j];`（候选代价）✓ |
| 87 | `merge` | merge_sort | `recursive_split` | 5 | 启动归并：处理区间 [0, 4] | 30 | M3 顶层调用帧、M5 文案多态 | ✅ | L30 `mergeSort(arr, 0, n - 1);` 顶层调用帧，文案「启动归并：处理区间 [0, 4]」为入口语义 → 可接受；变体「将数组区间 [#, #] 递归分成两半」挂 L21/L22 ✓（M5 良性） |
| 88 | `merge` | merge_sort | `merge` | 1 | 合并两个有序子数组 | 23 | — | ✅ | L23 `merge(arr, left, mid, right);` 调用帧入口语义 ✓；合并体内部（L7-15）无标注 |
| 89 | `primMST` | prim_mst | `add_vertex` | 4 | 顶点 1 加入生成树 | 24 | — | ✅ | L24 `lowcost[k] = 0;` ✓（一审的「顶点 -1」已修） |
| 90 | `primMST` | prim_mst | `update` | 1 | 更新邻接顶点的最小边权 | 26 | — | ✅ | L26 `if (lowcost[j] != 0 && G[k][j] < lowcost[j])` ✓；缺 select_min（L17-21）phase |
| 91 | `quick` | quick_sort | `recursive` | 6 | 启动快速排序：处理区间 [left=0, right=4] | 31 | M3 顶层调用帧、M5 文案多态 | ✅ | L31 `quickSort(arr, 0, n - 1);` 顶层调用帧，文案「启动快速排序：处理区间」入口语义 → 可接受（P1-3/P1-4 收口后合规） |
| 92 | `quick` | quick_sort | `partition_init` | 4 | 分区完成，枢轴落位下标 pivot=0 | 5 | — | ✅ | **复判**：文案已改「分区完成，枢轴落位下标 pivot=N」✓（#6），值/下标语义已正，原 ✗ 消除。残留观察：phase 名仍为 `partition_init`（「初始化」与「已完成」不符）——#6 已裁定「phase 名不动，词汇面改动另行裁定」；partition 体内扫描（L14-20）仍无 phase |
| 93 | `radixSort` | radix_sort | `digit_loop` | 3 | 按第 1 位进行分配-收集 | 11 | — | ✅ | L11 `for (exp = 1; max / exp > 0; exp *= 10)` ✓（第 1/2/3 位三变体） |
| 94 | `radixSort` | radix_sort | `count` | 1 | 统计当前位各数字出现次数 | 13 | — | ✅ | L13 `count[(arr[i] / exp) % 10]++` ✓（P1-2 `]++` 收紧生效） |
| 95 | `radixSort` | radix_sort | `prefix` | 1 | 计算前缀和，确定位置 | 14 | — | ✅ | L14 `for (i = 1; i < 10; i++) count[i] += count[i - 1];` ✓ |
| 96 | `radixSort` | radix_sort | `place` | 1 | 按前缀和放置元素 | 16 | — | ✅ | L16 `output[count[...] - 1] = arr[i];` ✓；缺「回写原数组」（L19）phase |
| 97 | `selection` | selection_sort | `outer_loop` | 4 | 第 0 趟：从第 0 个位置开始找最小值 | 4 | — | ✅ | L4 `for (i < n - 1)` ✓ |
| 98 | `selection` | selection_sort | `inner_loop` | 5 | 扫描 j=1，当前最小值在 min_idx=0 | 6 | — | ✅ | L6 `for (j = i + 1; j < n; j++)` ✓ |
| 99 | `selection` | selection_sort | `compare` | 4 | 比较 arr[1] 与当前最小值 arr[0] | 7 | — | ✅ | L7 `if (arr[j] < arr[minIdx])`，首现「arr[1] 与 arr[0]」✓（`?` 已修） |
| 100 | `selection` | selection_sort | `swap` | 5 | 将最小元素交换到位置 0 | 10 | — | ✅ | L10 `int temp = arr[i];` ✓；末趟「交换到位置 4」实为 i==minIdx 的空操作（5 变体中 1 条退化） |
| 101 | `seqList` | seq_list | `update_len` | 1 | 更新表长度 | 26 | — | ✅ | L26 `L->length--;` ✓ |
| 102 | `seqList` | seq_list | `finish` | 1 | 顺序表操作完成 | 27 | — | ✅ | L27 `return 1;` ✓；表内只覆盖 listDelete，插入/查找未标注 |
| 103 | `shellSort` | shell_sort | `outer_loop` | 2 | 取增量 gap=2，分组进行插入排序 | 4 | — | ✅ | L4 `for (gap = n / 2; gap > 0; gap /= 2)` ✓ |
| 104 | `shellSort` | shell_sort | `insert` | 1 | 保存当前元素，准备在同组内插入 | 6 | — | ✅ | L6 `int temp = arr[i];` ✓ |
| 105 | `shellSort` | shell_sort | `inner_loop` | 1 | 同组内元素后移，腾出插入位置 | 9 | — | ✅ | L9 `arr[j] = arr[j - gap];`（后移语句，非 for 头 L8）✓ 轻微错位 |
| 106 | `stringMatchBF` | string_match_bf | `compare` | 16 | 比较 S[0] 与 T[0] | 10 | — | ✅ | L10 `if (S[i] == T[j])` ✓（16 变体） |
| 107 | `stringMatchBF` | string_match_bf | `backtrack` | 1 | 字符不匹配，主串回溯 | 14 | — | ✅ | L14 `i = i - j + 1;` ✓；缺 match/finish（L18-19）phase |
| 108 | `stringMatchKMP` | string_match_kmp | `build_next` | 8 | 调用构建 next 数组 | 22 | M5 文案多态 | ✅ | L22 `getNext(T, next);` 调用帧入口语义 ✓；变体「构建 next 数组，next[#]=#」挂 L6/L12/L14 ✓ |
| 109 | `stringMatchKMP` | string_match_kmp | `compare` | 1 | 比较主串与模式串字符 | 26 | — | ✅ | L26 `if (j == -1 \|\| S[i] == T[j])` ✓；缺 finish phase |
| 110 | `topologicalSort` | topological_sort | `enqueue` | 1 | 入度为 0 的顶点入队 | 14 | — | ✅ | L14 `if (indegree[i] == 0) queue[rear++] = i;` ✓ |
| 111 | `topologicalSort` | topological_sort | `dequeue` | 1 | 取出队头顶点 | 17 | — | ✅ | **#5 新拆词条**：「取出队头顶点」@L17 `int u = queue[front++];` ✓（出队与输出分离） |
| 112 | `topologicalSort` | topological_sort | `output` | 6 | 输出顶点 0 | 18 | — | ✅ | **复判**：已挂真 printf 行 L18 `printf("%d ", u);` ✓（#5），原 ✗ 消除。残留：缺 init（L8-12 求入度）phase——教学上这是关键阶段 |
| 113 | `topologicalSort` | topological_sort | `decrease` | 1 | 删边，邻接点入度减 1 | 21 | — | ✅ | L21 `indegree[v]--;` ✓ |

> **M4 / M5 怎么读**：**M4**（4 键，均在 bst 家族）＝同一模板里两种算法交替产出标注，
> 归属已入 golden，**不是缺陷**——判据是"该条文案与其 `algorithm_name` 是否自洽"（§1.2 已逐条核）。
> **M5**（11 键，判据 = **去数值化后同键仍有 >1 种文案模板**）分三类：
> - **良性**（入口/语义变体，各自挂在正确语句上）：`bfs/enqueue`（起点入队 L16 ／ 邻居节点入队 L20）、
>   `merge/recursive_split`（启动归并 L30 ／ 将数组区间 [#,#] 递归分成两半 L21）、
>   `quick/recursive`（启动快速排序 L31 ／ 递归调用 quickSort L6）、
>   `hanoi/recursive`（问题陈述 L15 ／ 递归移动 # 个盘子 L8）、
>   `computeNextVal/build_next` 与 `stringMatchKMP/build_next`（调用构建 next 数组 ／ 构建 next 数组，next[#]=#）。
> - **M4 伴生**（4 键：bstSearch/bstDelete 的 `recursive`、`compare`）：两种算法的文案落在同一 phase 名下，
>   归属已入 golden，按 §1.2 逐条判 ✅。
> - **恶性**（互相矛盾的两态）：v4 的 `insertion/insert`（有位置 vs 无位置）已由 #4 消除；
>   v5 现存实例见 §1.1（`computeNextVal/build_nextval` 的 `nextval[-1]` 值/下标错位）。
>
> 注意：**"同键多文案"本身不是缺陷**——`hanoi/move`（第 1/2/3 个盘子）、`bubble/compare`（arr[i] 与 arr[i+1]）
> 这类只变数值的，去数值化后是**单一模板**，不计入 M5。

## 3. 覆盖缺口（45 个零标注模板）

**数量口径**：`88（全部模板）= 37（§2 有标注）+ 45（本节零标注）+ 6（cpp，见 §6）`；
`45 = 档 A 1 个 + 档 C 44 个`。

**档 A · 检测已收紧致漏检（1 个，改模板函数名即可）**：`linkedDelete`

> 三审前本档有 4 个（bstInsert / bstSearch / bstDelete / linkedDelete）；
> **bst 三个已由 P0-B 批复亮**（名字+语义词双条件 + `is_treenode_ctx`），
> 故本档缩为 `linkedDelete`。**注意**：改名会改 `src`（源码行文本）而 golden 逐字段
> 比对 `src` → **须同步更新 golden 并附红→绿锚**（三审前"对 golden 无影响"的说法已纠正）。

**档 C · 完全未实现（44 个，需检测分支 + 步骤模板双写）**：

`activitySelection`、`array`、`bTree`、`bellmanFord`、`bucketSort`、`circularLinkedList`、`circularQueue`、`criticalPath`、`deque`、`doublyLinkedList`、`externalSort`、`factorial`、`fib`、`fibonacciSearch`、`infixEvaluation`、`inorder`、`interpolationSearch`、`isPrime`、`josephus`、`linear`、`linked`、`linkedInsert`、`linkedListTail`、`linkedQueue`、`linkedStack`、`linkedTraverse`、`mergeSortedLists`、`parenthesesMatch`、`pointer`、`polynomialAdd`、`postorder`、`queueArray`、`redBlackTree`、`sieveOfEratosthenes`、`spfa`、`stackArray`、`staticLinkedList`、`stringBasicOps`、`stringReverse`、`threadedBinaryTree`、`treeNode`、`treePreorder`、`treeToForest`、`unionFind`

> 其中 4 个是**本次新增登记**（三审意见 P0-3 指出的"文档黑洞"——原表既无行、原档也无名）：
> `activitySelection`、`externalSort`、`mergeSortedLists`（误判消除后转零，属代价非成果）、
> `stringBasicOps`（`has_word` 修复后零标注）。`huffmanTree` 仍有一个 phase（`merge`），
> 不入本档，但其 `select` 分支零触发。

## 4. 缺 phase 登记（覆盖面，不阻断 golden）

| 模板 | 缺什么 |
|------|--------|
| `avlTree` | 旋转分型（LL/RR/LR/RL）未分 phase；bf 来源（L89 case -1）未标 |
| `bfs` | 无 finish / 无路可达 |
| `binary` | 默认参数一次命中 → found / narrow_left 未出现（覆盖面） |
| `binarySearchTreeValidation` | 无「合法/非法」结论 |
| `bstSearch` | 无 miss / not_found |
| `countingSort` | 无 count（统计频次，L5-6） |
| `dijkstra` | 无 select_min（L16-20）、无 init |
| `floyd` | 截断 → 无 finish |
| `heapSort` | 无 finish |
| `huffmanTree` | select（选两棵最小树）零触发 |
| `levelOrder` | 无层分界 / finish |
| `merge` | 合并体内部无 phase |
| `primMST` | 无 select_min（L17-21） |
| `quick` | partition 体内扫描（L14-20）无 phase |
| `radixSort` | 无「回写原数组」（L19） |
| `seqList` | 只覆盖 listDelete；listInsert / listFind 未标注 |
| `stringMatchBF` | 无 match / finish（L18-19） |
| `stringMatchKMP` | 无 finish |
| `topologicalSort` | 无 init（L8-12 求入度） |
| `dp 家族` | 无 init phase（dpCoinChange 的 L7-8 初始化未标） |
| `dpKnapsack / dpLCS / matrixChain` | 截断 → 无 finish |
| `bst_search / bst_delete` | 自身无 create / finish（这两个 phase 只由建树阶段的 bst_insert 提供，见 §1.2） |
| `computeNextVal` | nextval 段展示格式不统一（`nextval[-1]` 值/下标错位，见 §1.1） |

## 5. 提取口径限制（非缺陷，登记备查）

- **截断 6 个**（顶格 3998 ≈ 4000 步预算）：`criticalPath`、`dpKnapsack`、`dpLCS`、
  `floyd`、`matrixChain`、`radixSort`。**这 6 个在 §2 表内的行只代表"跑到截断点为止
  已出现的 phase"，不代表全集**，不得据此判定"缺 phase"或固化全集 golden。
  实测复核：`floyd` 只到 `outer_loop k=3`；`matrixChain` 的 `outer_loop i` 只到 `i=5`。
- **`bTree` 不在截断之列**：实测**只跑 221 步即结束**（不是"4000 步仍不足"），
  零标注属"未实现"，归 §3 档 C。（三审前 §4 把它列为截断，头注称它与 `criticalPath`
  "归档 C 非截断"——后半句对 `criticalPath` 不成立：它确实顶格 3998、零标注。）
- **golden 与步数预算绑定**：`algorithm_annotation_golden_test.rs` 的
  `STEP_BUDGET = 4000`。改预算、或修性能、或补管道（如 P1-1 的跨行上下文）都会让
  截断模板的 golden 变化——**红的原因可能与判据无关**，须先看这 6 个。
- **cpp 模板 6 个**（无 `source.c`，需 C++ 编译口径）：`cpp_class_basic`、`cpp_hello`、
  `cpp_range_for`、`cpp_unique_ptr`、`cpp_vector_int`、`cpp_vector_struct`。
  **它们不在 golden/CI 内**（测试按 `source.c` 存在过滤）→ **CI 实际覆盖 = 82 个 C 模板**。
- 提取环境：本轮修复后的 release 构建（serve 会话，phase 键自 `AlgorithmStepSnapshot.phase`）。

## 6. 固化路径与待落地

**固化路径（不变）**：审阅勾选 → golden JSON → serve 会话收集比对进 CI（U1#1① 收口）
→ ② property 层（标注-行为一致性抽样）。

**已落地（v4→v5 期间，提交 `a528a4a` / `9cd1aa8`）**：

| 项 | 提交 | 复核依据 |
|----|------|----------|
| 1. golden 增 `algorithm` / `display_name` 归属字段 | `a528a4a` | golden 310 条全部带 `algorithm`；本表「算法」列与 M4 标记；§1.2 的 8 个 ⛔ 由此可判 |
| 2. dp 初始化循环排除 | `a528a4a` | `dpKnapsack`/`matrixChain` 的 `inner_loop` 缺陷键**整体消失**；`dpLCS/inner_loop` 首现回真内层 L14 |
| 3. dp 子族具名（币种 / 物品 / 金额） | `9cd1aa8` | `遍历币种 i` / `遍历物品 i` / `遍历金额 j` 实测 |
| 4. `insertion/insert` 位置 0 降级消除 | `9cd1aa8` | 4 变体文案统一为「将 key=K 插入到正确位置 P」 |
| 5. `hanoi/finish` 按 func_name 区分；`topologicalSort/output` 挂真 printf 行 + `dequeue` 拆分 | `9cd1aa8` | §2 表对应行 |
| 6. `quick/partition_init` 值/下标语义 | `9cd1aa8` | 文案改「分区完成，枢轴落位下标 pivot=N」 |
| 7. `build_nextval` 词条分离 | `9cd1aa8` | 新键 10 条；**同时引入 §1.1 的 `nextval[-1]` 格式问题** |
| 9. `step.next` 结束后重复发布末帧 | `a528a4a` | 终态末帧重放根治（各模板 `last_step_index` 较 v4 减 1 即为佐证） |

**未落地（本表 ✗ 与残留，需改代码）**：

1. **`build_nextval` 文案值/下标错位**（§1.1 首条）：`nextval[-1]` 应为 `nextval[0]=-1`，并统一"带值"格式。
2. **`binarySearchTreeValidation/recursive` 首现挂载点**：顶层调用行 vs 递归体描述——改入口语义，或把首现移焦到 L22。
3. **`hanoi/recursive` 首现文案**：问题陈述改入口语义（#5 只覆盖了 `finish`）。
4. **`quick/partition_init` 的 phase 名**：`partition_init` 与"分区已完成"不符；#6 已裁定"词汇面改动另行裁定"，
   需走词汇表单源（`native/src/unified/vocabulary.rs` + schema 附录 B）。
5. **`computeNextVal/build_next` 的 L25 展示**：「构建 next 数组，next[#]=-1」实为**读取** next[j]，
   建议改「读取 next[j]=-1」。
6. **登记未修（历史遗留，见附录 A）**：P1-3/P1-4 的顶层调用区分、P0-4 prev 操作数管道——本版 §0 判定规则 ③
   把"顶层调用帧 + 入口语义"判为可接受；若后续按 `at_callee_entry && caller_is_main` 做区分，
   请同步复核 §2 表中 9 个 M3 键。
7. **golden 文件名**：`algorithm_annotations_v3.json` 的内容已迭代到第五次全量提取（310 条），
   文件名与内容版本脱节——建议改名或在文件内加 `baseline`/日期字段（改名前须同步
   `algorithm_annotation_golden_test.rs` 的路径常量）。

## 7. 同族风险

- **命名子串 5 项**（未修，待裁定）：`merge` / `binary` / `quick` / `heap` / `select`
  ——收紧时**必须用 `features::has_word` 整词匹配**，不得再用裸 `contains()`：
  `contains("insert")` 命 `insert_node`、`contains("bst")` 命 `subString`（小写内含 `bst`）。
  同类误判已两次实锤（`contains("insert")` 命 `insert_node`、`contains("bst")` 命 `subString`，
  详见附录 A.2）。
- **结构特征分支已清查收紧 4 处**；其余（如快排的
  `is_recursive && has_partition_pattern && has_nested_loops`）理论上仍有误判面
  ——建议由 golden 回归暴露，不再预先猜测收紧。

## 附录 A · 修订史（**冻结·只读**，数字为当日状态，不得回填）

> 本节保留 v1→v3 的原始批注与数字（含互相矛盾者），仅作追溯用。
> 当前有效状态一律以 §0 为准。完整原文见 git 历史。

### A.1 v2 修订（用户审阅反馈驱动）
v1 清单被否决——"表格里的数值列系统性不可信，勾选会把错误固化成 golden"。
两项 P0（首帧旧值 / 结构特征误判）当轮修复，v2 为修复后的重提取（serve 会话 × 4000 步）。
**审阅方法**：逐行勾选 ☐ 通过 / ✗ 改写。

### A.2 三审修复批头注（2026-09-13，用户机器复核驱动）
§2 主表为修复前提取——用户复核（`算法标注golden审阅意见.md`，100 行逐行判定
62✅/25✗/13⛔）驱动的第三批修复已落地（P0-1 has_word 缩写、P0-2 四算法误判、
P0-3 dp 判据、P1 判据批 10 项）；**主表待重提取对账后方可勾选**；P0-4 登记下一批。
**教训（本节保留）**：命名判据一律用 `features::has_word`（词边界 + 驼峰整词），
禁止裸 `contains()`——`contains("insert")` 命 `insert_node`、`contains("bst")` 命
`subString`（小写 `substring` 内含 `bst`），同一个坑踩过两次。

### A.3 第二批（同日，二审 P0-C/P1 批）
- P0-C gcd mod 排除 IO 行（首现回真语句 L6）；
- P1-1 dp 初始化循环排除（单行 for 形态修复 2/4——dpLCS/matrixChain 的多行 for 体不在行内，
  单行判定不可达，登记边界）；
- P1-2 radix count 收紧 `]++` 自增形态；
- P1-5 签名行统一排除（infer_algorithm_step 总入口，类型关键字开头 + `{` 结尾）；
- §7.5 hanoi 零盘递归不产出；§7.3 step_next 改 append 语义防 batch>1 丢帧。
- **登记未修**：P1-3/P1-4 顶层调用区分——带标注帧是 callee entry（`func_name` 已是被调函数、
  `code_line` 仍是 main 调用行），需 `at_callee_entry` 传入 inferrer（与 P0-4 prev_vars 同批管道改动）。
- **P1-6 缺口登记**：activitySelection / externalSort / mergeSortedLists 因误判消除转零标注
  （外部排序/置换选择/链表归并是真实教学内容，属代价非成果）；huffmanTree 的 select 分支零触发。
- 该批注自报"实测 36+46"——**同日二审文件已注明其过期（现为 34+48）**，两者均冻结于此。

### A.4 v3 重提取与 golden 固化（2026-09-14）
二审全部 P0/P1 修复（含管道批 P0-4/P1-3/P1-4 收口）后全量重提取：
**82 模板 = 37 有标注（317 条首现 / 113 个 (模板,phase) 行）+ 45 零标注**
（比二审 §4.2 预期的 34 多出的 3 个 = P0-B 批复亮的 bstInsert/bstSearch/bstDelete）。
行为基线固化为 `native/tests/golden/algorithm_annotations_v3.json` 并接入 CI
（`algorithm_annotation_golden_test`：Rust 直调 session_api 全量比对，双向防漂移；
J9 埋雷已证红）。原始数据：`tmp/annot_extract_v3_20260914.json`。

### A.5 v5 复判（2026-09-14，§6 代码侧两批落地后）
代码侧落地（`a528a4a` golden 归属 + 终态末帧重放根治 + dp 初始化内层排除；`9cd1aa8` 文案/挂载点五项）
后重跑全量提取（`tmp/annot_extract_v5_20260914.json`，38.6s）并复判：
基线 **310 条 / 113 键 / 37 模板**（v3 为 317 条；300→310 = nextval 变体分离等，与提交自报一致），
判定 **✅ 110 / ✗ 3 / ⛔ 0**。8 个 ⛔ 键按算法分行复判**全部转 ✅**；
11 个旧 ✗ 中 **7 键转 ✅、2 键（dpKnapsack/matrixChain 的 inner_loop）整体消失、2 键仍 ✗**；
新词条 2 个（`topologicalSort/dequeue` ✅、`computeNextVal/build_nextval` ✗）。

> 二审文件对这组数字的建议是"34 模板 × 95 条（现 38 × 100）"——**两套口径**：
> 二审按 `(算法, phase)` 计数，本表按 `(phase, desc)` 计数（=113 键 / 317 条）。
> v4 已在 §0 口径 1 中钉死，不再混用。
