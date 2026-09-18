# MoonBit 迁移 · `diagnostics` 与 `algorithm_steps` 模块勘察报告

> 勘察时间：2026-09-18
> 对象：`D:\code\Vitro` @ `4b57191`（与 `MoonBit迁移方案评估报告20260918.md` 同一基线）
> 性质：**只读勘察**。未执行任何代码改动、未切换工具链、未 git 提交。
> 数字口径：本文件为 **AS-OF 冻结**（文件名含日期）——本文裸数字不参与 `facts` 回填；
> 全局数字一律标 `reports/facts.json` 的 key 与 `as_of`；标注"本次实测"的为 2026-09-18 现场实测。
> 证据纪律：每条结论附 `file:line`，读代码下结论，不从文档倒推代码；不确定项标"待证"并给验证方法。
> `docs/archive/` 下归档文档一律未引用。

## 0. 勘察范围与边界

| 目标 | 路径 | 文件数 | 行数（本次实测） |
|---|---|---|---|
| 结构化诊断 / 知识图谱 / 教学推理 | `native/src/diagnostics/` | 11 | 2,318 |
| CFG / 数据流 / 意图推断 / 算法检测 | `native/src/compiler/` | 14 | 2,900 |
| 语义补全（CompletionEngine 五上下文） | `native/src/engine/completion/` | 3 | 1,257 |
| 27→**实测 43** 种算法步骤模板 | `native/crates/vitro_algorithm_steps/` | 8 | 2,273 |
| **四目标合计** | | **36** | **8,748** |
| 〔越界纳入〕认知推理链第一层 `TraceAnalyzer` + 语义标签单源 | `native/src/unified/{trace_analyzer/, root_cause.rs, collector.rs, vocabulary.rs}` | 11 | 1,643 |
| 〔越界参照〕`native/src/unified/` 全目录 | | 19 | 3,690 |

**为什么越界纳入**：任务书把"认知推理链四层"列为勘察对象，但第一层 `TraceAnalyzer` 实际落在 `native/src/unified/trace_analyzer/`（`mod.rs:26`），`RootCauseHint` 落在 `native/src/unified/root_cause.rs:7`，`semantic_label` 单源分类器落在 `native/src/unified/collector.rs:324`。本报告将其纳入并显式标注路径。

**行数口径**：`(Get-Content $f).Count`（含空行）。注意 `Measure-Object -Line` **会漏算空行**（`cfg.rs` 少算 43 行、`total` 少算约 4%），本次已统一改口径重测。

---

## ① 模块概览与规模

### 1.1 单文件行数（>200 行者）

| 文件 | 行数 | 文件 | 行数 |
|---|---|---|---|
| `compiler/algorithm_detector/features.rs` | 797 | `unified/collector.rs`（越界） | 478 |
| `diagnostics/knowledge_graph.rs` | 621 | `diagnostics/error_catalog/semantic.rs` | 419 |
| `diagnostics/error_catalog.rs` | 556 | `compiler/intent.rs` | 403 |
| `compiler/cfg.rs` | 527 | `crates/vitro_algorithm_steps/src/tree.rs` | 375 |
| `engine/completion/mod.rs` | 512 | `compiler/data_flow.rs` | 323 |
| `engine/completion/candidates.rs` | 512 | `crates/…/structures.rs` | 300 |
| `crates/vitro_algorithm_steps/src/sorting.rs` | 507 | `crates/…/search.rs` | 292 |

### 1.2 关键类型（每类给定义点）

| 类型 | 定义 | 角色 |
|---|---|---|
| `ErrorInfo` | `native/src/diagnostics/error_catalog.rs:14` | 错误码教学卡片（emoji / title / explanation / common_causes），全 `&'static str` |
| `ERROR_INFO_MAP: LazyLock<HashMap<i32, ErrorInfo>>` | `error_catalog.rs:27` | **实测 77 条**：lexer 7（`error_catalog/lexer.rs:3`）+ parser 7（`parser.rs:3`）+ semantic 58（`semantic.rs:3`）+ cpp 5（`cpp.rs:3`）。码值已逐条核对，**无重复键** |
| `generate_fix(...) -> (String,i32,i32,i32,i32,i32,String)` | `error_catalog.rs:49` | 七元组（建议文案 + `fix_kind` + 4 坐标 + 替换文本）；`fix_kind` 语义 0/1/2/3/4 见 `auto_fix.rs:7` |
| `Diagnostic` | `native/src/session.rs:14`（`fix_kind` 在 `:21`） | 出口 DTO |
| `ConceptNode` / `ConceptEdge` | `knowledge_graph.rs:12` / `:23` | 知识图谱节点 / 有向边 |
| `NODES` / `EDGES` | `knowledge_graph.rs:51` / `:259` | **实测 25 节点 / 25 边**（非文档所称 24 / 30+，见 ⑥-19） |
| `ERROR_CONCEPT_MAP` | `knowledge_graph.rs:422` | **实测 28 条** `i32 → Vec<String>` |
| `CompileRecord` / `MisconceptionPattern` / `DetectedMisconception` | `misconception_patterns.rs:10` / `:23` / `:37` | 6 模式（`default_patterns()` `:48`） |
| `PathStep` / `LearningPath` | `learning_path.rs:11` / `:26` | 6 条路径（`build_path` `:39`，按 `M01..M06` 硬分支） |
| `RootCauseHint` | `unified/root_cause.rs:7` | 8 个 `category` 字符串域（`:9-11`） |
| `TraceAnalyzer` | `unified/trace_analyzer/mod.rs:26`，入口 `analyze_trap` `:35` | 5 类 trap 文本分派 `:41-52` |
| `StepCollector` | `unified/collector.rs:9`，`collect` `:12` | 每步 payload 采集；`infer_semantic_label` `:324`；`collect_pointer_snapshots` `:167` |
| `SEMANTIC_LABEL_VOCABULARY` | `unified/vocabulary.rs:41` | 10 active（C 域）+ 4 reserved（C# 域）；`classify` `:163` |
| `FuncFeatures` | `compiler/algorithm_detector/features.rs:9` | 27 个布尔 / 计数特征 |
| `AlgorithmMatch` | `session.rs:31`（出口版，带 `vis_events`） | 与 `vitro_algorithm_steps::AlgorithmMatch`（`steps/lib.rs:16`，无 `vis_events`）**双类型**，手工字段搬运见 `session.rs:423-430` |
| `ControlFlowGraph` / `BasicBlock` / `Terminator` | `compiler/cfg.rs:39` / `:15` / `:23` | `from_func` `:46`、`find_loops` `:73`、`compute_dominators` `:130` |
| `LiveVarResult` / `analyze_live_variables` | `data_flow.rs:21` / `:27` | 反向迭代定点活跃变量分析 |
| `CodeIntent` / `IntentScore` | `intent.rs:14` / `:49`，`infer_intent` `:57` | 加权打分（命名 +50/+40、CFG +20/+15/+10、变量名 +15/+10） |
| `CompletionSnapshot` / `CompletionContext` / `CompletionCandidate` | `completion/mod.rs:63` / `:128` / `:17` | 五上下文枚举 `:130-138`；`CompletionKind` 12 类 `:27` |
| `AlgorithmStepSnapshot` / `AlgorithmContext` / `InferEnv` | `steps/lib.rs:34` / `:42` / `:60` | 43 个 infer 分派臂 `lib.rs:90-135` |

### 1.3 规模关键实测（供迁移排期）

- **算法模板 43 个**：detector `build_match` 文案表 43 条（`features.rs:654-720`）；steps 分派 43 臂（`lib.rs:90-135`）；两侧名字集合**逐名 diff 完全一致**（本次实测）。文档口径 41（`统一整备路线图.md:111`、`CSharp前端引入计划.md:276`），`AGENTS.md` Phase 16 口径 27——**三处口径不一，以实测 43 为准**。
- `templates/`：**88 个目录，82 个含 `source.c`，88 个含 `meta.yaml`**（本次实测）。`meta.yaml` 字段：`key/name/category/params/tutorial/knowledge_nodes`（样例 `templates/bubble/meta.yaml`）。
- 单元测试（`#[test]` 计数，本次实测）：`diagnostics` 10、`compiler` 33、`vitro_algorithm_steps` 11、`trace_analyzer` 12、**`engine/completion` 0**（14 条断言全在集成测试 `native/tests/completion_unit_test.rs`）。`error_catalog.rs` 自身 **0 直接测试**。
- 防线数字（引 `reports/facts.json` key + as_of）：`shadow_c_cases` 675 / `shadow_c_match` 668（as_of 2026-09-15）、`shadow_cpp_cases` 99 / `shadow_cpp_match` 95（as_of 2026-09-15）、`c_e2e_baseline_cases` 359 / `c_e2e_gap_cases` 15 / `c_e2e_knr_cases` 81 / `c_e2e_leetcode_cases` 138 / `cpp_e2e_cases` 83（as_of 2026-09-18）、`abi_version` 2.1.0（as_of 2026-09-14）、`cpp_failures_active` 1（as_of 2026-09-14）。
- `cargo_test_passed` 在真值台账中为 **`status: unavailable`**（`how_to_get: --run-slow 或 --cargo-log`）——**本模块的测试总数无法用真值台账佐证**，只能给源码实测计数。真值基线：`reports/facts.json` → `generated_at` = 2026-09-18T13:01:41+08:00，`git.rev` = `4b57191`。

### 1.4 最关键的接线事实（决定"资产 vs 死码"）

| 能力 | 生产调用点 | 结论 |
|---|---|---|
| `error_catalog::lookup_error_info` + `generate_fix` | `engine/compile_pipeline.rs:112-119` | ✅ 已接线（编译期诊断） |
| `error_catalog::export_json` | `session_api.rs:162` → capi `capi/first_batch.rs:169` + serve `bin/vitro_cli.rs:593` | ✅ 已接线（机器出口） |
| `auto_fix::apply_fix` | **仅** `native/tests/crash_regression_tests.rs:487,491,495` | ⚠️ **生产零调用**（三出口全无） |
| `knowledge_graph::*` | 仅自身单测（`activate_from_ast` 仅 `:601`、`find_prerequisite_path` 仅 `:609`） | ❌ **零出口** |
| `misconception_patterns::detect_misconceptions` | 仅自身单测（`:187,200,211,219`） | ❌ **零出口** |
| `learning_path::recommend_learning_paths` | 仅自身单测（`:203,213`） | ❌ **零出口** |
| `completion::get_completion_candidates` | 仅 `completion_unit_test.rs`（14 处） | ❌ **零出口**（快照写入路径是活的：`compile_pipeline.rs:547,786`） |
| `TraceAnalyzer::analyze_trap` | `unified/engine.rs:208` | ✅ 已接线（统一模式） |
| `infer_algorithm_step` | `unified/collector.rs:116` | ✅ 已接线 |
| `detect_algorithms` | `compile_pipeline.rs:544,783` | ✅ 已接线 |
| `infer_intent` / `analyze_live_variables` / `evaluate_constant_condition` | 各自文件内单测，**无任何外部引用**（本次实测：`infer_intent` 5 处命中全在 `intent.rs`；`analyze_live_variables` 3 处全在 `data_flow.rs`） | ❌ **孤儿代码** |

> **诚实结论（防线哲学第 0 条）**：任务书把"认知推理链四层"当作在役系统描述，但**第二层（误区模式）、第三层（学习路径）、第四层（知识图谱）在引擎里没有任何出口，`completion` 同样只有测试消费者**。Phase 21/22/23/24 交付的是**库代码 + 单测**，不是可用能力。这决定了它们在迁移方案里的定位应是"**决定是否重建**"，而不是"逐行搬"。同理，`cfg`/`data_flow`/`intent` 三件中只有 `cfg` 有生产消费者（`features.rs:119`、`intent.rs:136`）。

---

## ② 可复用资产清单

### 2.0 机制层 / 数据层严格分列（本次勘察核心交付）

**判据**：数据层 = 不含控制流的常量表 / 文案 / 坐标映射，删掉代码后仍可作为 JSON 独立存在；机制层 = 有算法（迭代、搜索、图激活、滑窗、分派）的部分。

| # | 资产 | 层 | 当前形态 | 证据 | 移植成本 | 理由 |
|---|---|---|---|---|---|---|
| A1 | 错误卡片文案（77 条：emoji/title/explanation/common_causes[]） | **数据** | Rust 硬编码数组（`&'static`）**但已有 JSON 出口** | `error_catalog.rs:14,27`；导出 `:524-550`；码分段 `lang_of_code:476` / `category_of_code:488` | **低** | 纯数据；`export_json()` 产物（码升序 `:525-526`、只增不改承诺 `:519`）可直接当迁移输入，语言无关 |
| A2 | 错误码枚举（**137 变体**） | **数据** | Rust `#[repr(i32)] enum` | `vitro_shared/src/error_codes.rs:8-157` | **低** | 值语义映射；MoonBit `enum` 直接对应，且白拿穷尽检查 |
| A3 | `generate_fix` 的**替换载荷表**（~25 个码 → 建议文案 + 替换文本） | **数据** | 混在 match 里 | `error_catalog.rs:65-436` | **中** | 载荷是数据，但坐标计算（`col0` / `trimmed_len` / `find_single_equals_in_condition`）是机制，必须拆分 |
| A4 | `ERROR_CONCEPT_MAP`（28 条 错误码→概念） | **数据** | Rust 硬编码 `HashMap::insert` | `knowledge_graph.rs:422-453` | **低** | 纯映射；**含 1 处语义错**（3020→Recursion，见 ⑥-5），迁移前须修 |
| A5 | 概念图节点 25 / 边 25 | **数据** | Rust 硬编码 `vec![...]` 字面量 | `knowledge_graph.rs:51-257` / `:259-416` | **低** | 纯数据；`related_card_ids` 存在**悬空引用**（见 ③-D5） |
| A6 | 6 条误区模式（id/name/desc/error_codes/min_occurrences/time_window） | **数据** | Rust 硬编码 | `misconception_patterns.rs:48-99` | **低** | 5 个模式纯属"码集合 + 阈值"数据；M05 例外（靠 trap 文本关键词 `:150-157`，属机制） |
| A7 | 6 条学习路径 + `PathStep` 文案 | **数据** | Rust 硬编码 | `learning_path.rs:39-183` | **低** | 纯文案 + `target_id`；**`target_id` 一半悬空**（见 ③-D5） |
| A8 | `semantic_label` 受控词汇表（10 active + 4 reserved） | **数据** | Rust 常量 + 前缀匹配分类器 | `vocabulary.rs:41-155`，`classify:163` | **低** | **已是"资产外置"形态**（常量表 + `classify` 只做映射），且已上 serve 出口（`session_api.rs:170`）+ 词汇闭包测试（`step_payload_schema_v0_1_test.rs:353`）；`domain` 字段（`:27`）已内置分语言维度 |
| A9 | 43 条算法教学 `suggestion` 文案 | **数据** | Rust 硬编码 match | `features.rs:654-720` | **低** | 纯文案，键 = 算法名 |
| A10 | 43 个 infer 函数的 phase + description 模板 | **数据 + 机制混合** | 文案嵌在行文本判据里 | `steps/{sorting,tree,graph,search,structures,math,dp}.rs` | **中** | 文案可提取，但取值槽（`arr[j]`、`i`、`n`）依赖判据上下文，不能纯外置 |
| A11 | 算法标注 golden（**37 模板 / 311 条**首现，67,020 字节） | **数据** | JSON（已外置）✅ | `native/tests/golden/algorithm_annotations_v3.json`；消费 `algorithm_annotation_golden_test.rs:112-116` | **低** | 与实现语言无关的行为基线；**双向断言**（有标注集 == golden 键集 `:166-170`） |
| A12 | 88 个 `meta.yaml` + 82 个 `source.c` | **数据** | YAML / C | `templates/*/` | **低** | `tutorial.steps` + `knowledge_nodes` 是教学资产；C 源码是 golden 原料 |
| A13 | `has_word` 词边界 / 驼峰切分等价表（21 条用例） | **数据（测试资产）** | Rust `#[test]` 表驱动 | `features.rs:761-788` | **低** | 直接可搬的判定表，是"分词语义不漂移"的锚 |
| A14 | 编译 / 运行期教学 trap 文案（UAF、Double-Free、Overflow、invalid free） | **数据** | Rust 硬编码在 VM 内 | `vitro_vm/src/core/memory.rs:197`（E3060）、`vitro_vm/src/host/memory.rs:67,123`（E3061）、`:115,131`（E3027）、`vitro_vm/src/host/string.rs:92,194`、`vitro_vm/src/host/utils.rs:511`（E3070） | **中** | 文案是资产，但**与 error_catalog 双轨重复**（见 ③-D6） |
| M1 | 错误码查找 / 修复生成分派 | 机制 | Rust match | `error_catalog.rs:41,49` | 低 | 查表 + 分支 |
| M2 | 自动修复**坐标安全化**（`safe_byte_col`） | 机制 | Rust 三级退化 | `auto_fix.rs:26-34` | 中 | 见 ③-S1：症状治疗，不是资产 |
| M3 | 误区检测滑窗计数 + 置信度 | 机制 | Rust | `misconception_patterns.rs:102-137` | **低** | **实测语言无关**：输入仅 `Vec<i32>` + `Option<String>`，无 C 类型（`:10-19`） |
| M4 | 学习路径组装 | 机制 | Rust match on `pattern_id` | `learning_path.rs:35-37,39` | **低** | 输入仅 `Vec<DetectedMisconception>`，零 C 依赖 |
| M5 | 概念图激活 / 1-hop 邻居 / 前置路径 DFS | 机制 | Rust HashMap + DFS | `knowledge_graph.rs:460,480,517,561` | **低** | 输入 `i32` 与 `String`，输出 DTO；**唯一语言耦合点是 `activate_from_ast(Vec<String>)` 的"特征关键词"约定**（`:486-499` 硬编码 `"malloc"`/`"arr["`/`"*p"` 等 C 词法），且该入口零调用者 |
| M6 | CFG 构造 + 自然循环 + 支配树 | 机制 | Rust | `cfg.rs:46,73,130` | **中** | 图算法语言无关，但 `BasicBlock.stmts: Vec<Stmt>` 直接持有 AST（`cfg.rs:17`），依赖 AST 形状 |
| M7 | 活跃变量分析 / 常量条件求值 | 机制 | Rust | `data_flow.rs:27,219` | **中** | 同上依赖 AST；且**当前是孤儿** |
| M8 | 代码意图打分 | 机制 | Rust | `intent.rs:57-263` | **低** | 加权规则清晰（分数 + 理由串），语言无关；**当前是孤儿** |
| M9 | 特征提取（AST walk + 循环深度 + 交换 / 移位 / 中点等模式识别） | 机制 | Rust | `features.rs:103-533` | **高** | 与 AST 形状强耦合（`Stmt`/`Expr` 全枚举 walk），且含 6 次历史误判修复的补丁层（`:70-101, 246-267`） |
| M10 | 43 路算法判据 | 机制 | Rust 分散 7 文件 | `sorting.rs:6`、`tree.rs:6`、`graph.rs:6`、`search.rs:6`、`structures.rs:6`、`string.rs:6`、`math.rs:6` | **中高** | 判据是"命名 + 结构特征"启发式，与语言无关（纯语义），但判据质量是本模块最大坑源 |
| M11 | 43 个 infer 步骤推断 | 机制 | Rust 行文本 `contains` 启发 | `steps/lib.rs:67-136` | **高** | 核心是**用源码文本反推执行语义**；D6 已标记需按 `SourceLang` 参数化（`CSharp前端引入计划.md:52`） |
| M12 | `infer_semantic_label` 全库唯一分类器 | 机制 | Rust 单函数 136 行 | `collector.rs:324-459` | **高** | 判据顺序敏感（`:405-412` 记录了首日抓到的缺陷）；含 C 库函数硬编码候选（`:425,434,436,438,440`） |
| M13 | `collect_pointer_snapshots` 指针四态 | 机制 | Rust | `collector.rs:167-211` | **中** | 线性内存假设硬编码：`:184` `(NULL_TRAP_SIZE..MEM_SIZE).contains(&target_addr)`、`:186` `is_freed_heap(session.memory.regions, …)`——D6 明确列为需参数化项（`CSharp前端引入计划.md:52`） |
| M14 | `TraceAnalyzer` 5 类根因推断 | 机制 | Rust，按 trap 文本分派 | `trace_analyzer/mod.rs:41-52` | **中** | 机制（回看轨迹切片、找分配 / 释放行）语言无关；**耦合点是"靠中文 / 英文 trap 文本子串判断错误类型"**（`:41-50`）——必须消灭 |
| M15 | 补全五上下文检测 + 候选生成 | 机制 | Rust 文本启发 | `completion/context.rs:3`（`text_before_cursor` 200 字符窗口 `:81-100`）、`candidates.rs:3,102,188,370,413` | **中高** | 判据全是"光标前文本 `rfind`/`ends_with`"（`:59-76,220-232`），无 AST 参与；含 UTF-8 切片定时炸弹（`context.rs:88-89,95-96`） |

**分列结论**：

- **数据层（A1~A14）** 可直接复用为 MoonBit 包内的 JSON / 常量表，剥离成本低。**唯一硬前置**是把当前还在 Rust 里的 5 张表（A1 / A3载荷 / A4 / A5 / A6+A7）用现成的 `export_json` 模式固化到 JSON——这正是仓库既有的"资产外置 JSON、代码只做解释器"纪律（`AGENTS.md` 脚本章节）。**诊断模块目前只做到了 1/5**：只有 `error_catalog` 有出口，概念图 / 模式表 / 路径表 / 修复载荷表都没有。
- **机制层（M1~M15）** 全部是 MoonBit 重写对象。其中 **M9 / M10 / M11 / M12 是本模块 90% 的风险与工作量**；M3 / M4 / M5 / M8 机制简单且已实测语言无关，是"低风险平移"区。

### 2.1 语义设计资产（不随语言走的"想清楚了"的部分）

| 设计 | 证据 | 移植成本 | 理由 |
|---|---|---|---|
| 三层教学诊断分层（L1 感知 emoji / L2 解释 / L3 修复） | `error_catalog.rs:1-7` 文档 + `ErrorInfo` 字段设计 | 低 | 字段即契约 |
| `fix_kind` 五态协议（0 None / 1 Replace / 2 Insert / 3 Delete / 4 ManualHint） | `auto_fix.rs:7` + `error_catalog.rs:48` | 低 | 已是可枚举协议 |
| 认知链四层数据流（trap → `RootCauseHint` → Misconception → LearningPath，图谱横向支撑） | `root_cause.rs:7`、`misconception_patterns.rs:37`、`learning_path.rs:35`、`knowledge_graph.rs:32` | 低 | 类型边界清晰，几乎无反向依赖 |
| 一帧发布缓冲（流式协议下行末判定） | `session_api.rs:248-291` 长注释 | 中 | 语义必须保留（`step_index` 严格递增是冻结不变量），实现可换 |
| 词汇即契约（label 只增不改 + UI 直读不推断） | `vocabulary.rs:1-19,163` | 低 | MoonBit 侧用 enum + match 比 Rust 常量表更硬 |

### 2.2 测试 / 防线资产

| 资产 | 规模 | 证据 | 移植成本 |
|---|---|---|---|
| 算法标注 golden 回归（防线 6①） | 1 测试、37 模板、311 条首现、双向断言 | `native/tests/algorithm_annotation_golden_test.rs:110,166-170` | **低**（换消费端即可） |
| 检测器管线级锚 | 2 测试（真实 `Lexer→Parser→detect_algorithms`） | `algorithm_detector_pipeline_test.rs:78,94` | 低 |
| 检测器 / 认知层单测 | 33 + 10 + 11 + 12 | 见 1.3 实测 | 低（**注意**：`tree.rs:84`、`graph.rs:86` 类测试手工构造 `FuncFeatures`，是假绿来源，见 ⑥-13） |
| 补全集成测试 | 14 断言 | `native/tests/completion_unit_test.rs` | 低 |
| 已知防线缺口 | 诊断切面 38/810 测试；error_catalog 文案无 golden | `核心资产重构裁定.md:194-195` | —（迁移期须补，见 ④ W3） |

---

## ③ 抛弃清单

> 每条：抛弃理由 + 风险。**缺陷就是缺陷，不粉饰**。

### 3.1 Rust 特有机制（照搬即负债）

| # | 对象 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| R1 | `LazyLock<HashMap<..>>` 静态表（5 处） | `error_catalog.rs:27`、`knowledge_graph.rs:51,259,422` | MoonBit 无 `LazyLock`；顶层不可变值即可承载 | 惰性初始化时机**待证**（见 ⑦-spike 3）；若顶层值非惰性，`NODES`（25 个含 `String` 的节点）会在加载期构造，需测启动开销 |
| R2 | `Arc` / `Clone` 深拷贝（`ActivatedConcept{node, neighbors}` 全量克隆） | `knowledge_graph.rs:468-472,504-508` | MoonBit 无 `Arc`；不可变结构天然共享 | 反向收益：MoonBit 下"图激活即克隆子图"的写法**性能反而更好**，不必手写 `Arc` |
| R3 | 手写 JSON 转义 + 手工拼 JSON 字符串 | `error_catalog.rs:500-514`（`json_escape`）、`:524-550`（逐字符 `push_str`） | 造轮子；仓库纪律要求资产外置 + 解释器 | 迁移须用 `@json` 并**保持字段序与码升序**，否则 `capi_string_ownership_contract_test.rs:47` 类断言会红 |
| R4 | `Vec<(String,String)>` / `HashMap<String,..>` 当 DTO | `completion/mod.rs:91`、`knowledge_graph.rs:462` | 类型不表达语义；MoonBit 用 record / enum 表达 | 低；但**会改变 JSON 字段名**，须与 `docs/spec/` 的 wire format 对齐 |
| R5 | `fn(...) -> (String,i32,i32,i32,i32,i32,String)` 七元组 | `error_catalog.rs:49-55` | 位置参数不可读、易错 | 低；改 record 时须同步 `compile_pipeline.rs:113` 解构 |
| R6 | `&'static str` / `&'static [&'static str]` 静态借用 | `error_catalog.rs:16-19` | MoonBit 无借用；`String` 不可变可直接静态持有 | 低 |
| R7 | `#![forbid(unsafe_code)]` + `match { _ => }` 兜底 | `diagnostics/mod.rs:1`、`error_catalog.rs:435` | 属声明式纪律而非机制；MoonBit `enum + match` 编译期穷尽更强 | 正向收益 |

### 3.2 症状治疗代码（根因未除，不要搬）

| # | 对象 | 位置 | 根因 | 风险 |
|---|---|---|---|---|
| S1 | `safe_byte_col` 三级坐标退化 | `auto_fix.rs:26-34`（注释自承"坐标来源混杂：lexer/错误目录按字节计列，部分诊断与前端按字符计列"） | **诊断坐标口径不单源**——同一 `Diagnostic` 里 `line/column` 与 `replace_*_column` 语义不同 | 不搬；新设计须把坐标定义成**单一单位**（建议字节偏移 + 显式声明单位），否则 MoonBit 的 UTF-16 String 会让同类缺陷从 panic 变**静默错位**（见 ⑥-1） |
| S2 | `completion/context.rs` 的 UTF-8 切片（`prev[start..]`、`current[..col]`） | `context.rs:88-89,95-96` | 同 S1 | **当前仍是活的定时炸弹**（登记于 `统一整备路线图.md:194`，未修）；MoonBit 下同样越界 raise |
| S3 | `is_function_definition_line` 单行形态判断 | `collector.rs:217-219`（注释自承"左花括号写在下一行时仍会误判"） | 用行文本判断函数定义，而非 AST / 符号表 | 不搬；新设计用 AST 节点类型判断 |
| S4 | `pick_inner_index` 命名白名单回退（`j→i→k→idx→index`） | `collector.rs:248-265` | 循环变量识别靠命名约定 | 不搬；应从循环结构（哪个循环体的回边）判定 |
| S5 | `infer_semantic_label` 全函数（`contains("temp")` 判交换、`starts_with("printf")` 判 IO） | `collector.rs:375-448` | **用源码文本反推执行语义** | 本模块最深的架构债：`algorithm_step` 与 `semantic_label` 两条描述链都建在它上面；新架构应改为**执行期事件**（`vis_events` 机制已存在，`session.rs:192`） |
| S6 | `is_cpp_mode` 布尔穿透 / 语言靠扩展名与字面量判定 | 唯一检测点 `compile_pipeline.rs:603`（记载于 `CSharp前端引入计划.md:30`）；本次实测 `SourceLang` 全仓 **0 引用**、`"main.c"` 字面量 7 处 | 语言维度未单源化 | D6 参数化（指针快照 + 语义标签）**必须在 `SourceLang` 落地之后**，否则会在布尔穿透上再叠一层 |
| S7 | `cfg_has_back_edge = edges.any(\|(a,b)\| a >= b)` | `features.rs:120` | 用块 ID 大小序冒充回边——**这正是 `intent.rs:139-140` 已修掉的 B38 同族判据**，features 侧未同步清查 | 本次实测：`cfg_has_back_edge`/`cfg_num_blocks`/`cfg_has_unreachable`/`cfg_has_early_return` **生产零引用**（仅 `tree.rs:84`、`graph.rs:86` 两处测试手工构造）。**要么删、要么修**，不能带着搬 |

### 3.3 组织债 / 死代码

| # | 对象 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| D1 | `data_flow.rs` 整文件（323 行） | `data_flow.rs:27,219` | 零生产引用（唯一消费者是自己单测） | 若重建为孤儿则白写；先在 Rust 侧接一个消费者（哪怕 CLI 子命令）再谈搬 |
| D2 | `intent.rs` 整文件（403 行） | `intent.rs:57` | 同上，零生产引用 | 同上；`CSharp前端引入计划.md:275` 的 CS5 计划里也未提 intent |
| D3 | `knowledge_graph.rs` / `misconception_patterns.rs` / `learning_path.rs` 三模块 | 见 1.4 | 零出口；且其数据（A5/A6/A7）有语义错与悬空引用 | 重写目标应是"**先定接线形态，再写机制**"，否则复刻一批无消费者的代码 |
| D4 | `completion` 五上下文 | `completion/mod.rs:275` | 零出口（快照写入路径活着，读取端无人） | `U7#4` 已登记"补全增量缓存"（`统一整备路线图.md:207`）——若补全不接线，该登记项应一并作废 |
| D5 | **悬空的知识卡片 / 模板引用** | `knowledge_graph.rs:60,84,100,108,125,133,141,149,157,173,189,198,222,246,254`（15 处 `related_card_ids`）；`learning_path.rs:50,57,64,78,85,92,104,113,127,134,147,155,169,176`（14 处 `target_id`） | 全仓检索 `card_id` / `KnowledgeCard` 命中 **0 个卡片定义**；`target_id` 中的 `bubble`/`binary`/`pointer`/`array`/`factorial`/`linkedInsert` **templates 目录里并不都存在**，`EX_BOUNDARY_FIX` 全仓 0 命中 | 照搬等于把悬空引用带进新项目。**建议**：卡片模型 + 引用完整性校验（fail loud）作为独立包重建，`target_id` 必须能解析到 `templates/<key>/meta.yaml` |
| D6 | 教学 trap 文案双轨（`error_catalog` 卡片 vs VM 内硬编码 trap 串） | `error_catalog/semantic.rs:392-405`（E3060/3061 卡片） vs `vitro_vm/src/core/memory.rs:197`、`vitro_vm/src/host/memory.rs:67,123` | 同一知识（原因 + 解决方法）写了两遍；且 **E3070/E3071/E3072 只有 trap 文案、无卡片**（本次实测：catalog 全部 77 个码值不含 3070/3071/3072） | 双轨必然漂移；新设计应"卡片单源 + trap 只带结构化 code + 参数"，渲染归一处 |
| D7 | `AlgorithmMatch` 双类型 + 手工字段搬运 | `session.rs:31` vs `steps/lib.rs:16`；搬运 `session.rs:423-430` | 同名不同类型，7 个字段逐一手抄 | 低但真实（漏字段即静默）；MoonBit 单类型 + 可选字段 |
| D8 | detector 与 steps 的**文件划分不一致**：detector 侧 `math.rs`/`search.rs`/`string.rs` 各仅 19~25 行，steps 侧 `search.rs` 292 行、`structures.rs` 300 行 | `compiler/algorithm_detector/{math,search,string}.rs:6` vs `steps/{search,structures}.rs` | 同一组 43 个算法被两套切分轴组织 | 影响 ⑨ 包切分：应以"算法族"为唯一切分轴重整 |

---

## ④ 在途工作接纳方案

> 来源：`统一整备路线图.md`、`三语化整备审计计划.md`、`核心资产重构裁定.md`、`代码审阅与修复追踪20260906.md`、`CSharp前端引入计划.md`、`CHANGELOG.md`。逐条给"修复后搬 / 按目标架构实现 / 放弃并记录理由"。

| # | 在途项 | 出处 | 现状核实 | 接纳方案 |
|---|---|---|---|---|
| W1 | **U1#1① 算法标注 golden** | `统一整备路线图.md:111`、`核心资产重构裁定.md:195` | ✅ **已落地**：golden JSON 37 模板 / 311 条 + 测试 + 双向断言（`algorithm_annotation_golden_test.rs:110,166-170`）。**注**：测试头注写"317 条首现 / 45 零标注"（`:9`），JSON 实测 **311 条 / 37 模板**——`45 零标注 = 82 - 37` 成立，但 317≠311，**待证** | **修复后搬**（先修正注释口径）→ golden 直接作为 ⑧ 锚点 E2 |
| W2 | **U1#1② 标注-行为一致性 property**（标"交换"的 step 前后数组确实交换） | `统一整备路线图.md:111` | ❌ 未做（仓内无对应测试 / 脚本） | **按目标架构实现**：MoonBit 版应在"执行事实事件"上断言（配合 ⑤-4 重构），而非在文本 label 上断言 |
| W3 | **U1#1③ 诊断文案 golden**（error_catalog 码→文案片段表进测试） | `统一整备路线图.md:111`；裁定 `:194` 记录"error_catalog 文案无 golden，M4 margin=1" | ❌ 未做；`error_catalog.rs` 自身 0 测试（本次实测） | **按目标架构实现**：文案外置 JSON 后，golden = JSON 快照 + 码集合对账（含"每个码有卡片"的穷尽断言——可直接抓住 E3050 / E3070 类漏配） |
| W4 | **U7#7 `vitro_algorithm_steps/math.rs` 取模改用执行前快照** | `统一整备路线图.md:210`（"前端审查 #9"） | ⚠️ **疑似已完成**：`steps/math.rs:28-43` 已用 `env.prev_vars` 拼算式，注释明确"U1#1 管道批（P0-4 收口）"。登记项**可能过时** | **待与路线图维护者确认**（验证：读 `math.rs:31-32`；若确认，路线图该条应标完成） |
| W5 | **U7#7 error_catalog ~36 个零引用变体清理 + E4105/E4106 接线裁定** | `统一整备路线图.md:210` | 部分核实。**本次实测两种口径**：① 按"变体名在 `native/{src,crates}`（排除 `diagnostics/`）零引用"计，137 变体中 **123 个零引用**；② 按"77 条卡片中无发射点"计，**11 条**：E1004、E3026、E3027、E3056、E3060、E3061、E4100~E4104。其中 E3060/3061/3070/E3027 实际由**数值文本**发射（`vitro_vm/…/memory.rs:197` 等），故"零引用"须区分"枚举名引用"与"数值 / 文本发射"。E4105/E4106 确认：枚举已定义（`error_codes.rs:155-156`）但**无 catalog 条目、全仓零发射** | **放弃前先对账**：新项目用 `enum ErrorCode` + 穷尽 `card(code)` 让"有码无卡片"编译期不通过；E4105/E4106 要么接线（C++ 深浅拷贝 / 临时量引用检测）要么从 enum 删除并记录理由 |
| W6 | **U6#4 completion `context.rs` UTF-8 切片修复**（"pub API 定时炸弹"） | `统一整备路线图.md:194` | ❌ 未修（`context.rs:88-89,95-96` 原样） | **按目标架构实现**：不在 MoonBit 里复刻 `safe_byte_col`，而是把"坐标单位"写进 `Diagnostic` / 光标参数契约（见 ⑤-2） |
| W7 | **U7#4 补全增量缓存（源码哈希 + 失效标记 + 只增不减）** | `统一整备路线图.md:207` | ❌ 未做；当前每次调用都 `build_snapshot_from_source`（全量 Lexer + Parser，`completion/mod.rs:159-166,286`） | **按目标架构实现**（MoonBit 不可变快照 + 内容哈希键）；但**前置**：先决定 completion 是否接线（见 ③-D4） |
| W8 | **CSharp D5**：诊断管线"机制复用、数据表分语言" | `CSharp前端引入计划.md:51` | 裁决存在，代码未动（`SourceLang` 0 引用） | **裁决的代码依据经实测成立**：M3 输入仅 `Vec<i32>`+`Option<String>`（`misconception_patterns.rs:10-19,102`）；M4 输入仅 `Vec<DetectedMisconception>`（`learning_path.rs:35`）；M5 输入 `i32`+`String`（`knowledge_graph.rs:460,517`）→ **机制与数据分离度高**。**唯一例外**：`activate_from_ast` 入参是 C 词法关键词表（`knowledge_graph.rs:486-499` 硬编码 `"arr["`/`"*p"`/`"scanf"`），是"数据混进机制"的实锤，参数化时须一并处理 |
| W9 | **CSharp D6**：`collect_pointer_snapshots` 线性内存假设 + `infer_semantic_label` C 库函数启发 按 `SourceLang` 分派 | `CSharp前端引入计划.md:52` | 裁决存在；**代码依据已定位**：线性内存假设 `collector.rs:184`（`NULL_TRAP_SIZE..MEM_SIZE`）+ `:186`（`session.memory.regions`）；C 库启发 `collector.rs:425,434,436,438,440` | **按目标架构实现**：不做运行时 `if lang == CSharp`，而是把"指针 / 引用状态判定"与"语义标签分类器"抽成**按语言注入的策略**（MoonBit 无 trait 对象，用 record-of-functions 或 enum 分派） |
| W10 | **CSharp CS5**：TraceAnalyzer 异常类目 + C# 误区 / 概念表 + G9 算法验证 | `CSharp前端引入计划.md:275` | 未开工 | 注意 G9 澄清：`validate_algorithm()` / `ValidationResult` 在后端**从未落地**（`项目路线图.md:105`）——不要当"待搬资产" |
| W11 | **`apply_fix` 中文行 panic（E-P1-6）** | `代码审阅与修复追踪20260906.md:222`；回归 `crash_regression_tests.rs:8,463-495` | ✅ 已修复（`auto_fix.rs:26-34` + 3 条回归） | **修复后搬**——但搬的是**回归用例**（3 个坐标场景），不是 `safe_byte_col` 本身 |
| W12 | **`ERROR_INFO_MAP` 重复错误码静默覆盖无校验** | `代码审阅与修复追踪20260906.md:232`；同族 B52 已修（`CHANGELOG.md:2094` 记载 `ERROR_CONCEPT_MAP` 3035 重复） | 现状核实：**实测 77 条码值无重复**（当前安全），但**机制仍在**（数组 → HashMap 无断言） | **按目标架构实现**：MoonBit `enum` 穷尽 match 结构性消除；另加"码集合 == 卡片集合"对账测试 |
| W13 | **诊断切面测试覆盖（38/810）与 margin 门禁** | `核心资产重构裁定.md:194-195` | 未改善 | **放弃并记录理由**（属防线建设项，不是模块重写项）；迁移期以 ⑧ 的差分锚点替代部分职能 |

---

## ⑤ 架构优化建议（MoonBit 形态）

| # | 建议 | 标记 | 依据 / 收益 |
|---|---|---|---|
| 1 | **诊断数据全面外置为包内 JSON**：`catalog.json`（77 卡片）+ `concepts.json`（25 节点 / 25 边）+ `patterns.json`（6 模式）+ `paths.json`（6 路径）+ `fix_payloads.json`（~25 码替换载荷）；代码只做"加载 + 解释" | 〔结构优化〕 | 仓库既有纪律（`AGENTS.md`"资产外置 JSON，代码只做解释器"）；诊断模块当前只做到 1/5（仅 `error_catalog.rs:524` 有出口）。收益：人审数据不审代码；下游可 vendor |
| 2 | **坐标口径单源化**：`Diagnostic` 只携带一种单位（建议字节偏移 `start/end`）并显式声明；删除 `safe_byte_col` 类猜测 | 〔结构优化〕 | `auto_fix.rs:22-25` 自承坐标混杂；MoonBit String 是 UTF-16，"按错单位切片"从 panic 变静默错位，比 Rust 更危险 |
| 3 | **错误码 → 卡片改为 enum + 穷尽 match**：`enum ErrorCode { … }` + `fn card(ErrorCode) -> Card`，删除 `_ =>` 兜底 | 〔结构优化〕 | 结构性消除"有码无卡片 / 有卡片无码"——E3050 有修复无卡（`error_catalog.rs:276` vs 无卡片条目）、E3070 无卡、E4105/E4106 只有 enum，三处实证 |
| 4 | **语义标注从"行文本启发"改为"执行事件"**：`semantic_label` / `algorithm_step` 改由 codegen / VM 在语义动作点（数组交换、内存分配、函数进入）发结构化事件，文本渲染下沉为纯函数 | 〔新设计〕 | `collector.rs:375-448` 全部判据是 `contains`/`starts_with`；`vis_events` 机制已存在（`session.rs:192`，采集 `collector.rs:68`）。收益：① 消除 S3/S4/S5 三个坑族；② D6 参数化自然消失（事件由前端语言产出） |
| 5 | **trap 结构化**：VM 抛错带 `code: ErrorCode` + 参数元组，`TraceAnalyzer` 按 code 分派而非 `trap_message.contains("E3060")` | 〔结构优化〕 | `trace_analyzer/mod.rs:41-50` 依赖**中英文消息文本**；文案一改根因推断静默失效（`misconception_patterns.rs:150-157` 也靠 `"stack overflow"`/`"栈溢出"` 关键词） |
| 6 | **图结构用不可变邻接表，而非"每次扫边数组"** | 〔结构优化〕 | 现状 `collect_neighbors` 每次遍历全部 25 条边（`knowledge_graph.rs:561-581`）；`find_loops`/`compute_dominators` 每块扫全部边（`cfg.rs:100-107,151-156`）。MoonBit 用 `Map[Int, Array[Int]]` 预建 + `FixedArray` 定长邻接 |
| 7 | **数据带 `lang` 维度，而非"分语言分文件"** | 〔新设计〕 | `vocabulary.rs:27` 已有 `domain: "c"\|"csharp"` 先例，`:33,35` 的 `status/since` 已示范"预留位"模式；推广到卡片 / 概念 / 模式表，避免 C# 落地时复制三份文件 |
| 8 | **合并 `AlgorithmMatch` 双类型**，并把 43 个算法建成单一 enum（`enum Algorithm { BubbleSort, … }`），检测与标注共用 | 〔结构优化〕 | `session.rs:31` vs `steps/lib.rs:16` + 手抄 `session.rs:423-430`；且 43 个名字现在以**字符串**在 8 个文件间流动，改名不会编译报错 |
| 9 | **知识卡片实体 + 引用完整性 fail loud** | 〔新设计〕 | 29 处悬空引用（③-D5）。建议加"所有 `card_id`/`target_id` 均可解析"的对账测试，风格对齐 `scripts/facts` 的"坏引用即红" |
| 10 | **保留 CFG / dominance，但砍掉重复实现** | 〔沿革保留〕 | `cfg.rs:46,73,130` 三个算法（不可达块 / 自然循环 / 支配树）设计正确，且 B35/B36 已把两个构造缺陷修掉（`cfg.rs:277,422`）——**本模块少数"已想清楚"的机制**，应原样重建并保留 `:495-526` 两条回归 |
| 11 | **意图推断与活跃变量：接线或删除（二选一，不迁移孤儿）** | 〔沿革保留／待裁定〕 | `data_flow.rs` / `intent.rs` 零消费者（本次实测）。若 CS5 的 G9"算法验证"要落地，`infer_intent` 是天然候选；否则应删 |
| 12 | **统一模式一帧发布缓冲：语义保留、实现重写** | 〔沿革保留〕 | `session_api.rs:248-291` 记录了 3 次复发（克隆发布 → 重复投递 → 首调空数组）；语义已冻结（`step_index` 严格递增），实现应改为"显式队列 + 不变量断言" |

---

## ⑥ 坑清单（本模块事故史）

> 格式：现象 → 根因 → 修复 → 新语言下是否复发、为什么。

| # | 现象 | 根因 | 修复 | MoonBit 是否复发 / 为什么 |
|---|---|---|---|---|
| 1 | `apply_fix` 对含中文的行**按字节切片 panic**，前端"一键修复"崩溃 | 坐标体系混用（生成侧按字节，消费侧按字符） | `safe_byte_col` 三级退化（`auto_fix.rs:26-34`）；回归 `crash_regression_tests.rs:463-495` | **会**。MoonBit String 是 UTF-16，`s[a:b]` 在代理对边界会 raise（比 Rust 更早暴露，但仍是错误）；字节列 ≠ UTF-16 列。必须靠契约（⑤-2）而非兜底函数 |
| 2 | `completion/context.rs` 同类切片 | 同上；登记 U6#4 未修 | ❌ 未修（`context.rs:88-89,95-96`） | **会**，同上 |
| 3 | `ERROR_INFO_MAP` 重复错误码**静默覆盖** | 数组拼进 HashMap 无重复检测 | 现无重复（实测 77 条）但机制在 | **不会**（若用 enum + 穷尽 match）；照搬 Map 插入则**会** |
| 4 | `ERROR_CONCEPT_MAP` 键 3035 重复 → scanf 映射被覆盖（B52） | 同上，且是 28 条手工 `insert` | 删重复项、3030-3035 统一映射（`CHANGELOG.md:2094`） | **不会**（enum / 结构化表 + 对账测试） |
| 5 | `ERROR_CONCEPT_MAP` 把 **3020（单目运算类型错误）映射到 `Recursion`（递归）** | 手工映射表写错（疑为 3021 / 递归码之误） | ❌ **未修，当前仍在**（`knowledge_graph.rs:445` vs `semantic.rs:131-137`） | 与语言无关——**迁移前必须修，否则把错误映射带进新数据表** |
| 6 | E3050 有修复生成器、**无教学卡片** | 卡片表、修复表、枚举三处独立维护 | ❌ 未修；表现为该诊断 `emoji/title/explanation` 全空（`compile_pipeline.rs:155-168` 走 else 分支） | 与语言无关，**会**（除非 ⑤-3 落地） |
| 7 | **E3070（缓冲区溢出）无知识卡片**（任务书假设它存在） | 同上；3070/3071/3072 三码只有 trap 文案 | ❌ 未修（实测 catalog 77 个码值不含 3070） | 与语言无关，**会** |
| 8 | CFG 构造两缺陷：If 条件块**克隆整棵 If AST 子树**（B35）；`Return` 块仍向后 fall-through（B36） | 图构造复用 AST 节点时未裁剪 + 边生成未按终结符区分 | `cfg.rs:277-286`（改 `Expr` 占位）+ `:422-426`（只连 `FallThrough`）；回归 `cfg.rs:495-526` | **判据逻辑与语言无关 → 会复发**；须原样保留这 2 条回归 |
| 9 | 活跃变量分析 O(N·E) 全表扫边（B37） | 未建邻接表 | `data_flow.rs:37-41` 预建 `out_edges`；回归 `:303-322` | 会（若重写时再图省事）；但该文件当前是孤儿 |
| 10 | 意图推断用块 ID 大小序判回边（B38） | 把"分配顺序"当"支配关系" | `intent.rs:139-140` 改用 `find_loops`；回归 `:394-402` | 会；**且 `features.rs:120` 同族判据未同步清查**（S7）——这是"同类清查义务"（`统一整备路线图.md:193`）的反面实证 |
| 11 | 检测器**大规模误判**（用户审阅实锤，U1#1 P0-2 及二审 P0-B）：`isValidBST`→BST 插入；`subString` 含 "bst"→BST；`insert_node`→插入排序；`mergeSortedLists`→归并排序；`linearSearch`→BFS；递归 `binarySearch`→DFS；`hashTable.insert`→BST 插入 | ① 裸 `contains()` 子串判据；② 结构判据缺判别特征（无队列 / visited 就判 BFS）；③ 无语义消歧（valid vs insert） | `has_word` 词边界 + 驼峰缩写合并（`features.rs:70-101`）；命名 + 结构双条件；`is_treenode_ctx` 类型语境（`features.rs:44-57,115`）；15+ 条红锚测试（`tree.rs:76-211`、`sorting.rs:115-144`、`graph.rs:78-106`） | **必然复发**——判据是纯语义启发式，与语言无关。防线是：把判据**数据化**（规则表 + 权重），并把这一批红锚用例逐条搬进新项目 |
| 12 | 连续大写缩写被逐字母切开：`isValidBST` → `["is","valid","b","s","t"]` → **整体静默零标注**（4001 帧 / 0 条标注，零标注不触发任何断言） | 分词器缺"大写段末大写归属"规则 | `features.rs:71-101` 重写切分；21 条表驱动单测 `:761-788` | **会**（同 11）；"零标注 = 无断言"是防线设计缺陷，新项目需要"零标注集合"双向断言（golden 已做 `:166-170`） |
| 13 | `extract_features` 传空 `func_name` → `is_recursive` 恒 false；且真实前端把调用表示为 `CallPtr{callee}` 而 walk 只匹配 `Call{name}` → 两条路径都失效 | 单测全部手工构造 `FuncFeatures`，绕过真实提取路径 | 补 `CallPtr` 分支（`features.rs:246-267`）+ 管线级锚 `algorithm_detector_pipeline_test.rs:78,94` | **会**——"单测构造输入"的假绿模式与语言无关。对策：测试默认走真实管线 |
| 14 | `semantic_label` 判定顺序错：`loop_depth>=1` 优先于具体语句 → `free(p)`/`printf`/`return` 全被标成"循环 i=3"，词汇表里"释放内存 / 调用 printf"几乎不可达 | 判据顺序设计错误 | 重排（`collector.rs:405-412` 详注）+ 词汇闭合测试 `step_payload_schema_v0_1_test.rs:353` | **会**（同族顺序陷阱）；对策：分类器写成**有序规则表 + 首命中**并附顺序敏感性测试 |
| 15 | 交换标注下标取 `loop_vars.first()`（白名单含 `n`）→ `交换 arr[5]↔arr[6]` 与执行事实相反，同一 payload 里两条描述互相矛盾 | 候选列表语义未定义（规模量 vs 迭代量） | `pick_inner_index` 从源码行取下标（`collector.rs:248-265,416-422`） | **会**（同族） |
| 16 | 冒泡排序"第 n-i 大"**把概念教反**；且循环退出值上继续产出越界描述（5 元素数组产生 24 步越界） | 趟数与排名换算写反 + 无"变量是否仍在合法区间"守卫 | `sorting.rs:25-37`（改 `i+1`）+ `:16-20`（`j_in_inner_range` 守卫） | 与语言无关，**会**；靠 golden 锚 |
| 17 | gcd 用行末变量值拼算式（`48 % 12 = 0`，真值 `48 % 18 = 12`）；hanoi 在 main 调用点恒减一（"移动 2 个盘子"）；`contains('%')` 命中 printf 格式串 | 缺少"行入口快照"概念；单靠行文本无法区分顶层调用与递归 | `InferEnv.prev_vars`（`lib.rs:48-59`）+ `math.rs:28-43` + `:109-121` + `:20-26` 排除 IO 行 | **会**（同族）；`prev_vars` 这类"执行上下文"设计应作为新架构一等输入 |
| 18 | 一帧发布缓冲三个连环缺陷：首帧旧值 / 重复投递同一 step（序列 `0,0,1` 违反"`step_index` 严格递增"）/ 首调语义未定义 | 缓冲引入的时序语义未固化 + 赋值写在 `if` 分支内成为死代码 | 逐次修正 + 长注释（`session_api.rs:248-291`）；登记 `统一整备路线图.md:674` | **会**（时序语义与语言无关）；对策：把不变量写成可执行断言 |
| 19 | **文档数字漂移（本模块自身）**：① 知识图谱文档称"24 概念节点 + 30+ 关系边"，实测 **25 / 25**（`knowledge_graph.rs:51,259`）；② 文档称 41 模板，实测 **43** 分派臂；③ golden 测试头注称 317 条，JSON 实测 **311** 条；④ `AGENTS.md` Phase 16 称 27 种 | 人工维护的数字未随代码更新（`facts` 对账只覆盖 CURRENT 文档裸数字，模块内注释不在覆盖面） | ❌ 未修 | 与语言无关；迁移时应把这类数字纳入机器对账 |

---

## ⑦ MoonBit spike 清单

> 每条：依赖的语言特性 → 最小验证程序 → 判定标准。**先 `moon ide doc` 确认 API 再用**（不把记忆当事实，不用 Rust/Go 命名翻译 MoonBit API）。

| # | 依赖特性 | 最小验证程序 | 判定标准 | 关联证据 |
|---|---|---|---|---|
| 1 | **`@json` 解析外置规则表** | 把 `error_catalog::export_json()` 的真实产物（77 条，含嵌套 `common_causes[]` 与中文）落成 `catalog.json` → MoonBit 侧读文件 → 解析 → 建 `Map[Int, Card]` → 查 3004 与 9999（不存在） | ① 77 条全部解析成功且 `common_causes` 长度与原 JSON 一致；② 中文与 emoji（代理对）无损坏；③ 缺 key 时**返回 None 而非 default**（fail loud）；④ `moon test` 断言条数 == 77 | `error_catalog.rs:524-550`、`:519` |
| 2 | **图结构（邻接表）表示** | 25 节点 / 25 边搬进 `concepts.json` → 建 `Map[String, Array[String]]` → 实现 1-hop 邻居收集与前置路径 DFS（`find_prerequisite_path("ImplicitCast")`） | 输出**序列逐项等价** Rust 版：前置路径顺序由 `EDGES` 声明序决定（`knowledge_graph.rs:533-540` 先父后子 push）→ 期望含 `VarDecl, TypeSystem, ImplicitCast`；`activate_from_error(3051)` 含 `BoundaryCondition`（对齐 `:592-597`） | `knowledge_graph.rs:460,517,561,592,608` |
| 3 | **顶层不可变值的惰性一次性初始化**（替代 `LazyLock`） | 顶层 `let` 持有一张 77 条表 + 一个计数器（`Ref`）；两处调用各读一次 | ① 只构造一次（计数 == 1）；② 无重入 / 无 panic；③ 若语义是"启动即构造"，测冷启动耗时（目标：与 Rust `LazyLock` 首访同量级） | `error_catalog.rs:27`、`knowledge_graph.rs:51,259,422`。**待证**：MoonBit 顶层值初始化时机 |
| 4 | **不可变结构共享替代 `Arc`/`Clone`** | 25 节点表作为不可变值在函数间传递并放进返回结构（模拟 `get_all_concept_nodes` `:548` 与 `ActivatedConcept.node` 克隆 `:469`），循环 10⁵ 次取值 | ① 语义正确；② 时间不随"深拷贝 25 个含 4 个 `String` 的节点"线性增长（`moon run --profile --target native --release` 的 self-time 中**不出现**按元素复制的热点）。**间接判据**（无直接观测拷贝次数的 API） | `knowledge_graph.rs:468-472,548`；评估报告 §2.1 A5 |
| 5 | **String = UTF-16 的坐标语义** | 取三行源码（`// 中文注释 x`、`char* s = "a中b";`、含 emoji 的行），分别按 ①字节列 ②字符列 ③UTF-16 列取子串 | 记录哪些口径 raise、哪些静默错位；据此**钉死唯一坐标单位**并写成 `Diagnostic` 契约条目 | S1/S2（`auto_fix.rs:26-34`、`context.rs:88-89`） |
| 6 | **enum + 穷尽 match 的"码 → 卡片"完整性** | 定义含 137 变体的 `enum ErrorCode` + `fn card(ErrorCode) -> Card`（先只写 77 个分支），`moon check` | ① 缺分支**编译失败**（非穷尽）；② 加 `_ =>` 兜底后再删一个分支应**不再报错**（证明兜底会掩盖漏配，因此禁止兜底） | `error_codes.rs:8-157`、`error_catalog.rs:65-435`。**待证**：非穷尽 match 的诊断形态 → `moon check --explain` |
| 7 | **`Float` 精度与 JSON 往返**（`ConceptEdge.strength: f32`） | 25 条边的 `strength`（0.6/0.7/0.8/0.9）经 JSON 写 → 读 → 比较 | 往返后相等（MoonBit `Float` 是 f64）；若 `Eq` 语义有坑则记录 | `knowledge_graph.rs:27,266` 等 |
| 8 | **词边界 / 驼峰切分可移植性** | 把 `features.rs:761-788` 的 **21 条表驱动用例**逐条搬成 MoonBit 测试（`has_word`） | 21/21 一致，尤其 `getHTTPResponse→["get","http","response"]`、`bestEffort` 不含 `bst`、`bst2insert` 双向命中 | `features.rs:70-101,761-788` |
| 9 | **String 索引 / 切片的错误形态**（用于 S1/S2 契约） | 对含代理对的 String 做 `s[i]` 与 `s[a:b]` 越界 / 中间切 | 明确 raise 还是返回默认；据此决定是否需自建"字节视图"（`Bytes` 不可变 → 可能需 `FixedArray[Byte]` 自持缓冲） | 评估报告 §2.4；本模块只取子串，影响较小但必须实测 |

---

## ⑧ 等价性验收锚点

**总原则**：迁移期 Rust 版仍在，同一输入分别驱动两版，diff 结构化输出。**格式统一 NDJSON / JSON**，比对工具统一 Go 驱动（`AGENTS.md`：判定型脚本默认 Go，零第三方依赖，规则外置 JSON）。

### 8.1 锚点清单

| # | 输入 | Rust 侧产物 / 出口 | 格式 | 比对工具与口径 |
|---|---|---|---|---|
| **E1** | 同一 `source.c` + 同一 stdin 序列 | `serve` 会话逐帧 NDJSON：`compile` → `run` → `step.begin` → `step.next`×N（+ 可选 `seek`） | NDJSON 一行一帧 | 现役 `scripts/serve_smoke` 同族新驱动；**逐字段**比对 `step_index / code_line / func_name / semantic_label / algorithm_step{algorithm_name,display_name,phase,description} / pointer_snapshots[*].{name,target_addr,target_name,status} / root_cause_hint.*`。注意首调空帧语义（`session_api.rs:264-291`），两侧同口径 |
| **E2** | 82 个 `templates/<k>/source.c` | 算法标注**首现序列**（去重键 `(phase,desc)`）：`{algorithm,display_name,phase,desc,code_line,src}` | JSON，按模板名分组的 `BTreeMap<String, Vec<…>>` | **已有 golden 即锚**：`native/tests/golden/algorithm_annotations_v3.json`（37 模板 / 311 条）。抽取口径见 `algorithm_annotation_golden_test.rs:59-105`（`STEP_BUDGET=4000`）→ 三方 diff（Rust / MoonBit / golden）。**双向断言**：有标注模板集 == golden 键集 |
| **E3** | 无（静态出口） | capi `vitro_get_error_catalog_json` / serve `error_catalog` / `session_api.rs:162` | JSON 文本，码升序，字段序 `{code,code_str,lang,category,emoji,title,explanation,common_causes[]}`（`error_catalog.rs:535-547`） | **可逐字节 diff**（`:525-526` 已排序，保证跨构建可差分）。Go 驱动读两侧 + `bytes.Equal`，差异按码定位 |
| **E4** | 一组错误源码（含 E3050 / E3060 / E3070 场景） | `session_api::compile` 的 `diagnostics[]`：`{code,error_code,severity,line,column,end_line,end_column,message,fix_suggestion,filename}` | JSON | 逐条 diff。**口径注意**：`end_line/end_column` 当前是退化值 `line / column+1`（`session_api.rs:55-57`）；`fix_suggestion` 回落规则是"空则取 `explanation`"（`compile_pipeline.rs:131-135`），须一并复刻 |
| **E5** | 触发 trap 的程序（越界 / 除零 / UAF / DoubleFree / NULL） | 统一模式 payload 的 `root_cause_hint`：`{category,one_liner,related_lines,suggested_fix_kind,suggested_fix_line,suggested_fix_desc}` | JSON | 逐字段 diff（`root_cause.rs:7-28`）。`category` 是字符串枚举（8 个合法值 `:9-11`），应改 enum 后按枚举名比对 |
| **E6** | 同一源 + 同一 `(line, column, prefix)` 序列 | `get_completion_candidates` → 截断 50、按 `sort_text` 排序（`completion/mod.rs:304-313`） | JSON 数组 `{label,kind,detail,insert_text,sort_text}` | 逐项 diff（顺序敏感）。**前置缺口**：completion 当前**无出口**——须先在 Rust 侧加测试专用导出（或 serve 新增方法），否则无锚 |
| **E7** | 知识图谱查询序列（`activate_from_error(3051)`、`activate_from_ast([…])`、`find_prerequisite_path("ImplicitCast")`） | 无出口（仅单测 `knowledge_graph.rs:592,601,608`） | JSON | **锚点缺口**：须先在 Rust 侧补薄导出。否则第四层迁移**无等价性证据** |
| **E8** | 一份编译历史（`Vec<CompileRecord>`）序列 | 无出口（仅 `misconception_patterns.rs:180-221`） | JSON | 同上缺口。建议把 `:170-221` 的 4 组输入 / 期望外置为 JSON，两侧共读 |
| **E9** | 同上 → `recommend_learning_paths` | 无出口（仅 `learning_path.rs:194-214`） | JSON | 同上缺口 |

### 8.2 缺口与纪律

- **E1~E5 有现成出口或 golden，可直接用**；**E6~E9 无出口**——这是本次勘察发现的最要紧迁移风险：**认知链第二/三/四层与补全在 Rust 侧没有对外可观测面，"用 Rust 版做差分对照"这条唯一退路对它们不存在**（评估报告 §3.3/§7 把"趁 Rust 版仍在做差分扫描"列为唯一不重踩坑路径）。
- **对策（必须在阶段 1 之前做）**：为 E6~E9 在 Rust 侧补最小导出（serve 加 4 个方法或一个 `diagnostics_probe` 方法，返回结构化 JSON），并把这 4 组用例外置为 JSON。属"迁移前置工程量"，须计入排期（与 `error_catalog` 出口同族，量级小）。
- **跨两版都必须成立的不变量**：① `step_index` 严格递增（`session_api.rs:260` 记载为冻结不变量）；② 首调 `payloads` 为空数组；③ golden 双向断言（有标注集 == 键集）；④ `error_catalog` JSON 码升序且只增不改。
- **红→绿纪律延续**：每个差异都必须先有一个"能暴露它的用例"，禁止以"改了测试预期值"收口（`AGENTS.md` 防线哲学第 0 条）。

---

## ⑨ mooncakes 包切分草案

**切分轴**：依赖方向（不是文件划分——文件不构成命名空间，包 = 编译单元）。目标：数据包可独立发布、机制包单向依赖、协议 DTO 归口。

```
vitro/diagnostics                    ← umbrella（.mbti 只 re-export 稳定 DTO）
├── internal/catalog/               【数据包】卡片 + 码 + 修复载荷
├── cognitive/                      【机制包】误区 → 路径 → 概念图
├── autofix/                        【机制包】坐标安全的应用器
vitro/analysis
├── cfg/                            【机制包】CFG + 循环 + 支配树
├── flow/                           【机制包】活跃变量 + 常量条件 + 意图打分
├── algorithms/                     【机制包】特征提取 + 43 判据 + suggestion 表
vitro/teaching/steps                【机制包】43 个 infer + InferEnv + 上下文抽象
vitro/engine/completion             【机制包】快照 + 五上下文 + 候选生成
```

### 9.1 各包职责 / 依赖 / 发布形态

| 包 | 内容（迁移自） | 依赖 | 对上发布形态 |
|---|---|---|---|
| `vitro/diagnostics/internal/catalog` | `catalog.json`（A1+A3 载荷）、`concepts.json`（A4+A5）、`patterns.json`（A6）、`paths.json`（A7）、`ErrorCode` enum、`card(code)`、`fix_payload(code)`、加载与对账 | `moonbitlang/core/json`（仅此） | **公开包**：数据资产可被下游 vendor（同 `error_catalog.rs:519`"只增不改"承诺）；`mbti` 暴露 `Card`/`ErrorCode`/`Concept`/`Pattern` |
| `vitro/diagnostics/cognitive` | 误区滑窗检测（M3）、学习路径组装（M4）、概念图激活 / 邻居 / 前置路径（M5） | `catalog`；**零 AST / 零 VM 依赖**（实测支撑：`misconception_patterns.rs:10-19`、`learning_path.rs:35`、`knowledge_graph.rs:460`） | 公开包；`DetectedMisconception`/`LearningPath`/`ActivatedConcept` 属本包（用户会 match / 构造，不能放 internal） |
| `vitro/diagnostics/autofix` | 坐标契约 + replace/insert/delete 应用器（M2 的正解版） | `catalog`（fix 载荷） | 公开包（下游 IDE 一键修复）；**须先把坐标单位写进 DTO** |
| `vitro/analysis/cfg` | `cfg.rs` 全部（不可达块 / 自然循环 / 支配树 / loop info） | `vitro/ast` | 公开包；B35/B36 两条回归随之搬 |
| `vitro/analysis/flow` | `data_flow.rs` + `intent.rs` | `cfg`, `ast` | **暂缓发布**（先裁定接线，见 ⑤-11） |
| `vitro/analysis/algorithms` | `features.rs`（含 `has_word`）+ 7 个族检测器 + suggestion 表 | `ast`, `cfg`, `catalog`（suggestion 文案可移入数据包） | 公开包；暴露 `Algorithm` enum + `detect(program) -> Array[Match]` |
| `vitro/teaching/steps` | 43 个 infer + `InferEnv` + 上下文抽象 +（若不做 ⑤-4 重构）按语言注入的标签分类器 | `analysis/algorithms`、一个 `SourceLine` 抽象（**不要**依赖 VM） | 公开包；`AlgorithmStepSnapshot` 属本包；`infer_semantic_label` 建议**移出**（属 unified 采集职责），只保留 `classify(label) -> LabelKind`（对应 `vocabulary.rs:163`） |
| `vitro/engine/completion` | `completion/*`（五上下文 + 快照） | `ast`, `lexer`（实时解析取符号） | 公开包（IDE 补全）；**前置**：先决定接线（③-D4） |
| `vitro/unified/protocol`（**不在本模块内，但被依赖**） | `StepPayload`/`RootCauseHint`/`PointerSnapshot` 形状 + `TraceAnalyzer` 分派 | — | M14（`TraceAnalyzer`）应归协议 / 统一模式包，**而不是** diagnostics——它依赖 `StepPayload`（`trace_analyzer/mod.rs:24`）与 `Session`（`:17`），反向依赖会污染 diagnostics |

### 9.2 依赖图（单向，无环）

```
core/json
   ↓
catalog ──→ cognitive ──→ (umbrella vitro/diagnostics)
   ↓
autofix

vitro/ast ──→ cfg ──→ flow
      └────→ algorithms ──→ teaching/steps
      └────→ engine/completion
```

### 9.3 切分要点与风险

1. **`catalog` 必须是"零逻辑数据包"**：只要它还包含 `generate_fix` 式坐标计算，就无法满足"数据可独立 vendor、代码只做解释器"。建议 `catalog` 只导出 `card` / `fix_payload` 两张纯查表函数，坐标计算下沉到 `autofix`。
2. **`domain`/`status`/`since` 三元组进 catalog 每条记录**：`vocabulary.rs:27,33,35` 已有先例，直接推广到卡片 / 概念 / 模式（替代"分语言分文件"）。
3. **不要按现有文件切包**：`vitro_algorithm_steps` 的 7 个文件按"算法族"混切（`search.rs` 里放着 `infer_string_reverse`、`structures.rs` 里放着 `infer_hash_table`），而 detector 侧又是另一套划分（③-D8）。新包统一按"算法族"（sorting/graph/tree/search/string/math/dp/structures）单轴切。
4. **测试归位**：黑盒 `*_test.mbt` 放行为测试（golden 回放、E1~E9 差分）；白盒 `*_wbtest.mbt` 只用于 `has_word` 这类内部判据的表驱动测试。**默认黑盒**，以避免重演 ⑥-13 的"手工构造输入假绿"。
5. **风险与排序**：`vitro/analysis/algorithms` + `vitro/teaching/steps` 合计 5,173 行，是全模块 bug 密度最高的部分（⑥ 有 8 条坑落在这里）。**建议这两包放到迁移后期**：先迁 `catalog`/`cognitive`/`cfg`（低风险、高复用），把 steps 的重写与 ⑤-4（执行事件重构）绑定做，避免"照搬文本启发再改造"的双倍成本。

---

## 附录 A · 本次勘察的"待证"清单

| # | 待证命题 | 验证方法 |
|---|---|---|
| 1 | golden 测试头注"317 条首现"是否已过时（JSON 实测 311） | 重跑 `algorithm_annotation_golden_test` 的抽取口径并打印总数；对比注释 `algorithm_annotation_golden_test.rs:9` |
| 2 | `统一整备路线图.md:210` 的"math.rs 取模改用执行前快照"是否已完成 | 读 `steps/math.rs:28-43`（已用 `env.prev_vars`）；确认后关闭该登记项 |
| 3 | "error_catalog ~36 个零引用变体"的原始统计口径 | 本次两种口径分别测得 123/137（变体名零引用）与 11/77（卡片无发射点），均非 36；需查前端审查 #14 原文 |
| 4 | MoonBit 顶层不可变值的初始化时机（惰性 vs 启动期） | ⑦-spike 3 |
| 5 | MoonBit 非穷尽 `match` 的编译诊断形态与错误码 | ⑦-spike 6 + `moon check --explain` / `moon explain --diagnostic` |
| 6 | `ConceptEdge.strength`（f32）在 MoonBit `Float`(f64) 下的 JSON 往返无损 | ⑦-spike 7 |
| 7 | `data_flow`/`intent`/`completion`/`knowledge_graph` 是否确无隐藏消费者（wasm 出口 / scripts / 外部消费方） | 本次已全仓检索 `native/{src,crates,tests}` 与 `scripts/` 零命中；如需强证，再核 `capi`/`serve` 方法表（`vitro_cli.rs:593` 附近方法清单） |

## 附录 B · 本次实测口径

- **行数**：`(Get-Content <file>).Count`（含空行）；跨目录合计用同口径求和。
- **`#[test]` 计数**：`Select-String -Pattern '#\[test\]'` 逐文件计数（不入 `target/`）。
- **目录计数**：`Get-ChildItem -Recurse -Filter *.rs`；`templates/` 用 `Test-Path source.c|meta.yaml` 过滤。
- **码值/名字集合对账**：正则抽取后 `Sort-Object -Unique` 再双向 diff（detector 43 vs steps 43）。
- **真值引用**：一律读 `reports/facts.json`（`generated_at` = 2026-09-18T13:01:41+08:00，`git.rev` = `4b57191`），标 key 与 `as_of`。
- **未采用**：`cargo test` 全量执行（`cargo_test_passed` 在台账中为 `unavailable`）；`go run ./scripts/facts check`（本次为只读勘察，不生成报告产物）。
