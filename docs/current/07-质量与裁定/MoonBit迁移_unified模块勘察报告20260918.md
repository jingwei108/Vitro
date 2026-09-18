# MoonBit 迁移 · `native/src/unified/` 模块勘察报告

> 勘察时间：2026-09-18
> 勘察对象：`native/src/unified/`（统一模式 / 时间旅行引擎，19 个 `.rs`，**3,690 行**）及其调用面（`vitro_vm::snapshot` 检查点体系、`session_api`、capi / serve 出口）
> 性质：**只读勘察**。未修改任何仓库文件、未执行任何 git 写操作（仅执行 `git rev-parse` / `git status` / `git diff --stat` 三条只读查询用于新鲜度核对）。
> 上游依据：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)（§2 事实核查 / §7 四道证伪门 / §8 假设清单 A1–A9）、[`STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md)（协议 v0.1 冻结）、[`统一模式设计.md`](../05-教学体验/统一模式设计.md)、[`统一整备路线图.md`](统一整备路线图.md)
> 数字口径：全局数字一律引用 `reports/facts.json` 的 key 与 `as_of`；模块内数字（文件数 / 行数 / 测试数）为本次实测。本文件为 as-of 快照，裸数字不参与 `facts` 回填。
> 证据标记：**【实测】**= 本次现场跑命令所得；**【亲读】**= 代码 file:line；**【文档】**= 文档 file:line；**【待证】**= 未验证，附验证方法。
>
> **实测环境（可复现性声明）**
> - 仓库状态：HEAD = `0b243ca`（`reports/facts.json` 的 `git.rev = 4b57191` 为采集时点，早于 HEAD）
> - **新鲜度核对【实测】**：`git diff --stat 4b57191..HEAD -- native/src/unified native/crates/vitro_vm/src/snapshot.rs native/crates/vitro_runtime/src/runtime_state.rs scripts/replay scripts/serve_smoke` → 仅两处差异：① `native/src/unified/contracts.rs` **4 行**（`V0_2_ACTIVATION_CHECKLIST` 第③④条里的 `python scripts/…` 改成 `go run ./scripts/…`，纯 D5 工具链迁移，**无语义变更**）；② `scripts/serve_smoke/main.go` 为新文件（922 行，D5 Go 化）。→ **本报告分析的 unified 源码即 HEAD 源码**
> - MoonBit 工具链：`moon 0.1.20260915 (2e1a46d)` / `moonc v0.10.13+cbb11c36f` / `moonrun 0.1.20260915`（`C:\Users\liangjingwei\.moon\bin\`）已安装；**本次未运行任何 MoonBit 程序**，一切 MoonBit 语言/工具链结论均标【待证】并给验证命令
> - 全部勘察经 read/grep/只读命令完成，未运行引擎、未构建、未产生仓库内新文件

---

## 0. 一页速览（结论先行）

**三条核心判断：**

1. **头号工程风险 = 「每步一次 1MB 全量快照」**。`UnifiedEngine::run_batch` 在**每个 step 之前**无条件 `vm.snapshot_into(...)`（`engine.rs:169-170`），而这份快照**只有一个用途**——Trap 分支的回退（`engine.rs:196-198`）。代价是每步 1MB `copy_from_slice`（`core/snapshot.rs:86-88`）+ `stack`/`call_stack`/`snapshot_vars`/**`freed_logs`（稳态可达 16,387 条的 BTreeMap）**/`runtime`/`memory_state` 逐字段 clone（`core/snapshot.rs:93-113`）。**这条成本换语言一分不省**：MoonBit 的 `Array[Byte]` 拷贝与 Rust `memcpy` 同价，而"每步分配"在 GC 下只会更贵。项目内已有两处独立登记指向同一处（[`代码审阅与修复追踪20260906.md:192`](代码审阅与修复追踪20260906.md)、[`统一整备路线图.md:747-749`](统一整备路线图.md)），且已给出方向。**这是本次勘察认定的第一优化项。**

2. **头号架构机会 = 把"窗口化"从 O(n) memmove 改为 O(1)，并让 `stream.rs` 的 delta 设计升格为内存表示**。窗口**语义**（2000 帧 / 丢最早 20%）**必须逐字保留**——它是协议可见的（`STEP_PAYLOAD_SCHEMA_V0_1.md:242-244`、`:505`）且有 61 条签字回放断言锚定（`replay_s1_s5.go:794-805` 断言 2050 步后 `cache_start_step > 0`）。但**实现**可以更省：现版裁剪是 `split_off`（`engine.rs:95`，O(len) memmove + 逐个 `StepPayload` drop），且**在重放循环内每步都调用**（`engine.rs:346`）。若把 `StepStreamBatch`/`StepPayloadDelta`（`stream.rs:97-147`，**当前无任何生产调用点**，见 ③-2）升格为**内存窗口的表示**，则"丢弃最早帧"= 移动 base 偏移（O(1)），且帧间未变子结构由不可变值天然共享——**写一次同时满足窗口压缩与 wire 传输**。

3. **「不可变结构天然解决状态线性增长」对帧缓存 _不成立_——必须如实区分两件事**。帧缓存的每个 `StepPayload` 是**不同的值**（`local_vars` 逐帧变），不是同一值的重复；换成不可变结构**不会**减少"每步一份数据"。真正成立的是两条**判据式收益**（不是自动收益）：① **消除"逐字段 clone 必须手工保持一致"的义务**（`VMSnapshot` 定义 `snapshot.rs:54-80` / `snapshot_into` `core/snapshot.rs:93-113` / `restore` `:120-179` 三处字段列表靠人肉同步）——该项目**已经漏过两次**（`region_index` 修于 U2#2，`local_sym_map` **至今未修**，见 ⑥-9）；② **漏字段从运行期静默错值变成编译期不通过**——但这**要求把可变状态整体做成一个不可变值**，而不是继续写"逐字段 clone 函数"。

**规模事实【实测】**：19 文件 / **3,690 行**，占 `native/src/`（14,267 行 / 57 文件）的 **25.9%**；模块内 `#[test]` **28** 条；相关集成测试 8 文件 **74** 条。

**本次新发现（既有登记之外）4 项**：① `stream.rs` 差分编码**无生产调用点**而 `types.rs:1-2` 注释称"已通过 `unified::stream` 差分编码"（注释与代码矛盾）；② 统一模式下 **JIT 录制与编译是纯浪费**（产物永不执行，`jit_traces` 只在 `run()` 里被读）；③ `restore()` 未重建 `local_sym_map` → seek 后 `accessed_vars` 变量名可能错（**未修、零测试覆盖**）；④ `serve_param_domain_regression` 等三处 `panic` 同族缺陷说明"钳位顺序"是系统性隐性知识而非点状疏漏。

**防线缺口 3 项（如实记录）**：① `v01_payload_fields.json` 白名单**只判"多余字段"、不判缺失**（`replay_s1_s5.go:893-905` 遍历方向是 payload→白名单，A3 哨兵缺省放行 `:909-920`）；② S5 **A5 是恒真 PASS**（`:941-943` 原文 `"…记录性 PASS"`），差分编码在出口零覆盖；③ RSS 护栏口径（psapi `PeakPagefileUsage`）**在 Node/wasm 宿主下不可平移**，必须重新标定。

---

## ① 模块概览与规模（实测数字）

### 1.1 模块自身

| 项 | 实测值 | 口径 |
|---|---:|---|
| 文件数 | **19** 个 `.rs` | `native/src/unified/**`（含 `stream/`、`trace_analyzer/` 两个子目录） |
| 总行数 | **3,690** | 逐文件 `(Get-Content …).Count` 求和【实测】 |
| 占 `native/src/` | **25.9%** | `native/src/` = 14,267 行 / 57 文件【实测】 |
| 模块内 `#[test]` | **28**（4 个 `#[cfg(test)]` 块） | `stream.rs:253-481`(7) / `contracts.rs:242-295`(5) / `vocabulary.rs:229-263`(3) / `trace_analyzer/tests.rs`(13) |
| 相关集成测试 | 8 文件 / **74** `#[test]` | 见 ②-2.2 |

> 项目整体规模（257 个 `.rs` / 76,218 行 / 1,015 `#[test]`）出自 [`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md) 附录 B（**AS-OF 2026-09-18 冻结；该报告自述其裸数字不参与 `facts` 回填**），**不是** `reports/facts.json` 的值。

### 1.2 逐文件清单

| 行数 | 文件 | 职责 |
|---:|---|---|
| 516 | `unified/engine.rs` | **编排核心**：`UnifiedEngine`、`run_batch`、`seek_to`、窗口截断、Trap 回退 |
| 481 | `unified/stream.rs` | 差分编码（`StepStreamBatch` / `StepPayloadDelta` + 7 单测） |
| 478 | `unified/collector.rs` | `StepCollector::collect` + **全库唯一语义分类器** `infer_semantic_label` |
| 295 | `unified/contracts.rs` | schema 版本轨道 / 预留位台账 / 行为契约 / `check_unwinding_granularity` |
| 283 | `unified/stream/decode.rs` | `StepStreamBatch` → `Vec<StepPayload>` |
| 263 | `unified/vocabulary.rs` | `semantic_label` 受控词汇表（14 条：10 `active` + 4 `reserved`） |
| 252 | `unified/stream/encode.rs` | 逐字段差分编码器 |
| 240 | `unified/trace_analyzer/bounds.rs` | 越界根因细分（OffByOne / WrongInit / WrongIncrement / UninitializedIndex / Generic） |
| 213 | `unified/trace_analyzer/utils.rs` | **中文 trap message 文本解析** + 源码行扫描启发式 |
| 193 | `unified/trace_analyzer/tests.rs` | 13 单测 |
| 162 | `unified/types.rs` | `StepPayload` 及 13 个子结构 |
| 55 | `unified/trace_analyzer/mod.rs` | trap → 5 类分析器分派 |
| 52 | `unified/trace_analyzer/div_zero.rs` | 除零分析 |
| 50 | `unified/trace_analyzer/null_deref.rs` | 空指针分析 |
| 49 | `unified/stream/diff.rs` | 相等性比较辅助 |
| 41 | `unified/trace_analyzer/use_after_free.rs` | UAF 分析 |
| 29 | `unified/root_cause.rs` | `RootCauseHint`（6 字段纯数据） |
| 29 | `unified/trace_analyzer/double_free.rs` | 双重释放分析 |
| 9 | `unified/mod.rs` | 模块声明 + `pub use vitro_algorithm_steps as algorithm_steps;` |

### 1.3 关键类型

| 类型 | 位置 | 形态要点 |
|---|---|---|
| `UnifiedEngine` | `engine.rs:13-32`【亲读】 | 11 字段：`checkpoints: CheckpointManager`、`frame_cache: Vec<StepPayload>`（**`pub`**）、`max_steps: i32`、`is_paused`/`is_cancelled`/`is_finished: bool`、`pre_step_snap: Option<VMSnapshot>`（私有，复用 1MB buffer）、`frame_cache_window_size: usize`(2000)、`frame_cache_trim_ratio: f64`(0.2)、`frame_cache_start_step: i32`（`pub`） |
| `StepPayload` | `types.rs:16-31` | 14 字段，全为 `String`/`Vec`/`Option`，`#[derive(Clone, serde::Serialize)]` |
| `StepStreamBatch` / `StepPayloadDelta` | `stream.rs:135-147` / `:97-126` | 差分传输单元；**无生产调用点**（③-2） |
| `CheckpointManager` | `vitro_vm/src/snapshot.rs:162-172` | `checkpoints: Vec<(i32, VMSnapshot)>`、`interval`(20)、`smart_mode`(true)、`max_checkpoints`(50)、`full_every`(5) |
| `VMSnapshot` | `vitro_vm/src/snapshot.rs:55-80` | **无 serde**（快照不过 JSON）；`MemoryImage::Full(Vec<u8>)` \| `Delta{base_step, pages}`（`:9-15`） |
| `SemanticLabelKind` / `BehaviorContract` | `vocabulary.rs:23-36` / `contracts.rs:112-120` | 词汇表与行为契约的机器可读单源 |

### 1.4 调用面与边界（**边界画在哪里**）

```
出口（薄包装）
  capi:  vitro_step_begin / vitro_step_next_json / vitro_get_step_payloads_json / vitro_set_breakpoints ← 仅 4 符号
         native/src/capi/first_batch.rs:418 / :437 / :458 / :478
  serve: step.begin / step.next / payload.get / seek / breakpoints.set / session.reset / capabilities
         native/src/bin/vitro_cli.rs:724 / :738 / :742 / :750 / :759 / :620
         ── ★ seek 只有 serve 有，无 C ABI 导出（全仓 grep `vitro_.*seek` 零命中）【实测】
语言中立宿主（语义单源）
  native/src/session_api.rs:216(step_begin) / :239(step_next) / :420(payloads) / :432(seek) / :444(set_breakpoints)
  ── take/put 舞蹈：session.unified.take() … session.unified = Some(engine)   :240-246、:433-439
                    session.vm.take() …     session.vm = Some(vm)            :227/:243、:436
编排层（本模块）
  unified/engine.rs ── 依赖 CheckpointManager（vitro_vm crate）+ VitroVM::snapshot/restore/step
状态层（VM crate，跨 crate 边界）
  vitro_vm::snapshot::CheckpointManager ←→ VMSnapshot / MemoryImage / RuntimeSnapshot / MemorySnapshot
  vitro_vm::core::{VitroVM, StepResult} ←→ core/snapshot.rs: snapshot / snapshot_into / restore
```

**边界判读（三条硬结论）**：

1. **时间旅行策略寄生在 VM crate 里（分层破损）**【亲读】。`CheckpointManager` 定义在 `vitro_vm/src/snapshot.rs:162`，但 `should_checkpoint` 的判据是**教学语义标签字符串**（`snapshot.rs:199-204` 匹配 `"调用 "` / `"返回"` / `"内存分配"` / `"释放内存"` / `"交换"` / `"循环"`），而这套词汇的**单源定义与产出**都在 `native/src/unified/`（`vocabulary.rs:41-155`、`collector.rs:413-458`）。**VM crate 反向依赖了应用层教学语义**——可直接平移成 MoonBit 包的循环依赖（⑨ 给出去向）。
2. **快照语义跨 crate 且靠人肉成对演进**【亲读】。`restore()`（`core/snapshot.rs:120-179`）逐字段回写，而 `MemorySnapshot`（`snapshot.rs:105-118`）与 `MemoryState` 是**两个不同类型**（`snapshot.rs:104` 注释自述"与 `MemoryState` 是不同类型"）。评估报告 §4.1 所说"唯一只有换语言才能结构性消除的问题"（`#[derive(Clone)]` + 手工枚举字段，漏一个即静默错值）在本模块有**两处实锤**：`memory.rebuild_region_index()` 曾漏（U2#2 修复，`core/snapshot.rs:167-169` 留痕）、`local_sym_map` **至今仍漏**（⑥-9）。
3. **seek 是唯一"只在 serve 上存在"的时间旅行能力**【实测】。单出口 wasm-gc 形态下 serve JSON-lines 是**唯一活下来的通道**——好消息是协议层已语言中立（`STEP_PAYLOAD_SCHEMA_V0_1.md:35` 明示"本 schema 语言中立"）；缺口是若上游期待 C ABI 可 seek，需补 `seek_to_step`（[`项目路线图.md:82`](../01-定位与路线/项目路线图.md) 已排 capi 第三批）。

---

## ② 可复用资产清单

### 2.1 语义设计资产

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| A1 | **`StepPayload` 14 字段 + 子结构语义** | `types.rs:16-162`；协议 §1–§3 | **低** | 纯数据契约、已冻结、已有字段冻结测试。`struct` + `enum` 直接映射 |
| A2 | **受控词汇表（词汇即契约）** | `vocabulary.rs:41-213` | **低** | MoonBit 的 enum + 穷尽 match 是它的**更优载体**：`id: &'static str` 升格为 enum variant，`classify()` 的字符串前缀链（`:163-213`）变成构造器穷尽匹配，新增词汇漏登记从"运行期测出来"变成**编译期报错** |
| A3 | **版本轨道 + 预留位 tripwire** | `contracts.rs:16-42`、`:58-108`、`:123-159`、`:181-228` | **低** | 纯常量表 + 纯函数判据；`check_unwinding_granularity` 的五条规则（`:171-180` 注释）是完备的可执行规格 |
| A4 | **语义分类器判定顺序** | `collector.rs:324-459` | **中** | 判定是移植友好的（顺序判定 + 字符串启发），但**内容高度中文/形态耦合**（`source_line.starts_with("for ")`、`contains("temp") && contains("arr[")`）。照抄可行，需注意 Unicode 与 `String` 索引口径差异 |
| A5 | **窗口采样与 seek 顺序契约** | `engine.rs:267-407`；协议 §4.2（`STEP_PAYLOAD_SCHEMA_V0_1.md:246-254`） | **低** | 顺序契约已文档化，实现照搬 |
| A6 | **根因推断五分类** | `trace_analyzer/{bounds,div_zero,double_free,null_deref,use_after_free}.rs` | **中** | 判据是资产（尤其 `infer_bounds_category` 四条规则 `bounds.rs:108-144`）；但**入口是解析中文 trap message 文本**（`utils.rs:41-78` 三段 `msg.find("你访问了 ")` 式挖取）。教学价值高、迁移风险中 |
| A7 | **检查点链不变量（含悬空 Delta 禁令）** | `snapshot.rs:246-256` 注释 + `:369-383` `assert_no_dangling` 判据 | **低→中** | 不变量表述极清晰（"每个 Delta 的 `base_step` 必须解析到链内存在的 Full"）。照抄即得防线；若按 ⑤-5.4 改类型表达则成本升中，但该类缺陷消失 |
| A8 | 差分编码的 `null` vs `[]` 三态语义 | `stream.rs:109-124` 注释 + 协议 §5.2 末句"**`null` 与 `[]` 的区别是契约的一部分**" | **低** | `Option[Array[T]]` 与 `Option<Vec<T>>` 同构，语义无损 |

### 2.2 测试资产（**8 文件 / 74 `#[test]`**，逐文件实测）

| # | 文件 | 用例数 | 与本模块关系 | 移植成本 | 理由 |
|---|---|---:|---|---|---|
| T1 | `native/tests/step_payload_schema_v0_1_test.rs` | 10 | **协议冻结骨**：14 字段键集合、5 个子结构键集合、4 个枚举字面量、窗口 2000/20%、预留位缺省 tripwire、词汇闭合、文档↔代码单源 | **中** | 断言对象是 **capi 出口 JSON**（`:6` 原文"协议契约必须在出口成立"）。单出口后须改走 **serve**——改入口不改判据 |
| T2 | `native/tests/unified_engine_window_test.rs` | 4 | 窗口公共 API（`max_collected_step` / 负参 `get_payloads` / `reset`） | **低** | 不依赖 VM/Session，纯结构测试 |
| T3 | `native/tests/step_payload_vars_test.rs` | 8 | 变量可见性 / 跨函数隔离 / 数组摘要可读类型名 / 4 条标注一致性（P0-1~P0-4） | **中** | 走 `session_api` 全链路（`:21-49`），语义正确性主力资产 |
| T4 | `native/tests/serve_param_domain_regression.rs` | 6 | 三个 panic 最小复现 + 终态末帧不重放 | **低** | 全走 `session_api`，逐条可搬 |
| T5 | `native/tests/test_snapshot.rs` | 21（**14 条属 U2 批次**：u21/u24/u25/u27/u28/u210/u211） | 快照往返 / 增量链 / 智能检查点 / **seek 重放窗口与步数预算** / COW 共享 / 环形上限 | **中** | 与 VM 快照体系强耦合；`test_u21_seek_replay_*` 是 U2#1/#11 红锚 |
| T6 | `native/tests/output_log_budget_test.rs` | 3 | U2#3 有界输出 + O(1) 长度一致性 | **低** | 独立于 unified |
| T7 | `native/tests/capi_first_batch_tests.rs` | 19（**3 条**属统一模式） | `step_begin` 需已编译 / `step_next` 字段形状 / 断点暂停 | **低** | 3 条可搬，其余属 capi 通用 |
| T8 | `scripts/core_asset_verdict/mutation_facet_test.py` | **6 突变切面**，其中 **2 个对准本模块**：`M2-PTR-STATUS`、`M6-PAYLOAD-LABEL`（均 `native/src/unified/collector.rs`，`:45-52`、`:83-88`） | **按使用切面测"能拦截它的防线数"（margin）** | **高** | Python、依赖 `git checkout --` 还原、依赖 release DLL + 影子全量跑。但**方法论必须平移**：MoonBit 版要能回答"改坏 collector 的一个判据，有几条独立防线会红" |

### 2.3 文档资产

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| D1 | **`STEP_PAYLOAD_SCHEMA_V0_1.md`（614 行，v0.1 已冻结）** | `docs/spec/` | **低（直接复用）** | 语言中立协议。§4 窗口语义 / §4.2 越窗 seek 五步顺序 / §5 差分编码 / §6 出口形状 / 附录 A 不变量速查。**注意 §5 描述的差分编码当前无生产者**（③-2） |
| D2 | `统一模式设计.md`（996 行，唯一设计文档） | [`05-教学体验/`](../05-教学体验/统一模式设计.md) | **中** | §2.3 Seek 策略 / §4 六状态机 / §8 边界情况是有价值的**意图**资产；但 §5.3 的代码块已与实现脱节（③-O5） |
| D3 | 事故留痕档 `事故202609_Seek重放泄漏.md`（67 行） | [`INCIDENTS/`](INCIDENTS/事故202609_Seek重放泄漏.md) | **低** | 现象 / 峰值 / 最小复现 / 根因 / 留痕缺口五段完整，是 ⑥ 的一手来源 |
| D4 | `统一整备路线图.md`（912 行，U 系列唯一排期权威） | 同目录 | **低** | §2 批次表 + §8 状态流水 = 迁移期"哪些已完成 / 哪些在途"的权威台账（④ 全文依据） |
| D5 | schema 附录 B（词汇表） | `STEP_PAYLOAD_SCHEMA_V0_1.md:573-614` | **低** | 与代码同源，由 `test_schema_doc_v0_2_track_matches_code`（`step_payload_schema_v0_1_test.rs:425-457`）强制 |

---

## ③ 抛弃清单

### 3.1 Rust 特有机制（**必须替换，不能照搬**）

| # | 机制 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| R1 | **`Arc` + `Arc::make_mut` 写时复制** | `vitro_runtime/src/runtime_state.rs:85-89`、`:116-118`、`:133-164`；`core/executor/mod.rs:296` | MoonBit 无 Arc。**且该 COW 在"每步一次快照"的形状下已退化**：`snapshot_into` 每步 clone 三个 Arc（`core/snapshot.rs:112-113`）→ 下一步任一写入触发 `make_mut` 全量深拷。注释自称"消除 50× 克隆放大"（`runtime_state.rs:86`），**但同一句就写着"`snapshot_into` 每步"**——自相矛盾；真实收益只在"`save` 每 20 步"那条路径成立 | 换成不可变值时"共享"免费，但"每步一次快照"的拷贝成本**一分未减**；必须同时改快照粒度（⑤-5.1） |
| R2 | **`pre_step_snap` buffer 复用** | `engine.rs:24-25`、`:169-170`；`core/snapshot.rs:84-114` | 该优化的唯一目的是"避免每步分配新 1MB `Vec`"（`core/snapshot.rs:80-83` 注释；`CHANGELOG.md:2119-2123` 记为 O1/O4 优化）。MoonBit 下"复用一块可变缓冲"仍是同一形状——**但真正的成本是每步 1MB `copy_from_slice`，不是分配**。照搬 = 继承 Rust 的"半修" | 10 万步 × 1MB ≈ **100GB memcpy 流量**（`代码审阅与修复追踪20260906.md:192` 原文口径）；GC 下还要叠加垃圾压力 |
| R3 | **`#[derive(Clone)]` + 手工逐字段同步的快照** | `VMSnapshot` `snapshot.rs:54-80`；`snapshot_into` `core/snapshot.rs:93-113`；`restore` `:120-179` | 三处字段列表**必须手工一致**：结构定义 / 赋值 / 回写。任一处漏字段 = 静默错值（**已发生两次**：`region_index`、`local_sym_map`） | **这是换语言收益最实的一条**——前提是把可变状态整体做成不可变值（§0 判断 3） |
| R4 | **`take` / 放回舞蹈** | `session_api.rs:240-246`、`:433-439`；`vitro_cli.rs:445-447` | 纯借检查器产物：`engine.run_batch(&mut vm, session, 1)` 需要 `&mut Session`，而 engine 就在 session 里 | MoonBit 无借用检查器 → **该舞蹈整体消失**；但"对象回指持有者"的设计仍需一次想清楚（⑤-5.6） |
| R5 | **`usize::try_from(...)` 防御 + `as usize` 绕回** | `engine.rs:436-439` | **症状治疗**：根因是"用有符号步号差做下标"，负值 `as usize` 绕回 `usize::MAX` | MoonBit 下若写 `Int`→`UInt` 的裸转换，**同类绕回会重现**（【待证】spike S3）。正确做法是 `Option`/`Result` 或显式钳位 |
| R6 | **`std::env::var("VITRO_SEEK_DEBUG")` 调试出口** | `engine.rs:302-308`、`:382-388` | wasm-gc 单出口无环境变量 | 换显式 debug 开关参数或删除 |

### 3.2 症状治疗代码 / 事实性死代码（**如实记录，不粉饰**）

| # | 项 | 证据 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| S1 | **`stream.rs` 整套差分编码无生产调用点** | 全仓 grep `encode_payloads` / `decode_batch`【实测】：命中**只有 `stream.rs` 自身与其单测** + `stream/decode.rs:11`。而 `types.rs:1-2` 注释称"跨 FFI 传输时**已**通过 `unified::stream` 进行差分编码"——**该注释与代码矛盾**（FRB 时代为真，`flutter_bridge.rs` 已于 R2 整删） | 若照文档移植，会实现一个没人调用的模块。协议 §5 仍把它写成契约（`STEP_PAYLOAD_SCHEMA_V0_1.md:264-302`），S5 A5 更是**恒真 PASS**（`replay_s1_s5.go:941-943`） | **必须三选一并记录理由**：(a) 作为 wasm-gc 单出口的真实传输格式复活（原意图）；(b) 升格为**内存窗口表示**（⑤-5.2，最有价值）；(c) 走版本化从 spec §5 正式退役。**不可默认照抄** |
| S2 | **`trace`（`Arc<Vec<TraceEntryData>>`）与 `TRACE_LIMIT = 4096` 环形上限** | `runtime_state.rs:66-68`、`:91-94`；红锚 `test_snapshot.rs:828-847` | `test_snapshot.rs:829-831` 原文实测：**"codegen 已不发射 `__vitro_step` 标记（`host_step` 为死路径），runtime.trace 实际零写入"** | 死代码 + 防御性上限。移植时要么删、要么明确留作"未来重新插桩"的占位（现版是后者） |
| S3 | **`CheckpointManager` 的 `Default` 派生** | `snapshot.rs:161`（`#[derive(Clone, Default)]`）+ `:190`（`step % self.interval`） | `Default` 产出 `interval = 0` → 取余 panic 雷点。全仓 grep **无任何 `CheckpointManager::default()` 调用点**（`UnifiedEngine::default()` 走 `new()`，`engine.rs:34-38`）【实测】 | 潜在雷点、当前不可达。MoonBit 版应删 `Default` 或让 `interval` 为非零常量 |
| S4 | **手写字段映射无法在新增字段时强制补齐** | `stream/encode.rs` / `decode.rs` 全篇人工字段列表；`collector.rs:309` 的 `_ => v.value.to_string()` 静默兜底 | Rust 的 record 更新与手写映射**不提供穷尽性检查** | 见 ⑤-5.3：MoonBit 若用穷尽 match 表达 payload→delta 映射，**新增字段会编译失败**——真实收益 |

### 3.3 组织债（**抛弃的是组织方式，不是代码**）

| # | 债 | 证据 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| O1 | **`native/src/compiler/ast.rs` 是孤儿文件** | `native/src/compiler/mod.rs:1-12` **未声明 `mod ast`**（只有 `algorithm_detector`/`cfg`/`data_flow`/`intent` + `pub use vitro_ast as ast`）；`compiler/ast.rs:11-14` 声明的 `decl`/`expr`/`stmt`/`types` 子模块在磁盘上**不存在**（glob `native/src/compiler/**` 无 `ast/` 目录）【实测】 | crate 化前残留，**不参与编译**。这正是评估报告 §4.1 所指"两套组织方式并存"的具体形态 | 迁移勘察若按文件树照搬，会把这 116 行死代码一起搬走 |
| O2 | **`unified/` 未 crate 化，阻力已被识别** | [`工程债务维护方案.md:565`](../01-定位与路线/工程债务维护方案.md)：`⚠️ 部分下沉…types.rs/stream.rs/root_cause.rs/engine.rs/collector.rs 仍保留在 vitro_native 内部 —— FRB 已移除，原孤儿规则阻碍消失，可重新评估（尚未执行），剩余阻力是与 Session 的耦合` | Rust 侧只是"可重新评估"；MoonBit 是从零建包，**没有理由继承"与 Session 耦合"这个阻力**（⑨ 给依赖反转） | 若不做依赖反转，MoonBit 侧会重演同样耦合 |
| O3 | **`should_checkpoint` 用裸中文字符串前缀匹配词汇** | `snapshot.rs:199-204` | 词汇的**单源**在 `vocabulary.rs`，检查点策略却**没走** `vocabulary::classify()`。词汇一旦漂移（历史上发生过 `循环边界` → `循环`，`collector.rs:405-412` 留痕），检查点密度会**静默下降** | 中：不报错只变慢，最难发现 |
| O4 | **`frame_cache` 是 `pub` 字段，测试直接改** | `engine.rs:15`；`unified_engine_window_test.rs:33-34`；`serve_param_domain_regression.rs:104-105` | 为可测性放弃封装，把"窗口起点与缓存长度必须自洽"的不变量暴露给外部 | 低-中：MoonBit 版应把窗口做成自带不变量的类型（`FrameWindow`），测试走它的构造 API |
| O5 | **文档与代码脱节（两处）** | (a) `统一模式设计.md:485-491` 的 `UnifiedEngine` 结构体**缺 5 个字段**（`is_finished` / `pre_step_snap` / 窗口三字段）；`:516` 的 `should_checkpoint(step, &meta)` 与实现签名 `should_checkpoint(step, &str)` 不符；`:521` 写 `vm.snapshot(session)` 每步而实现是 `snapshot_into` 复用。(b) `flutter_bridge.rs` 已在 R2 整删，但 `工程债务维护方案.md:278/367/568`、`统一模式设计.md:922/984` 仍写其"仍存在" | 迁移期若照文档实现会实现成旧版 | 中：文档是本次勘察的输入之一，**必须以代码为准** |

---

## ④ 在途工作接纳方案

> U 编号原始登记均出自 [`统一整备路线图.md`](统一整备路线图.md)（§2 批次表；**该表无状态列**，状态散在 §8 文字流水，`:323-912`）。**§8 中不存在任何 U6 条目**。

| # | 条目（原文位置） | 现状 | 新项目接纳方式 |
|---|---|---|---|
| 1 | **U2#1** seek 重放循环内窗口化（`:134`） | **✅ 已完成**（`:596-612` 原文，2026-09-14）："峰值 75MB → 23MB（-69%），斜率 ≈57B/步 ≤ 验收线 64B/步 ✓；J5 部分收紧——serve_smoke 预算 512 → 64MB" | **修复后搬**：实现形态照搬（`engine.rs:341-346` push 后立即 trim + `:351`/`:362` 两个 return 分支同步补）。**但窗口丢弃建议改 O(1)**（⑤-5.2） |
| 2 | **U2#11** 重放步数预算（`:144`） | **✅ 前半 + 后半均完成**：前半 = 循环内补 `max_steps`（`engine.rs:327-335`）；后半 = `stdin_eof`/`heatmap` 随快照回滚（`:713-716`；`core/snapshot.rs:156-158`） | **直接按目标架构实现**：纯逻辑，无语言特性依赖 |
| 3 | **U2#6** 检查点快照内存预算实测归档（`:139`，**未做**，`:746` 仍列"U2 剩余"） | 未做 | **在新项目重做（口径必须变）**：Rust 侧测"20 × VMSnapshot 全量 1MB"；MoonBit 侧应测"活跃快照总字节 + 峰值 RSS"，宿主从 Rust 进程变为 Node/wasm → **数值不可比，须重新标定** |
| 4 | **U2#7** `run_batch` 每帧双份 clone（`:140`） | **半闭**（`:727-731` 原文）：早退丢帧已修（`engine.rs:219-224` 先 `finalize_batch` 再 Err）；"每帧双份 clone"**主动放弃**，理由原文"Arc 全链化波及 6+ 文件与 schema/serve/replay 链路而热路径每批仅 1 帧，收益不抵回归风险"（`finalize_batch` 仍在 `engine.rs:259` 做 `extend(payloads.iter().cloned())`） | **在新项目天然解决**：不可变值传参无需 clone。**但不要把"放弃"误读为"问题不存在"**——只是被判定性价比不足 |
| 5 | **新立项**：`snapshot_into` 每步深拷 `freed_logs`（BTreeMap，稳态可达 **16387** 条）（`:747-749`） | 未做 | **必须重新设计**：这是"每步一次快照"形状下最大的克隆放大项。MoonBit 若保留每步全量快照 = 每步深拷 16k 条 → **不可接受**。与 ⑤-5.1 合并 |
| 6 | **U6#4** unified 会话状态所有权（`:194`、`:314`、`:315`；**未开工**） | 目标形态 = "unified 专属状态收进 `UnifiedEngine`、`Session` 只持 `Option`"（症状已修 `e3f3525`，本项为生命周期所有权设计收口） | **直接按目标形态实现**，工作量评估见 4.1 |
| 7 | **U2#5** 检查点淘汰悬空 Delta | **✅ 已完成**（3 条红锚 `snapshot.rs:388/420/456`） | 修复后搬；或按 ⑤-5.4 用类型表达使其不可能 |
| 8 | **U2#10** 数组快照 256 截断（`:143`；`:738-741` 完成） | **✅ 已完成**：payload 级截断 + `truncated` 标记（`types.rs:80-82`；`stream.rs:48-50` serde default 向后兼容） | 修复后搬（含协议字段） |
| 9 | **U2#3** `OutputLog` 有界（`:713-716`） | **✅ 已完成**（`output_log_budget_test.rs` 3 条） | 修复后搬 |
| 10 | **VFS 不入 `VMSnapshot`**（`代码审阅与修复追踪20260906.md:187`，**未修**） | `snapshot.rs:55-80` 无 vfs 字段；VFS 自身有 `snapshot_files/restore_files`（`vfs.rs:656-674`）但未接入 → `fopen("w")→fwrite→回退→重放` **看到未来数据** | **按目标架构实现**（快照边界完整化），并**补一条断言**：回退后文件状态必须是当时的（现版**零覆盖**） |
| 11 | **`restore` 不重建 `local_sym_map`**（`代码审阅与修复追踪20260906.md:230` P2；本次静态核实**仍未修**） | `core/snapshot.rs:120-179` 只重建 `region_index`（`:169`），**无** `rebuild_local_sym_map()`；该映射只用于命名 `accessed_vars`（`executor/stack.rs:39-43,61-65`），上次重建点是 Call/Ret（`executor/control.rs:120,218-245`） | **按目标架构实现**：派生索引改惰性计算或"restore 后统一重建单入口"。**并补红锚**——当前零测试覆盖（⑥-9） |
| 12 | **每步 1MB memcpy 的 O(n²)**（`代码审阅与修复追踪20260906.md:192`、`:285`；[`项目路线图.md:83`](../01-定位与路线/项目路线图.md) Phase 3） | 未修（只修了"每步分配"） | **重新设计**（⑤-5.1）。原审阅已给方向："'执行前快照'仅用于 Trap 回退，可改用已有 CheckpointManager" |
| 13 | **`time()`/`clock()` 真墙钟破坏重放确定性**（[`项目路线图.md:100`](../01-定位与路线/项目路线图.md) G4） | 未修（`host/misc.rs:528,540`） | **按目标架构实现**（确定性伪时钟）。理由：时间旅行引擎全部语义建立在"从检查点重放能复现历史"上，墙钟直接击穿该前提 |
| 14 | **wasm 三护栏**（U6#4 扩容项 `:315`：panic 护栏 / 能力缺失显式报错 / 伪时钟） | 未开工 | **新设计**：单出口形态下三项全变一等需求（[`项目路线图.md:99`](../01-定位与路线/项目路线图.md) G3 明确"wasm 下统一模式无 `catch_unwind`"） |
| 15 | 低优先级：`LinkedListSnapshot` / `TreeSnapshot` 进 `StepPayload` | `统一模式设计.md:958`（未做） | **放弃并记录理由**：属协议增量且需版本化，与时间旅行核心无关；除非新前端提出需求 |
| 16 | `vitro_unified` crate 化 | [`工程债务维护方案.md:565`](../01-定位与路线/工程债务维护方案.md) | **在新项目天然落地**（从零建包，⑨） |

### 4.1 U6#4 直接按目标形态实现的工作量评估

**现状精确刻画**（本次逐字段核实——**不是"unified 状态散在 Session 里"，残留是 4 个字段**）：

| 字段 | 位置 | 唯一读写点 |
|---|---|---|
| `unified: Option<UnifiedEngine>` | `session.rs:369` | ✅ **已是目标形态**（`session_api.rs:234/240-246/421/433-439`） |
| `unified_pending: Option<StepPayload>` | `session.rs:374` | `session_api.rs:79`、`:224`、`:264-291` |
| `unified_last_line: i32` | `session.rs:379` | `collector.rs:83-86` |
| `unified_last_frame_vars: Vec<VariableSnapshot>` | `session.rs:380` | `collector.rs:84`、`:117` |
| `unified_row_entry_vars: Vec<VariableSnapshot>` | `session.rs:381` | `collector.rs:84`、`:111` |

**并且 U6#4 漏掉了同病的第二个实例：`Session.vm: Option<VitroVM>`**（`session.rs:365`）——`session_api.rs:227/243/436` 与 `vitro_cli.rs:445-447` 同样在做 `session.vm.take()` / 放回。

**Rust 侧的真实障碍**（决定工作量量级）：`StepCollector::collect` 是**静态方法**，签名 `(&mut VitroVM, &mut Session, i32)`（`collector.rs:12`），它必须写 `session.unified_last_*`（`:83-86`），而 engine 又持有在 session 里 → 借用冲突 → 只能继续 take/put。**这是 Rust 下被判"大"的原因**（波及 `session_api` 全部 5 个入口 + capi/serve/cli）。

**MoonBit 侧工作量：低**。语言级障碍消失：
- 无借用检查器 → engine 的方法接收 session（回指持有者）合法【待证：spike S6】；
- 4 个字段搬进 `UnifiedEngine` 后，`collect` 变成 engine 的方法，`session.unified_last_*` 读写变为 `self.unified_last_*`；
- take/put 舞蹈整体删除（VM 一并收进 `Execution` 后，`session.vm.take()` 也不再需要）。

**验收判据（可平移）**：`session_api` 的 5 个入口（`step_begin`/`step_next`/`payloads`/`seek`/`set_breakpoints`）**签名与语义不得变化**——这是三出口共用的语义单源（`session_api.rs:1-12` 模块头注的架构纪律第 2 条）。

---

## ⑤ 架构优化建议（MoonBit 形态）

### 〔结构优化〕5.1 消除"每步 1MB 全量快照"——Trap 回退改用**重放重建**（最高优先级）

**现状**【亲读】：`run_batch` 循环内每步无条件 `vm.snapshot_into(...)`（`engine.rs:169-170`），而该快照**只有一个用途**——`StepResult::Trap` 分支的 `vm.restore(pre_step_snap, ...)`（`engine.rs:196-198`）。代价：每步 1MB `copy_from_slice`（`core/snapshot.rs:86-88`）+ `stack`/`call_stack`/`snapshot_vars`/`freed_logs`/`runtime`/`memory_state` 逐字段 clone（`:93-113`）。

**建议**：Trap 分支改为"取最近检查点 + 正向重放到 `step-1`"——**该机器已经存在**（`checkpoints.nearest` `snapshot.rs:287` + `vm.restore` + `seek_to` 的重放循环 `engine.rs:313-378`）。变更后：
- 常态路径每步成本从 O(1MB) 降到 O(1)；
- Trap 路径成本 O(interval = 20 步)，而 Trap 每次运行至多一次；
- **语义必须精确保持**：现版 Trap 后是"回退到 pre-step 再 collect"，即 trap 帧描述的是 **trap 前**的状态（`engine.rs:198-199`）。重放版需精确重现（重放到 `step-1` 后以 `step_index = step` 收集）。

**风险**：重放要求确定性（stdin 游标 / `rand_seed` / `stdin_eof` / `heatmap` 都必须随检查点回滚——**这四项 U2#11 后半刚补齐**，`core/snapshot.rs:156-158`，说明前置条件已具备）；重放期间 `is_paused` / `waiting_input` 副作用必须屏蔽。
**验收**：`test_snapshot.rs::test_snapshot_continue_execution_equals_direct_run` 等价类 + 新增"trap 帧逐字段与旧实现一致"差分。
**依据**：`代码审阅与修复追踪20260906.md:192` 已给出同一方向；`:285` 建议第 12 条"trap 回退改用'最近检查点 + dirty 页'重建"。

### 〔结构优化〕5.2 frame_cache 用"持久差分帧"承载——窗口丢弃 O(1)，**内存表示与 wire 表示统一**

**现状**【亲读】：`frame_cache: Vec<StepPayload>`（`engine.rs:15`），裁剪 = `split_off(discard)`（`engine.rs:95`）→ 每次 O(len) memmove + 逐个 `StepPayload` drop（每个含 6+ 个堆分配 Vec，事故档 `:37` 原文）。且**重放循环内每步都调用它**（`engine.rs:346`）→ 远距 seek 时是 O(2000)/步的 CPU。

**建议**：窗口元素改为"**基准帧 + 相对前帧的 delta**"的持久序列：
- 丢弃最早帧 = 移动 base 偏移（**O(1)**）；
- 帧间未变子结构（`call_stack` / `array_snapshots` / `pointer_snapshots`）由不可变值天然共享；
- 容量从"2000 × 帧全量"降到"2000 × 变化量"，J5 斜率线（64B/步，`核心资产重构裁定.md:228`）更易达标；
- **副产品**：`stream.rs` 已有的 `StepPayloadDelta`（现无生产者，③-2）**直接升格为内存表示**——写一次同时满足窗口压缩与 wire 传输，消除"两套表示漂移"隐患。

**关键约束（不可越界）**：**窗口语义本身不能删**。协议已把窗口写进契约：`STEP_PAYLOAD_SCHEMA_V0_1.md:505` §8 #6「窗口外的历史 payload 不可查询 … **设计如此**（内存有界）」、`:242-244` §4.1「窗口外的部分**静默丢弃**」；且 `replay_s1_s5.go:794-805` **S3 A14 断言 2050 步后 `cache_start_step > 0`**——若"因为不可变结构省内存所以不截断"，**该断言会红**，等价性验收失败。

**「不可变结构天然解决状态线性增长」的判断：对帧缓存 _不成立_**。理由：每步的 `StepPayload` 是**不同的值**（`local_vars` 逐帧变），不是同一值的重复；不可变结构本身不减少"每步一份数据"。要真正减少，必须让帧之间**共享未变子结构**（持久化 + 结构共享）——那是**新设计**，收益真实但需要跨帧共享点的显式建模，不能靠"用了 GC 语言"自动获得。（回收性风险见 spike S2。）

### 〔沿革保留〕5.3 窗口与检查点参数**逐字保留**

`frame_cache_window_size = 2_000`、`frame_cache_trim_ratio = 0.2`、`CheckpointManager::new(20)`（`engine.rs:51,58,59`）、`max_checkpoints = 50`、`full_every = 5`、`smart_mode = true`（`snapshot.rs:177-182`）、`max_steps` 默认 100_000（`engine.rs:42`）。
理由：这些数字全部被协议文本（`STEP_PAYLOAD_SCHEMA_V0_1.md:232-238` 表格）与 61 条回放断言锚定。**改一个数字就是改协议。**

### 〔结构优化〕5.4 检查点链的不变量**升格为类型**

**现状**【亲读】：`checkpoints: Vec<(i32, VMSnapshot)>`（`snapshot.rs:163`），Delta 基准是 `base_step: i32`（`:14`），靠**运行期查找**解析（`nearest` `:293-298`）。悬空 Delta 禁令只写在注释（`:246-256`），防线是 3 条单测（`:388/420/456`）。U2#5 修复前它导致过 **"seek 静默错内存"**（无报错的最坏调试器缺陷）。

**建议**：`Checkpoint = Full(step, Snapshot) | Delta(base : FullRef, pages)`——Delta **直接持有基准 Full 的值引用**（不可变共享），则"删掉 Full 使 Delta 悬空"**在类型上不可表达**。代价：被引用的 Full 无法随链淘汰（内存 ↑），需与引用计数/显式断链权衡。
**判定**：这是**应做 spike 的取舍**（spike S4），不是无脑采纳。

### 〔结构优化〕5.5 派生状态统一重建入口（或改为惰性）

**现状**：`restore()` 必须手工重建派生索引，已漏两次（`region_index` 修于 U2#2；`local_sym_map` 至今漏）。
**建议**：把"可从快照推导的状态"（`local_sym_map`、`region_index`、`last_accessed_vars`、`jit_traces`、`snapshot_vars`）全部改为**惰性视图**（不随快照存储、按需从主状态计算）或收敛到单一 `rebuild_derived()`。
**收益**：把"漏重建"从运行期静默错误变成**结构上不可能**。

### 〔结构优化〕5.6 U6#4：unified 专属状态收进 `UnifiedEngine`，并**顺带收拢 `vm`**

见 4.1。建议引入 `Execution { vm, engine }`，`Session` 只持 `Option[Execution]`，则两类 take/put 舞蹈一并消失。**输出约束**：`session_api` 5 入口签名不变。

### 〔结构优化〕5.7 `should_checkpoint` 改用词汇 enum 而非裸字符串前缀

`snapshot.rs:199-204` 的 6 个 `starts_with`/`==` 判据改为对 `vocabulary` 的 enum variant 匹配。收益：词汇漂移在**编译期**暴露（现版是静默降低检查点密度）。同时把 `CheckpointManager` **移出 VM 包**（它依赖教学语义，不应由 VM 拥有）。

### 〔新设计〕5.8 统一模式与 JIT 的关系必须显式裁定

**实测事实**【亲读】：JIT fast path 只在 `VitroVM::run()`（`executor/mod.rs:12-48`，bulk 在 `:19-20`）；统一模式只调 `vm.step()`（`engine.rs:173,337`），而 `step()` **从不读 `jit_traces` 做批量**——它只在 `:311-347` 录制并 `compile_trace` + `jit_traces.insert`（`:337-338`）。`jit_enabled` 默认 `true`（`state.rs:189`），且 `native/src/` 内**无任何 `set_jit_enabled` 调用**【实测】。
→ **统一模式下 JIT 录制与编译是纯浪费**（产物永不执行）。影响小（每热循环一次编译）但语义上是"偶然未生效"而非"设计禁用"。
**建议**：MoonBit 版把"时间旅行路径 = 纯解释执行"写成**类型/构造上的显式选择**（`Execution` 构造时关掉 JIT），而不是靠"没人调 `run()`"。

### 〔沿革保留〕5.9 三出口薄包装纪律

`session_api` 是语义单源、出口只做参数编解码与所有权（`session_api.rs:1-12`）。单出口 wasm-gc 后仍应保留这个分层（serve 协议层 vs 引擎层），否则协议会与实现耦合。

---

## ⑥ 坑清单（本模块事故史）

| # | 现象 | 根因 | 修复 | MoonBit 下是否复发？ | 为什么 |
|---|---|---|---|---|---|
| 1 | **63.6GB 物理内存 + 33.9GB 页面文件，机器不可用**（第一次） | `push_or_replace_in_replay` 的 `step - start_step` 为负 → `as usize` 绕回天文数字 → 占位填充循环无限 push | 越窗重放前重置窗口到检查点步（`engine.rs:299-300`）+ `usize::try_from` 防御（`:436-439`） | **⚠️ 会复发** | 根因是"有符号差值当无符号下标"。MoonBit 若写 `Int`→`UInt` 裸转换，**同类绕回同样存在**（【待证】spike S3）。缓解：用 `Option`/显式钳位，**禁止**依赖裸数值转换 |
| 2 | **同路径第二次复发**：正向远距重放循环内零窗口截断 → 每步一个完整 payload（6+ 堆分配 Vec）常驻，上界 = `max_steps` × 帧尺寸 | `trim_frame_cache` 只在 `run_batch` 路径调用，`finish_replay_window` 在循环**之后**才裁剪 | **U2#1**（2026-09-14）：重放 push 后立即 trim（`engine.rs:341-346`）+ 两个 return 分支同步（`:351`、`:362`）；峰值 75MB → 23MB、57B/步 | **会复发（若照搬）／结构上难复发（若采纳 5.2）** | "循环内忘记维持有界不变量"与语言无关。若窗口丢弃做成 O(1) 且由**类型**保证（`FrameWindow` 只有 push 与丢弃两种操作），就不会出现"某个 return 分支忘了裁剪"的形态——这是真实的**结构消除缺陷类**论证 |
| 3 | **seek 静默错内存**：淘汰删 Full 未级联删其 Delta → 悬空 Delta 被叠到更早的 Full 上（无报错、结果错） | 链不变量只写在注释里；原实现仅在删 Delta 时从链头级联（`snapshot.rs:257-284`） | U2#5：删 Full 时级联删到下一个 Full（`:266-275`）+ 3 条红锚（`:388/420/456`，含 `assert_no_dangling` `:369-383`） | **会复发（若照搬"i32 base_step + 运行期查找"）／类型上不可能（若采纳 5.4）** | 不变量未被类型表达 = 靠纪律维持 |
| 4 | **seek 越过程序末尾 → panic（exit 101、会话死亡）** | `finish_replay_window` 的 `split_off(discard)` 理论 discard > 缓存长度 | `discard.min(len)` + 起点按实际推进量记账（`engine.rs:471-485`）；红锚 `serve_param_domain_regression.rs:41-60` | **会复发** | 同类"先算索引再切"的边界算术。MoonBit 切片越界是报错还是陷阱需 spike 确认（S5） |
| 5 | **`payload.get(end=-1)` → 切片 panic** | `(-1).min(cache_end) as usize` 绕回 `usize::MAX`（`min` 对负数不封底） | 先钳到 `[start_step, cache_end]` 再转（`engine.rs:490-497`）；红锚 `serve_param_domain_regression.rs:82-117` | **会复发** | 与 #1/#4 同族：**"钳位顺序"是隐性知识**。三次发生在同一模块三个不同函数 → 系统性缺陷，非点状疏漏 |
| 6 | **末帧无限重放**：binary 90 步程序 call#92+ 持续重发 s=89，污染 frame_cache / payload.get 窗口 | `StepResult::Finished` 每次调用都 `collect`+`push` 同一帧 | §6-9：`is_finished` 短路（`engine.rs:23`、`:121-132`）+ `seek_to` 复位（`:270`）；红锚 `serve_param_domain_regression.rs:126-168` | **会复发** | 终态是"粘性状态"，与语言无关。MoonBit 可用类型把"已终结"编码进状态（`Running \| Finished`）降低复发率 |
| 7 | **两处"发布缓冲"回归**：① 缓冲字段赋值误写在 `if let Some` 内 → 整段从未执行（静默失效）；② 首调把 curr 克隆发布后又入缓冲 → 同一真实步投递两次（实测序列 `0,0,1`，违反 spec 附录 A "`step_index` 严格递增"） | "滞后一帧"语义只存在于注释中；跨调用边界的状态机无形式化表达（`session_api.rs:248-291` 逐条留痕） | R2 修正为"首调只建立缓冲、返回空 payloads"；协议 §6.1（`STEP_PAYLOAD_SCHEMA_V0_1.md:325-337`）固化四条语义 | **会复发** | 状态机跨 API 调用边界 + 语义靠注释 = 高复发形态。建议把发布缓冲建成显式状态类型（`Empty \| Pending(payload)`），用穷尽 match 强制处理两分支 |
| 8 | **断点暂停粘性但只恢复一层**：断点命中同时置 VM `paused` 与 engine `is_paused`，清断点只恢复 VM 层 → 引擎循环入口直接 break，无法推进 | 两个"暂停位"分散在两个对象上（`session_api.rs:452-463` 注释） | `set_breakpoints(空)` 同时 `vm.resume()` + `engine.resume()`（`:458-463`） | **会复发** | 同构的"状态分散"。收进 5.6 的 `Execution` 后由单一 `resume()` 处理 |
| 9 | **seek 后 `accessed_vars` 变量名错误**（**未修**；`代码审阅与修复追踪20260906.md:230` P2 登记；本次静态核实） | `restore()`（`core/snapshot.rs:120-179`）恢复了 `call_stack`（`:126`）却**未** `rebuild_local_sym_map()`；该映射只用于命名 `accessed_vars`（`executor/stack.rs:39-43,61-65`），上次重建点是 Call/Ret（`executor/control.rs:120,218-245`） | **未修** | **会复发** | 与 `region_index`（已修，`core/snapshot.rs:167-169`）**完全同构**——"派生索引必须随快照恢复重建"这一义务已被违反两次。5.5 是根治 |
| 10 | **时间旅行回退后文件状态来自"未来"**（**未修**；`代码审阅与修复追踪20260906.md:187`） | `VMSnapshot`（`snapshot.rs:55-80`）无 vfs 字段；VFS 有 `snapshot_files/restore_files`（`vfs.rs:656-674`）但未接入 | **未修** | **不会自动消失；但这正是换语言的结构性收益点** | 评估报告 §4.1 论断：MoonBit"不可变值派生新状态"能把漏字段从**运行期静默错误**变成**编译期不通过**——前提是把可变状态整体做成一个不可变值（R3 / 5.5），而**不是**继续写"逐字段 clone" |
| 11 | **数组快照 5~20GB 隐患**：`int a[50000]` + 窗口 2000 帧，`Vec::with_capacity(array_size)` 按声明长度预分配且逐元素 String 化 | 登记原文 `统一整备路线图.md:143` | U2#10：payload 级 256 截断 + `truncated` 标记（`types.rs:80-82`） | **会复发** | 与语言无关的资源上界；已由协议字段锚定，照搬即可 |
| 12 | **输出无界**：失控 `putchar`（10M 步上限）累积数百 MB~GB | 登记 `统一整备路线图.md:5` 评估 R11 | U2#3：`OutputLog` 有界 + O(1) 长度（`output_log_budget_test.rs`） | **会复发** | 同族 |
| 13 | `trace` 无界累积（10M 步 → 数百 MB 只写不读） | `host_step` 每步写一条（`runtime_state.rs:66`） | U2#4：`TRACE_LIMIT = 4096` 环形。**但实测该路径是死代码**（`test_snapshot.rs:829-831`：codegen 已不发射 `__vitro_step`） | **不适用（死路径）** | 上限是"为未来重新插桩预留的防线"，不是活缺陷的修复 |
| 14 | **`CheckpointManager::default()` 的 `interval = 0` → `step % 0` panic**（潜在雷点，**无触发路径**） | `snapshot.rs:161` `#[derive(Default)]` + `:190` 取余 | 未修（无触发路径） | **【待证】** | 取决于 MoonBit 整数取余/除零语义（spike S5）。建议直接删 `Default` |
| 15 | **JIT 与统一模式交互错位（本次新发现，非事故）** | 统一模式只调 `vm.step()`，而 `step()` 会录制并编译 trace（`executor/mod.rs:311-347`）却**永不执行**（`jit_traces` 只在 `run()` 的 `:19` 被读） | 未修（无正确性影响，只有浪费） | **会复发** | "偶然未生效"不是设计。5.8 |
| 16 | **回放断言口径缺陷（防线侧）**：`v01_payload_fields.json` 只判"多余字段"，**不判缺失**；`replay_s1_s5.go:893-905` 遍历方向是 `for k := range p`（payload→白名单），A3 哨兵缺省放行（`:909-920`） | 唯一字段存在性断言在 `serve_smoke/main.go:319-329`（只查 5 个字段） | 未修 | **会复发** | "字段只增不改"契约里"既有字段不得消失"这一半**当前没有防线**。MoonBit 复刻时必须补"缺失即红" |
| 17 | **S5 A5 恒真 PASS** | `replay_s1_s5.go:941-943` 原文 `rep.check("S5","A5", true, "serve 出口无差分批量编码；由引擎 stream 单测覆盖（记录性 PASS）")` | 差分编码无出口生产者（③-2） | — | **会复发** | 一条恒真断言会让"61/61 PASS"看起来覆盖了它其实没有。复刻时要么给真实覆盖，要么显式标注为占位 |

---

## ⑦ MoonBit spike 清单（本模块依赖的语言特性）

> 每条给出**最小验证程序**与**判定标准**。涉及精确 API 的一律先以 `moon ide doc "<query>"` 现场确认，不凭记忆断言。

| # | 依赖的语言特性 / 风险 | 最小验证程序 | 判定标准 | 失败后果 |
|---|---|---|---|---|
| **S1** | **可变字节载体 + 1MB 快照拷贝的每步成本** | `.mbtx`：`Array[Byte]`(1MB) 填满 → 循环 100k 次拷贝（等价 `core/snapshot.rs:86-88`）→ 打印墙钟与进程 RSS | 与 Rust 版同口径比对：**慢 3× 以上即出局**（沿用评估报告门 1 阈值）；并记录 GC 暂停 | ⑤-5.1 的"照搬每步全量快照"直接不可用，必须走重放重建 |
| **S2** | **大数组状态的滚动窗口在 GC 语言下的行为**（**必查项**） | 三组对照：(a) `Array[Frame]` + `start` 偏移，每步 push、超 2000 时"复制到新数组丢最早 20%"（等价现版 `split_off`，`engine.rs:95`）；(b) 不可变持久序列（每帧 = 前帧 + delta，共享未变子结构）；(c) 两者各跑 100 万步 | **(a) 回收性**：旧窗口引用断开后必须无条件可回收（RSS 回落）；**(b)** 测"只保留窗口起点帧"时整条共享链是否仍可达（**持久结构的经典空间泄漏**）。**(b) 斜率**：RSS/步 ≤ 64B（J5 线，`核心资产重构裁定.md:228`）。**(c) CPU**：窗口丢弃摊还 O(1) | 若 (b) 出现共享链挂住不回收 → "不可变结构天然解决内存增长"必须**放弃**，回到 (a) 并用 5.2 的 O(1) 丢弃 |
| **S3** | **有符号→无符号转换的负值语义**（决定 ⑥-1/4/5 是否复发） | `.mbtx`：`let d = 5 - 10` → 各种目标类型转换及索引/切片表达式中的隐式转换 → 打印结果与是否 raise | 必须能**显式区分**"负值"与"巨正数"；**不允许**静默绕回。若语言强制显式转换且可 raise，则 ⑥-1 形态被消除 | 若存在静默绕回 → 写代码规范禁止 + 在 ⑧ 加"负参 seek/payload.get 逐条断言"（对齐 `serve_param_domain_regression.rs` 6 条） |
| **S4** | **检查点链不变量的类型表达**（5.4） | `.mbt` 小工程：`Checkpoint = Full(step, Snap) \| Delta(base : FullRef, pages)`；实现 `nearest(target)`；写"淘汰链头 Full"用例 | 编译期能否让"删掉 Full 后 Delta 悬空"**不可表达**；被引用 Full 的内存代价实测（对比现版 `Vec<(i32, VMSnapshot)>`） | 若内存代价不可接受（Full 被钉住无法淘汰）→ 保留运行期判据 + 移植 `assert_no_dangling`（`snapshot.rs:369-383`）作防线 |
| **S5** | **整数取余/除零与切片越界语义** | `.mbtx`：`1 % 0`、`1 / 0`、`arr[arr.length() + 5]`、`arr[-1]` | 必须**明确**是 raise / panic / 未定义；据此决定 `Default` 是否必须删，以及 ⑥-4/5 的防线形态 | 若为静默 UB → 取消余数用法，改显式 `if interval == 0` |
| **S6** | **对象回指持有者（Session ↔ Engine）**（决定 U6#4 工作量） | `.mbt`：`struct Session { exec : Option[Execution] }`、`struct Execution { vm, engine }`，`Execution::collect(self, session : Session)` 读写 session 上的状态（模拟 `collector.rs:83-86`） | 编译通过且行为正确；engine 方法内同时访问自身状态与 session 状态无限制 | 若受限（如可变字段别名规则）→ 4.1 的"低工作量"评估须上调 |
| **S7** | **`.mbtx` 是否适合承载重放对账自动化**（**必查项**） | `.mbtx` 起子进程（serve 等价 CLI），**写一行读一行** NDJSON，`moonbitlang/core/json` 解析，逐条判定并输出 `  [PASS] S1 A1` 与中文汇总块（含全角空格 `　`） | 四项全过：(a) 双向流式管道可用（不是只能 `each_line` 读输出）；(b) stdout 字节可控（中文/全角/退出码）；(c) `--wasm-policy` 能精确授权 `process.allow{program, args_prefix}`；(d) 输出格式与 Go 版**逐行 diff 一致** | 若 (a) 不成立 → 放弃 .mbtx 复刻，**保留 Go 驱动**（serve 协议语言中立、Go 零第三方依赖），`.mbtx` 只做片内工具。**API 需 `moon ide doc "@async/shell"` 确认** |
| **S8** | **J9 埋雷义务的语言无关落地** | `.mbtx`：实现 `--selftest`，对每个判定 helper 注入必然违反的输入（对齐 `replay_s1_s5.go:1027-1099` 现有 14 条） | 注入不红即 `exit 2` 并拒绝运行 | 无退路——**语言无关义务**（AGENTS.md 红→绿纪律第 3 条） |
| **S9** | **JSON 序列化的可比性**（决定 ⑧ 的 diff 形态） | 两边各序列化同一 `StepPayload`（含 `None`、空数组、中文、`u64` 大数） | 判定"**解析后结构等价**"而非字节等价：键集合（BTreeSet）+ 值语义；确认 `Option` 为 `None` 时输出 `null` 还是缺省字段 | 若字段缺省策略不同 → diff 工具必须先规范化，否则等价性验收假红 |
| **S10** | **快照往返完整性（评估报告门 3 的模块版）** | 建最小 VM 快照（1MB 字节 + 栈 + 调用帧 + `freed_logs` + `regions`/`free_list`/`quarantine` 三件套 + `stdin_eof` + `heatmap`）→ 快照 → 前进 → 恢复 → 逐字段断言；再 seek 回第 5 步验证 UAF 检测无假阴性 | 逐字段全等；隔离区三件套随往返（`snapshot.rs:109-113`）；**并且**恢复后派生索引（`local_sym_map` 等价物）自动正确 | 对应评估报告门 3"P4 降级结论被推翻 → 回退" |

---

## ⑧ 等价性验收锚点

### 8.1 主通道与 diff 对象

| 层级 | diff 什么 | 格式 | 工具 |
|---|---|---|---|
| **主通道** | serve 等价的 **NDJSON 帧流** | 每行一个 JSON 对象（`id` 关联、`ok`/`result`/`error` 同构） | **直接复用 Go 版驱动**：`go run ./scripts/replay/replay_s1_s5.go`、`go run ./scripts/serve_smoke`、`go run ./scripts/core_asset_verdict/interaction_probe` |
| **逐帧内容** | `StepPayload` 的**解析后 JSON 结构**（14 字段 + 5 个子结构键集合 + 4 个枚举字面量） | JSON（字段顺序无关，键集合 + 值比对） | `step_payload_schema_v0_1_test.rs` 判据搬到 serve 出口 + `v01_payload_fields.json` 白名单 |
| **窗口状态** | `cache_start_step` / `max_collected_step` / `payloads[].step_index` 序列 | JSON | `replay_s1_s5.go:794-813`（S3 A13b/A14/A15） |
| **seek 往返** | `seek(N)` 后 `local_vars`/`call_stack`/`array_snapshots`/`pointer_snapshots`/`heatmap_count` **全部等于第 N 步快照** | JSON | 协议 §4.3（`STEP_PAYLOAD_SCHEMA_V0_1.md:256-260`）+ `replay_s1_s5.go:690-715`（S3 A8/A8b/A9） |
| **追踪/调试帧** | `step.next` 一帧发布缓冲四语义（首调空 / 逐帧 / 结束冲刷 / 暂停也冲刷 + 恢复后重建） | JSON | `replay_s1_s5.go:392-417`（S1 A7b）+ `:587-594`（S3 A3） |
| **资源斜率** | 远距 seek 压力形状下的**宿主提交峰值** | 数值（MB） | `serve_smoke/main.go:820-921`。**⚠️ 必须重新标定**（口径见 8.3） |
| **版本锚定** | `engine_version` 含当前 git 短哈希 | 字符串 | `replay_s1_s5.go:996-1000` preflight（**exit 2**）+ S5 A4b（`:934-940`） |
| **协议自描述** | `capabilities` / `semantic_labels` / `contracts` 三方法 | JSON | `session_api.rs:170-180`、`:633-701`；`serve_smoke:422-434` |

### 8.2 现成可复用的锚点（**迁移期最大的杠杆**）

| # | 锚点 | 规模 | 来源 key / as_of | 复用方式 |
|---|---|---|---|---|
| E1 | **签字回放 S1–S5** | **61 条断言**（S1=11 / S2=26 / S3=18 / S5=6；**S4 未实现**，仅 `:923` 文案引用） | `reports/facts.json` → `replay_assertions` = **61**，`as_of 2026-09-14T01:56:25+08:00`，`status cached`，`source scripts/replay/replay_s1_s5.go` | **逐条保留断言编号与判定口径**（编号是 SharpTutor 签字材料的一部分）。S1–S3/S5 全走 serve → 只要 MoonBit 版能起 serve，**驱动原样可用** |
| E2 | **serve 冒烟** | **57 条断言**（主批 43 + 边界批 10 + 串帧批 3 + RSS 1） | `reports/facts.json` → `serve_smoke_assertions` = **57**，`as_of 2026-09-18T01:19:58+08:00`，`status cached` | 同上；RSS 那条需重标定 |
| E3 | **字段白名单** | 14 + 4 字段 | `scripts/replay/v01_payload_fields.json`（26 行） | **直接复用**，且保持设计原则：`:3` 原文"**语义快照随本文件 git 版本化**——schema 激活 v0.2 时在此登记新字段，两驱动自动跟随…；**不从引擎运行时拉取**"；`replay_s1_s5.go:852-855` 原文"加载失败/为空/schema 不符一律 fail loud：白名单是 S5 断言的判据，静默降级等于拔掉防线 5 的牙" |
| E4 | **交互切面探针** | 随机交互序列（固定种子加权 step/seek/payload.get/…）+ 畸形 fuzz | `scripts/core_asset_verdict/interaction_probe/main.go:8-14` | 复用（走 serve）；覆盖"≥1000 会话零 panic"（`核心资产重构裁定.md:191` 验收判据 (a)） |
| E5 | **seek 累积 / 斜率长跑** | 判定型探针 | `scripts/core_asset_verdict/{seek_accumulation,resource_longrun}/main.go` | 复用（走 serve）；J5 判据 = 峰值提交 / 重放步数 ≤ 64B/步 |
| E6 | **突变切面测量** | 6 切面，2 个对准 `collector.rs` | `scripts/core_asset_verdict/mutation_facet_test.py:45-52,83-88` | **方法论平移**（不是脚本平移）：MoonBit 版必须能回答"改坏一个判据，有几条独立防线红" |

### 8.3 **必须改造的锚点**（不能照抄，须明确工程项）

| # | 项 | 现状 | 改造 |
|---|---|---|---|
| M1 | `engine_version` 获取通道 | replay 直调 DLL 符号 `vitro_engine_version_into`（`replay_s1_s5.go:955-960`），**绕过**共享包 `scripts/internal/capi` 的符号集与新鲜度校验 | 单出口后无 DLL → 改经 serve `capabilities.engine_version`（`session_api.rs:641`；`serve_smoke` 已是这条路）。**断言本身（S5 A4b）不变，只改取值方式** |
| M2 | RSS 护栏口径 | psapi `PeakPagefileUsage`，预算 64MB，`VITRO_RSS_BUDGET_MB=5` 证红（`serve_smoke/main.go:825-830,915-917`；注释记录"U2#1 后实测 23MB"） | Node 宿主改用 `process.memoryUsage().rss`；**必须重新标定基线与预算**（23~24MB 的绝对值不可比）。**浏览器宿主下该护栏无法照搬** → 需另定（如引擎侧自报帧数/字节预算） |
| M3 | 字段白名单判定方向 | 只判"多余"，**不判缺失**（`replay_s1_s5.go:893-905`；A3 缺省放行 `:909-920`） | **补"缺失即红"**（对照 14 项全集双向判），否则"字段只增不改"只守了一半 |
| M4 | 差分编码覆盖 | S5 A5 恒真 PASS（`:941-943`） | 三选一（③-2）后给**真实覆盖**或显式占位标注 |
| M5 | `step_payload_schema_v0_1_test` 入口 | Rust 单测直调 capi（`step_payload_schema_v0_1_test.rs:46-58`） | 复刻为**经 serve 的黑盒测试**（判据不变） |
| M6 | replay 前置新鲜度门禁 | `capabilities.engine_version` 必须含 `git rev-parse --short HEAD`，否则 exit 2 | 保留（MoonBit 版仍需在版本串注入 git 哈希；**必做项**，否则回放会在验证陈旧产物时全绿——`STEP_PAYLOAD_SCHEMA_V0_1.md:483-488` 记录过这次踩坑） |

### 8.4 ⚠️ 运行时前置风险【待证】

子代理本次做了字节扫描（**未运行任何驱动**）并报告：`native/target/release/vitro_cli.exe` 与 `vitro_native.dll` **不含 HEAD `4b57191`**，而 debug 版含（release mtime `2026-09-15 00:05:31` / debug `2026-09-18 13:01:42`）。由于 replay 固定走 release（`replay_s1_s5.go:50`）且 preflight 要求版本串含 HEAD（`:996-1000`），**现在直接跑 replay 会 exit 2**，`facts --run` 会退化为 unavailable 而沿用 61 的缓存值。
**验证方法**：`cd native && cargo build --release` 后 `go run ./scripts/replay/replay_s1_s5.go`；或比对产物内嵌哈希与 `git rev-parse --short HEAD`。**本报告不对该数值作断言**——标注为待证。

---

## ⑨ mooncakes 包切分草案

> 出发点：Rust 侧已有 crate 化计划（[`工程债务维护方案.md:565`](../01-定位与路线/工程债务维护方案.md)：`vitro_unified` "部分下沉…剩余阻力是与 `Session` 的耦合"），但**未执行**。MoonBit 是从零建包，**没有理由继承这个阻力**。

### 9.1 包划分（5 包 + 1 facade）

```
vitro/step_payload          ← 协议包（可独立发布）
  ├─ payload.mbt            StepPayload 及 13 个子结构
  ├─ contracts.mbt          SCHEMA_VERSION / RESERVED_FIELDS_V0_2 / 激活清单 / 字段台账 / 行为契约
  ├─ vocabulary.mbt         SemanticLabelKind enum（id 从 &str 升格为 enum variant）+ classify
  └─ json.mbt               ToJson / FromJson（wire 形态单源）

vitro/step_stream           ← 依赖：step_payload
  └─ StreamBatch / PayloadDelta / encode / decode / diff

vitro/step_collect          ← 依赖：step_payload + 抽象观测面（不依赖 vm 包）
  ├─ collector.mbt          StepCollector（语义分类器唯一单源）
  ├─ root_cause.mbt         RootCauseHint
  └─ trace_analyzer/*       5 类分析器 + 中文 trap message 解析

vitro/time_travel           ← 依赖：step_payload + snapshot 抽象
  ├─ window.mbt             FrameWindow（封装 pub 字段，自带不变量）
  ├─ checkpoint.mbt         CheckpointManager（★ 从 vm 包迁出）
  ├─ engine.mbt             UnifiedEngine（run_batch / seek_to / trap 回退）
  └─ derive.mbt             派生状态统一重建 / 惰性视图

vitro/vm                    ← 依赖：step_payload（仅为观测面类型）+ 自身快照
  └─ snapshot.mbt           VMSnapshot / MemoryImage / snapshot_into / restore

vitro/session_api           ← facade（pub using），依赖以上全部
  └─ step_begin / step_next / payloads / seek / set_breakpoints / capabilities
```

### 9.2 依赖方向（**必须无环**）

```
step_payload  ←── step_stream
      ↑     ←── step_collect  ←── (observer 抽象接口)
      ↑     ←── time_travel   ←── (checkpoint 抽象接口)
      ↑     ←── vm
      └────── session_api (facade)
```

**关键修正（相对 Rust 现状）**：`CheckpointManager` 现居 `vitro_vm/src/snapshot.rs:162`，而其智能检查点判据依赖教学语义标签（`:199-204`）——即 **vm 包反向依赖应用层词汇**（①-1.4 结论 1）。MoonBit 版把 `CheckpointManager` 放进 `time_travel`，`vm` 只提供无策略的 `VMSnapshot` / `snapshot_into` / `restore`。这消除了 Rust 侧现存的分层破损。

### 9.3 依赖反转（**避免重演"与 Session 的耦合"**）

`step_collect` 与 `time_travel` **不得**依赖 `session_api`。它们需要的三样东西抽成接口（MoonBit 的 trait）：
- `VmObserver`：`get_current_line` / `get_call_stack` / `get_variable_snapshot` / `get_array_snapshots` / `take_vis_events` / `get_last_accessed_vars` / `find_variable_name_at_addr`（对应 `collector.rs:12-140` 的全部 VM 调用面）；
- `SourceProvider`：`source_line_at(global_line)`（对应 `session.rs:88-110`；**多文件全局行号口径不可丢**，`step_payload_vars_test.rs:355-380` 有专门红锚）；
- `AlgorithmContext`：Rust 侧已有该抽象（`vitro_algorithm_steps::AlgorithmContext`，实现于 `session.rs:412-431`），照搬。

### 9.4 对上发布形态

| 包 | 发布形态 | 理由 |
|---|---|---|
| `vitro/step_payload` | **公开、独立版本号** | 它是**协议**（任何消费 StepPayload 的前端/评测器/下游都需要），与引擎实现解耦。schema v0.1 的冻结纪律（"字段只增不改语义"）在包版本号上体现 |
| `vitro/step_stream` | 随 `step_payload` 同版本 | 协议的一部分（§5） |
| `vitro/time_travel` / `vitro/step_collect` | 可后置（先内部） | 除非社区要独立消费"时间旅行引擎" |
| `vitro/session_api` | 主发布入口（facade） | `pub using` 重导出，用户只需一个 import |

### 9.5 与 `.mbti` 的关系

每包公共接口（`pkg.generated.mbti`）应入版本控制并**作为协议变更信号人工审阅**。`step_payload` 的 `.mbti` diff = 协议变更——这与 Rust 侧 `test_step_payload_top_level_fields_frozen` 的职责重合，但**从运行期前移到接口层**。

---

## 附录 A · 待证清单（不确定项 + 验证方法）

| # | 待证结论 | 验证方法 |
|---|---|---|
| 1 | release 产物不含 HEAD → replay 现会 exit 2 | `cd native && cargo build --release` 后 `go run ./scripts/replay/replay_s1_s5.go`；或比对产物内嵌哈希与 `git rev-parse --short HEAD` |
| 2 | `restore()` 后 `local_sym_map` 陈旧导致 `accessed_vars` 名字错（本次**静态亲读**得出） | 动态验证：跨函数 seek（seek 回 `main` 帧而 `local_sym_map` 属 `helper`），断言 `payload.accessed_vars[].name` 与实际变量名一致 |
| 3 | 每步 1MB memcpy 的实际 CPU 占比（未实测） | 对照"有 / 无 `pre_step_snap`"两版墙钟（`unified_perf_baseline.py` 口径） |
| 4 | `frame_cache.split_off` 在重放循环内的 O(len)/步 CPU 成本（未实测） | 测 seek 到 10 万的墙钟随 `frame_cache_window_size` 的敏感度 |
| 5 | 统一模式下 JIT 录制/编译的浪费量级（未实测） | 统计 unified 路径 `jit_stats.traces_compiled` + 对照 `set_jit_enabled(false)` 的墙钟 |
| 6 | MoonBit `Int`→`UInt` 负值转换、取余除零、切片越界语义 | spike S3 / S5 |
| 7 | `.mbtx` 双向流式子进程管道能力 | spike S7 + `moon ide doc "@async/shell"` |
| 8 | Node/wasm 宿主下 RSS 护栏的可测性与新基线 | spike（Node 宿主跑长程序 + 采样 `process.memoryUsage().rss`）；与 Rust 版 23~24MB **不可比**，需独立标定 |
| 9 | mooncakes 上是否有可复用的 JSON / 序列化包（**不臆测包名**） | 项目内 `moon ide doc ''` 列可用包；`moon add <候选>` 在一次性工程验证 |

## 附录 B · 本次引用的证据文件

**代码（逐行读过）**：`native/src/unified/{mod,engine,types,collector,stream,contracts,vocabulary,root_cause}.rs`、`native/src/unified/stream/{encode,decode,diff}.rs`、`native/src/unified/trace_analyzer/{mod,utils,bounds,div_zero,double_free,null_deref,use_after_free,tests}.rs`、`native/src/{lib,session,session_api}.rs`、`native/src/capi/first_batch.rs`、`native/src/bin/vitro_cli.rs`、`native/src/compiler/{mod,ast}.rs`、`native/crates/vitro_vm/src/snapshot.rs`、`native/crates/vitro_vm/src/core/{snapshot,state,memory,mod}.rs`、`native/crates/vitro_vm/src/core/executor/{mod,stack}.rs`、`native/crates/vitro_runtime/src/runtime_state.rs`、`native/tests/{unified_engine_window_test,step_payload_schema_v0_1_test,step_payload_vars_test,serve_param_domain_regression,output_log_budget_test}.rs`、`scripts/replay/{replay_s1_s5.go,v01_payload_fields.json}`、`scripts/serve_smoke/main.go`、`scripts/core_asset_verdict/mutation_facet_test.py`

**文档**：[`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md)、[`05-教学体验/统一模式设计.md`](../05-教学体验/统一模式设计.md)、[`统一整备路线图.md`](统一整备路线图.md)、[`核心资产重构裁定.md`](核心资产重构裁定.md)、[`代码审阅与修复追踪20260906.md`](代码审阅与修复追踪20260906.md)、[`INCIDENTS/事故202609_Seek重放泄漏.md`](INCIDENTS/事故202609_Seek重放泄漏.md)、[`01-定位与路线/项目路线图.md`](../01-定位与路线/项目路线图.md)、[`01-定位与路线/工程债务维护方案.md`](../01-定位与路线/工程债务维护方案.md)、[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)、`CHANGELOG.md`、`AGENTS.md`

**真值**：`reports/facts.json`（`generated_at 2026-09-18T13:01:41+08:00`，`git.rev 4b57191`；`replay_assertions = 61` as_of 2026-09-14、`serve_smoke_assertions = 57` as_of 2026-09-18、`abi_version = 2.1.0` as_of 2026-09-14）

---

## 附录 C · 一句话总结

本模块是**时序编排 + 协议契约**的混合体——协议层（`types`/`contracts`/`vocabulary`/`stream`）几乎可无损平移且 MoonBit 的 enum 会让它**更安全**；编排层（`engine`/`collector`）的真正病根不是 Rust 特有机制，而是**「每步一次 1MB 全量快照」与「重放循环内维持有界不变量」这两个结构选择**——前者换语言一分不省（头号风险），后者换语言才有机会被类型消除（头号机会）。勘察另发现 **4 项既有登记之外的缺陷/死代码**与 **3 项防线缺口**，均按防线哲学第 0 条如实记录。
