# MoonBit 迁移 · `vitro_vm` + `vitro_runtime` 模块勘察报告

> **日期**：2026-09-18 ｜ **性质**：只读勘察（迁移前置证据采集），**未执行任何仓库内文件改动、未执行任何 git 命令、未运行 cargo/go 构建**
> **勘察对象**：`native/crates/vitro_runtime/`（13 个 `.rs` / 2,043 行）+ `native/crates/vitro_vm/`（27 个 `.rs` / 8,858 行），合计 **40 文件 / 10,901 行**
> **上游依据**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md) §2 事实核查 / §7 四道证伪门 / §8 假设清单 A1–A9
> **对应证伪门**：**门 1（VM 热循环性能，决定生死）**、**门 3（快照往返正确性）**——本模块是全部勘察里唯一同时承载两道门者
> **真值口径**：全局数字一律引 `reports/facts.json`（本次读取的 `generated_at` = 2026-09-18T13:01:41+08:00，`git.rev` = `4b57191`）并标 `as_of`；本报告内的规模/计数为**本次实测**，属 as-of 快照，冻结不回填。**本文件为 AS-OF 冻结（文件名含日期），裸数字不参与 `facts` 回填**
> **勘察方法**：亲读（40 个文件全部读毕）+ **3 路并行只读子勘察**（host funcs 全量清单 / JIT 差分防线与基线 / Bytecode Libc 加载器）。⚠️ **与其他模块勘察报告的差异（诚实标注）**：**本次未运行任何运行时探针**——无 release/debug 二进制调用、无 clang 对照实验、无 `%TEMP%` 产物。全部结论为**静态亲读 + 四路计数互证**；凡未经运行时验证者一律标〔待证〕并给验证方法
> **纪律**：`docs/archive/` 全程未读未引用；任务书前提与代码不符处已显式纠正（§0）；缺陷按防线哲学第 0 条如实记录，不粉饰；行号以 `(Get-Content).Count` 口径计（PowerShell `Measure-Object -Line` 会漏空行，本报告不使用）

---

## §0 先纠正三处任务书前提（读代码得出的反证）

| # | 任务书/速记中的表述 | 实测事实 | 证据 |
|---|---|---|---|
| 1 | "C 的有符号溢出在 Vitro 定义为 wrapping" | **错误**。`Add/Sub/Mul/Neg/Div/Mod` 溢出**一律教学 trap**（i64 中间值 + 范围检查）；只有 `UAdd/USub/UMul/UNeg` 是 `wrapping_*` | `core/executor/arithmetic.rs:6-15`（Add 用 i64 中转 + `r > i32::MAX` trap）、`:16-20`（UAdd `wrapping_add`）、`:21-30`、`:36-45`、`:96-107` |
| 2 | "与 codegen 的 `source_digest` 协作" | **codegen 完全不参与**。digest 由 Go 脚本在 `vitro_cli export` 之后**注入产物 JSON**；Rust 侧结构体**没有该字段**，serde 静默忽略 → **运行时不校验**。唯一消费者是构建期 `--check` 门禁 | 注入 `scripts/precompile_bytecode_libc/main.go:92-107`、`:219-220`；Rust 结构体 `bytecode_libc_loader.rs:13-25`（11 字段无 digest）；比对 `main.go:427-458` |
| 3 | "`HostFuncId` 枚举 / `from_u8` 查表 / freed_logs 检测在 host 层" | **无枚举、无查表、检测不在 host 层**。host id 是裸 `u32` 常量，`execute_host_func` 用 `match id` 直接匹配；检测点在 core 层 `check_uaf`，host 层只做"登记/清理/直查" | `host_func_id.rs:6-144`（110 常量）、`host_funcs.rs:33-147`（110 臂）、`core/memory.rs:188-192` |

**另有三处规模数字需修正**：`host_funcs.rs` 是 **161 行**（非 156）、`host_func_id.rs` 是 **284 行**（非 265）；VFS 文件族在 **`host/file.rs`**（`host/io.rs` 是 printf/scanf 族）。

---

## ① 模块概览与规模（实测）

### 1.1 规模

| 项 | 值 | 数法 |
|---|---|---|
| `vitro_runtime/src` | **13 文件 / 2,043 行** | `Get-ChildItem -Recurse -Filter *.rs` + 逐文件 `(Get-Content).Count` 求和 |
| `vitro_vm/src` | **27 文件 / 8,858 行** | 同上 |
| **合计** | **40 文件 / 10,901 行** | 占全项目 76,218 行（评估报告附录 B，AS-OF 2026-09-18 冻结）的 **14.3%** |
| 最大三文件 | `core/state.rs` **821**、`vfs.rs` **771**、`jit_templates.rs` **752** | 后两者贴近 800 行阈值（`工程债务维护方案.md` 任务 B 判据） |
| `#[test]`（crate 内） | **11 个 / 4 个 `#[cfg(test)]` 块** | `executor/mod.rs:358-491`(7)、`snapshot.rs:333-484`(3)、`bytecode_libc_loader.rs:56-67`(1) |
| crate 身份 | `vitro_runtime` v0.1.0 / `vitro_vm` v0.1.0，edition 2021 | 两个 `Cargo.toml` |
| 依赖 | `vitro_runtime`：`vitro_ast` + `vitro_shared` + `serde`(features `derive`,`rc`)；`vitro_vm`：+`serde_json` + `libm` + `js-sys`(仅 wasm32) | 零 C 依赖；`vitro_vm/src/lib.rs:1` `#![forbid(unsafe_code)]` |

### 1.2 关键类型

**`OpCode`：132 条指令**（`opcode.rs:23-157`），由 `define_opcode!` 宏生成 `#[repr(u8)]` 枚举 + `from_u8`（`:1-21`）。值分四段：

| 段 | 值域 | 条数 | 内容 |
|---|---|---|---|
| 1 | 0–43 | 44 | 栈/局部/全局/内存/`i32` 算术/比较/逻辑/位/跳转/调用 |
| 2 | 50–110 | 61 | `F`(f32) / `D`(f64) / `Q`(i64) 三族：常量、四则、比较、局部/全局/内存、`Split`、类型转换 |
| 3 | 111–129 | 19 | `CallPtr` / 无符号比较与四则 / `LShr` / `StackAlloc` / `Memcpy`/`Memset`/`Strlen` / `PushArgc`/`PushArgv` / `TrapBoundsVla` |
| 4 | 130–137 | 8 | E1 B 档 64 位位运算（`BitAndQ`…`LShrQ`） |

**空洞 44–49 未定义**，`from_u8` 对其返回 `None`（`:13`）；**最大实际值 137**。

> **缺陷（组织债）**：`opcode.rs:18-19` 注释称"当前最大 opcode 值为 PushArgv = 128"，与实际（`LShrQ = 137`，`:156`）**不符** —— 该注释正是"若超 255 需改 `repr(u16)`"的容量判据，漂移后失去预警价值。

**`VitroVM`：36 个字段**（`core/state.rs:77-138`）。关键字段族：指令流（`code`/`ip`）、**内存（`memory: Vec<u8>`，1MB，`:80`）**、值栈（`stack: Vec<u64>`）、调用（`call_stack`/`func_table`/`func_names`/`mem_stack_top`）、符号（`symbols`/`local_sym_map`/`global_sym_map`）、教学观测（`vis_event_lines`/`vis_event_queue`/`breakpoints`/`current_line`/`error`/`last_accessed_vars`）、快照（`last_snapshot_step`/`snapshot_vars`/`dirty_pages:[u64;4]`）、诊断（`step_count`/`max_steps`/`call_depth_limit`/`snapshot_vars`）、**UAF（`freed_logs: BTreeMap<u32,FreedRegionInfo>`，`:120`）**、C++（`pending_array_construction`/`qsort_depth`）、**JIT（`ip_hits`/`trace_recorder`/`jit_traces`/`jit_stats`/`jit_enabled`，`:130-137`）**。

**常量单源**（`vitro_runtime/src/memory_state.rs`）：`MEM_SIZE=1MB`(:4)、`NULL_TRAP_SIZE=0x1000`(:6)、`GLOBAL_START=0x1000`(:8)、`HEAP_START=0x5000`(:10)、`GLOBAL_REGION_LIMIT=MEM_SIZE/16=64KB`(:16)、`STACK_START=MEM_SIZE`(:18)、`SNAPSHOT_INTERVAL=100_000`(:20)、`MAX_STACK_DEPTH=10_000`(:22)、`DEFAULT_QUARANTINE_BUDGET=256KB`(:30)。R1 后 `core/state.rs:10-14` 改为再导出（同值双写已消除）。

**`VitroVM::reset()`（`:193-233`）**：清执行状态与内存（`memory.fill(0)`，`:231`），但**刻意保留** `max_steps`/`call_depth_limit`/`jit_enabled`（`:208-209` 明文"会话级配置不随执行状态重置"）—— 这一区别对 JIT 开关的有效性至关重要（见 §⑦ spike 与 §⑧ 8.3）。

### 1.3 执行器分发结构（门 1 核心）

**两级分发**：

1. **`run` 循环**（`core/executor/mod.rs:12-73`）：先查 JIT fast path（`:18-48`），未命中才 `step()`（`:50`）。返回 `StepResult::{Ok,Paused,Finished,Trap,WaitingInput}`（`core/state.rs:46-53`）。
2. **`step()`**（`:255-355`）：步数 `saturating_add(1)` → 每 `SNAPSHOT_INTERVAL`(10 万) 刷新 `snapshot_vars`（`:267-276`）→ `max_steps`/`cancelled` 检查 → 取指（`:289`）→ **heatmap 记录（`:295-297`，仅 `loc.line>0 && file_id==0`，走 `Arc::make_mut`）** → 清 `last_accessed_vars`（`:300`）→ **JIT 热点计数（`:303-308`）** → **JIT 录制触发（`:311-317`）** → `dispatch_single_instruction`（`:320`）→ 录制（`:332-347`）→ 错误转 `Trap`（`:349-354`）。
3. **`dispatch_single_instruction`**（`:75-251`）：**一个大 match，按 opcode 族分 11 组臂**（栈/局部/全局/内存/算术/比较/位/F/D/Q/控制流/debug），组内转对应私有 `execute_*` 方法，**方法内二次 match**。

⇒ **每条指令实际经过两次 match**（外层族 match + 内层 opcode match）。11 个 `execute_*` 方法分布在 7 个文件：`stack.rs`(2)、`memory.rs`(2)、`arithmetic.rs`(3)、`float.rs`(3)、`control.rs`(1 + `do_call`/`do_call_inner`)、`debug.rs`(1)。

**栈与调用约定**：
- 值栈 `Vec<u64>` 存**位模式**（i32/f32 用低 32 位，i64/f64 用满）；`pop()` 空栈即 trap "运行时错误：栈下溢"（`core/state.rs:795-803`）；`push()` 超 `MAX_STACK_DEPTH` 即 trap（`:805-814`）。
- 局部变量在**线性内存**（`mem_stack_top` 向下增长，`locals_base + offset` 编址，`stack.rs:48-58`）。
- `CallFrame`（`core/state.rs:34-44`）：`return_ip`/`locals_base`/`local_count`/`func_name: String`/`original_stack_top`/`caller_line`/`local_buffers: Vec<LocalBuffer>`。
- **宿主回调哨兵**：`call_user_function` 用 `return_ip == usize::MAX` 标识"从 host 进入用户函数"（`control.rs:214`、`:236`；`state.rs:457`），是 `qsort`/`bsearch` 回调的机制（`state.rs:384-509`，含**完整状态保存/恢复**：ip、call_stack、mem_stack_top、stack、error、finished、step_event_hit、current_line、vis_event_queue、breakpoints）。

### 1.4 JIT 四段（门 1 关键）

| 段 | 位置 | 数据结构与判据 |
|---|---|---|
| **① 热点检测** | `core/executor/mod.rs:303-308` | `ip_hits: HashMap<usize,u64>`；仅当 `inst.op ∈ {Jump, JumpIfZero, JumpIfNotZero}` 且 `target < self.ip`（回边）时 `ip_hits[target] += 1` |
| **② 录制触发** | `:311-317` | `jit_enabled && !trace_recorder.is_recording() && !jit_traces.contains_key(&ip_before) && ip_hits[ip_before] >= JIT_THRESHOLD(100)` → `trace_recorder.start(ip_before)` |
| **③ 录制** | `jit_trace.rs:71-116` | `TraceRecorder{start_ip,end_ip,instructions:Vec<Instruction>,recording}`。三出口：**`Finish`**（backward jump 回到起点，`:104-108`）、**`Abort`**（遇 `Call/CallPtr/CallHost/Ret/RetVoid` `:77-83`；超 `MAX_TRACE_LEN=256` `:90-96`；跳转 taken 且非回起点 `:109-112`）、`Continue`。**`StepEvent` 是透明指令：不入 trace、不终止录制（`:86-88`）** |
| **④ 编译** | `jit_templates.rs:642-666` | `opcode_to_jit_fn` 映射到 **47 个模板函数**（49 个显式臂；`Add/UAdd`、`Sub/USub`、`Mul/UMul` 等 **6 组共用同一模板**）；未覆盖走 `tpl_generic`（`:567-576`）回退 `dispatch_single_instruction` |
| **⑤ bulk 执行** | `jit_templates.rs:694-752` | 最多 `MAX_TRACE_ITERATIONS=1000` 轮；每轮 `execute_trace_once`（逐 entry 调 `JitFn`，`:677-687`）；条件跳转 taken 且 ip≠起点 → 退出；`ends_with_conditional` 且 ip 仍在起点 → 条件为假、`set_ip(trace.end_ip)` 退出 |

**`JitFn`（`:14`）= `fn(&mut VitroVM, i32, i32, &SourceLoc, &mut VmContext) -> Option[StepResult]`** —— 纯函数指针 + 3 个标量参数，**无闭包捕获**。`JitEntry`（`:17-25`）= `{func, arg0(operand), arg1(generic 时携带 opcode u8), loc, is_conditional_jump}`。

> **症状治疗**：`compile_trace` 用 **函数指针地址比较**判定是否 generic（`func as usize == (tpl_generic as *const ()) as usize`，`:647`）—— 文件头注释 `:23-24` 自称"避免依赖函数指针地址比较"却仍在使用（部分缓解：`is_conditional_jump` 已是显式字段）。

**fast path 守卫（2026-09-13 P0 修复本体）**：`executor/mod.rs:18` `if self.jit_enabled && !self.trace_recorder.is_recording()`，注释 `:14-17` 写明"录制期间必须禁用：否则外层 trace 推进到已 JIT 化的内层循环头时 bulk 一次跑完内层，外层 trace 缺失内层指令却被注册（R-2026-09-13 静默错值）"。

### 1.5 快照体系（门 3 核心）

#### 1.5.1 `VMSnapshot` 全字段清单（22 个，逐字段列；`snapshot.rs:55-80`）

| # | 字段 | 类型 | 语义/风险 |
|---|---|---|---|
| 1 | `memory` | `MemoryImage`（`Full(Vec<u8>)` \| `Delta{base_step, pages: Vec<(u16,Vec<u8>)>}`，`:9-15`） | 全量 1MB / 4KB 脏页增量 |
| 2 | `stack` | `Vec<u64>` | 值栈 |
| 3 | `call_stack` | `Vec<CallFrame>` | **唯一非标量嵌套**（内嵌 `String` + `Vec<LocalBuffer>`） |
| 4 | `ip` | `usize` | — |
| 5 | `mem_stack_top` | `u32` | — |
| 6 | `step_count` | `i32` | — |
| 7 | `current_line` | `i32` | — |
| 8 | `finished` | `bool` | — |
| 9 | `exit_code` | `i32` | — |
| 10 | `error` | `String` | trap 文本 |
| 11 | `paused` | `bool` | — |
| 12 | `cancelled` | `bool` | — |
| 13 | `step_event_hit` | `bool` | — |
| 14 | `last_snapshot_step` | `i32` | — |
| 15 | `snapshot_vars` | `HashMap<String,u64>` | 无限循环诊断用（10 万步级刷新） |
| 16 | `qsort_depth` | `i32` | — |
| 17 | `vis_event_queue` | `Vec<VisEventData>` | 有界 1024（`executor/debug.rs:6`） |
| 18 | `breakpoints` | `HashSet<i32>` | — |
| 19 | `global_count` | `usize` | **恒 0**（历史遗留，`:264` 注释点名"旧实现以 `global_count` 编址…预存 bug"） |
| 20 | **`freed_logs`** | `BTreeMap<u32,FreedRegionInfo>` | **UAF 检测窗口，必须往返** |
| 21 | `runtime` | `RuntimeSnapshot`（11 字段） | — |
| 22 | `memory_state` | `MemorySnapshot`（8 字段） | — |

**`RuntimeSnapshot`（11 字段，`:84-102`）**：`output: Arc<OutputLog>`、`trace: Arc<Vec<TraceEntryData>>`、`current_line`、`input_index`、`input_char_offset`、`stdin_eof`、`heatmap: Arc<ExecutionHeatmap>`、`waiting_input`、`rand_seed`、`vis_event_cache`、`ungetc_char`。

**`MemorySnapshot`（8 字段，`:106-118`）**：`regions`、`free_list`、**`quarantine`**、**`quarantine_bytes`**、**`quarantine_budget`**、`heap_base`、`heap_offset`、`alloc_counter`。

**`VMSnapshot` 无 serde**（仅 `#[derive(Clone)]`，`:54`）→ 快照是**进程内结构、不过 wire**。但 `RuntimeState` 本身是 `Serialize/Deserialize`（`runtime_state.rs:71`）且含 `Arc` 字段 → 需要 `serde` 的 `rc` feature（U2#4 引入）。**这个区分对迁移很重要：快照无需兼容 wire，输出/状态出口需要。**

#### 1.5.2 隔离区三字段的往返语义（门 3 验收点）

- `quarantine: VecDeque<FreeBlock>`（FIFO，队首最老）、`quarantine_bytes`、`quarantine_budget` 三者同步维护（`memory_state.rs:143-151`）。
- `restore` 时**必须三件套一起恢复**（`core/snapshot.rs:171-173`，注释 `:164-165` 明文"否则回退后 UAF 检测出现假阴性"）。
- `regions` 整体重装后**必须调 `rebuild_region_index()`**（`:169`）——`region_index` 是 `#[serde(skip)]` 的 addr→下标索引，**不入快照**，漏调则"恢复后所有按 addr 的 O(1) 定位失配"。
- ⚠️ **口径不统一**：`quarantine_budget` 是**会话级配置**（capi `vitro_set_quarantine_budget`），却随快照往返；而 `max_steps`/`call_depth_limit` 被 `reset()` 刻意保留、不进快照。⇒ 回退到过去会连隔离预算一起回退。标〔待证〕（附录 A-5）。

#### 1.5.3 `CheckpointManager`（`snapshot.rs:161-331`）

| 项 | 值/算法 | 位置 |
|---|---|---|
| `PAGE_SIZE` / `PAGE_COUNT` | 4096 / 256 | `:46-47` |
| `interval` | 统一模式构造为 **20** | `unified/engine.rs:51` |
| `smart_mode` / `max_checkpoints` / `full_every` | `true` / **50** / **5** | `:174-183` |
| `should_checkpoint` | 固定间隔保底 **或** 语义标签命中（`调用 `/`返回`/`内存分配`/`释放内存`/含`交换`/`循环`）+ 最小间隔 `interval/4` | `:188-219` |
| `save` | `len % full_every == 0` 取全量，否则增量（`base_step` = 链中最近 Full） | `:224-244` |
| `evict_over_limit` | **step-0 锚点永不裁剪**；**删 Full 时级联 drain 到下一个 Full**；删 Delta 不级联 | `:257-284` |
| `nearest` | 找 ≤ target 的最近检查点；Delta 则从最近 Full 起逐个 `apply_to` 重建为 Full（**`m.clone()` 1MB**） | `:287-316` |

#### 1.5.4 **不在快照里的状态（漏字段风险面）**

VFS（`files`/`descriptors`）、`argc`/`argv_addr`（`state.rs:124/126`）、`dirty_pages`（`:128`）、**JIT 全部状态**（`ip_hits`/`trace_recorder`/`jit_traces`/`jit_stats`）、`local_sym_map`/`global_sym_map`、`last_accessed_vars`、`max_steps`/`call_depth_limit`，以及全部编译期产物（`code`/`func_table`/`symbols`/常量池 —— 设计上从 `Session` 重建，`snapshot.rs:51-53` 有明文说明）。

**VFS 缺失是已登记的 P1**（`代码审阅与修复追踪20260906.md:187`："VFS 不纳入 VMSnapshot，时间旅行回退后文件状态来自'未来'"）。注：`vfs.rs:687-705` 已有 `snapshot_files`/`restore_files`，但**全仓零调用点**（死代码）。

### 1.6 1MB 线性内存的全部访问模式（门 1 的完整设计输入）

**内存本体**：`memory: Vec<u8>`，`vec![0; 1MB]`（`state.rs:80`、`:151`），`reset()` 时 `fill(0)`（`:231`）。

| 类 | API | 实现 | 检查 |
|---|---|---|---|
| **A. 单字节读** | `load_i8` | `memory[a] as i8 as i32`（`core/memory.rs:151`） | NULL 区 + 上界 + UAF |
| **B. 单字节写** | `store_i8` | `memory[a] = val as u8`（`:163`） | 同上 |
| **C. u32 读** | `load_i32` | **4 次独立索引** + `i32::from_le_bytes([m[a],m[a+1],m[a+2],m[a+3]])`（`:92-97`） | 同上（`size=4`） |
| **D. u32 写** | `store_i32` | `to_le_bytes()` + `memory[a..a+4].copy_from_slice`（`:109-110`） | 同上 |
| **E. u64 读/写** | `load_i64`/`store_i64` | `copy_from_slice(&memory[a..a+8])` + `from_le_bytes` / `to_le_bytes`（`:123-125`、`:137-138`） | 同上（`size=8`） |
| **F. 字符串** | `write_cstring`（`:14-30`） | `as_bytes()` + `copy_from_slice` + 写 `\0` | 三重 |
| | `read_cbytes`/`read_cstring`（`host/utils.rs:4-16`） | 逐字节 `take_while(!=0)`；**`from_utf8_lossy`** | **仅下界**，无 NULL/UAF |
| **G. 批量** | `write_memory`/`read_memory_to`/`copy_memory`（`:33-81`） | `copy_from_slice`；`copy_memory` 用 `to_vec()` 避重叠（`:77-78`） | `read_memory_to` 无 UAF |
| | `OpCode::Memcpy`（`executor/memory.rs:195-230`） | `to_vec()` + **逐字节 for 循环写**（`:225-227`） | NULL+上界+双向 UAF |
| | `OpCode::Memset`（`:231-254`） | `slice.fill(byte_val)` | NULL+上界+UAF |
| | host `memset`/`memcpy`/`memmove`（`host/string.rs:165-166`、`:252-259`、`:275-282`） | 裸切片 / `to_vec()` | **无 UAF、无 NULL** |
| **H. 快照** | `Full` | `self.memory.clone()`（1MB，`core/snapshot.rs:12`） | — |
| | Delta 页 | `memory[off..off+4096].to_vec()`（`:50`） | — |
| | `apply_to`/`snapshot_into` | `copy_from_slice`（`snapshot.rs:32`、`core/snapshot.rs:87`） | — |

**每类访问的固定开销链（门 1 关键）**：任何受检访问 = ① `check_mem_access`（NULL 区比较 + `addr+size > MEM_SIZE` 的 **u64 提升比较**，`:202-220`）→ ② `check_uaf`（**BTreeMap `range(..end).next_back()` 区间查询**，`:188-192` + `state.rs:747-755`）→ ③ 实际字节操作 → ④ `mark_dirty_page`（**除法定位页** + 位图 `|=`，`:168-181`）。

⇒ **每条指令的内存访问至少含一次有序树查询与一次页位图更新**。这是 MoonBit 侧必须逐项实测的成本基线。

**`freed_logs` 的有序性依赖（正确性，非性能偏好）**：
- `freed_logs_remove_overlapping`（`state.rs:694-743`）用 `range(..end).rev()` **降序扫描 + `l_end <= start` 提前 break**，正确性建立在"块互不重叠 ⟹ addr 序即 end 序"（注释 `:112-119` 明写）。
- `freed_logs_find_overlapping`（`:747-755`）用 `range(..end).next_back()` **单次探测**。
- ⇒ **MoonBit 侧必须有"有序映射 + range 查询"或"有序数组 + 二分"的等价物**；普通 hash map 会破坏该算法（不是慢，是**静默漏检 UAF**）。

**"同名三实现"完整样本 —— `memset`**：

| 实现 | 位置 | 谁发射 |
|---|---|---|
| VM 内联 `OpCode::Memset` | `executor/memory.rs:231-254`（**有 `check_uaf`**） | 数组初始化：`vitro_codegen/src/stmt/var_decl.rs:464`、`:556`、`:633` |
| Rust host `host_memset` | `host/string.rs:146`（**裸切片，无 UAF**） | 用户按名调用 `memset`（未被 Bytecode 遮蔽） |
| Bytecode Libc | 固定索引段 | 无（`memset` 不在 88 个预注册名中） |

⇒ **同一句 `memset(p,0,8)` 在"数组初始化形状"与"函数调用形状"下 UAF 检测行为不同** —— "host 层绕过检测"是**按发射形状分叉的**，比"笼统绕过"更精确。

### 1.7 host funcs 全量清单（110 个，四路计数互证）

**计数**：常量 **110**（`host_func_id.rs:6-144`）= 分发臂 **110**（`host_funcs.rs:35-144`）= handler **110**（`host/` 下 108 个 `pub fn host_*` + `host_funcs.rs:149/158` 两个私有）= 名字映射 **109 行 / 110 模式**（`:167` 一行含 `"print_int" | "__vitro_output"`）→ 108 个不同 id（`OUTPUT`、`ABS` 各被 2 个模式映射）。

ID 空间稀疏（4→15、21、49→50、58→60、66→70、84→85、89→90、105→106、108→109、114→115、124/125→126、130→131、137→140；且 131/132 物理位置在 127 之前）。

| 族 | 数 | 成员（id = 值）与定义位置 |
|---|---|---|
| **A 引擎内建** | 6 | `OUTPUT=0`(`host_funcs.rs:158`)、`STEP=1`(`:149`)、`ASSERT_FAIL=125`(`host/misc.rs:545`)、`SET_ARRAY_GUARD=131`(`:550`)、`CLEAR_ARRAY_GUARD=132`(`:558`)、`UNREACHABLE=144`(`:594`)。**后两者无名字映射**，仅 codegen 发射（`vitro_codegen/src/expr/new_delete.rs:87`、`:134`） |
| **B 堆** | 4 | `MALLOC=2`/`FREE=3`/`REALLOC=51`/`CALLOC=80` → `host/memory.rs:5/60/158/313`；辅助 `trap_invalid_free:106`、`report_heap_exhausted:144` |
| **C printf/scanf/字符** | 10 | `PRINTF_N=15`(:4)、`SCANF_N=21`(:25)、`GETCHAR=33`(:286)、`PUTCHAR=34`(:344)、`FPRINTF=50`(:349)、`UNGETC=78`(:279)、`PUTS=79`(:376)、`SPRINTF=82`(:383)、`SNPRINTF=83`(:405)、`SSCANF=84`(:431)（全部 `host/io.rs`）。**不存在 `fscanf`/`vprintf`/`vfprintf`/`vsnprintf`/`vscanf`** |
| **D VFS 文件** | 17 | `FOPEN=60`(:4)、`FREAD=61`(:63)、`FWRITE=62`(:76)、`FCLOSE=63`(:89)、`FEOF=64`(:102)、`FGETS=65`(:109)、`FPUTS=66`(:119)、`FGETC=85`(:146)、`FPUTC=86`(:155)、`FSEEK=87`(:165)、`FTELL=88`(:176)、`REWIND=89`(:185)、`FFLUSH=120`(:194)、`PERROR=121`(:208)、`CLEARERR=122`(:220)、`REMOVE=126`(:227)、`RENAME=127`(:236)（全部 `host/file.rs`）。VFS 本体 `vfs.rs` |
| **E 字符串/内存** | 19 | `STRLEN=30`(:4)、`STRCPY=31`(:73)、`STRCMP=32`(:118)、`MEMSET=37`(:146)、`STRCAT=39`(:170)、`STRNCPY=56`(:219)、`MEMCPY=57`(:241)、`MEMMOVE=58`(:264)、`STRDUP=77`(:10)、`STRNCAT=90`(:287)、`STRNCMP=91`(:311)、`MEMCMP=92`(:342)、`STRCHR=93`(:367)、`STRRCHR=94`(:389)、`STRSTR=95`(:408)、`MEMCHR=96`(:426)、`STRPBRK=128`(:446)、`STRSPN=129`(:460)、`STRCSPN=130`(:476)（全部 `host/string.rs`） |
| **F 字符串→数值** | 6 | `ATOI=40`(:126)、`ATOF=97`(:329)、`ATOL=98`(:336)、`STRTOL=117`(:368)、`STRTOD=118`(:417)、`STRERROR=119`(:461)（全部 `host/misc.rs`） |
| **G 数学** | 22 | `ABS=41`(:4)、`SIN=70`(:11)、`COS=71`(:16)、`SQRT=72`(:21)、`POW=73`(:26)、`ATAN=74`(:32)、`LOG=75`(:37)、`EXP=76`(:42)、`TAN=99`(:47)、`LOG10=100`(:52)、`FABS=101`(:57)、`CEIL=102`(:62)、`FLOOR=103`(:67)、`ROUND=104`(:72)、`FMOD=105`(:77)、`ASIN=109`(:83)、`ACOS=110`(:88)、`ATAN2=111`(:93)、`SINH=112`(:99)、`COSH=113`(:104)、`TANH=114`(:109)、`LLABS=115`(:114)（全部 `host/math.rs`，走 `libm` + `from_bits/to_bits`）。`labs` 无独立 id，别名映射到 `ABS`（`host_func_id.rs:254`） |
| **H ctype** | 14 | `ISDIGIT=42`(:21)、`ISALPHA=43`(:26)、`ISLOWER=44`(:37)、`ISUPPER=45`(:42)、`TOLOWER=46`(:47)、`TOUPPER=47`(:56)、`ISSPACE=48`(:65)、`ISALNUM=49`(:82)、`ISPRINT=53`(:89)、`ISCNTRL=54`(:94)、`ISXDIGIT=55`(:99)、`ISGRAPH=106`(:107)、`ISPUNCT=107`(:112)、`ISBLANK=108`(:121)（全部 `host/misc.rs`） |
| **I 其他** | 12 | `RAND=35`(:4)、`SRAND=36`(:11)、`EXIT=38`(:16)、`QSORT=52`(:153)、`BSEARCH=81`(:234)、`ABORT=116`(:363)、`TIME=123`(:521)、`CLOCK=124`(:533)、`VA_START=140`(:566)、`VA_ARG=141`(:576)、`VA_END=142`(:587)、`VA_COPY=143`(:603)（全部 `host/misc.rs`） |

**分发机制**：`OpCode::CallHost=33`（`opcode.rs:57`）→ 归类到控制流（`executor/mod.rs:232-239`）→ `execute_control_flow` 的 `CallHost` 臂（`control.rs:197-204`，**全仓唯一调用点**）→ `execute_host_func(vm, session, id)`（`host_funcs.rs:33`）→ `match id`（编译为跳转表，无运行时表）。参数经值栈传递，`waiting_input` 时 `ip -= 1` 并返回 `WaitingInput`（可续跑）。

> **防御性缺口**：**未知 id 走 `_ => {}` 静默 no-op**（`host_funcs.rs:145`）—— 不 panic、不 trap、不压返回值 → 值栈失衡。正常编译不可能产生未知 id（`by_user_name` 只返回表内常量），但**无 fail-loud 护栏**。

**路由遮蔽（重要派生结论）**：`vitro_codegen/src/lib.rs:136-143` 把 88 个 `BYTECODE_LIBC_ALL_FUNCS` **预注册**进 `func_index`（**只跳过 `strcpy`/`strcat`**，理由：Host 版带 E3070 栈缓冲校验）；调用点 `expr/call.rs:214` 先查 `func_index` → 命中发 `OpCode::Call <固定索引>`，未命中才走 `:237` 的 `by_user_name → CallHost`。

⇒ **110 个 host handler 中有 20 个在"用户按名调用"路径上被遮蔽**：`ISDIGIT ISALPHA ISLOWER ISUPPER TOLOWER TOUPPER ISSPACE ISALNUM ISPRINT ISCNTRL ISXDIGIT ABS ATOI SRAND RAND STRLEN STRCMP STRNCPY MEMCPY MEMMOVE`。生效的 host 路径 = 110 − 20 = **90 个 id 可按名触达**（外加 131/132 由 codegen 直接发射）。

**同族分裂**：`atoi` 走 Bytecode 而 `atof/atol/strtol/strtod` 走 Host；`iscntrl/isxdigit/isprint` 走 Bytecode 而 `isgraph/ispunct/isblank` 走 Host；`memcpy/memmove/strncpy` 走 Bytecode 而 `memcmp/memchr/memset` 走 Host。

**与 `freed_logs` 的耦合点（host 层 13 处 + core 层）**：

| # | 位置 | 动作 |
|---|---|---|
| 1 | `host/memory.rs:27` | malloc 后清重叠 |
| 2 | **`host/memory.rs:66-70`** | **free 查 Double-Free → trap E3061 并 return（不执行隔离区释放）** |
| 3 | `host/memory.rs:78-88` | free 登记（`alloc_step:0`、`freed_step:当前步数`） |
| 4 | `host/memory.rs:120` | invalid-free 查重叠（E3061 变体） |
| 5 | `host/memory.rs:172-182` | `realloc(p,0)` 等价 free 登记 |
| 6–8 | `host/memory.rs:229-232`、`:256-266`、`:279` | realloc 三次清理/登记（`:232` 在写新内存前，防 UAF 误报） |
| 9 | `host/memory.rs:336` | calloc 清理 |
| 10 | `host/string.rs:29` | strdup 清理 |
| 11 | `host/file.rs:21` | fopen 的 `FILE*` 清理 |
| 12 | `host/misc.rs:485` | strerror 缓冲清理 |
| 13 | `core/memory.rs:188-192` | **`check_uaf` 本体**，被 9 个受检访问器 + 12 个 opcode 分支调用 |
| 14 | `core/state.rs:642-671` | `free_memory`（`new[]` 构造回滚路径） |

**两个已存在的耦合缺口**：① `host_fclose` → `memory.free_region(stream)`（`host/file.rs:97` → `memory_state.rs:339-355`）**只置 `is_freed` 不写 `freed_logs`** → FILE* 二次 fclose 不报 E3061；② `host_memset/memcpy/memmove` 裸切片**写已释放块不触发 E3060**（U5#1 已登记）。另 `host_strdup` 堆耗尽时静默 `push(0)` 且不调 `report_heap_exhausted`（`host/string.rs:20-26`，对比 `host/memory.rs:19-23`）。

**错误码是"字符串协议"**：`E3060`/`E3061`/`E3027`/`E3070` 的 `ErrorCode` 枚举变体**定义了但全仓零引用**（`vitro_shared/src/error_codes.rs:67,100,101,114`）；`trap()` 只写 `self.error: String`（首错优先，`core/trap.rs:145-156`），下游靠 `contains("Use-After-Free")||contains("E3060")` 识别（`unified/trace_analyzer/mod.rs:43-45`）。**迁移时必须一并搬运或结构化**。

**边界检查现状（逐点）**：多数写路径走受检访问器（`store_i8/i32/i64`、`write_memory`）；**例外**：`host/string.rs:165-166`（memset，手写 NULL trap + 三重 min，无 UAF）、`:252-259`/`:275-282`（memcpy/memmove，无 UAF 无 NULL）、`host/file.rs:52-54`（地址刚分配，界内无风险）。`qsort`/`bsearch` 有 `checked_mul`/`checked_add`（`host/misc.rs:171-177`、`:254-261`）与 key 越界检查（`:268-271`）。

**两个 host 层口径缺陷**：① **`sprintf`/`snprintf`/`sscanf` 缺栈深守卫**（`printf` 有 `io.rs:8-11`、`fprintf` 有 `:354-357`，三者没有）→ 参数不足时落到 `pop()` 的"栈下溢"而非教学诊断；② **6 个字符串比较/查找函数用"越界当 0"软夹紧**（`strcmp:118-143`、`strncmp:311-340`、`memcmp:342-365`、`strchr:367-387`、`strrchr:389-406`、`memchr:426-442`）→ 与 `strlen` 的"扫到内存末尾即停"（`executor/memory.rs:270-275`）不成体系。

### 1.8 Bytecode Libc 加载器与固定索引段

**产物**：`native/crates/vitro_vm/src/bytecode_libc_data.json`（**550.8 KB / 32,990 行**），`include_str!` 编译期嵌入（`bytecode_libc_loader.rs:38`）。字段：`version=1`、`code`（**3,485 条指令**）、`func_table`/`func_index`（**88 条**）、`globals_init_32/64`、`string_data`、`f64_constants`、`i64_constants`、`globals_size=4`、`source_digest`。

**加载**：`serde_json::from_str` → 失败 **panic** 并提示 `go run ./scripts/precompile_bytecode_libc`（`:41-46`）；加载后**无条件把所有指令 `loc.file_id` 置 1**（`:49-51`，把 libc 排除出用户覆盖率）。**`version` 与 `source_digest` 运行时不校验**（结构体无 digest 字段，serde 静默忽略未知键）。

**固定索引段**：`BYTECODE_LIBC_CODE_LEN=3485`、`BASE_INDEX=1000`、`GLOBALS_RESERVED=1024`、`FUNC_COUNT=88`（`bytecode_libc_index.rs:7-10`）。

| 区间 | 内容 |
|---|---|
| 1000–1010 | ctype 11（isdigit…isxdigit） |
| 1011–1014 | abs, atoi, srand, rand |
| 1015–1021 | strlen, strcmp, strcpy, strcat, strncpy, memcpy, memmove |
| 1022 | `__vitro_force_instantiate_vitro_sort_int` |
| 1023–1087 | C++ 容器/模板符号 66 个（`vitro_list_int` / `vitro_string` / `vitro_vec_{int,float,char}` 各 11、6 个 `__move` 构造、3 个 sort 符号） |
| **1088** | **保留空洞** |
| **1089+** | **用户函数起点**（`vitro_codegen/src/lib.rs:144`） |

全局区：用户侧 `next_global_offset` 从 1024 起；library mode 从 0 起（`:176-188`，防"每次重生成膨胀 1KB"的自引用漂移，注释记 `lc_67`/`lc_76` 事故）。

**拼接与重定位**（`compile_pipeline.rs:243-372`）：load → 用户代码 `Jump/JumpIfZero/JumpIfNotZero` 的 operand `+= libc_code_len`（`:254-263`）→ 合并 `libc.code ‖ user_code`（`:265-268`）→ 注册 libc 函数（按固定索引，`:275-295`）→ 注册用户函数（IP + `libc_code_len`，`:297-316`）→ 全局初始化/常量池/字符串数据/入口。**注册顺序即覆盖语义**（先 libc 后用户，同索引后写覆盖，`core/state.rs:301-328`）。

**签名表**（`bytecode_libc_sig.rs`，108 行）：`bytecode_libc_sig(name) -> Option<(Type, Vec<Type>)>`，**给 TypeChecker 用**（唯一消费者 `vitro_typeck/src/decl.rs:827`，用户函数查找失败后的 fallback，校验实参个数 E3037 + 逐参 `check_assignable`）；覆盖 **76/88**，未列入的 12 个是容器构造/`__move` 构造/`__vitro_force_instantiate_*`。与 index 表、产物 `func_table` **三处各自手写、无一致性门禁**。

**`source_digest` 协作链（四段，全程无 codegen 参与）**：
1. 计算与注入（Go，唯一真值源）：`sha256("schema=1\n" + Σ(posix 相对路径 + \0 + CRLF→LF 归一内容 + \0))`，覆盖 `native/runtime_libc/{src,vitro}/` 下 7 个源文件（`main.go:92-107`）；注入点 `:219-220`（在 `vitro_cli export` 与固定索引重定位 `:171-217` 之后）。
2. 产物形式：`bytecode_libc_data.json:32987` 的 `"source_digest": "sha256:1fdb34bf…"`。
3. 比对侧（唯一消费者）：`main.go:427-458` `checkUpToDate()`，失配 → `ERROR: Precompiled artifacts are out-of-date` + **exit 1**；CI 接线 `.github/workflows/ci.yml:40-45`。
4. **运行时不比对**（§0-2）。

**两处口径缺口**：① `includeHeaders=true` 的设计意图（`main.go:62-63`）**空转** —— `srcDirs` 只含 `src`/`vitro`，而 14 个头文件全在 `native/runtime_libc/include/` → **改头文件不会让 `--check` 变红**；② `checkUpToDate` 对 `bytecode_libc_index.rs` **只做 `os.Stat` 存在性检查**，不比对内容 → .rs 与 JSON 不同步**无门禁可发现**。

**`OpCode::Strlen`（`opcode.rs:126`）是死路径**：VM 有实现（`executor/memory.rs:255-274`）且有 3 个单测（`executor/mod.rs:376-414`），但 **codegen 从不发射**（`strlen` 走固定索引 1015）。

### 1.9 `RuntimeState` 的三处 `Arc`（A5 假设的直接证据）

| 字段 | 声明 | 原地修改点 | 修改语义 |
|---|---|---|---|
| `output: Arc<OutputLog>` | `runtime_state.rs:89` | `push_stdout`/`push_stderr`/`push_note`/`clear_output`（`:133-150`、`:163-165`），全部 `Arc::make_mut` 后调用 `OutputLog` 的 `&mut self` 方法 | **增量 append**（≤64B 小段合并、16MB 预算环形丢弃、note 不占预算不丢弃） |
| `trace: Arc<Vec<TraceEntryData>>` | `:94` | `push_trace`（`:153-160`）：`make_mut` → `push` → 超 `TRACE_LIMIT=4096` 则 `drain(..overflow)` | **环形 push + 头部裁剪**（`Vec` 头删 O(n)） |
| `heatmap: Arc<ExecutionHeatmap>` | `:118` | `executor/mod.rs:296`：`Arc::make_mut(...).record(inst.loc.line)` —— **每步一次** | **每步 HashMap 计数**（`entry().or_insert(0) += 1`） |

**对 A5（不可变结构无代价替代 Arc）的实测判定**：

1. **三处都是"每步原地增量修改"**，不是"只在快照点冻结"。"不可变结构天然可共享"因此**需要一个持久化/结构共享的数据结构**，不是零成本——**`heatmap` 最严重**（每步一次 map 更新，是热路径）。
2. **`Arc` 的实际语义被高估**：`Arc::make_mut` 在 `strong_count > 1` 时**克隆整个结构**（`OutputLog` 含 `VecDeque<OutputChunk>`；`Vec` 含最多 4096 条；`ExecutionHeatmap` 含 HashMap）。快照（`RuntimeSnapshot`）持一份引用、50 个检查点各持一份 → **只要还有任何快照存活，"窗口内首次写入"就触发一次全量克隆**。⇒ 现版是"**每快照窗口一次全量克隆 + 窗口内独占写入**"，**不是"永不复制"**。
3. **`OutputLog` 内部状态无法做成纯不可变**：`total_bytes`/`kind_bytes[3]`/`dropped_bytes`/`truncation_noted` 是增量记账（`output.rs:128-138`），`enforce_budget` 会**从头部部分截除**（`:265-312`）—— 这是"有界环形缓冲"的本质。

⇒ **A5 的方向成立但代价未量化**：`output`/`trace` 可用"共享不可变 + 持久化追加结构"或"有界环形 + 快照时冻结"；**`heatmap` 必须重新设计**（分片计数 + 快照时合并，或直接深拷 —— 它是三处中最小最冷的）。

### 1.10 数据结构清单（`vitro_runtime` 其余部分）

- **`OutputLog`**（`output.rs:126-339`）：`chunks: VecDeque<OutputChunk>` + `total_bytes` + `kind_bytes[3]` + `dropped_bytes` + `truncation_noted` + `budget`(16MB)。`OutputKind::{Stdout,Stderr,Note}`（`:24-37`）+ `OutputChunk{kind,text}`（`:71-75`）。**这是 E-P1-5 的核心资产**：把"程序 stdout / 程序 stderr / 引擎附注"在源头分流，废除了十余处正则清洗。
- **`MemoryState`**（`memory_state.rs:130-158`，8 字段）：`regions: Vec<MemoryRegionData>` + **`region_index: HashMap<u32,usize>`（`#[serde(skip)]`，快照后必须 `rebuild_region_index`）** + `free_list: Vec<FreeBlock>` + `quarantine: VecDeque<FreeBlock>` + `quarantine_bytes` + `quarantine_budget` + `heap_base` + `heap_offset` + `alloc_counter`。堆模型 = **bump + 有界隔离**（`allocate_raw:257-272` → `evict_quarantine` → `take_from_free_list` → bump）。
- **`ExecutionHeatmap`**（`runtime_state.rs:37-56`）：`line_counts: HashMap<i32,u64>` + `record`/`max_count`/`clear`。
- **`Instruction`**（`instruction.rs:5-10`）：`{op: OpCode, operand: i32, loc: SourceLoc}`；`SourceLoc` 来自 `vitro_shared::source_loc`（`{line, column, file_id}` 三个 i32，`executor/mod.rs:366` 的测试构造可见）。
- **`FuncMeta`**（`func_meta.rs:15-35`）：`ip`/`arg_count`(总 word 数)/`param_count`/`local_count`/`param_sizes: Vec<i32>`/`return_type`/`is_variadic`/`local_buffers: Vec<LocalBuffer>`。`LocalBuffer{offset,size,name}`（`:5-13`，V-P1-6 栈缓冲容量校验用）。
- **`FreedRegionInfo`**（`core/state.rs:22-32`）：`addr`/`size`/`alloc_line`/`freed_line`/`alloc_step`/`freed_step`。
- **`Symbol`**（`symbol.rs:3-18`）：`name`/`addr`/`is_local`/`ty: vitro_ast::Type`/`scope_depth`/`func_name`/`decl_line`。
- **`type_utils`**（114 行）：`base_kind`/`immediate_base_kind`/`type_display_name`（类型名单一来源，P2-7c）。
- **`unified_types`**（47 行）：`ArraySnapshotData`（含 `truncated`）/`PointerSnapshotData`/`PointerStatusData{Valid,Freed,Null,Dangling}`/`AccessedVarData`/`MAX_ARRAY_SNAPSHOT_ELEMENTS=256`。

---

## ② 可复用资产清单（标注移植成本）

### 2.1 语义设计（可直接照抄的"已决设计"）

| 资产 | 证据 | 成本 | 理由 |
|---|---|---|---|
| **扁平快照 schema**（22+11+8 字段）+ `MemoryImage` Full/Delta 二分 | `snapshot.rs:9-15`、`:55-118` | **低** | 纯数据结构，record + enum 一一对应；最大改动是 `Arc` 三处（§1.9） |
| **检查点淘汰不变量**（step-0 永不裁剪 / 删 Full 级联删 Delta / 删 Delta 不级联） | `snapshot.rs:257-284` | **低** | 算法与语言无关；含 3 条红锚可照搬（`:388-483`） |
| **堆模型：bump + 有界隔离（FIFO 驱逐 + first-fit）** | `memory_state.rs:257-355` | **低** | 纯算法，可直接翻译 |
| **`freed_logs` 区间结构 + 精确裁剪算法** | `state.rs:694-755` | **中** | 算法无关，但**依赖有序映射 range 查询 + 降序提前 break** → 需先确认 MoonBit 等价容器（⑦ spike-6） |
| **UAF/Double-Free 检测时机与语义** | `core/memory.rs:188-192` + 13 耦合点 | **低** | 语义清晰；需一并搬运**字符串错误码**或改造为结构化 |
| **Bytecode Libc 三段式流水线** | `main.go:171-220` + `compile_pipeline.rs:243-372` | **中** | 模式与语言无关（Go 脚本可原样保留）；但**产物 `code` 是 Vitro 自有 ISA 镜像**（`{"op":"Nop","operand":0,"loc":{…}}`），opcode 助记符与 `StepEvent` 等教学指令需随 opcode 表重建 |
| **VFS-lite 设计**（fd 表 + 数据存 VM 堆 + 文本模式 CRLF 转换） | `vfs.rs:44-86`（logical/physical 偏移互转 + `read_text_byte`） | **中** | 语义可搬；**路径是 `HashMap<String,_>` 的 UTF-8 键**（`vfs.rs:15`），MoonBit String 是 UTF-16 → 路径键与字节语义都需重新设计 |
| **输出通道三分类（stdout/stderr/note）** | `output.rs:24-37`、`:1-16` 模块头（E-P1-5 完整论证） | **低** | 口径资产，直接决定 shadow 比对合法性 |
| **JIT"模板超级指令"策略**（无动态代码生成） | `jit_templates.rs:1-6`（注释明写因 `forbid(unsafe_code)` 无法生成机器码） | **中** | **与 MoonBit/wasm 天然契合**（wasm 同样不允许运行时代码生成），是最佳"沿革保留"候选；函数指针 → FuncRef/闭包需重做（⑦ spike-5） |
| **JIT 作用域收缩原则**（只 JIT 最内层循环） | `core/executor/mod.rs:14-18` | **低** | 一行守卫的语义，设计意图完整记录在 `jit_nested_counting_loop.c` 头注 |
| **调用约定**（值栈位模式 + 局部在线性内存 + `usize::MAX` 宿主回调哨兵） | `control.rs:31-122`、`:214/236` | **中** | 语义必须逐条搬运（与 golden 位级一致相关）；哨兵在 MoonBit 需换表达（`Option`/显式标签） |

### 2.2 测试与防线资产

| 资产 | 位置 | 成本 | 理由 |
|---|---|---|---|
| **5 个 JIT baseline `.c` + `.out`** | `cases/baseline/jit_{nested_counting_loop,nested_counting_loop_longlong,single_hot_loop,trace_overflow_abort,zero_init_trace_boundary}.c` + `cases_golden/baseline/*.out` | **低** | 源码含红→绿留痕与手算推导，golden 纯文本 |
| **7 个 JIT 差分形状 + 手算期望** | `jit_path_parity.rs:112-229` | **低** | 可机械抽成 JSON（shape + expected_stdout + ret + expect_accel） |
| **三锚判据语义**（parity / 手算期望 / 生效性 J9） | `jit_path_parity.rs:10-14` | **低** | 方法论，语言无关 |
| **`vm_bench` 方法学四条** | `vm_bench.rs:3-18`、`:83-112` | **中** | 方法学可搬；**"JIT 关"这一路当前无 C ABI 出口**（§4.4 阻塞点） |
| **快照往返 3 条测试** | `snapshot.rs:388-483` | **低** | 纯数据构造 + 断言 |
| **`test_snapshot.rs` 语义套件** | `native/tests/test_snapshot.rs`（增量重建、`snapshot_into` 等价、语义标签检查点） | **中** | 依赖 `Session`/`execute_run`，需重写驱动 |
| **host 契约 / fuzz / 差分压力** | `host_contract_tests.rs`、`fuzz_stress_test.rs`、`differential_stress.rs`(**18** test)、`bytecode_libc_consistency.rs`(**12** test) | **中** | 判据可搬（区间语义、隔离窗口、复用复位），均依赖 Rust 入口 API |
| **Shadow 硬门禁（Go 驱动）** | `scripts/shadow_verify` | **低** | **与 Rust 无关**，实时调 clang 作 golden；`scripts/**.go` 对 `jit` 零命中 ⟹ JIT 用例靠"引擎默认开 JIT"被动覆盖 |
| **facts 真值台账** | `reports/facts.json`（`generated_at` = 2026-09-18T13:01:41+08:00，`rev` = `4b57191`） | **低** | `shadow_c_cases=675`/`shadow_c_match=668`（as_of 2026-09-15 00:05:33）、`c_e2e_baseline_cases=359`、`replay_assertions=61`、`serve_smoke_assertions=57` 可作迁移验收基线 |

### 2.3 文档资产

| 资产 | 位置 | 成本 | 理由 |
|---|---|---|---|
| **JIT 缺陷全链条分析**（触发条件三条 / 事件链六步 / 四组交叉验证 / 防线盲区形状表 / 修法与"禁止"项） | `核心资产重构裁定.md` §14.1–14.11 | **低** | "新语言下不再犯"的最直接输入；§14.11.5 已明示"只要仍留 ≥2 条路径，新代码同样会长出分叉" |
| **堆有界隔离决议**（ASAN quarantine 语义 / 三释放路径统一出口 / 窗口外漏检的诚实记录） | `堆有界隔离决议.md`、`C语言子集规范.md:447` | **低** | 语义契约完整，可作 MoonBit 侧实现规格 |
| **快照与时间旅行设计**（状态机 / seek 一致性契约 / 帧缓存窗口） | `统一模式设计.md` §5/§6 | **低** | 语言中立 |
| **U2 批次状态记录**（哪些资源已界化、残余是什么） | `统一整备路线图.md:713-746` | **低** | 直接决定"哪些在途工作不需要搬" |
| **`opcode.rs` / `bytecode_libc_index.rs` 的生成注释** | `opcode.rs:18-19`、`bytecode_libc_index.rs:1-14` | — | ⚠️ **两处均有漂移**（见 ①1.2、§1.8），作资产引用时须先核对 |

---

## ③ 抛弃清单

### 3.1 Rust 特有机制（必须重写）

| # | 机制 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 1 | **`JitFn = fn(…)` 函数指针 + `Arc<CompiledTrace>`** | `jit_templates.rs:14`、`core/state.rs:132` | MoonBit 无 Rust 式 fn 指针（有 `FuncRef`/闭包）；**且 `compile_trace` 用 `func as usize == (tpl_generic as *const ()) as usize` 做地址比较**（`:647`），MoonBit 无裸指针 | **中**：改 `enum JitOp {…, Generic(OpCode)} + match` 反而更干净；间接层成本需实测（⑦ spike-5） |
| 2 | **`Arc` 零拷贝快照共享（三处）** | `runtime_state.rs:89/94/118` | MoonBit 无 Arc | **中高**：见 §1.9 —— `heatmap` 每步更新不能简单换不可变结构；`output` 含增量记账与部分截除 |
| 3 | **`BTreeMap<u32,_>`（freed_logs）+ `VecDeque`（quarantine）** | `core/state.rs:120`、`memory_state.rs:145` | 需有序映射/双端队列等价物 | **高**：`freed_logs` **正确性依赖有序 range 查询**，换 hash map = 静默漏检 UAF |
| 4 | **`Vec<u8>` 作 1MB 线性内存 + `&mut [u8]` 批量** | `core/state.rs:80` | `Bytes` 不可变；`FixedArray[Byte]`/`Array[Byte]` 的 packed 与边界检查行为未知 | **高 = 门 1 本体**（⑦ spike-1/2/3） |
| 5 | **`HashSet<i32>` / `HashMap` 家族** | `core/state.rs:90,102,109,110,130,132` | 需 `@hashmap`/`@hashset` | **低**：本模块所有 HashMap **均按 key 查、无一处依赖迭代序**（已逐点验证：`snapshot_vars` 仅 `trap.rs:113`；`ip_hits` 仅 `executor/mod.rs:306/312`；`jit_traces` 仅 `:19/311/338`；`local/global_sym_map` 仅按 offset 查）。**唯一例外** `vfs.rs:689` 遍历 `self.files`，但该函数**零调用点**（死代码） |
| 6 | **`#![forbid(unsafe_code)]` 约束** | `lib.rs:1` | MoonBit 天然无 unsafe | **低（收益）**：JIT 因该约束才选模板路线，MoonBit 同样不能运行时代码生成 → 约束自动满足 |
| 7 | **serde（含 `rc`）/ `include_str!` / `serde_json`** | `Cargo.toml`、`bytecode_libc_loader.rs:38` | MoonBit 用 `derive(ToJson/FromJson)` 与编译期嵌入 | **低-中**：`RuntimeState` 的 JSON 形态是 serve/capi wire 契约，需保字段名；**`VMSnapshot` 无 serde → 无需兼容** |
| 8 | **`libm` + `f64::from_bits/to_bits`** | `host/math.rs:11-112`、`executor/float.rs` | MoonBit 有 `@math`；位转换需等价物 | **中**：浮点位精确语义（f32 常量以 u32 位模式进 i32 operand，`float.rs:6-8`；d 常量走 `f64_constants` 池，`:87-97`）必须逐位对齐 |
| 9 | **`Instant`/`SystemTime`/`js_sys::Date`** | `host/utils.rs:19-31` | MoonBit 时间 API 不同 | **低**：`deterministic` 模式已固定返回 0（`runtime_state.rs:125-128`） |

### 3.2 症状治疗代码（可借迁移一次性消除）

| # | 症状治疗 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 1 | **`tpl_generic` 靠函数指针地址比较识别** | `jit_templates.rs:647` | 用"地址相等"表达"回退路径"，缺少显式标签 | **低**：改 enum 后语义明确；注意 `arg1` 携带 opcode 值的机制需一并重建（`:646-651`） |
| 2 | **`global_count` 字段（恒 0）** | `core/state.rs:83`、`:154` | 历史实现遗留（`:264-266` 注释明写旧实现用错它） | **低**：直接删；它在 `VMSnapshot:75` → 快照字段可减 1 |
| 3 | **`OpCode::Strlen`（有实现、有单测、零发射）** | `opcode.rs:126`、`executor/memory.rs:255-274`、`executor/mod.rs:376-414` | "无生产者实现"（U5#5 已列为待裁定，`统一整备路线图.md:180`） | **低**：停机删除或显式登记为"字节码产物可用" |
| 4 | **`BYTECODE_LIBC_PURE_FUNCS` + `is_bytecode_libc_pure` 冗余护栏** | `host_func_id.rs:149-157`、`:163-165` | 与 `func_index` 预注册重复；且**自动生成注释声称它控制路由，实际不控制**（`bytecode_libc_index.rs:13-14`） | **低**：单源化（建议保留 `func_index` 判据，删冗余表或改注释） |
| 5 | **`meta = self.func_table[idx].clone()` 整表克隆** | `control.rs:11`、`:40`；`state.rs:395` | "借用检查器冲突 → 先 clone 再调用"模式（AGENTS.md 明列该模式）；`local_buffers`/`param_sizes` 随每帧克隆 | **低-中**：不可变结构共享天然解决；`local_buffers` 语义不能丢 |
| 6 | **逐字节零初始化/拷贝** | `control.rs:108-110`（帧清零）、`host/memory.rs:240-247`（realloc 拷贝+补零）、`:331-333`（calloc） | 用逐字节受检写入做批量操作 | **中**：性能热点（每次分配 O(n) 次受检访问）；应改批量接口（⑤） |
| 7 | **`execute_trace_bulk` 的 `total_steps` 双重计数** | `jit_templates.rs:747` + `:750` | 循环内每轮 `+= steps_per_iter`（满 1000 轮 = 1000 次），退出后**又** `+= steps_per_iter * MAX_TRACE_ITERATIONS` → 统计虚高 2× | **低**：只影响 `jit_stats.steps_accelerated`（`step_count` 由 `bulk_step_check` 正确维护，`:587-599`）；已登记 P1（`代码审阅与修复追踪20260906.md:197`） |
| 8 | **`strtol`/`strtod` 混用 `String` 与其字节视图** | `host/misc.rs:372-373`、`:420-421`；`strtod` 的 `std::str::from_utf8(&bytes[start..pos]).unwrap_or("")`（`:449`） | 输入含非法 UTF-8 时，切片边界（由 String 字节视图给出）与原始内存不再对应 → **`endptr` 可能指错**（静默错值） | **中**：这是"用 String 承载字节流"的必然次生伤害；MoonBit 侧必须用 `Bytes` |
| 9 | **`host_strerror` 的 `msg.len()` 把内嵌 `\0` 计入分配与写入长度** | `host/misc.rs:471-472`（字面量如 `:464 "Invalid argument\0"`） | "有意为之但易错"：`size`/`aligned`/`write_memory` 长度全含 NUL | **低**：与 U2#13 的 `host_strerror` 补 region 登记同批（`路线图.md:754`）；MoonBit 字符串字面量不隐式带 NUL → 此处极易写错长度 |

### 3.3 组织债

| # | 债 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 1 | **`core/state.rs` 821 行 God Object（36 字段）** | `core/state.rs` | VM 状态、快照构造、符号管理、常量池、JIT 开关、宿主回调执行全塞一处 | **中**：拆分需先确定"快照边界"（⑨ 包切分） |
| 2 | **`vfs.rs` 771 / `jit_templates.rs` 752 贴阈值** | — | 与 800 行阈值约束相邻 | **低** |
| 3 | **`snapshot.rs`（446 行）类型+算法+测试混装** | `vitro_vm/src/snapshot.rs` | `VMSnapshot`/`RuntimeSnapshot`/`MemorySnapshot`/`CheckpointManager` + 3 测试同文件 | **低** |
| 4 | **注释密度极高（事故史资产）** | `state.rs:110-119`（单字段 6 行）等 | 不是债而是资产 | **低**：建议**保留为设计文档而非删掉**（迁移期最省事的"SOP 复刻"来源） |
| 5 | **错误码字符串协议** | `core/memory.rs:197`、`host/memory.rs:67`、`vitro_shared/src/error_codes.rs:67,100,101,114` | 下游靠 `contains` 匹配 | **中**：结构化改造会波及 `trace_analyzer`/知识卡片/capi 出口；建议迁移期**先照搬字符串**，结构化作独立批 |

---

## ④ 在途工作接纳方案

来源：`统一整备路线图.md`（U2 资源生命周期 / U5 语义单源）、`代码审阅与修复追踪20260906.md` §3.3/§4.3、`CHANGELOG.md` [Unreleased]、`工程债务维护方案.md`。

### 4.1 已完成的在途工作（**不需要搬**，只需把结论写进目标架构规格）

| 条目 | 状态 | 证据 | 接纳方式 |
|---|---|---|---|
| U2#2-a/b：`regions` addr 索引化 + `freed_logs` BTreeMap 化 | ✅ | `memory_state.rs:130-220`、`state.rs:694-755`；实测 1M churn 60.4s → 34.8s → **1.45s**（累计 41.7×，`路线图.md:636-638`） | **直接按目标架构实现** |
| U2#3：`OutputLog` 有界化 | ✅（残余：note 通道无界） | `output.rs:100-138`、`:265-312`；残余 `:122-125` | 直接实现；note 无界作为开放项 |
| U2#4：`trace` 环形 4096 + 三字段 Arc+COW | ✅ | `runtime_state.rs:68/91-94/116-118` | **重设计**（§1.9：heatmap 不可照抄） |
| U2#5：检查点淘汰悬空 Delta 级联 | ✅ + 3 红锚 | `snapshot.rs:257-284`、`:388-483` | **照搬算法 + 测试** |
| U2#8：`vis_event_queue` 环形 1024 | ✅ | `executor/debug.rs:6`、`:29-32` | 照搬 |
| U2#9：printf 宽度/精度 1MB 预算 + trap | ✅ | `host/utils.rs:143-172` | 照搬 |
| U2#10：数组快照 payload 截断 256 + `truncated` | ✅ | `unified_types.rs:10`、`core/memory.rs:402-403` | 照搬 |
| U2#11 后半：快照补 `stdin_eof`/`heatmap` 回滚 | ✅ | `snapshot.rs:96-97`、`core/snapshot.rs:157-158` | 照搬（**门 3 必测**） |
| U2#13：libc 边界批六项 | ✅ | `state.rs:20`(`MAX_FUNCTIONS`)、`:694-743`、`:399-408`；红锚 `u213_libc_boundary_test.rs` | 照搬 |
| U1#5：trace 长度上限 Abort 而非注册半截 | ✅ + 2 baseline 锚 | `jit_trace.rs:90-96` | 照搬 |
| 常量单源化（`GLOBAL_REGION_LIMIT`/`compute_heap_base`） | ✅ | `memory_state.rs:16-63`、`state.rs:10-14` | 照搬 |

### 4.2 未完成 / 已登记未做 → 接纳决策

| # | 在途条目 | 出处 | 接纳方案 | 理由 |
|---|---|---|---|---|
| 1 | **U5#1 溢出行为统一（JIT `wrapping_*` vs 解释器教学 trap）** | `路线图.md:176` | **不在 Rust 侧修，直接在 MoonBit 侧确立单一语义** | 现存**未覆盖的静默错值**（10 条 parity/baseline 用例全部无溢出）；"三路径收敛"的正确形式是减路径而非让路径等价（裁定 §14.11.6） |
| 2 | **U5#1：`host memset/memcpy/memmove` 接入 `check_uaf`** | 同上；实测 `host/string.rs:165-166/252-259/275-282` | **直接按目标架构实现** | 目标架构只保留**一个写内存入口**（"统一写路径校验器"，`代码审阅…:284` 已建议），此缺陷结构性不可能发生 |
| 3 | **U5#1：JIT 化区间断点保留策略** | 同上；契约 `unified/contracts.rs:138-144`（`jit_breakpoint_integrity`，active） | **留在 Rust 侧修完再搬，或直接按目标架构实现"bulk 也走 StepEvent 记账"** | 见 ⑥-5；对以单步/断点为卖点的白箱引擎是**功能性缺陷**（`代码审阅…:185` 原文） |
| 4 | **U2#6：检查点快照内存预算实测归档** | `路线图.md:139` | **放弃并记录理由** | 是"测量任务"非代码；Rust 版测量结果对新实现无约束力 → 并入门 3 验收口径 |
| 5 | **`snapshot_into` 每步深拷 `freed_logs`（稳态可达 16,387 条）** | `路线图.md:747-749`（审阅登记后续立项） | **直接按目标架构实现（不可变共享）** | 快照侧克隆放大大头；MoonBit 的不可变值派生天然消除，且顺带消掉"逐字段同步义务"（评估报告 §4.1 认定的唯一"换语言才能结构性消除"的问题） |
| 6 | **U2#1：seek 重放循环内窗口化**（`frame_cache` 滚动截断） | `路线图.md:134` | **不搬（属出口层）**，但**接口需预留**：`nearest` 返回 owned `VMSnapshot`（`:287-316`），每次重建都是 1MB `Vec` 克隆 | 照抄 `nearest` 形态会把同一泄漏复发洞一起搬过去 |
| 7 | **VFS 未纳入 `VMSnapshot`** | `代码审阅…:187`；`vfs.rs:687-705` 零调用点 | **直接按目标架构实现**：把 VFS 纳入快照（文件数据在 VM 堆内 → 只需快照 fd 表元数据） | 重写 VFS 时一并解决，成本最低 |
| 8 | **`set_errno` 写错地址（errno 子系统整体失效）** | `代码审阅…:186`、U5#2 | **直接实现**（若保留 errno）或**放弃并记录** | 需先裁定 errno 是否属教学子集承诺 |
| 9 | **`fprintf` 到自定义 `FILE*` 不落盘** | AGENTS.md 已知限制 | **修复后搬**（实现 VFS 写路径时直接做对） | 现版不解析 `stream` 实参（`host/io.rs:349-357`） |
| 10 | **`scanf` 的"按行消费"与 C 流式语义不一致** | `代码审阅…:196` | **修复后搬**：统一"字节流游标"（`getchar` 已有机制 `io.rs:163-198`，scanf 未复用） | **同一语义两个实现**的典型，迁移期正是收敛时机 |
| 11 | **`strncpy` 负数 `n` 清零内存高端** / **VFS append 死分支** / **bsearch 比较器静默 Equal** | U5#3 | **直接按目标架构实现 + 每模式独立测试** | MoonBit 的 `Int`→`UInt` 转换语义需实测；`enum VfsMode` 穷尽 match 可结构性防"死分支" |
| 12 | **D14/D16 债务（`vitro_typeck/src/decl.rs`）** | `工程债务维护方案.md` | **不适用本模块** | — |
| 13 | **U7#1 随机差分常设化（三执行路径 × clang × 随机形状）** | `路线图.md:204` | **新项目必须重建**，是"裸奔期"的核心补偿（评估报告 §3.3） | 与 ⑧ 三联 diff 设计合并考虑 |

### 4.3 明确"不搬"清单

| 项 | 理由 |
|---|---|
| `vitro_get_jit_stats` 等 JIT 统计出口 | 属出口层；但**开关缺失是迁移阻塞点**（§4.4） |
| `native/benches/vm_benchmark.rs`（criterion） | 裁定已认定"只测编译+初始化、从不执行程序，不是 VM 性能基准"（`核心资产重构裁定.md:842-843`） |
| `vfs.rs:687-705` `snapshot_files`/`restore_files` | **死代码**（零调用点）；语义由"VFS 纳入快照"取代 |
| `executor/mod.rs:358-491` 的 7 个 `#[test]` | 直驱私有 API 的白盒单测（`set_test_code` 是 `#[cfg(test)]`）→ MoonBit 用 `*_wbtest.mbt` 等价重写 |
| `global_count` / `OpCode::Strlen` / `BYTECODE_LIBC_PURE_FUNCS` | 见 §3.2 |

### 4.4 迁移阻塞点（必须先决策）

**C ABI 只导出 JIT 统计的读，没有禁用开关**：`native/src/capi/mod.rs:528-552` 仅 `vitro_get_jit_stats`；`native/src` 内 `(?i)jit` 全部命中仅 9 处（capi ×3 + `unified/contracts.rs` ×6 契约文本）；CLI/serve 也无开关参数。

⇒ 若 MoonBit 侧走 wasm-gc 单出口 + 自带驱动，**这不是问题**（开关在引擎内部）；但**若迁移期要"Rust oracle vs MoonBit"双跑对账，"JIT 关"这一路必须可达**。建议：迁移期给 Rust 侧临时加测试用导出，或把"关 JIT"对照降级为 Rust 侧内部测试。**需在阶段 0 拍板**，否则 ⑧ 的 parity 锚会退化成"只测 JIT 开"。

---

## ⑤ 架构优化建议（MoonBit 形态）

### 5.1 内存承载（门 1）

- **〔结构优化〕把"1MB 内存"与"内存元数据"分离为两个包**：现版每个访问器同时触碰两者（`check_mem_access` + `check_uaf`），使纯字节操作被迫携带诊断元数据。分离后 `Memory` 可独立做载体 spike，`MemoryMap` 可独立做正确性测试。
- **〔沿革保留〕"一次受检访问"的四步链（NULL 区 → 上界 → UAF → 脏页）语义保留，但合并为单一入口**：现版 9 个访问器各自复制这四步（`core/memory.rs:83-165`）。建议 **一个 `checked_access(addr, size, kind) -> Option[Range]`**，所有读写走它 → "host 层绕过检测"类缺陷从结构上不可能发生。
- **〔新设计〕脏页追踪改移位而非除法**：现版 `addr / PAGE_SIZE`（`:172-173`）在 wasm-gc 下无硬件除法指令；`PAGE_SIZE` 是 2 的幂 → 用 `>> 12`。
- **〔新设计〕批量操作走"无检查快路径 + 一次前置检查"**：先一次 `checked_access(整段)`，再走载体原生 `copyFrom`/`fill`，而非逐字节 `store_i8`（现版 `host/memory.rs:240-247/331-333`、`control.rs:108-110`）。**纯收益且不损语义**（边界已在段级验证）。

### 5.2 快照（门 3）

- **〔沿革保留〕扁平值快照 + Full/Delta 二分 + 检查点淘汰不变量**——照抄，不重新设计（评估报告 §7 阶段 1 第 4 条已定）。
- **〔结构优化〕用不可变 record 派生快照，消除"逐字段同步义务"**：现版四处各自枚举 22 字段（`core/snapshot.rs:11-34/54-77/84-114/120-179`），**漏一个就是静默错值**。建议：VM 状态本身是不可变 record（或少量可变槽 + 一个不可变快照视图），`snapshot = 直接返回该值`，`restore = 整体替换`。**判定标准：字段增删不得要求改动快照代码。**
- **〔新设计〕`quarantine_budget` 不应随快照往返**（现版携带，`snapshot.rs:113`；与 `max_steps`/`call_depth_limit` 相反）。建议显式分区"会话配置"与"执行状态"。**需先裁定**（附录 A-5）。
- **〔新设计〕`nearest` 返回"重放指令"而非 owned 1MB 快照**：现版每次 `m.clone()` 重建（`:302-312`）；可让调用方提供可复用缓冲（`snapshot_into` 已有该思想，`:84-114`，但 `nearest` 未用）。

### 5.3 执行器

- **〔结构优化〕两级 match 合并为一级**：MoonBit 的 `enum + match 穷尽检查` 允许单层穷尽 match 到 132 分支且编译器可检查遗漏。建议**一个穷尽 match 覆盖 132 条**，"族"降级为代码组织（文件拆分）而非运行时分支。
- **〔沿革保留〕值栈 `Vec<u64>` 的位模式设计**：i32/f32 低 32 位、比较时 `as i32`/`as u32` 重解释，必须逐条搬运（如 `SplitD`/`SplitQ` 拆位，`executor/memory.rs:123-129/170-176`）。建议 `Array[UInt64]` 并**保持同一约定**（不用 `enum Value`——会引入装箱并破坏与 golden 的位级一致）。
- **〔新设计〕教学 trap 的文案生成移出执行器**：`format_bounds_error`（`core/trap.rs:6-61`，遍历符号表找最近数组）、`format_div_zero_error`（`:63-102`，扫全部变量含 4 次内存读）都在出错时执行，可接受；但 `trap()` 本身只写字符串（`:145-156`）→ 建议迁到独立 diagnostics 包，执行器只发**结构化事件**（错误类 + 上下文）。

### 5.4 JIT

- **〔沿革保留〕模板超级指令 + 只 JIT 最内层 + 录制期禁 fast path**——三者联合是 9.16x 的完整形态（裁定 §14.11.2），且**天然适配 MoonBit/wasm**。
- **〔结构优化〕模板表改 `enum` 分派，消除函数指针地址比较**（§3.2-1）。
- **〔新设计〕JIT 生效区间必须保留 `StepEvent` 语义**：现版从 trace 剔除（`jit_trace.rs:86-88`）→ bulk 期间行号/断点/热力图/可视化事件全丢（⑥-5）。建议 `JitEntry` 显式保留"本 entry 的源码行"，bulk 结束批量补记账；**含断点的循环在录制时就标记为不可 JIT**（fail-safe 方向）。
- **〔新设计〕JIT 溢出语义与解释器统一**（⑥-4）：模板必须用与解释器**同一个 checked 实现**，而非 `wrapping_*`。

### 5.5 host 层与路由

- **〔结构优化〕"三实现合一"**：现版同一函数最多三条实现（VM opcode 内联 / Rust host / Bytecode Libc），且路由分叉不可见（20 个 handler 被 `func_index` 遮蔽，两张表各自手写）。建议**一张单一路由表**（名字 → 实现来源），生成期检查"每个名字恰好一个来源"，把隐式覆盖变成显式声明。
- **〔沿革保留〕固定索引段 + 全局区预留**（1000/1024/用户起点 1089）——数字可保留，但**索引段与产物的一致性门禁必须补上**（现版对 `bytecode_libc_index.rs` 只做存在性检查）。
- **〔新设计〕输出通道改为字节累积**：现版 `push_stdout(impl Into<String>)` + `OutputLog.text: String` 使**非 UTF-8 字节永远无法保真**（`putchar(200)` 输出 2 字节 `C3 88` 而非 1 字节 `C8`，`host/io.rs:346`；`printf("%c",200)` 同 `host/utils.rs:324-327`）。MoonBit `String` 内部是 UTF-16，问题更严重 → `OutputLog` 的 chunk 承载改 `Bytes`（不可变可共享，正好与"快照零拷贝"契合）；`chunks_of`/`join` 返回 `Bytes`，`stdout()` 按字节比较。
- **〔结构优化〕`freed_logs` 用"有序数组 + 二分"替代树**：互不重叠 ⟹ addr 序即 end 序；稳态条目**有界**（隔离预算 256KB ÷ 最小块 16B ≈ **16,387 条**，实测饱和值，`路线图.md:663-666`）→ 16k 量级的排序数组 + 二分 + 区间裁剪比树更简单更快。**前提**：变更走"合并写入"（保留 `remove_overlapping` 一次改多条的语义）。
- **〔新设计〕把"内存不变量自检"作为 debug 断言**：现版已有 `verify_region_index()`（`memory_state.rs:223-247`）。建议做成**每批触发的 debug 断言** —— 迁移期最高风险正是"索引与 regions 失配"（U2#2-a 已因此抓出存量缺陷）。

---

## ⑥ 坑清单（现象 → 根因 → 修复 → 新语言下是否复发）

| # | 现象 | 根因 | 修复 | 新语言下是否复发 / 为什么 |
|---|---|---|---|---|
| 1 | **嵌套纯计数循环 JIT 静默错值**：`inner=20200 i=200 j=0`（正确 `40000/200/200`），零诊断 | `run()` 的 fast path 在**录制期间仍生效**：外层录制推进到内层循环头时命中已编译的内层 trace，bulk 一次跑完内层 → 外层 trace 缺内层指令却被注册（`executor/mod.rs:14-16` 修复前未检查 `is_recording()`） | 一行：`!self.trace_recorder.is_recording()`（`:18`）；副作用即"只 JIT 最内层" | **会复发**，且**同类已经复发过一次**：裁定 §14.11.5 明写"分叉的根源是存在多条路径且无等价性合同……本次缺陷恰恰是在 Rust 重写之后长出来的"。⇒ MoonBit 侧**必须先立 parity 差分防线再写 JIT** |
| 2 | **防线全绿却漏掉该缺陷** | 覆盖形状**互补地**漏掉：教学用例 <100 次（不录制）、排序类有分支（Abort）、单层循环无穿透 | J10：`jit_path_parity.rs` 八条三锚 + 2 baseline；突变实测 margin=20（`脚本埋雷验证记录.md:17-30`） | **有复发倾向**：新防线必须覆盖**形状维度**（随机控制流形状），不能只随机数据（裁定 §14.11.3 D1b） |
| 3 | **`vm_bench` 曾得出"JIT 0.66x 不赚反亏"** | 三处方法学缺陷：① `jit_traces_mut().clear()` 只清表不禁用；② 两分支入口不同；③ 旧版只断言 `ret==0`（错值程序照样绿） | 结构性开关 + 统一入口 + 期望值锚 + best-of-5（`vm_bench.rs:83-112`）；重测 9.16x/9.43~9.72x | **会复发**：任何性能对照都易再犯"两个变量同时变"。⇒ 四条方法学必须写进规格而非靠记忆 |
| 4 | **JIT 与解释器算术溢出语义分歧（现存、未修复、未覆盖）** | 解释器 `Add/Sub/Mul/Neg` 用 i64 中转 + 范围检查 → **trap**（`arithmetic.rs:6-15` 等）；JIT 模板用 `wrapping_add/sub/mul` → **静默 wrapping**（`jit_templates.rs:246/253/260`，经 `:598-603` 映射） | **未修复**（U5#1 待裁定，`路线图.md:176`；`代码审阅…:184` 原文"同一程序同一错误第 101 次执行起行为不同"） | **必然复发**（两条路径各写一遍语义的必然产物）。**判定性用例**：`int main(){int i,x=0;for(i=0;i<200;i++){x=x+20000000;}printf("%d %d\n",x,i);}` → 解释器（`unified`）第 108 轮超 `INT_MAX` → 教学 trap；JIT（`run`，第 103 轮起 bulk）→ 静默 wrapping 输出负值。**未覆盖证据**：10 条 JIT 用例期望值全在 int 域内 |
| 5 | **JIT 生效区间断点失效、热力图漏计、行号冻结** | ① `StepEvent` 被录制器当透明指令剔除（`jit_trace.rs:86-88`）；② `StepEvent` 是 `current_line`/断点/`vis_event_queue` 的**唯一更新点**（`executor/debug.rs:11-37`）；③ heatmap 记录在 `step()` 内（`executor/mod.rs:295-297`），bulk 不经过 | **未修复**；已立契约 `unified/contracts.rs:138-144`（`jit_breakpoint_integrity`，active） | **必然复发**（同 4）。若保留 JIT，**必须把"bulk 期间的观测记账"作为一等设计问题** |
| 6 | **`realloc`/`calloc` 复用驱逐块时同 addr 双条目 → 泄漏虚报 + 索引失配** | 新块无条件 `push_region` | 改"复位 or push"（`host/memory.rs:31-56/284-308/339-364`）；红锚 `test_u22_realloc_reuse_no_duplicate_entries` | **不会复发**（照抄 addr 索引 + 单一写入入口）；但**等价风险**在"index 与 regions 失配"——`restore` 漏 `rebuild_region_index()`（`core/snapshot.rs:169`）即全线失配。建议**取消索引字段，改为从 regions 派生的不可变视图** |
| 7 | **检查点淘汰删 Full 后 Delta 悬空 → seek 静默错内存**（"最坏调试器缺陷"） | 淘汰逻辑与注释意图相反：只在删 Delta 时级联；pinned 场景（`[0]` 恒 Full）掩盖了它 | `evict_over_limit` 抽出 + 删 Full 级联 drain（`snapshot.rs:257-284`）；3 红锚（`:388-483`） | **不会复发**（照抄算法+测试）；**必须照搬那 3 条测试**——链不变量缺陷人工审阅抓不住（原实现被审两次未看出） |
| 8 | **`INT_MIN % -1` / `i64::MIN / -1` panic 击穿 FFI** | Rust `%`/`/` 在 MIN/-1 时 panic；`Div` 有防护而 `Mod` 漏 | 三处（`arithmetic.rs:57-58/79-81`、`float.rs:212-213/223-225/234-235`）+ 3 个 JIT 模板（`jit_templates.rs:271-274/286-289/296-299`） | **不会复发**（MoonBit 无需为此 panic），但**溢出语义需实测**（⑦ spike-4）。真正教训 = "**同一语义两处实现，只修了一处**"（V-P0-1 有防护、V-P0-4 JIT 复刻同一 panic） |
| 9 | **`nmemb * size` 溢出 → `qsort(base,2^32,2^32,cmp)` 分配 ~32GB OOM** | 尺寸链无 checked 运算 | `checked_mul`+`checked_add`（`host/misc.rs:171-177/254-261`）+ key 越界检查（`:268-271`） | **不会复发**（"尺寸链全 checked"应作设计约束）；同族风险 `int a[100000][100000]`（U5#4） |
| 10 | **`freed_logs` 部分重叠整条删除 → UAF 假阴性** | `retain` 按重叠整条删：`free X(100)` 后 `malloc Y(60)` 复用前段，X 尾部检测窗口随整条记录消失 | 精确裁剪（前缀缩 size / 后缀改键插新 / 嵌套拆两条，`state.rs:694-743`）；红锚 `test_u213_freed_logs_partial_overlap_trimmed` | **不会复发**（照抄算法+测试）；**这是"区间语义"类缺陷的典型**——换容器必须重跑这几条红锚 |
| 11 | **`strncpy` 负数 `n` 清零整个内存高端（含栈区）** | `n as usize` 对负数变巨值 | U5#3 待修（`代码审阅…:190`） | **可能复发**：MoonBit 的 `Int`→`UInt` 转换语义需实测 |
| 12 | **调用深度无独立上限 → 零局部变量递归使 `call_stack` 无限增长** | 栈堆碰撞检测依赖局部变量占用 | `call_depth_limit`（`control.rs:43-53`，默认 `MAX_STACK_DEPTH=10_000`，可经 capi 调小） | **不会复发**（照抄）；但 **`rebuild_local_sym_map()` 每次 Call/Ret 遍历全部 symbols**（`state.rs:770-779`，`control.rs:120/225/245`）是未被 U2 覆盖的性能热点 |
| 13 | **VFS `fopen(path,"a")` 建文件分支是死代码**（追加写静默丢弃返回 0） | `Append` 分支嵌在 `Write` 块内部恒 false（`vfs.rs:171-191`） | U5#3 待修 | **会复发**（若照抄嵌套 match）；建议 `enum VfsMode` 穷尽 match + 每模式独立测试 |
| 14 | **`snapshot_vars` 每 10 万步才刷新** | 有意设计（`SNAPSHOT_INTERVAL=100_000`，`executor/mod.rs:267-276`） | 无需修 | **口径混淆风险**：`SNAPSHOT_INTERVAL`（10 万步，变量快照）与 `CheckpointManager.interval`（20 步，快照点）是**两个完全不同频率**；文档"每 20 步快照"与 `snapshot_vars` 极易混淆，迁移期须在规格中显式区分 |
| 15 | **生成物与手写物同步无门禁**：`OpCode::Strlen` 死路径 / `bytecode_libc_index.rs:13-14` 注释与代码不符 / `opcode.rs:18-19` 注释漂移（128 vs 137） | 生成器与手写代码的不变量未纳入门禁 | 未修复（`precompile_bytecode_libc --check` 只覆盖 `runtime_libc/` 源文件） | **会复发**：MoonBit 侧必须把"生成物与手写物的不变量"变成门禁 |
| 16 | **`strlen(NULL)` 静默返回 0，不 trap** | `read_cbytes`（`host/utils.rs:4-11`）**只做 `start >= mem.len()` 下界检查，无 NULL 区检查**；而 1MB 零初始化（`core/state.rs:151`）→ `memory[0]==0` | 未修复 | **会复发**，且 MoonBit 下更隐蔽（零初始化是默认行为）。⇒ 目标架构必须让"内存读取"只有一条入口，NULL 检查不可绕过 |
| 17 | **`sprintf`/`snprintf`/`sscanf` 缺参数时诊断口径与 `printf`/`fprintf` 不一致** | `printf` 有栈深守卫（`io.rs:8-11`）、`fprintf` 有（`:354-357`），三者没有（`:383-441`）→ 落到 `pop()` 的"运行时错误：栈下溢"而非教学诊断 | 未修复 | **会复发**（同族函数守卫漏加）→ 属"同类清查义务"（`路线图.md:193` S4④）未清项 |
| 18 | **6 个字符串比较/查找函数"越界当 0"软夹紧，不 trap** | `strcmp:118-143`、`strncmp:311-340`、`memcmp:342-365`、`strchr:367-387`、`strrchr:389-406`、`memchr:426-442` 一律 `if addr+i < mem.len() {…} else {0}` | 未修复 | **口径风险**：与 C 的 UB 相比是"静默截断语义"，且与 `strlen` 的"扫到末尾即停"（`executor/memory.rs:270-275`）**不成体系**。迁移期需一次性裁定"越界是 trap 还是软夹紧"并写入规范差异表 |

---

## ⑦ MoonBit spike 清单（门 1 / 门 3 的完整设计输入）

### 7.1 载体候选与"访问模式 × 载体"写法矩阵

**候选**：(a) `FixedArray[Byte]`；(b) `Array[Byte]`；(c) `FixedArray[Int]`/`Array[Int]`；(d) `Bytes`（**不可变，直接排除作内存本体**，但可作快照/输出载体）。

> **同批次的已知与未知（避免重复劳动）**：**已知** —— `FixedArray[Byte]` **可写**（`MoonBit迁移_编译管线与会话层模块勘察报告20260918.md` spike 2 实测 `arr[3] = b'A'` → 65）；`Bytes` 不可变；`b"abc".length() == 3`。**未知（= 本 spike 的全部价值所在）** —— **packed 表示**（1MB `FixedArray[Byte]` 实际占 1MB 还是 4MB）与**每类访问模式的 ns/op 吞吐**：`MoonBit迁移_scripts与测试防线模块勘察报告20260918.md:249` 亦把"`FixedArray[Byte]` / `Array[Byte]` 的 packed 表示与装箱"列为待验证 —— 同批 10 份报告**无一份测过吞吐**，故 7.1 的四条判定标准全部保持"未测"状态，是门 1 的最终空白。

| 访问模式（§1.6 A–H） | (a) `FixedArray[Byte]` | (b) `Array[Byte]` | (c) `FixedArray[Int]` |
|---|---|---|---|
| A/B 单字节 | `mem[a]` / `mem[a] = b.to_byte()` | 同 (a) + 长度检查 | `mem[a>>2]` + 移位掩码（4 倍空间） |
| C u32 读 | **4 次索引**：`m[a].to_int() \| (m[a+1].to_int() << 8) \| …` | 同 (a) | **1 次索引** + 移位 |
| D u32 写 | 4 次索引写 | 同 (a) | 1 次索引 + 掩码合并 |
| E u64 读/写 | **8 次索引**（风险最高） | 同 (a) | 2 次索引 + 移位 |
| F 字符串扫描 | `while mem[i] != 0` | 同 (a) | 需字节视图 |
| G 批量 `copyFrom`/`fill` | **须实测是否有 `blit_from`/`fill`** | `Array::blit_from` | `Array::blit_from`（字节级？） |
| H 快照 1MB clone | **须实测**（若只能逐元素复制，全量快照极慢） | 同 (a) | 同 |

**判定标准**：
1. **每类模式的每字节开销**：micro-bench 对 1MB 载体做 `N=10^8` 次混合访问（30% u32 读 / 30% u32 写 / 20% 单字节 / 10% u64 / 10% 批量 4KB），记录 **ns/op**。
2. **门槛**：与 Rust 版同形状 micro-bench 对比，**慢 3× 以上即门 1 出局**（评估报告 §7 门 1）；同时记录"最慢的一类访问"（决定优化方向）。
3. **packed 判定**：用 `FixedArray[Byte]` 构造 1MB 与 256K 两档测内存占用（若 1MB Byte 数组实际占 4MB → 无 packed 表示）→ **这是 (a) vs (c) 的关键实验**。
4. **边界检查开销**：对比"索引恒在界内"与"可能越界"两种循环（MoonBit 是否插检查、能否被编译器消除）。

### 7.2 特性 spike 表

| # | 待测特性 | 最小验证程序 | 判定标准 |
|---|---|---|---|
| **1–3** | 载体 packed / 吞吐 / 批量（见 7.1） | micro-bench | 见 7.1 四条 |
| **4** | **`Int` 在 wasm-gc 下的宽度与溢出行为** | ① `let x:Int = 2147483647; x + 1` 的结果/是否 trap；② `Int` 与 `Int64` 的 `to_string` 长度对比；③ `(-1):Int` 的 `to_uint()` | 必须明确三件事：**位宽 32 还是 64**、**溢出 trap 还是 wrap**、**有/无符号转换的位模式**。Vitro 要求"**溢出必须能被检测**"（trap 教学诊断）→ spike 要测的是**检测成本**：`a.to_int64() + b.to_int64()` 的溢出判定 ns/op，以及是否有内建 `add_checked`。**✅ 前半已被同批报告现场实测，无需重测**：`MoonBit迁移_cpp_frontend模块勘察报告20260918.md` §7 F1 取证 `core/builtin/intrinsics.mbt:236`（`inspect(2147483647 + 1, content="-2147483648") // Overflow wraps around`）+ `:239/:264/:288`（`Add/Sub/Mul for Int = %i32_*`）+ `Int64` 同（`core/builtin/int64.mbt:216,220`），并实测"**全 core 扫 `checked\|overflow\|wrapping` 仅命中一个无关的 `Bytes::to_unchecked_string`**" ⇒ 结论 = **静默回绕且无内建 checked API**。**spike-4 的剩余部分因此收缩为**：① `Int` 位宽确认（该报告未直接给位宽，但 `%i32_add` 已强指 32 位）；② **溢出检测的实现途径与 ns/op**（无 checked API ⇒ 必须走 `to_int64()` 中转 + 范围判定，与 Rust 版 `arithmetic.rs:9-10` 同构）；③ **该检测成本占每条算术指令的比例**（门 1 的直接输入） |
| **4b** | **`Int64`/`UInt` 混用与位重解释** | `Int64`↔`UInt`↔`UInt64` 的显式/隐式转换；`to_int64`/`to_uint64` 是否零成本 | Vitro 值栈是 u64 位模式，要求 **`Int64 ↔ UInt64 ↔ Double` 的位重解释零成本且精确**（`Double::to_bits/from_bits` 等价物必须存在） |
| **4c** | **f64/f32 精度语义** | ① `0.1 + 0.2 == 0.3` 是否为假；② f32/f64 是否分离类型、f32→f64→f32 是否逐位往返；③ `(1.0/3.0).to_string()` 位数；④ `nan != nan`、`-0.0 == 0.0` | Vitro **已改为 IEEE 精确语义**（`float.rs:142-150` 注释明写废除 epsilon 容差）→ 必须逐位一致；**f32 常量以 u32 位模式进 i32 operand**（`executor/float.rs:6-8`）的约定需保留 |
| **5** | **函数值 / `FuncRef` 作 JIT 模板表元素** | 定义 5 个同签名顶层函数 `f0..f4` 存入 `Array[(Vm,Int,Int) -> StepResult?]`，循环调用 10^7 次；再定义**捕获变量的闭包**做同样的事 | ① 顶层函数入数组是否零装箱；② 经数组元素调用的间接成本 vs 直接调用；③ **`FuncRef` 相等比较是否可用**（现版 `jit_templates.rs:647` 用它判 generic）；④ 闭包捕获是否引入分配（若引入 → JIT 表必须只用顶层函数 + `enum` 分派） |
| **6** | **有序映射（`freed_logs` 等价物）** | 构造 **16,387** 条不重叠区间（实测饱和值），测：① 按 addr 插入；② `range(..end)` **降序反向扫描 + 提前 break**；③ 部分重叠裁剪（删/缩/拆） | 判定 MoonBit 是否有 `BTreeMap` 类有序容器；若只有 hash map，必须自建**排序数组 + 二分**，并验证"降序扫描 + `l_end <= start` 提前 break"的正确性（**算法正确性依赖，非性能偏好**）。ns/op 目标：与 Rust `BTreeMap` 同量级 |
| **7** | **`HashMap[String,UInt64]`（`snapshot_vars`）的迭代确定性** | 插入 100 key，遍历两次打印顺序；不同进程各跑一次对比 | **本模块实测结论：不需要确定性** —— 全部 HashMap **无一处迭代**（已逐点验证）。判定：只需确认"按 key 查 O(1) 且不受迭代序影响"，**不必强求有序**。唯一例外 `vfs.rs:689` 遍历（**死代码**）→ 若复活该功能必须显式排序 |
| **8** | **不可变结构的"派生新状态"成本（A5 验证）** | ① 含 1MB 内存 + 若干容器的 record 做 `{..s1, step_count: s1.step_count + 1}`，测内存与耗时；② 仅改标量字段时数组是否被共享（测内存增量） | **改一个标量字段后 1MB 数组不得被复制**（否则快照成本 O(n)，Arc 替代方案不成立）；再测 `output`（追加）/`heatmap`（每步更新）两类结构的共享成本 |
| **9** | **测试与工具链形态** | 跑通 `moon check`/`moon test`；建黑盒 `*_test.mbt` + 白盒 `*_wbtest.mbt`，验证白盒可访问私有字段 | 判定白盒测试能否替代现版 `executor/mod.rs:358-491` 的 `set_test_code` 直驱单测；`moon.pkg` 的 `test-import`/`wbtest-import` 配置形态 |
| **10** | **`String` 与 `Bytes` 的分工形态**（由 ⑥-16/17 与 ③-8 驱动） | ① `String::length()` 是字节数还是 UTF-16 码元数；② `String`↔`Bytes` 转换是否零成本；③ 用 `String` 承载含 `0xFF` 的字节序列时的往返行为；④ `Bytes` 的 builder/追加形态 | 判定：**能否用 `Bytes`（不可变）表达 `OutputLog` 的"有界环形 + 部分截除 + 每通道 O(1) 长度"**。若 `Bytes` 只能整体拼接（无 O(1) 头部截除）→ `OutputLog` 仍需内部可变累积器 → 快照共享方案需重新设计。**✅ ① 已被同批报告实测**：①a `String` 索引返回 **UTF-16 code unit**（`MoonBit迁移_编译管线与会话层模块勘察报告20260918.md` spike 2 实测 `"hello"[0].to_int() == 104`；`MoonBit迁移_codegen模块勘察报告20260918.md:222` 经 `moon ide doc 'String::length'` 核实 "UTF-16 code units"）；①b **`FixedArray[Byte]` 可写**（同上 spike 2：`arr[3] = b'A'` → 65）、`Bytes` 不可变、`b"abc".length() == 3`；①c **`String::to_bytes` 已 deprecated**（codegen 报告 `:222`）⇒ 转换点必须另找路径。**剩余待测 = ②③④（本 spike 的实质）**：尤其 **④ `Bytes` 是否支持 O(1) 头部截除/有界环形语义** —— 这一条决定 §1.9 的 `OutputLog` 共享方案是否成立 |

### 7.3 spike 顺序（后项依赖前项）

```
spike-1（载体 packed 判定 + 内存占用）
   └─> spike-2（每类访问模式 ns/op，门 1 主判据）
          └─> spike-3（1MB clone / 批量 copyFrom 成本）→ 门 3 可行性前提
spike-4 / 4b / 4c（算术语义三件套）—— 可并行
spike-6（有序映射）—— 可并行
spike-5（FuncRef/闭包）—— JIT 存废裁定的输入
spike-10 + spike-8（字节通道 / 不可变共享）—— 需 spike-3 数据
```

**门 1 的最小完整 VM 形态**（评估报告 §7 门 1 原始要求）：1MB 载体 + **20 个 opcode（须覆盖 §1.6 全部 8 类访问模式）** + 值栈 + 常数分发 + `run` 循环；跑 `vm_bench` 的 `bench_nested_loop_1k`（1k×1k 嵌套计数）**与 Rust 版同口径**（best-of-5、统一入口、期望值锚）；**慢 3× 以上即出局**。

---

## ⑧ 等价性验收锚点

### 8.1 三联 diff 的通道与格式

| # | 通道 | Rust 侧来源 | 格式 | 比对方式 |
|---|---|---|---|---|
| 1 | **最终输出（stdout）** | `RuntimeState::stdout()`（`runtime_state.rs:175-177`，注释自称"Shadow Verification / 差分对比的**唯一**合法来源"，不含引擎附注与 stderr） | **原始字节**（E-P1-5 后驱动侧零清洗，`engine_note_lookalike.c` 固化该口径） | 字节级 `diff`；golden 来自 clang（`cases_golden/<subdir>/<name>.out`） |
| 2 | **返回码** | `execute_run` 的 `ret` | 十进制整数 | 直接相等 |
| 3 | **单步事件流** | ⚠️ **现版无批量导出格式**（见 8.2） | — | **需新设计** |
| 4 | **最终内存映像** | `VitroVM::memory_ref() -> &[u8]`（`core/state.rs:618-620`）；capi 无直接导出口 | **1MB 原始字节** | 逐字节 `diff`（`cmp`/`fc /b`）；建议同时产出**页级摘要**（每 4KB SHA-256）以便定位差异页 |
| 5 | **JIT 统计** | `vm.jit_stats()`（`traces_compiled`/`steps_accelerated`） | 两个整数 | 用于**生效性锚**（J9），非等价性判据 |
| 6 | **步数** | `vm.get_step_count()`（`core/state.rs:531-533`） | 整数 | ⚠️ `steps_accelerated` 统计虚高 2×（§3.2-7）；`step_count` 本身正确 |

### 8.2 ⚠️ 通道 3 的现状（诚实记录）

现版**没有"单步事件流"的可导出形态**。最接近的三者：
- `StepPayload`（capi/serve 出口，走 JSON）—— 由 `native/src/unified/stream.rs` 与 collector 生成，**不在本模块**；
- `vis_event_queue: Vec<VisEventData>`（`ty/line/extra0/extra1/extra2/context`）—— 有结构但**非 unified 出口只进不出**（U2#8 已改环形上限 1024，`executor/debug.rs:6`）；
- `session.runtime.trace: Arc<Vec<TraceEntryData>>`（`line` + `operation`）—— **实测为死路径**：codegen 不发射 `__vitro_step`（`路线图.md:724` 原文"trace 实测为死路径（codegen 不发射 `__vitro_step`，事实记录于测试注释）仍加 4096 环形防御"）。

⇒ **"单步事件流"锚点必须在迁移期新建**（本身是一项设计任务）。建议格式（文本行协议，便于 diff）：

```
# vitro-step-trace v1
<step_index>\t<line>\t<opcode助记符>\t<operand>\t<stack_depth>\t<sp_delta>\t<mem_dirty_pages>
```

每步一行、`\t` 分隔、`\n` 结尾；可选 `--hash-every N` 产出每 N 步的 `(stack_hash, dirty_pages_hash)` 以压缩长程序。**理由**：现版 `Instruction` 的 serde 形态已是 `{"loc":{…},"op":"Nop","operand":0}`（`bytecode_libc_data.json:2-11`），**opcode 助记符字符串是稳定契约** → 文本行协议可直接复用助记符，且人类可读（符合"人审数据，不审代码"的仓库纪律）。

### 8.3 精选 baseline 用例矩阵（30 个，按"每 opcode 族 ≥1 + 每已知坑 ≥1 + 每 JIT 形状 ≥1"）

| 类别 | 用例 | 锚定内容 |
|---|---|---|
| **JIT 五形状（全部 5 条）** | `jit_nested_counting_loop.c`、`_longlong.c`、`jit_single_hot_loop.c`、`jit_trace_overflow_abort.c`、`jit_zero_init_trace_boundary.c` | 嵌套穿透 / 归因固化 / bulk 主战场 / `MAX_TRACE_LEN` Abort / 零初始化指令数边界 |
| **JIT 溢出（须自造）** | `int main(){int i,x=0;for(i=0;i<200;i++)x+=20000000;printf("%d %d\n",x,i);}` | **⑥-4 判定性用例**（解释器 trap vs JIT wrapping）—— 现版零覆盖 |
| **JIT 观测（须自造）** | 含 `__vitro_step` 形状的热循环 + 断点行 | **⑥-5 判定性用例**（bulk 期间行号/断点/heatmap） |
| 算术与溢出 | `crash_regression_tests.rs:88-100`（Mod MIN/-1）、`:345`（long long 溢出）对应源 | 解释器溢出 trap 语义（**与 JIT 组交叉**） |
| 内存与 UAF | malloc/free/UAF/double-free 用例 + `u213_libc_boundary_test.rs` 7 锚对应源 | E3060/E3061/E3027 文案与触发点 |
| 隔离区语义 | `test_heap_churn_beyond_quarantine_budget_no_wall`、`test_heap_uaf_within_quarantine_window_detected`、`test_heap_double_free_within_quarantine_window_detected` 对应源 | 窗口内必检出 / 窗口外复用 / 预算 0 立即复用 |
| 快照往返 | `test_snapshot.rs` 增量重建 + `test_u25_seek_after_eviction_matches_reference` 对应程序 | **门 3**：seek 回第 5 步后 `quarantine`/`quarantine_bytes`/`quarantine_budget` 三字段 + UAF 不出假阴性 |
| **字节通道（须自造）** | `putchar(200)` / `printf("%c",200)` / `printf("%s", 含 0xFF 的内存)`；对照 `baseline/engine_note_lookalike.c` | **输出通道非字节保真**（§5.5）—— 现版预期 FAIL，正是需 clang golden 锚定处 |
| 浮点 | `baseline/float_func_return.c`、`0.1+0.2==0.3` 类 | IEEE 精确语义（`float.rs:142-150`） |
| 调用与栈 | 递归 / 深递归撞 `call_depth_limit` / `qsort` 回调（`state.rs:384-509` 状态保存恢复路径） | 调用帧 / 宿主回调 / 状态回滚 |
| 字符串与 VFS | `baseline/scanf_*.c`、`file_*.c`（gap 用例）、`vfs_io_extensions.c` | 文本模式 CRLF / EOF 粘滞位 / `fprintf` 偏差 |
| 路由分叉 | `atoi` / `isgraph` / `memcmp` / `strcpy` 各一例 | 遮蔽行为（20 个 host handler）与 `strcpy`/`strcat` 例外 |

### 8.4 工具与流程

| 工具 | 用途 |
|---|---|
| `go run ./scripts/shadow_verify` | **C 侧硬门禁**（实时 clang golden、`--jobs N`、`.clang_cache/`）；Rust 版与 MoonBit 版各自产报告后对账 |
| `go run ./scripts/facts check` | 文档数字漂移门禁（`--cargo-log` 从日志解析真值） |
| `native/tests/vitro_e2e.rs`（`test_vitro_e2e_baseline`） | 静态 golden 比对。⚠️ **弱化点**：`load_golden` 返回 `Option`（`:130`）且缺失时**只跳过比对不报错**（`:186`）→ 迁移期应修掉 |
| `native/tests/jit_path_parity.rs` 三锚 | **必须移植为 MoonBit driver**，`shape + expected + expect_accel` **外置 JSON** |
| **新增**：三联 diff 驱动 | 建议 Go（仓库纪律：判定型脚本用 Go + 规则外置 + J9 埋雷记录） |
| **新增**：内存 dump 出口 | 现无（`vitro_get_heap_stats_json` **未实现**，`内存安全规范.md:269`）→ MoonBit 侧加 `--dump-memory <file>` |

### 8.5 判据清单（迁移验收）

1. **门 1**：同口径 `vm_bench` 嵌套 1k×1k，**慢 3× 以上即出局**。
2. **门 3**：`VMSnapshot` 全字段在 MoonBit 建一遍；**seek 回第 5 步**；验证 `quarantine`/`quarantine_bytes`/`quarantine_budget` 三字段随往返；**UAF 检测不出假阴性**。
3. **输出等价**：30 例 × stdout 字节级 + 返回码 + 最终 1MB 内存映像三联 diff 全等（golden 来自 clang）。
4. **单步事件流等价**：新协议下逐步行 diff 全等（含 JIT 生效区间）。
5. **JIT parity**：三锚全绿；**JIT 溢出用例与 JIT 观测用例必须存在且判定方向正确**（否则 parity 是空的）。
6. **防线自检（J9）**：每条新防线必须有一条"注入必然违反 → 必须变红"的埋雷记录（现版先例：`tpl_add` 加→减，margin=20）。
7. **全局数字引用**：`shadow_c_cases=675`/`shadow_c_match=668`（as_of 2026-09-15 00:05:33）、`c_e2e_baseline_cases=359`、`replay_assertions=61`、`serve_smoke_assertions=57`（均以 `reports/facts.json` 为准，**不沿用评估报告的冻结数字当现值**）。

---

## ⑨ mooncakes 包切分草案

### 9.1 切分（6 包，严格单向依赖）

```
vitro_core_types ─────────────────────────────────────┐
   常量（MEM_SIZE / NULL_TRAP_SIZE / GLOBAL_* / HEAP_* / PAGE_* / SNAPSHOT_INTERVAL / MAX_STACK_DEPTH）
   Instruction / OpCode(132) / FuncMeta / LocalBuffer / Symbol / FreeBlock
   OutputKind / OutputChunk / VisEventData / TraceEntryData / VariableSnapshotData
   MemoryRegionData / HeapStatsData / ErrorCode（结构化错误码，取代字符串协议）
   零依赖
    │
    ├──> vitro_memory ────────────────────────────────┐
    │      Memory（字节载体抽象，可替换 spike 候选）
    │      MemoryMap（regions / free_list / quarantine / freed_logs 有序结构）
    │      单一受检访问入口 checked_access(addr, size, kind)
    │      堆模型（bump + 有界隔离 + FIFO 驱逐 + first-fit）
    │      ⚠️ 不依赖 host / 不依赖执行器
    │
    ├──> vitro_bytecode ──────────────────────────────┐
    │      bytecode_libc_index（固定索引段 1000..=1087）
    │      bytecode_libc_sig（签名表，给 typeck）
    │      产物 JSON schema + 加载（编译期嵌入）
    │      ⚠️ 依赖 core_types，不依赖 memory/executor
    │
    ├──> vitro_host ──────────────────────────────────┐
    │      host_funcs（单一路由表：名字 → 实现来源，生成期校验唯一性）
    │      host/{memory,string,io,file,math,misc}
    │      vfs（VirtualFileSystem，纳入快照）
    │      ⚠️ 依赖 core_types + memory
    │
    ├──> vitro_jit ───────────────────────────────────┐
    │      TraceRecorder + CompiledTrace（enum 分派，非 FuncRef 地址比较）
    │      47 模板 + 溢出语义与解释器同源
    │      ⚠️ 依赖 core_types + executor 接口（不是反过来）
    │
    └──> vitro_vm ────────────────────────────────────┐
           executor（穷尽 match 132 opcode）
           snapshot（VMSnapshot + CheckpointManager）
           ⚠️ 依赖以上全部
```

**依赖方向**：`core_types → memory → host → vm`，`core_types → bytecode → vm`，`core_types → jit → vm`。**禁止反向依赖**（现版 `vitro_runtime::MemoryState` 被 host 层直接操作 → 迁移时应把 `MemoryState` 归入 `vitro_memory`）。

### 9.2 关键设计约束（写进包规格）

| # | 约束 | 理由（现版证据） |
|---|---|---|
| 1 | `vitro_host` **不得直接访问字节载体** | 现版 host 层 10+ 处裸切片绕过 UAF 检查（`host/string.rs:165-166/252-259/275-282`）→ 结构上禁止 |
| 2 | `vitro_vm` 的快照**不得逐字段枚举** | 现版四处枚举 22 字段（`core/snapshot.rs:11-34/54-77/84-114/120-179`），漏一个即静默错值 |
| 3 | `vitro_jit` **必须可整体移除**且不影响其余包编译 | JIT 存废是阶段 0/2 的独立裁定（评估报告 §3.1）；现版 `jit_templates.rs` 与 `executor/mod.rs` **双向耦合**（executor 调 `execute_trace_bulk`，jit 调 `dispatch_single_instruction`）→ 迁移期应先抽接口 |
| 4 | `vitro_host` 的输出写入**必须携带字节而非 String** | 现版 `push_stdout(impl Into<String>)` 使非 UTF-8 字节不可保真 |
| 5 | `vitro_memory` 的 `freed_logs` **必须有序**，区间算法附红锚测试 | 正确性依赖有序 range 查询（`state.rs:694-755`） |
| 6 | 常量与 opcode 号段**单源**，且**跨语言可核对** | 现版两处注释漂移（`opcode.rs:18-19`、`bytecode_libc_index.rs:13-14`） |

### 9.3 对上发布形态

| 形态 | 内容 | 依据 |
|---|---|---|
| **主包** | `vitro_vm` + 全部依赖，编译为 **wasm-gc 单出口**；无 C ABI（消掉 45 个导出函数与 `vitro_abi_version()` 版本化承诺，当前 `abi_version=2.1.0`，as_of 2026-09-14，facts key `abi_version`） | 评估报告 §5-5 |
| **`.mbti` 接口文件** | 只暴露 `compile_and_run(source) -> RunResult`、`Session`（step/seek/feed/config）、`StepPayload`/`MemoryImage` 的 JSON 投影、checkpoint API；**不暴露** `OpCode`/`dispatch_single_instruction`/`freed_logs` 等内部结构 | — |
| **JSON-lines 协议层** | 与现版 `serve` wire format 保持兼容（已语言中立，评估报告 §5-4）；`docs/spec/` 的 schema 直接复用 | `docs/spec/` |
| **Bytecode Libc 产物** | ⚠️ **不建议直接照搬"构建期预编译 + 固定索引段"完整机制** | 该机制的存在理由是 Rust 侧编译启动成本与产物/源码同步；wasm-gc 下可评估改内建原语（但会失去"libc 与用户代码共享同一 VM 执行路径"的教学价值）。**建议阶段 0 不裁定，阶段 2 与 JIT 存废、C++ 子集存废一并裁定**（三者同属"是否保留重量级子系统"） |
| **测试包** | 黑盒 `*_test.mbt`（对外语义）+ 白盒 `*_wbtest.mbt`（直驱内部，替代现版直驱单测）；**形状与期望值外置 JSON** | 仓库纪律"规则外置、代码只做解释器" |

---

## 附录 A · 待证清单（汇总，均附验证方法）

| # | 待证 | 验证方法 |
|---|---|---|
| **A-1** | **JIT 与解释器的溢出语义分歧的实际可观测差异**（⑥-4） | `vitro_cli run` vs `vitro_cli unified` 跑 `int main(){int i,x=0;for(i=0;i<200;i++)x+=20000000;printf("%d %d\n",x,i);}`：`run`（第 103 轮起 bulk）预期静默 wrapping；`unified`（逐条 step）预期第 108 轮教学 trap |
| **A-2** | **JIT bulk 期间 StepEvent 是否完全未执行**（⑥-5） | 含断点的热循环（>100 轮回边）在断点行前后设断点，`step` 与 `run` 对比；或对比 heatmap `line_counts` |
| **A-3** | **`putchar(200)` / `printf("%c",200)` 的字节输出** | 断言 `runtime.stdout().as_bytes() == [0xC8]`；预期 Vitro 给 `[0xC3,0x88]`（`host/io.rs:346`、`host/utils.rs:324-327`） |
| **A-4** | **全部 HashMap 无迭代依赖**（已静态验证，需运行时确认） | 逐点确认 `get_variable_snapshot`/`format_infinite_loop_error`/capi 出口；当前证据：`snapshot_vars` 仅 `trap.rs:113`，`ip_hits` 仅 `executor/mod.rs:306/312`，`jit_traces` 仅 `:19/311/338` |
| **A-5** | **`quarantine_budget` 是否应随快照往返** | 读 capi `vitro_set_quarantine_budget` 会话语义 + 构造"设预算 → 快照 → 改预算 → 恢复"探针，观察恢复后预算值 |
| **A-6** | **20 个 host handler 被 Bytecode Libc 遮蔽** | `vitro_cli export` 含 `atoi/rand/strncpy/memcpy/strlen/isdigit/abs` 的源，检查产物是 `Call <固定索引>` 还是 `CallHost`（静态链已证：`vitro_codegen/src/lib.rs:136-143` + `expr/call.rs:214`） |
| **A-7** | **`strlen(NULL)` 静默返回 0**（⑥-16） | 用例 `strlen((char*)0)`：无 NULL 检查 + `memory[0]==0` → 预期返回 0 且无 trap |
| **A-8** | **`memset`/`memcpy`/`memmove` 写已释放块漏检 E3060** | `p=malloc(8); free(p); memset(p,0,8);` → 预期无 E3060；对照 `p[0]=1` 必报 |
| **A-9** | **二次 `fclose` 是否真不报 E3061** | `FILE*f=fopen(...); fclose(f); fclose(f);` → 预期不 trap（`memory_state.rs:339-355` 不写 freed_logs） |
| **A-10** | **`scanf("%s")` 与 `sscanf("%s")` 非 ASCII 落点是否真不同** | 同字节输入分别走两路径，dump 目标缓冲区（`io.rs:233-242` vs `:552-557`） |
| **A-11** | **`rand`/`srand` 的 `deterministic` 语义归属哪一侧** | `srand(42); rand()` 序列在 `run`（走 Bytecode 固定索引 1013/1014）与直调 `host_rand`（`host/misc.rs:4-14` LCG）下逐值对比 |
| **A-12** | **`sprintf`/`snprintf`/`sscanf` 缺参诊断文案**（⑥-17） | 构造参数不足调用（typeck 若拦则改单测 host 层），确认报"栈下溢"而非"参数多于提供" |
| **A-13** | **`OpCode::Strlen` 确为死路径** | 已静态确认（codegen 零发射点）；再用 export 确认 `strlen` 走 `Call 1015` |
| **A-14** | **baseline 的 JIT 专项口径** | AGENTS.md 写"3 个 JIT 专项"，实测 baseline 有 **5 个** `jit_*.c` → 查 `scripts/shadow_verify` 的分类逻辑 |
| **A-15** | **`--builtin-libc` 产物能否直接作 `bytecode_libc_data.json`** | 不能（library mode 索引从 0 起 + 无 digest），固定索引重定位由 Go 脚本后置（`main.go:171-220`）。验证：手跑 export 对比 `func_index` |
| **A-16** | **`include/*.h` 改动是否触发 `--check` 变红** | 改 `native/runtime_libc/include/stdio.h` 一行 → `--check`；**预期仍 exit 0**（digest 只扫 `src`/`vitro`，`main.go:50/64-86`）→ 门禁缺口 |
| **A-17** | **`bytecode_libc_index.rs` 与 JSON 不同步是否有门禁** | `checkUpToDate` 只做 `os.Stat` 存在性检查（`main.go:428-434`）→ 预期无门禁 |
| **A-18** | **`jit_stats.steps_accelerated` 双重计数的量级**（§3.2-7） | 跑恰好满 `MAX_TRACE_ITERATIONS=1000` 轮的用例，对比 `steps_accelerated` 与 `step_count`（`jit_templates.rs:747 + :750`） |
| **A-19** | **`GETCHAR`/`stderr` 类宿主函数的 `stream` 实参解析** | `host_io.rs` 的 `read_fd_from_stream`（`host/utils.rs:474-484`）为唯一解析点，验证 `fprintf(stderr,...)` 与 `fprintf(fp,...)` 的分流差异 |

---

## 附录 B · 勘察方法与边界声明

**证据来源**：
1. **亲读**：`vitro_runtime`（13 文件）+ `vitro_vm`（27 文件）**全部 40 个源文件**；另读 `vitro_runtime/Cargo.toml`、`vitro_vm/Cargo.toml`、`bytecode_libc_data.json`（结构抽取）、`native/tests/jit_path_parity.rs`（全 286 行）、`native/tests/cases/baseline/jit_*.c`（5 个）、`native/src/engine/compile_pipeline.rs`（`setup_vm` 段）、`scripts/precompile_bytecode_libc/main.go`（digest 链）。
2. **3 路并行只读子勘察**：host funcs 全量清单（含 110 的四路计数互证与 20 个遮蔽项）/ JIT 差分防线与性能基线（8 条测试全表、`vm_bench` 方法学四条、5 个 `.out`）/ Bytecode Libc 加载器与 digest 协作链。
3. **计数互证**：常量数 / 分派臂数 / handler 数 / 名字映射模式数——四路独立计数得同一结论；opcode 132 条由值域分段求和（44+61+19+8）复核。

**本次未做的事（诚实标注）**：
- **未运行任何运行时探针**——未调用 release/debug 二进制、未跑 clang 对照、未构造最小复现程序、未写 `%TEMP%` 产物。因此**全部"缺陷"均为静态证据链推导**；凡涉及"实际输出是什么"的结论（A-1/A-3/A-7/A-8/A-9/A-10/A-11/A-12）一律标注为待证并给出可执行验证方法。
- **未运行 `cargo test` / `cargo clippy` / `moon check`**，未核验编译健康度。
- **未修改任何文件、未执行 git 操作。**

**已知的方法学边界**：
- **行数口径**：本报告用 `(Get-Content).Count`（含空行）。PowerShell `Measure-Object -Line` 会漏空行（同一文件可差 17~18 行），两者不可混用。
- **`host_funcs.rs` 与 `host_func_id.rs` 的行数**在任务书与实测间有差异（156/161、265/284），本报告以实测为准。
- **`opcode.rs:18-19` 与 `bytecode_libc_index.rs:13-14` 的注释均与代码不符**，引用这两处注释作为判据时须先核对代码。

**与同批次报告的接口**：
- [`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md) §2.1/§2.4 的两处代码证据（快照扁平化、`state.rs:80` 的 1MB `Vec<u8>`）本报告已逐条复核并**细化**（§1.5、§1.6、§1.9）。
- [`MoonBit迁移_codegen模块勘察报告20260918.md`](MoonBit迁移_codegen模块勘察报告20260918.md) 的"codegen 只引用 132 个 opcode 中的 127 个、从不发射 `Strlen`"与本报告 §1.8 的"`OpCode::Strlen` 死路径"**互为独立证据**（一侧看发射端、一侧看执行端）。
- [`MoonBit迁移_lexer模块勘察报告20260918.md`](MoonBit迁移_lexer模块勘察报告20260918.md) §⑥ 的"H-5 wasm-gc 无文件系统"与本报告 §4.2-7（VFS 纳入快照）、§5.5（VFS 路径 UTF-8 键）**同源**——两者共同指向"VFS 在目标形态下的存废与形态需一并裁定"。
- [`MoonBit迁移_unified模块勘察报告20260918.md`](MoonBit迁移_unified模块勘察报告20260918.md) 的"**每步一次 1MB 全量快照（换语言一分不省）**"与本报告 §1.5.3（`nearest` 的 `m.clone()` 1MB）**同源**：该报告从 `unified/engine.rs` 侧观察到每步快照，本报告从 `CheckpointManager` 侧观察到每次重建 1MB —— 两侧共同构成"快照成本必须在新实现里改口径"的完整证据。同报告还指出"**统一模式下 JIT 是纯浪费**"，与本报告 §1.4 观察到的机制一致（`unified/engine.rs:173/337` 走 `vm.step()` ⟹ 无 fast path，但 `step()` 内仍会**录制并编译 trace** ⟹ 纯开销 + `traces_compiled>0` 而 `steps_accelerated==0` 的统计污染）。
- [`MoonBit迁移_cpp_frontend模块勘察报告20260918.md`](MoonBit迁移_cpp_frontend模块勘察报告20260918.md) §7 F1 的实测（MoonBit `Int` 静默回绕、全 core 无 checked API）**独立佐证并部分回答**了本报告 ⑦ spike-4，见 §7.2 表内回填。⚠️ **一处需要精确化**：该报告 `:293` 以"JIT 侧 `jit_templates.rs:272,287,297`"作为"JIT 也 trap"的证据，但**那三行是 `tpl_div` / `tpl_mod` / `tpl_neg`**（除法/取模/取反），**`tpl_add` / `tpl_sub` / `tpl_mul` 仍是 `wrapping_*`（`:246` / `:253` / `:260`）**。⇒ JIT 与解释器的分歧**在加/减/乘三项上真实存在**（本报告 ⑥-4 给出判定性用例），不能被"JIT 也 trap"的概括掩盖。这一分歧正是"MoonBit 默认静默回绕"与"Vitro 要求教学 trap"两者相撞时最危险的交汇点：**若新实现照抄 Rust 的 JIT 模板写法，静默错值会同时存在于两条路径**。
- [`MoonBit迁移_typeck模块勘察报告20260918.md`](MoonBit迁移_typeck模块勘察报告20260918.md) 的 **"HashSet 迭代序致 Bytecode Libc 产物布局不可重现"历史实锤** + 本机 core lib 实测（**`Hasher` 默认种子 wasm/wasm-gc = 0 而 native/llvm/js 随机**；`HashMap::iter` 文档明写 unspecified order）**补充了本报告附录 A-4 的反方向证据**：本报告验证的是"VM 侧消费点无一依赖 HashMap 迭代序"（结论成立，逐点已列）；typeck 报告验证的是"**产物的序列化顺序会依赖 HashMap 顺序**"——两者作用于不同环节。⚠️ **由此推出一条本报告 §1.8 未写明的义务**：`BytecodeLibcArtifact.func_table` 是 `HashMap<String,FuncMeta>`，`vitro_cli export` 的中间产物走 `serde_json::to_string_pretty`（`native/src/bin/vitro_cli.rs:382`）⇒ **键序随进程随机**；最终 `bytecode_libc_data.json` 之所以确定，**完全依赖 Go 脚本 `writeOutputs` 的 `sort_keys` 递归键序**（`scripts/precompile_bytecode_libc/main.go:404-424`）。**这条"排序义务在 Go 侧、不在 Rust 侧"的事实必须在迁移时显式继承**，否则产物 diff 锚点（⑧ 工具表）会变成随机失败。
