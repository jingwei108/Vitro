# Vitro 自研 VM 体验优势设计文档

> **核心理念**：自研 VM 的优势不是省内存，而是做通用 IDE/可视化工具做不出来的教学体验。  
> **性能原则**：中端手机 50MB 内存换零延迟交互，完全可接受。拒绝为省 47.5MB 做过度工程化。
>
> **2026-09-11 前端切割后现状**：教学体验的载体已从 Dart widget 转为**协议载荷 + 三出口 API**。本仓库负责在后端产出这些能力（快照/检查点/热力图/语义标注/变量历史/变量级高亮），经 C ABI、wasm32、`vitro_cli serve` 交付；渲染与交互由社区前端实现。载荷字段以 [`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md 为准。
>
> **最后核对日期**：2026-09-11（修订：实现位置表中的 Dart widget 改为后端能力 + 三出口载荷；修正正文中的旧模块路径，并如实记录 `unified/checkpoint.rs` 已不存在等现状差异）

---

## 目录

- [1. 执行路径热力图（Execution Heatmap）](#1-执行路径热力图)
- [2. 语义进度条（Semantic Timeline）](#2-语义进度条)
- [3. 变量变化历史（Variable History）](#3-变量变化历史)
- [4. 智能检查点密度（Smart Checkpoints）](#4-智能检查点密度)
- [5. 运行时异常自动回退（Auto-Rollback on Trap）](#5-运行时异常自动回退)
- [6. 变量级高亮（Variable-Level Highlighting）](#6-变量级高亮)
- [7. 技术底座：全量快照 + 三件套元数据](#7-技术底座)
- [8. 与统一模式的协作关系](#8-与统一模式的协作关系)
- [9. 实施优先级](#9-实施优先级)

---

## 1. 执行路径热力图

### 1.1 用户看到什么

代码编辑器左侧行号旁边，出现一列彩色条带，颜色深浅表示该行被执行的次数：

```
  1  ░░░░░░░░░░  #include <stdio.h>          (0 次)
  2  ░░░░░░░░░░  void bubbleSort(int arr[], int n) {
  3  ▓▓▓▓▓▓▓▓▓▓  for (int i = 0; i < n - 1; i++) {      (45 次)
  4  ██████████    for (int j = 0; j < n - i - 1; j++) {   (100 次) 🔥
  5  ████████        if (arr[j] > arr[j + 1]) {            (78 次)
  6  ▓▓▓▓              int temp = arr[j];                   (34 次)
  7  ▓▓▓▓              arr[j] = arr[j + 1];                 (34 次)
  8  ▓▓▓▓              arr[j + 1] = temp;                   (34 次)
  9  ░░░░░░░░░░      }
 10  ░░░░░░░░░░    }
 11  ░░░░░░░░░░  }
```

- **灰色 `░░░░`**：未执行的代码（如注释、未触发的 else 分支）
- **浅蓝 `▓▓▓▓`**：执行次数中等
- **深红 `████`**：执行次数最高的热点代码

鼠标悬停在彩色条带上，弹出 Tooltip：
> `第 4 行：执行 100 次（占总执行步数的 67%）`

### 1.2 教学价值

| 场景 | 学生看到的洞察 |
|:---|:---|
| **冒泡排序** | 内层循环执行了 `n*(n-1)/2` 次，直观理解 O(n²) |
| **二分查找** | 循环体只执行了 `log₂(n)` 次，理解 O(log n) |
| **未覆盖分支** | `else` 分支是灰色的，学生意识到"我的测试用例没覆盖到这里" |
| **死代码** | 某行始终是灰色，学生发现"这行代码永远跑不到" |

### 1.3 技术实现

后端（Rust）在 VM 执行时收集行号计数，并把**截至本步**的累计值放进协议载荷：

```rust
// 后端：行号 → 执行次数
// 结构定义：native/crates/vitro_runtime/src/runtime_state.rs 的 ExecutionHeatmap
// 收集与投递：native/src/unified/collector.rs + native/src/unified/types.rs
pub struct ExecutionHeatmap {
    pub line_counts: HashMap<i32, u64>,      // 行号 → 执行次数
    pub line_total_ms: HashMap<i32, u64>,    // 行号 → 总耗时（可选扩展）
}
```

载荷字段（消费方据此自行渲染侧边栏，本仓库不再提供渲染代码）：

| 字段 | 含义 |
|------|------|
| `heatmap_line` | 热力图行号（当前实现恒等于 `code_line`） |
| `heatmap_count` | 该行**截至本步**的累计执行次数（单调不减） |
| 归一化/配色/悬浮提示 | **消费方职责**（社区前端 / Web / headless 各自实现） |

> **口径保证（后端已完成）**：heatmap 只统计用户源码行（`SourceLoc.file_id == 0 && line > 0`），标准库/预编译字节码行号不会混入，消费方**不需要**再按总行数做过滤（见 `模板维护指南.md` §6.3）。
>
> 历史实现（已迁出）：原 Dart 侧 `HeatmapGutter extends LeafRenderObjectWidget` 在编辑器侧边栏绘制彩色条带，该代码随 `CideFlutter/` 移出仓库（历史资产，已迁出）。

### 1.4 与统一模式的结合

用户拖动进度条到第 X 步时，热力图可以**动态变化**：
- 只显示"执行到第 X 步为止"的累计次数
- 学生可以看到"随着执行推进，哪些行逐渐变热"

---

## 2. 语义进度条

### 2.1 用户看到什么

不是冷冰冰的"第 247 / 500 步"，而是：

```
[初始化]═══[外层循环 i=0]═══[i=1]═══[i=2]═══[i=3]═══[i=4]═══[完成]
               ↑
            当前位置
```

拖动时，进度条吸附到语义边界，不会停在无聊的 `i++` 上。

### 2.2 技术实现

> **现状（2026-09-11 核实）**：`StepMeta` **真实存在**于 `native/src/unified/types.rs`，但字段比下例精简——实际为 `{code_line, func_name, loop_depth, semantic_label}`；下例中的 `step_index` / `loop_iters` / `is_loop_boundary` / `is_func_call` / `is_swap` **未实现**。语义标签的推断实现在 `native/src/unified/collector.rs` 的 `infer_semantic_label()`，文本经载荷 `semantic_label` 交付（schema §1），进度条吸附由消费方按标签边界实现。

```rust
// 历史设计稿（部分字段未实现）
pub struct StepMeta {
    pub step_index: i32,
    pub code_line: i32,
    pub func_name: String,
    pub loop_depth: i32,
    pub loop_iters: Vec<i32>,       // [外层第几次, 内层第几次]
    pub is_loop_boundary: bool,     // 是否循环边界（语义检查点）
    pub is_func_call: bool,         // 是否函数调用
    pub is_swap: bool,              // 是否数组交换
}
```

每步自动推断语义标签：
```rust
fn infer_semantic_label(meta: &StepMeta) -> String {
    if meta.is_swap {
        return format!("交换 arr[{}] ↔ arr[{}]", meta.loop_iters[0], meta.loop_iters[0]+1);
    }
    if meta.loop_depth > 0 {
        return format!("循环 {}", meta.loop_iters.iter().map(|i| format!("i{}={}", i, i)).collect::<Vec<_>>().join(", "));
    }
    if meta.is_func_call {
        return format!("调用 {}", meta.func_name);
    }
    format!("第 {} 行", meta.code_line)
}
```

---

## 3. 变量变化历史

### 3.1 用户看到什么

拖动进度条时，悬浮球的"局部变量"面板零延迟更新：

```
┌─ 局部变量 ─────────────┐
│ i      = 3     ▓▓▓░░░ │  ← 变化趋势条
│ j      = 5     ▓▓▓▓▓░ │
│ temp   = 8     ░░░▓░░ │  ← 刚刚被赋值
│ arr[0] = 1     ░░░░░░ │
│ arr[1] = 3     ▓▓░░░░ │  ← 之前交换过
└────────────────────────┘
```

每个变量旁边有一个**迷你趋势图**，显示该变量在执行过程中的变化轨迹。

### 3.2 技术实现

> **现状（2026-09-11 核实）**：`VarHistory` 结构体与 `collect_var_history()` **在仓库中不存在**。变量变化历史现在的实现方式是**由载荷窗口推导**：每步 `StepPayload.local_vars` 已含 `name` / 值 / `ty_name`，消费方按 `step_index` 纵向拼接即可得到趋势；`seek` 到第 N 步时各视图统一以第 N 步快照为准（schema §4 的一致性契约），无需后端额外存历史。
>
> **未完成缺口**：后端没有"变化点索引"（即没有把 `changes: Vec<(step_index, value)>` 物化），消费方要画趋势图必须自行缓存/扫描载荷窗口。若未来需要 O(1) 趋势查询，应作为 payload 的增量能力落地，而不是复活 `VarHistory` 专用类型。

```rust
// 历史设计稿（仓库中不存在）
pub struct VarHistory {
    pub name: String,
    pub ty: Type,
    pub changes: Vec<(i32, String)>,   // (step_index, display_value)
}

// 执行过程中自动收集（历史设计稿，仓库中不存在）
fn collect_var_history(vm: &VitroVM, step: i32) -> Vec<VarHistory> {
    vm.symbols.iter()
        .filter(|s| s.scope == Scope::Local)
        .map(|s| VarHistory {
            name: s.name.clone(),
            ty: s.ty.clone(),
            changes: vec![(step, format_value(vm.read_var(s)))],
        })
        .collect()
}
```

1000 步 × 20 个变量 × 每个值 20 字节 = 400KB。忽略不计。

---

## 4. 智能检查点密度

### 4.1 原则

不是机械地"每 20 步一个检查点"，而是**代码结构感知**：

| 代码特征 | 检查点密度 | 理由 |
|:---|:---|:---|
| 循环边界（for/while 迭代开始） | **必存** | 学生关心"第几轮循环" |
| 数组元素交换 | **必存** | 排序算法的关键帧 |
| 函数调用/返回 | **必存** | 调用栈变化的关键点 |
| 条件分支（if/else）首次进入 | **存** | 覆盖路径变化 |
| 普通语句（`i++`、`a = b + c`） | **不存** |  boring，重放 5 步内无感知 |

100 步的冒泡排序可能只有 **15 个检查点**，但覆盖了所有"关键时刻"。

### 4.2 用户感知

拖动进度条时，磁铁吸附到最近的语义检查点。学生感觉自己在"关键帧之间跳跃"，不会卡在无聊的赋值语句上。

---

## 5. 运行时异常自动回退

### 5.1 用户看到什么

学生代码写错了：

```c
for (int i = 0; i <= n; i++) {   // 应该是 i < n
    arr[i] = i;                    // 第 7 行：数组越界！
}
```

**通用 IDE**：程序崩溃，终端输出 `Segmentation fault`，学生一脸懵逼。

**Vitro**：

```
⚠️ 运行时错误：数组越界
   第 7 行：arr[i] = i;
   
   🔙 自动回退到上一步：
   第 6 行：for (int i = 0; i <= n; i++)
   
   📊 变量状态：
   i = 10, n = 10, arr = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
   
   🔧 智能诊断：
   循环条件 i <= n 导致 i 取到 10，而 arr 大小为 10（有效索引 0~9）。
   建议改为 i < n。
   
   [一键修复]  [查看知识卡片：数组越界]
```

### 5.2 技术实现

```rust
impl VitroVM {
    pub fn step_next_safe(&mut self) -> Result<(), TrapInfo> {
        let checkpoint = self.snapshot();  // 执行前保存
        match self.step_next() {
            Ok(_) => Ok(()),
            Err(trap) => {
                self.restore(&checkpoint);  // 自动回退
                Err(trap)
            }
        }
    }
}
```

配合诊断系统：
- 如果 trap 是数组越界 → 检查循环条件是否有 `<=` 应为 `<`
- 如果 trap 是空指针解引用 → 检查 `malloc` 返回值是否被检查
- 如果 trap 是栈溢出 → 检查递归终止条件

---

## 6. 变量级高亮

### 6.1 用户看到什么

回放时，代码编辑器不仅高亮当前行，还**高亮当前正在读写的变量**：

```c
for (int i = 0; i < n; i++) {           // i 边框橙色（正在自增）
    for (int j = 0; j < n - i; j++) {   // j 边框橙色
        if (arr[j] > arr[j + 1]) {      // arr[j] 和 arr[j+1] 底色淡红（参与比较）
            int temp = arr[j];           // temp 边框绿色（新声明）
            arr[j] = arr[j + 1];         // arr[j] 闪烁黄色（被写入）
            arr[j + 1] = temp;           // arr[j+1] 闪烁黄色
        }
    }
}
```

### 6.2 技术实现

编译器已经知道每个标识符的 `SourceLoc`（行、列、长度）。VM 的 `step_next` 可以通过符号表反查当前指令访问了哪些变量。

```rust
pub struct VariableHighlight {
    pub name: String,
    pub line: i32,
    pub column: i32,
    pub length: i32,
    pub highlight_type: HighlightType,  // Read / Write / Declare / Compare
}

// VM 每步输出
pub fn get_variable_highlights(&self) -> Vec<VariableHighlight> {
    // 通过当前指令的符号引用反推高亮信息
}
```

消费方在编辑器中按 `line` / `column` / `length` 绘制下划线、边框或底色——**渲染属消费方职责**（社区前端自行实现，原 Dart `VitroEditor.spanBuilder` 已随 `CideFlutter/` 迁出，历史资产，已迁出）。后端需保证的是：每步能给出"这一步访问了哪些变量、以何种方式访问"。

> **已落地的等价载荷（口径需对齐）**：`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md` 的 `accessed_vars[]`（`AccessedVar`）目前只有两个字段：`name` 与 `access_type`（枚举仅 `"Read"` / `"Write"`，见 schema §2.3、§3.2，大小写敏感，无第三种取值）。上例的 `VariableHighlight`（含 `line` / `column` / `length` / `Declare` / `Compare`）在仓库中**不存在**：**精确到列范围的高亮与"声明/比较"语义属未完成缺口**，消费方目前只能按"变量名 + 读写类型"高亮。

---

## 7. 技术底座：全量快照 + 三件套元数据

### 7.1 为什么选择全量快照？

| 方案 | 1000 步内存 | 恢复耗时 | 复杂度 | 结论 |
|:---|:---|:---|:---|:---|
| 全量快照（1MB/20步） | 50MB | 2~3ms | 极低 | ✅ **采用** |
| 差分编码（50KB/检查点） | 2.5MB | 1~2ms | 中 | 省 47.5MB，但用户感知为 0 |
| COW 页表（4KB/步） | 4MB | <1ms | 高 | 过度工程化 |

**50MB 在中端手机上连一个微信小程序都不如。为了省这点内存引入复杂度，是过度工程化。**

### 7.2 三件套元数据

除了全量 VM 快照，每步保存轻量级元数据。

**实际结构（`native/src/unified/types.rs`，2026-09-11 核实）**：

```rust
pub struct StepPayload {
    pub step_index: i32,
    pub code_line: i32,
    pub func_name: String,
    pub semantic_label: String,                     // ① 语义元数据（进度条 / 教学标注）
    pub algorithm_step: Option<AlgorithmStepSnapshot>,
    pub local_vars: Vec<ApiVariableSnapshot>,       // ③ 调试摘要（变量面板零延迟）
    pub call_stack: Vec<ApiFrameInfo>,
    pub vis_events: Vec<VisEvent>,                  // ① 动画数据（可视化事件）
    pub heatmap_line: i32,                          // 热力图（截至本步）
    pub heatmap_count: u64,
    pub accessed_vars: Vec<AccessedVar>,            // 变量级高亮（Read/Write）
    pub array_snapshots: Vec<ArraySnapshot>,
    pub pointer_snapshots: Vec<PointerSnapshot>,    // 指针四状态
    pub root_cause_hint: Option<RootCauseHint>,     // 异常根因提示
}
```

设计稿把三者包装成一个 `StepPayload`，实际并非如此（2026-09-11 核实）：

| 设计稿类型 | 实际状态 |
|:---|:---|
| `vis_state: VisState` | **不存在**（`VisState` / `VisArrayState` / `VisStructureState` 等系列类型在 Rust 源码中均无定义，见 `ARCHIVE_零侵入可视化设计.md` §3.1） |
| `meta: StepMeta` | **存在**于 `native/src/unified/types.rs`，字段为 `{code_line, func_name, loop_depth, semantic_label}`（设计稿中的 `step_index` / `loop_iters` / `is_loop_boundary` / `is_func_call` / `is_swap` **未实现**）；用途是进度条标签与智能检查点，不是 `StepPayload` 的字段 |
| `debug_summary: DebugSummary` | **存在**于同一文件，字段为 `{local_vars, call_stack, output_len}`（设计稿中的 `memory_summary` **未实现**） |

实际载荷是上表的扁平字段，**字段级口径以 [`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md 为准**（该 schema 是对外承诺的 wire format）。

> **命名债（诚实记录）**：`native/src/unified/types.rs` 中仍有 "传输到 Flutter 前端作为 FrameCache"、"FRB 友好的变量快照" 等注释（所述 Flutter 前端与 FRB 桥接均为历史资产，已迁出），以及 `ApiVariableSnapshot` 等 `Api*` 前缀命名——这些属切割前的遗留措辞，代码本身已是语言中立 Rust 层，**名称待后续重构收敛**（本次文档翻新不改 `.rs` 代码）。

每步 `StepPayload` 大小：
- `vis_events` / `array_snapshots` / `pointer_snapshots`：几百字节到几 KB（取决于数据结构复杂度）
- `semantic_label`：~50 字节
- `local_vars` + `call_stack`：~1KB（20 个局部变量 + 5 层调用栈）

1000 步 ≈ 2~5MB。进一步压缩手段已落地：`StepPayloadDelta` 字段级差分 + 符号表索引化（`native/src/unified/stream.rs`，schema §5）。

### 7.3 检查点管理

> **现状（2026-09-11 核实）**：`CheckpointManager` **确实存在**，但位于 `native/crates/vitro_vm/src/snapshot.rs`（不是设计稿暗示的 `unified/checkpoint.rs`——**该文件不存在**），且实际 API 比下例更丰富：`new(interval)`、`should_checkpoint(step, semantic_label)`（**语义感知**，非机械的 `step % interval`）、`save`、`nearest(target)`、`seek` 重放，并支持 `MemoryImage::Full` 与增量内存映像的链式重建。下例为历史设计稿，仅示意思路。

```rust
// 历史设计稿（实际实现在 native/crates/vitro_vm/src/snapshot.rs）
pub struct CheckpointManager {
    pub checkpoints: Vec<(i32, VMSnapshot)>,  // (step_index, snapshot)
    pub interval: i32,                        // 20 步
}

impl CheckpointManager {
    pub fn maybe_save(&mut self, step: i32, vm: &VitroVM) {
        if step % self.interval == 0 {
            self.checkpoints.push((step, vm.snapshot()));
        }
    }
    
    pub fn seek_to(&self, target: i32, vm: &mut VitroVM) {
        // 找到最近检查点
        let (idx, snap) = self.checkpoints.iter().rfind(|(s, _)| *s <= target).unwrap();
        vm.restore(snap);
        // 正向重放到目标步
        for _ in *idx..target {
            vm.step_next();
        }
    }
}
```

---

## 8. 与统一模式的协作关系

```
用户点击"运行"
    ↓
[自动执行模式] VM 连续执行
    ├── 每步：构造 StepPayload → 三出口（capi / serve / wasm）→ 消费方缓存
    ├── 每步：更新 Heatmap（heatmap_line / heatmap_count，截至本步）
    ├── 每步：产出 semantic_label（语义标签）/ algorithm_step
    ├── 每步：产出 accessed_vars + local_vars（变量历史由消费方按窗口推导）
    ├── 每 20 步（且语义边界命中）：保存 VM 快照（智能检查点）
    └── 遇 Trap：自动回退到检查点 + 产出 root_cause_hint
    ↓
执行结束 / 用户暂停
    ↓
[统一模式] 用户自由探索
    ├── 拖动进度条 → seek 到第 N 步，各视图以第 N 步快照为准（schema §4）
    ├── 变量面板 → 读取 local_vars（零延迟）
    ├── 代码编辑器 → 按 accessed_vars 高亮被读/被写变量
    ├── 侧边栏 → Heatmap 显示累计执行次数
    └── 点击"继续执行" → 从当前步恢复 VM 并继续
```

**用户完全不需要区分"调试模式"和"回放模式"。只有一个模式：写代码 → 运行 → 自由探索。**（渲染与交互由社区前端实现；本仓库保证载荷语义一致。）

---

## 9. 实施优先级

> **2026-09-11 现状对齐**：表中"实际文件"一列原写的是 Dart widget 路径，那些文件已随 `CideFlutter/` 迁出（历史资产，已迁出）。下表改为**后端能力 + 交付出口/载荷**口径；渲染侧一律属消费方职责。

| 优先级 | 功能 | 状态 | 后端实现位置 / 交付载荷 |
|:---|:---|:---|:---|
| P0 | VM 全量快照/恢复 + 检查点管理 | ✅ 已实现 | `native/crates/vitro_vm/src/snapshot.rs`（`VMSnapshot` / `CheckpointManager`）+ `native/src/unified/engine.rs`（seek 重放） |
| P0 | 自动执行模式（收集 StepPayload） | ✅ 已实现 | `native/src/unified/engine.rs` `run_batch()` + `native/src/unified/collector.rs` |
| P1 | 执行路径热力图（Heatmap） | ✅ 已实现 | `native/crates/vitro_runtime/src/runtime_state.rs`（`ExecutionHeatmap`）+ 载荷 `heatmap_line` / `heatmap_count`（渲染属消费方） |
| P1 | 排序动画 MVP + 语义进度条 | ✅ 后端载荷已实现 | `vis_events[]`（`AlgorithmStepSnapshot` + `VisEvent`）+ `semantic_label`（动画渲染属消费方） |
| P1 | 变量变化历史 | ⚠️ 部分 | 载荷 `local_vars` 已按步给出；**变化点索引未物化**（缺口，见 §3.2），趋势图由消费方按窗口自行推导 |
| P2 | 运行时异常自动回退 | ✅ 已实现 | `native/src/unified/engine.rs` `pre_step_snap` + `root_cause_hint` 载荷 |
| P2 | 变量级高亮 | ⚠️ 部分 | 载荷 `accessed_vars[]`（`Read` / `Write`）已落地；**列范围（`column`/`length`）与 `Declare`/`Compare` 语义未落地**（缺口，见 §6.2） |
| P3 | 链表/树可视化增强 | ⚠️ 未落地 | `ArraySnapshot` / `PointerSnapshot`（四状态）已落地；**链表/树节点遍历载荷依赖数据结构检测器，该检测器不存在**（缺口，见 `ARCHIVE_零侵入可视化设计.md` §4） |

**实际用时：约 2 周（后端 5 天 + 前端 5 天 + 联调 4 天）。**（原统计口径含已迁出的前端工作；当前后端侧剩余缺口见上表 ⚠️ 项。）

---

## 附录：竞品对比

| 功能 | VisuAlgo | Python Tutor | GDB/LLDB | VisualBinaryTree | **Vitro（本方案）** |
|:---|:---|:---|:---|:---|:---|
| 算法动画 | ✅ | ❌ | ❌ | ✅ | ✅ |
| 进度条拖动 | ❌ | ❌ | ❌ | ✅ | ✅ |
| 真实代码执行 | ❌ | ✅ | ✅ | ✅ | ✅ |
| 局部变量查看 | ❌ | ✅ | ✅ | ❌ | ✅ |
| 内存查看 | ❌ | ❌ | ✅ | ❌ | ✅ |
| 调用栈查看 | ❌ | ❌ | ✅ | ❌ | ✅ |
| 执行热力图 | ❌ | ❌ | ❌（Profiler 有，但非教学向） | ❌ | ✅ |
| 异常自动回退 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 变量级高亮 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 语义进度条 | ❌ | ❌ | ❌ | ❌ | ✅ |
| **统一模式（无需区分调试/回放）** | ❌ | ❌ | ❌ | ❌ | ✅ |
