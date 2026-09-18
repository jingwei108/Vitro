# Vitro 项目文档

> 教学 C/C++ 子集参考执行引擎（白箱后端）——架构设计、语言子集规范、协议与测试防线
>
> 最后核对：2026-09-13（归类翻新：current 全部文档逐个取证后重命名中文化 33 份、归档 7 份；
> 前一沿革：2026-09-11 前端切割后重新整理，归档 17 份旧文档、删除英文文档、重写核心文档）

> **目录约定**：自 2026-09-13 起 `current/` 下按分类存放于 7 个子目录（01-定位与路线 / 02-构建与上手 / 03-语言子集 / 04-标准库与防线 / 05-教学体验 / 06-出口与协议 / 07-质量与裁定）；新文档请放入对应子目录。
>
> **命名约定**：`current/` 下文档自 2026-09-13 起使用中文文件名（专有名词如 C++/CLI/VM/schema 保留英文）；
> 旧英文名在其他分支或本地检出中可能仍被引用，对照关系见各文档自身头部。

## 文档目录

### 📁 [current/](current/) — 当前有效文档

#### 定位、路线与架构

| 文档 | 说明 |
|------|------|
| [`current/01-定位与路线/后端定位与白箱计划.md`](current/01-定位与路线/后端定位与白箱计划.md) | **后端定位主计划**：前端切割决策、三出口一核心架构、协议先行、Phase 0~3 路线（原 `VITRO_BACKEND_SPLIT_WASM_WHITEBOX_PLAN.md`） |
| [`current/01-定位与路线/项目更名记录.md`](current/01-定位与路线/项目更名记录.md) | **项目更名记录**：Cide → Vitro 决策依据、命名映射、ABI 2.0.0 迁移指引、诚实边界与验证记录（2026-09-14） |
| [`current/01-定位与路线/架构设计.md`](current/01-定位与路线/架构设计.md) | 架构总纲（编译器管线 / VitroVM / 内存模型 / 时间旅行 / 诊断 / 协议 / 关键决策）（原 `DESIGN.md`） |
| [`current/01-定位与路线/项目路线图.md`](current/01-定位与路线/项目路线图.md) | 项目路线图：当前状态、已完成里程碑、下一步、已知缺口 G1~G13（诚实记录）（原 `ROADMAP.md`） |
| [`current/01-定位与路线/结构重构与C23锚定决议.md`](current/01-定位与路线/结构重构与C23锚定决议.md) | 结构重构决议（R1~R4，已全部交付）+ C23 语言锚定 + E2 模块化预处理器 + E3 C23 语义级（原 `VITRO_RESTRUCTURE_PLAN.md`） |
| [`current/01-定位与路线/工程债务维护方案.md`](current/01-定位与路线/工程债务维护方案.md) | 工程债务偿还与长期维护方案（`#DXX` 债务编号体系的事实源）（原 `MAINTENANCE_PLAN.md`） |
| [`current/01-定位与路线/内存安全规范.md`](current/01-定位与路线/内存安全规范.md) | 内存安全规范（Rust 边界、线性内存、堆隔离与检查清单）（原 `MEMORY_SAFETY.md`） |

#### 构建与上手

| 文档 | 说明 |
|------|------|
| [`current/02-构建与上手/快速入门.md`](current/02-构建与上手/快速入门.md) | 快速入门：命令行 / JSON-lines 会话 / wasm32 三条主路径（原 `QUICKSTART.md`） |
| [`current/02-构建与上手/构建指南.md`](current/02-构建与上手/构建指南.md) | 构建指南：引擎、CLI、wasm32、测试防线、脚本清单与排障（原 `BUILD.md`） |
| [`current/02-构建与上手/CLI使用手册.md`](current/02-构建与上手/CLI使用手册.md) | `vitro_cli` 使用手册（含 `serve` JSON-lines 协议契约与方法一览）（原 `VITRO_CLI.md`） |

#### 语言子集规范（行为契约）

| 文档 | 说明 |
|------|------|
| [`current/03-语言子集/C语言子集规范.md`](current/03-语言子集/C语言子集规范.md) | C 教学子集规范（支持语法 / C23 锚定 §2.10~2.12 / 排除清单 / 与 Clang 的已记录差异）（原 `C_SUBSET_SPEC.md`） |
| [`current/03-语言子集/C++子集规范.md`](current/03-语言子集/C++子集规范.md) | C++14 教学子集规范（面向学生/教师，含 Honest Subset 边界与模板活约束）（原 `CPP_SUBSET_SPEC.md`） |
| [`current/03-语言子集/C++拓展实施计划.md`](current/03-语言子集/C++拓展实施计划.md) | C++ 子集拓展实施计划（Stage 0~6 已完成；Phase 42 进行中的活进度载体）（原 `CPLUSPLUS_EXTENSION_PLAN.md`） |
| [`current/03-语言子集/CSharp前端引入计划.md`](current/03-语言子集/CSharp前端引入计划.md) | **C# 教学子集前端引入计划**（v3 定稿：ARC 降解 / 异常与栈展开 / CS0~CS6 批次；排期以 U 系列路线图为准，SharpTutor 锚定）（原 `CSHARP_EXTENSION_PLAN.md`） |

#### 标准库与测试防线

| 文档 | 说明 |
|------|------|
| [`current/04-标准库与防线/标准库支持矩阵.md`](current/04-标准库与防线/标准库支持矩阵.md) | 标准库支持矩阵（头文件 × 函数 × 实现层 × 验证状态）（原 `SUPPORTED_LIBC.md`） |
| [`current/04-标准库与防线/标准库架构与测试防线.md`](current/04-标准库与防线/标准库架构与测试防线.md) | 标准库四层架构（VM Builtin / Rust Host / Bytecode Libc）与测试设计（原 `STDLIB_AND_TEST_DESIGN.md`） |
| [`current/04-标准库与防线/影子验证框架.md`](current/04-标准库与防线/影子验证框架.md) | 影子验证框架（Clang 对照、门禁语义、提速设施、已知限制）（原 `SHADOW_VERIFICATION_FRAMEWORK.md`） |
| [`current/04-标准库与防线/学生错误用例集.md`](current/04-标准库与防线/学生错误用例集.md) | 学生常见错误测试用例集（⚠️ 人工整理的假想清单，未接防线；真实失败路径语料见裁定 G1）（原 `STUDENT_ERROR_TEST_CASES.md`） |
| [`current/04-标准库与防线/TODO注释规范.md`](current/04-标准库与防线/TODO注释规范.md) | 代码内 TODO/FIXME/HACK/SAFETY 标签与 `#DXX` 编号约定（原 `TODO_CONVENTION.md`） |

#### 统一模式、可视化与教学体验

| 文档 | 说明 |
|------|------|
| [`current/05-教学体验/统一模式设计.md`](current/05-教学体验/统一模式设计.md) | 统一模式 / 时间旅行设计（状态机、检查点、帧缓存、seek 契约）（原 `UNIFIED_MODE_DESIGN.md`） |
| [`current/05-教学体验/VM教学体验优势.md`](current/05-教学体验/VM教学体验优势.md) | 自研 VM 的体验优势（热力图 / 语义进度条 / 变量历史 / 异常回退）（原 `VM_EXPERIENCE_ADVANTAGE.md`） |
| [`current/05-教学体验/算法与数据结构教学设计.md`](current/05-教学体验/算法与数据结构教学设计.md) | 算法与数据结构支持总设计（模式识别 / 运行时验证 / 轨迹分析；G9 缺口权威证据源 §7）（原 `ALGORITHM_DATASTRUCTURE_DESIGN.md`） |
| [`current/05-教学体验/认知推理系统设计.md`](current/05-教学体验/认知推理系统设计.md) | 认知推理系统（根因分析 / 认知误区 / 知识图谱 / 意图推断，P0~P3 全部落地）（原 `COGNITIVE_REASONING_ROADMAP.md`） |
| [`current/05-教学体验/模板维护指南.md`](current/05-教学体验/模板维护指南.md) | 算法模板维护指南（目录结构、meta.yaml、占位符、生成链路；生成器已由 R4 G1 恢复）（原 `TEMPLATE_GUIDE.md`） |
| [`current/05-教学体验/模板与验证解耦设计.md`](current/05-教学体验/模板与验证解耦设计.md) | 模板与验证解耦方案（模板即合法 C + Clang Golden + 双重验证）（原 `TEMPLATE_AND_VERIFICATION_DECOUPLING.md`） |

#### 出口、协议与引擎决议

| 文档 | 说明 |
|------|------|
| [`spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](spec/STEP_PAYLOAD_SCHEMA_V0_1.md) | **StepPayload v0.1 语言中立协议 schema**（已冻结，S1–S5 签字回放 61/61；§9 v0.2 激活轨道、附录 B 受控词汇表） |
| [`current/06-出口与协议/CAPI评审回复与实现状态.md`](current/06-出口与协议/CAPI评审回复与实现状态.md) | capi 签名评审定稿（外部消费者诉求逐条回应 + 第一批 13 入口实现台账）（原 `VITRO_CAPI_REVIEW_RESPONSE.md`） |
| [`current/06-出口与协议/下游需求处置回执.md`](current/06-出口与协议/下游需求处置回执.md) | 下游需求清单处置与窗口表态（A/B/C/D 逐项回执；第二批 capi 窗口、三段式内存地图、会话语义）（原 `VITRO_DOWNSTREAM_REQUESTS_RESPONSE.md`） |
| [`current/06-出口与协议/堆有界隔离决议.md`](current/06-出口与协议/堆有界隔离决议.md) | 堆内存决议：bump 分配 + 有界隔离（三道墙；已拍板已实施，U2 不可破坏项）（原 `VITRO_HEAP_QUARANTINE_DECISION.md`） |

#### 质量、裁定与工作记录

| 文档 | 说明 |
|------|------|
| [`current/07-质量与裁定/统一整备路线图.md`](current/07-质量与裁定/统一整备路线图.md) | **统一整备路线图 U0~U7（排期权威）**：三语化 S 系列与重构评估 Phase 系列的合并执行方案（波次总览 / CS 硬门禁 / 防伪绿机制）（原 `VITRO_OVERHAUL_ROADMAP.md`） |
| [`current/07-质量与裁定/核心资产重构裁定.md`](current/07-质量与裁定/核心资产重构裁定.md) | **核心资产重构裁定 v1（独立裁定）+ 重构执行方案**：分区裁定 / 判据 J1~J10 / 候选对比 / 中止条件 / §13 五域执行方案（D1 防线自身、D5 工具链语言 Python→Go 迁移边界与双轨纪律）；**§14 JIT trace 路径 P0 静默错值**（嵌套纯计数循环被外层 trace 穿透；**归因修正为与 `long long` 无关**；根因 = JIT fast path 在录制期间未禁用；含四组双向验证实验与 `vm_bench` 两处方法学缺陷）；**§14.11 对重构范围的影响**：新增子域 **D6（JIT 加速器存废重裁：先校正 `vm_bench` 重测加速比 → 删 JIT 或收缩作用域）**、D1b 形状对抗生成、J10 前置到 W1——**裁定①（核心不重写）维持**。实测脚本与证据 JSON 在 [`scripts/core_asset_verdict/`](../scripts/core_asset_verdict/)（原 `VITRO_CORE_ASSET_RECONSTRUCTION_VERDICT.md`） |
| [`current/07-质量与裁定/三语化整备审计计划.md`](current/07-质量与裁定/三语化整备审计计划.md) | 三语化整备计划 v1.1（§1~§4 渗出证据链 / 防伪绿解剖 / 10591ad 事故裁定仍为权威记录；§5 批次表已并入 U 系列路线图）（原 `VITRO_TRILINGUAL_OVERHAUL_PLAN.md`） |
| [`current/07-质量与裁定/重构评估报告20260912.md`](current/07-质量与裁定/重构评估报告20260912.md) | 重构评估（§1~§4 权威证据：泄漏复发洞实锤 / 7 项动态探针 / 分模块风险清单；§5 计划已并入 U 系列路线图）（原 `VITRO_REFACTOR_ASSESSMENT_2026_09_12.md`） |
| [`current/07-质量与裁定/代码审阅与修复追踪20260906.md`](current/07-质量与裁定/代码审阅与修复追踪20260906.md) | 全面代码审阅报告（137 条发现）与四批修复追踪（**修复进度权威追踪**；0911 复核已闭环归档）（原 `code_review_report_2026-09-06.md`） |
| [`current/07-质量与裁定/实测发现登记20260913_性能与头文件.md`](current/07-质量与裁定/实测发现登记20260913_性能与头文件.md) | **实测发现登记（性能 + 头文件语义，2026-09-13）**：P-1~P-6（malloc churn 超线性 / 解释单步 35~41ns / 步数膨胀 18 步 / JIT 覆盖面 / 统一模式 58.5μs 步 / 基准方法学）、H-1~H-5（include 静默跳过零诊断、__has_include 口径不一、<> 宽松、存根遮蔽、wasm 降级）、D-1~D-4 文档漂移、跨平台备查——全部实测登记并经独立复核（§8），已挂接路线图批次（H-1/H-2 入 U1 第三批；P-1 完成 G6 复核一半，解锁 U2#2 开工前置） |
| [`current/07-质量与裁定/脚本埋雷验证记录.md`](current/07-质量与裁定/脚本埋雷验证记录.md) | **J9 台账**：三个判定型脚本（shadow_verify / ci_three_tier_check / serve_smoke）各一条"注入→必须红"埋雷实证——判定型脚本埋雷记录 = 0 时其全绿不得作为结论依据（W0-1 / U0#8 验收达成）；含 serve_smoke 边界批与两处脚本自身缺陷的发现记录 |
| [`current/07-质量与裁定/工作记录20260912_突变测试.md`](current/07-质量与裁定/工作记录20260912_突变测试.md) | 工作记录：影子防线突变测试首次实测（3/3 检出；M3 裕度=1 实证用例形状盲区）（原 `WORKLOG_2026_09_12_MUTATION_TEST.md`） |
| [`current/07-质量与裁定/实测发现登记20260913_性能与头文件.md`](current/07-质量与裁定/实测发现登记20260913_性能与头文件.md) | **实测发现登记（2026-09-13，登记未修复）**：性能域 P-1~P-6（三引擎同源基准量化：JIT 8.80×/7.92×、解释 35~41 ns/步、`malloc` churn 44.6 μs/次超线性、统一模式 58.5 μs/步；含"空载单线程非稳定条件"方法学告警）+ 头文件域 H-1~H-5（**两条 P0 新发现**：找不到头文件静默跳过零诊断、`__has_include` 与 `#include` 口径不一致）+ 文档漂移 D-1~D-4 + 跨平台备查 4 项；15 个 include 探针 × Clang 对照，含复现命令与外推边界 |
| [`current/07-质量与裁定/MoonBit迁移方案评估报告20260918.md`](current/07-质量与裁定/MoonBit迁移方案评估报告20260918.md) | **MoonBit 迁移方案评估（2026-09-18，只读评估 / 未执行任何改动）**：主语言 Rust→MoonBit、WasmGC 单出口提案的事前评估——纠正提案四处判断（快照"深克隆对象图"命题不成立、`MOONBIT_NEW_NATIVE=0` 语义有误、服务端 wasm 宿主缺证、头号风险实为 **VM 热循环性能**）、补三项未覆盖决策（JIT 归宿 / C++ 子集归宿 / 防线裸奔期）、成本重估（真成本 = 218 条 `*FAILURES.md` 知识资产归零，非代码）、A1~A9 **待实证假设清单** + 阶段 0 四道证伪门（门 0 LLM 生成效率 / 门 1 热循环性能 / 门 2 Wasmtime / 门 3 快照往返） |
| [`current/07-质量与裁定/MoonBit迁移_lexer模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_lexer模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_lexer` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：词法 + E2 模块化预处理器内核九节勘察——① 规模实测（14 文件 3,523 行；**预处理器 1,786 行 > 词法核心 1,737 行**）+ 116 变体 token 全量清单 + C23 特性核查；② 16 项可复用资产（含移植成本评级）；③ 16 项抛弃清单（`Vec<char>`+`splice` 寄生架构 / 哨兵指令 / 行号补偿 / 静默上限）；④ 在途工作逐条接纳（已修待搬 U1#6·U1#7·U1#11 + 已登记未做 14 项 + 范围外 2 项）；⑤ 8 条架构建议（独立 pass + LineMap / 双坐标 / 宿主 IO 抽象 / 类型分层）；⑥ **25 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现 3 条：`#if` 的 `\|\|` 短路不归一、`#if` 不支持位运算与三目、number token 列号偏移）；⑦ 8 个 MoonBit spike（S1 字节承载 / S2 溢出语义 / S5 宿主 IO 为**门禁级**）；⑧ 四层等价性锚点（token 流序列化 diff + 弱断言硬约束）；⑨ 4 包切分草案。**头号工程风险 = H-5（wasm-gc 无文件系统 → 自定义 include 在目标形态下整体不存在）；头号架构机会 = 项目自裁定的"预处理器独立成 pass + LineMap"** |
| [`current/07-质量与裁定/MoonBit迁移_shared与ast模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_shared与ast模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_shared` + `vitro_ast` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：诊断骨架与 AST 类型系统九节勘察——① 规模实测（9 文件 1,781 行；**两个 crate 各 0 个 `#[test]`**，唯一直接测试为 `ast_unit_test.rs` 2 例）+ `ErrorCode` 137 变体分段实测（**E5xxx 为空段**：仅范围映射落地，`下游需求处置回执.md` 的"已落地"口径需更正）+ `Expr` 26 / `Stmt` 16 / `Type` 17 全量清单；② 10 项可复用资产（含移植成本评级）；③ 7 项抛弃清单（`Box` 全族间接层 / `compute_type_size` 三副本含一个**从未编译的死文件** / 自相矛盾的 `Type::PartialEq` / `as i32` 脱类型范式 / 未被任何代码消费的 AST serde 派生）；④ 12 条在途工作逐条接纳（U5#4 / U7#4 / U7#7 / CS0·CS3·CS5 / T-P0-8）；⑤ 7 条架构建议（**错误码穷尽 match 单源 + `moon check -d` 门禁** / 类型相等与渲染各自单源 / `Bytes` 载体 / `SourceLoc` 列单位契约）；⑥ 8 条坑清单（现象→根因→修复→新语言是否复发）；⑦ 8 个 MoonBit spike（S1 溢出语义 / S2 137 臂穷尽与 `-d` / S6 UTF-16 列单位）；⑧ 6 个等价性锚点（**AST dump JSON 为首要锚**，与 lexer 报告的 L1 token 流锚点互补分层）；⑨ 3 包切分草案。**实测发现登记 M-0~M-13**（release 产物陈旧 3 天、`error_catalog` 覆盖 77/136、W/H 码在 serve JSON 契约内被打成 `E` 前缀、`\xff` 落成 UTF-8 双字节、非 ASCII 列号偏差 −4 独立复现）；含 §0.2 **记录对账（12 条既有登记）与三处自我更正** |
| [`current/07-质量与裁定/MoonBit迁移_codegen模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_codegen模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_codegen` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：字节码生成器九节勘察——① 规模实测（28 文件 6,237 行；`BytecodeGen` **47 字段**可变状态机 / `CompileOutput` 13 字段 / 产出 132 条 opcode 中仅引用 **127** 条）+ 无 IR 单遍发射形态；② 12 项可复用资产（含移植成本评级：132 opcode 表 / R1 布局纯函数 / 调用约定 ABI / Bytecode Libc 固定索引段与 3,485 条预编译产物可直接复用）；③ 11 项抛弃清单（`temp_slot0..3` 固定槽 + 3 个止血补丁 / 四张平行 HashMap 符号表 / `Vec<Instruction>`+`patch_jump` 事后回填 / 字符串前缀启发式类型判定）；④ 在途工作逐条接纳（U3#1 槽位分配器·U3#3·U3#7·U3#6/#9·C7·D08/D10 + **2 条本次新发现未登记活缺陷**）；⑤ 9 条架构建议（Layout Planner 纯函数 / LIFO 帧槽分配器含 diff 风险论证 / 两阶段发射 / 确定性发射器 / 内建指令预算）；⑥ **39 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现 2 条：**全局/静态 `char *p="…"` 静默错值**、**`b = *new A[2]` 赋值被静默丢弃且改写 `new[]` count 头**——两者均 clang++ golden 对照，五层防线全绿）；⑦ 9 个 MoonBit spike（**S2 已核实 `String::length` 是 UTF-16 code unit，直接照抄必错**）；⑧ 七层等价性锚点（**产物 JSON 逐字节 diff**，含实测确定性结论：`code` 段已确定、键序随进程随机 + 三条"必须冻结"）；⑨ 8 包切分草案。**头号结论 = 槽位机制在占用过度/不足/物理相邻/跨函数生命周期四个方向全出错（实测 8 条，裁定依据的"3 起"低估），且裁定"6 个月改判计时器"因前置项 U3#1 未落地而尚未启动** |
| [`current/07-质量与裁定/MoonBit迁移_diagnostics与algorithm_steps模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_diagnostics与algorithm_steps模块勘察报告20260918.md) | **MoonBit 迁移 · `diagnostics` + `algorithm_steps` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：结构化诊断 / 认知推理链 / 算法检测标注 / 语义补全九节勘察——① 规模实测（36 文件 8,748 行；`error_catalog` 77 卡片 / 概念图 **25 节点 25 边**（文档称 24 / 30+ 不实）/ 算法判据 **43** 路（文档口径 41、Phase 16 口径 27，三处不一））+ **接线实测：`misconception_patterns`/`learning_path`/`knowledge_graph`/`completion` 四个模块零出口、`data_flow`/`intent` 为孤儿**；② 15 项机制 + 14 项数据**严格分列**（数据层外置进度仅 1/5：仅 `error_catalog` 有 JSON 出口）+ 移植成本评级；③ 抛弃清单 R1~R7 / S1~S7 / D1~D8（`safe_byte_col` 坐标症状治疗、`cfg_has_back_edge` 块 ID 判据死代码、教学 trap 文案双轨、29 处悬空卡片/模板引用、`AlgorithmMatch` 双类型）；④ 13 条在途工作逐条接纳（U1#1①已落地·②③未做 / W4 登记项**疑似已完成** / **CSharp D5 裁决经实测成立、D6 两处代码依据已定位**）；⑤ 12 条架构建议（数据全面外置 JSON / 坐标口径单源 / 错误码穷尽 match / **文本启发改为执行事件** / trap 结构化 / 图邻接表 / `lang` 维度）；⑥ **19 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现：**`ERROR_CONCEPT_MAP` 3020→Recursion 映射错仍然在役**、**E3050 有修复无卡片 / E3070 无卡片**）；⑦ 9 个 MoonBit spike（S1 `@json` 规则表 / S2 邻接图 / S5 UTF-16 列语义 / S6 137 臂穷尽 match）；⑧ 9 层等价性锚点（**E6~E9 无出口 = 锚点缺口，须先在 Rust 侧补薄导出**，含算法标注 golden 37 模板 311 条 + `error_catalog` 逐字节 diff）；⑨ 9 包切分草案。**头号结论 = 认知推理链第二/三/四层与补全"有代码无出口"，迁移前须先补观测面，否则差分对照这条唯一退路对它们不存在** |
| [`current/07-质量与裁定/MoonBit迁移_unified模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_unified模块勘察报告20260918.md) | **MoonBit 迁移 · `native/src/unified/`（统一模式 / 时间旅行引擎）模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：九节勘察——① 规模实测（19 文件 **3,690 行**，占 `native/src/` 25.9%；模块内 28 单测 + 8 文件 74 集成测试）+ 调用面与**边界判读**（`CheckpointManager` 在 `vitro_vm` crate 却匹配教学语义字符串 = **vm 包反向依赖应用层词汇**；**seek 无 C ABI 导出，只有 serve 有**）；② 可复用资产（语义设计 8 / 测试 8 / 文档 5，含移植成本评级；**61 断言 replay + 57 断言 serve_smoke + 字段白名单可直接复用**）；③ 抛弃清单（`Arc`+COW 在每步快照下**已退化**、`pre_step_snap` buffer 复用只修了一半、`#[derive(Clone)]` 逐字段同步义务、take/put 舞蹈 / **差分编码无生产调用点而注释称已启用** / `trace` 死路径 / `Default` 的 0 取余雷点 / 孤儿文件 `compiler/ast.rs`）；④ 在途工作 16 条逐条接纳（U2#1 已完成待搬 / U2#6·U2#7 半闭 / **U6#4 未开工，实测残留仅 4 字段且同病第二实例 `Session.vm` 未被登记**）；⑤ 9 条架构建议（〔结构优化〕Trap 回退改**重放重建**消除每步 1MB memcpy / 持久差分帧 + 窗口丢弃 O(1) / 检查点链不变量升格为类型 / 派生状态惰性化；〔沿革保留〕窗口与检查点参数逐字保留；〔新设计〕**统一模式下 JIT 是纯浪费**须显式关停）；⑥ **17 条坑清单**（现象→根因→修复→新语言是否复发，含 63.6GB 事故两洞、seek 静默错内存、三处"钳位顺序"同族 panic、**未修的 `local_sym_map` 未重建**、**未修的 VFS 不入快照**）；⑦ 10 个 MoonBit spike（**S2 大数组滚动窗口在 GC 下的回收性 / S7 `.mbtx` 承载回放对账为必查项**，S3 数值转换与 S5 取余越界决定事故是否复发）；⑧ 等价性锚点（主通道 = **serve NDJSON**，diff 解析后 JSON 结构；E1~E6 可复用 + **M1~M6 必须改造**，含 RSS 护栏口径不可平移、白名单**只判多余不判缺失**、S5 A5 恒真 PASS）；⑨ 5 包 + facade 切分草案（**`CheckpointManager` 迁出 vm 包** + 依赖反转避免重演"与 Session 的耦合"）。**头号工程风险 = 每步一次 1MB 全量快照（换语言一分不省）；头号架构机会 = 窗口语义照搬但实现改 O(1)，并把 delta 设计升格为内存表示** |
| [`current/07-质量与裁定/MoonBit迁移_出口层模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_出口层模块勘察报告20260918.md) | **MoonBit 迁移 · 出口层（capi / serve / wasm，`native/src/capi/` + `session_api` + `vitro_cli serve`）模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：**被砍对象 capi + 保留蓝本 serve + wasm 接线面**九节勘察 + 特别交付宿主接口契约草案——① 规模实测（5 文件 **3,271 行**；capi 1,044 / session_api 702 / session 432 / vitro_cli 929 / session_ops 164）+ **导出 45 = 头文件声明 45 零缺口**（与评估报告 §5 第 5 条逐数吻合）+ `VITRO_ABI_VERSION` = **2.1.0**（`abi_version` 台账，as_of 2026-09-14，read_const）+ **`unsafe` 47 处（mod.rs 27 / first_batch.rs 20）的关键字计数严重低估真实义务面**（edition 2021 下另有 **约 40 处无关键字的隐式裸指针操作**：Session 裸解引用 33 / `CStr::from_ptr` 3 / `from_raw` 族 3 / `from_raw_parts_mut` 与裸指针写各 1）+ serve **21 方法** + **wasm 接线面实测 = `#[wasm_bindgen]` 全仓仅 1 处且只挂 panic hook**；② 12 项可复用资产（含移植成本评级：E-P1-5 输出通道三分为**宪法级**、JSON-lines 协议层、schema v0.1 + 10 用例冻结测试、Clang golden）；③ 抛弃清单 17 项（Rust 特有机制 6 / 症状治疗 6 / 组织债 5），含 **45 导出中仅 17 个被非 Rust 消费方引用**、`vitro_get_program_output_delta` **零消费零测试**、`get_engine_notes*` "必需但不消费"、serve 独有 5 项能力在 C 导出里**无对应符号**；④ 11 条在途工作逐条接纳（capi 第二批 4 + 第三批 5 函数 **实测 0 处源码出现** → 直接按目标架构实现；**`memory.regions` 的"过渡形态"标签因第二批取消而成终态，必须当场定型**）；⑤ 10 条架构建议（协议 schema 脱离 capi 独立 + 建字段冻结测试 / 单出口三宿主单协议 / 错误帧字段名 `kind` 定性 / `ping.abi` 重锚 / 产物新鲜度改常量对比）；⑥ **14 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现 2 条：**`wasm_smoke.js:45` 的 ABI 断言是空洞断言**（测指针非空、标签写"ABI 版本"）、**wasm 证据链断在文档**（3.75MB 与磁盘 3,602,162 B 及文档 3.44MB 三方不符且无机器断言；"安全检测在 wasm 下工作"库内零断言，证据源为未入库手工脚本））；⑦ 8 个 MoonBit spike（**S1 字节流语义 + S6 wasm-gc 导出面形状为门禁级**）；⑧ 9 层等价性锚点（**协议帧 diff 与提示词 8 共用**，落地方案 = 把 `serve_smoke` 期望值外置为 `protocol_frames.jsonl`，兼补"serve 无字段冻结测试"缺口）；⑨ 3 包切分草案（`vitro/session` / `vitro/protocol` / `vitro/host`）+ **宿主接口契约草案**（wasm-gc 函数面从 45 个压到 **4 个**：`invoke` / `reset` / `protocol_version` / `engine_version`；21 方法逐条标注现役语义来源 file:line 与裁剪判定；帧契约 9 条逐条保留理由）。**头号判断 = wasm 出口没有独立接线面可迁移（暴露的就是 C ABI 原样）——对迁移是净利好，但须背上 3.75MB 体积与安全检测两项未偿证据债** |
| [`current/07-质量与裁定/MoonBit迁移_cpp_frontend模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_cpp_frontend模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_cpp_frontend`（C++ 子集，Phase 31~42）模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：为评估报告 §3.2「C++ 子集搬或砍须单独裁定」提供裁定材料——**§0 先修正四处前提**（该 crate 仅 186 行 Rust + 502 行 JSON，**实为内置容器布局加载器而非 C++ 前端**；**U3#2 是变参 8 字节中转、与类类型模板实参无关**，后者实为 U3#5/U3#8；4 例 `clang_compile_fail` **有** golden 且系手写，真正缺 golden 的是另 4 例；"历史累计约 40 条"机械复算不可复现，人工归类 **20 条**）；① 规模实测（C++ 专属 **≈6,514 行 = 全仓 8.5%**；**真正独立文件仅 `parser/cpp.rs` 223 行**，其余全部寄生共享文件；`is_cpp_mode` 43 处/9 文件**全在 lexer+parser**，typeck/codegen/vm 零处）+ **VM 侧 C++ 感知度实测 = 0**（仅 3 处无关命中）；② 10 项可复用资产（含移植成本：**布局加载器零硬编码已验证**且与 `.cpp` 真源逐字段无漂移 / 99 例现场 clang++ 防线 / C++ 子集规范 409 行 / 容器 AOT 预编译 + digest 门禁）；③ 9 项抛弃清单（**生成器 `extract_cpp_builtin_layout.py:22` 输出路径指向 crate 化前老路径、无 drift 门禁**（U7#7 已登记）/ **83 处字符串命名合成实体散写 11+ 文件**、codegen 缺符号时静默不生成 / `placeholder_class` 空壳 hack 靠调用方纪律 / 15 处死 token·死 AST 变体·死字段 / `bytecode_libc_sig.rs` 手写 60 条签名）；④ 13 条在途工作逐条接纳（U3#1·#3·#4·#5·#6·#7·#8·#9 / U7#7 / D08·D10 / 函数式构造存量缺陷 / lambda 双 E3014）；⑤ 7 条架构建议（**用类型替换字符串 = 核心建议**〔结构优化〕/ 空壳不可表示 / 无条件 Pass 改显式阶段 / 静默缺符号改 fail-loud）；⑥ **12 条坑清单**（现象→根因→修复→新语言是否复发；**A 类"多 Pass 顺序/A 类跨层语义失配"占 4 条**）；⑦ 9 个 MoonBit spike（**S1 已实证 MoonBit `Int` 静默回绕而 Vitro 解释器六算子全部 trap——直接照抄必变静默错值**；S2 1MB 字节载体为门禁级；F2 `String` 为 UTF-16 LE、F3 `Bytes` 不可变，均直读本机 core 源码取证）；⑧ 七层等价性锚点 + **现有锚点四处必须修补**（4 例无 golden 致 stdout 不比对、4 例 golden 系 Vitro 自证属循环论证、`category: gap` 使回归不触红）；⑨ 5 包切分草案（`cpp_layout` 为唯一可整包平移项）；⑩ **砍/搬两方案影响面与规模估算**（砍 −6,500 行 = −8.5%，但 **VM 收益 0 行**）——**头号结论 = C++ 与 VM 完全解耦，但项目已裁定 CS2 直接复用 C++ 铺路区（类布局/this 注入/单态化/raii/容器加载器），砍 C++ 与该既定策略直接冲突**；含附录 A **8 项待证清单**与附录 B 勘察边界（2 个子代理中止、其范围已由第一手读码覆盖） |
| [`current/07-质量与裁定/MoonBit迁移_编译管线与会话层模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_编译管线与会话层模块勘察报告20260918.md) | **MoonBit 迁移 · 编译管线与会话层模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：`native/src/engine/`（compile_pipeline / session_ops / completion）+ `session_api.rs` + `session.rs` + `vitro_cli` **六子命令与管线耦合深度** + 对照出口 `capi/` 九节勘察（与 `出口层模块勘察报告` 范围在 capi/serve 处重叠，本报告另覆盖 **管线编排与 CLI 其余四子命令**，两条独立路线实测值一致：serve **21 方法**、capi 导出 **45**、`abi_version` **2.1.0**）——① 规模实测（本模块 ≈5,200 行；**`compile_pipeline.rs` 793 行中 ~340 行是两份管线的互相复制**，`run_compile_pipeline` **生产代码零调用**且语言检测被硬编码为 C 模式 → C++ 在单文件管线下不可达）+ **语言分派实测**（唯一检测点 `:631-634`；穿透面 28 处；**两个形似 bool**：`is_cpp_mode` 与语言无关的 `is_library_mode`）+ **`"main.c"` 兜底全仓 54 处实测分布**（生产 **7** / tests 46 / bench 1）逐处给出 CS0 改造方向，并登记文档口径差异（`CSharp前端引入计划.md:29` 称 `:603` 实为 `:631-634`；称 9 处实为 7 处）；② 18 项可复用资产（含移植成本评级；`setup_vm` **12 步**、serve 21 方法表、`session.reset` 四字段白名单、`split_stdin` 保留换行口径、**`serve_smoke` 57 断言驱动原样可复用**、`bundle.json` schema）；③ 21 项抛弃清单（`catch_unwind` 三层护栏 / take-归还模式 / `Arc` 三处 / **`unified/stream/` 差分编码整体死代码**（全仓零引用而注释仍称"跨 FFI 已通过其编码"）/ re-export 垫片 / 45 个 C ABI 导出）；④ **在途状态实测：本模块零未提交改动、纯"已登记未做"**——16 条逐条接纳（**U0#7 C ABI 契约止血与"砍 capi"决策直接冲突，须与重写是否开工同批拍板** / U2#12·U6#4·U7#4 / U5#5 / 诊断 `E` 前缀仅在目标架构做对）+ 4 处台账状态滞后登记；⑤ 10 条架构建议（`SourceLang` 显式会话输入 / 双管合一 / **协议层表驱动 dispatch** / 错误帧两层解耦 / `Bytes` 与 C 字节流正面对决 / `SourceMap` 一等公民 / `SessionConfig` 消除 reset 白名单）；⑥ **18 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现：capi 注释称 rust-alloc 实为租借指针、**`session_api` 内 9 处裸 not_compiled 帧形状不一致**）；⑦ **9 个 MoonBit spike（本次现场实测 6 项）**：**S1 溢出语义已实测回绕**（`Int` 静默回绕 / `-7/2=-3` 与 C 一致）、**S2 `String` 索引为 UInt16 code unit**、**S4 JSON 浮点序列化已实证不等价**（`1.0`→`1`、`1e20`→全数字、`-0.0`→`0`）、S5 stdin（**`moon run -` 占用 stdin；`@env` 无 stdin API**）、S6 退出码（正常 0 / `abort` 与未捕获 `raise` 均 1）、S9 checked error（待证 `abort` 可否 `try/catch`）；⑧ 三层等价性锚点 + **`bundle.json` schema 可沿用性裁定**（12 字段实测 / Instruction 是 `{op,operand,loc}` 且 `op` 为字符串名 / **3 处语义非中立** / 2 处形态差异 / `--check` 口径边界）+ 3 个锚点缺口（补全快照无出口等）；⑨ 17 包切分草案（P17 `vitro_capi` 砍掉）。**头号等价性风险 = JSON 数字序列化格式差异：迁移期禁止对 `bundle.json` 做逐字节 diff**（缓解事实：当前产物 `f64_constants`/`i64_constants`/`string_data`/`globals_init_64` 实测**全为空**）；**头号架构机会 = `session_api` 只覆盖协议语义约 3/4（`config.set` 写入侧 / `session.create`·`destroy` / 错误 `kind` 映射三块仍在 `vitro_cli.rs`），且 CLI 四子命令全部绕开它——补齐并下沉为 `vitro_protocol` 是本模块唯一剩下的价值主张** |
| [`current/07-质量与裁定/MoonBit迁移_scripts与测试防线模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_scripts与测试防线模块勘察报告20260918.md) | **MoonBit 迁移 · `scripts/`（Go 驱动集）+ `native/tests/`（测试与防线）模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：golden 与防线资产盘点 + 驱动链重建方案九节勘察——① 规模实测（Go **21 文件 9,922 行**；`native/tests/` 49 个 `.rs` / **900 个 `#[test]`** / 用例 758 / golden **733 个 `.out`**）+ `reports/facts.json` 14 键逐项实测值 + 影子报告明细（675 = 668 match + 3 known_issue + 4 gap_extension；C++ 99 = 95 + 4 `clang_compile_fail`）；② 15 项可复用资产（含移植成本评级）+ **`pyrandom` 角色判定 = 测试侧宿主 RNG，非引擎侧 `rand()`**（引擎 `rand` 走 golden `srand_rand.c`，刻意只测"同 seed 可重复"不测数值；wasm-gc 下 `@env.rand_internal` 恒 false = 无熵源）；③ 10 项抛弃清单（ctypes/DLL 绑定层 / psapi / Go flag 陷阱 / 展示视图口径 / 约 240 个白盒单测 / 展示视图与 E-P1-5 两套口径并存）；④ 10 条在途工作逐条接纳（**8 处台账数字漂移** / 4 个新 C++ 用例缺 `.out` golden / 6 个 baseline 无 golden 属语义正确 / `KNOWN_BASELINE_COMPILE_FAILURES` 缺反向测试 / 3 个台账对门禁不可见 / golden 生成器 `sync_templates.py` 未随 D5 迁移 / C++ 驱动缺瞬态重试与缓存）；⑤ 16 条架构建议（8 条〔沿革保留〕含"六类隐性口径"逐条落位 / 4 条〔结构优化〕 / 4 条〔新设计〕，含 **`spectest.print_char` 码点语义为必接隐式契约**）；⑥ **12 条坑清单**（现象→根因→修复→新语言是否复发：并行顺序敏感静默绿 / `pathlib` casefold 平台差异 / 同名 exe 映像竞态 / DLL 并发堆损坏 / 瞬态重试落缓存 / `ExitError` 语义错位 / 清洗假阳性 / 陈旧产物假绿 / stdin 未喂虚假 match / golden 生成与消费口径不一致 / 文档数字腐坏 / 裸奔期静默错值）；⑦ 8 个 MoonBit spike（**S1/S2 已实测**：wasm-gc 产物 imports 仅 `spectest.print_char`、exports 仅 `_start`，回调收 Unicode 码点；`@env` 在 wasm-gc 下无任何宿主实现）；⑧ 八层等价性锚点 + **迁移期防线重建里程碑表 M-0~M-9** + **裸奔期最小防线 = 24 例**（选例五标准 + 三条防假绿纪律，含逐例实测 `.out` 字节数）；⑨ 8 包切分草案（依赖单向无环 + 公开/不公开形态分工 + `.mbti` 作为"砍 C ABI 后对外义务的新载体"须阶段 0 裁定）。**头号工程风险 = 裸奔期静默错值；头号可复用资产 = 733 个 Clang golden（不依赖实现语言）** |
| [`current/07-质量与裁定/MoonBit迁移_typeck模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_typeck模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_typeck` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：为工程债务 D14/D16 所在模块提供裁定材料——**§0 先修正三条任务前提**（`is_cpp_mode` 在 typeck **出现 0 次**，全仓 46 处**全在 lexer/parser/compile_pipeline**；第四职责不是「语言分派」而是 **AST→AST 改写（lowering）**——实测 **88 处 AST 字段就地赋值**；**D14 代码侧已修复、台账未销项**（`decl.rs:46/69/706` 已 let-else，全 crate `unwrap()` 命中 0，旁证 health 报告「生产代码 unwrap/expect」= 1 且非 typeck））+ 顺带纠正 **U3#2 不在 typeck**（`temp_slot` 全 crate 0 命中，实修点在 `vitro_codegen/src/expr/call.rs:172-201`）；① 规模实测（35 文件 **8,816 行**；`decl.rs` 862 总行 / **842 非空行**；`TypeChecker` **27 字段**可变状态 + `TypeError` 无 `file_id` 无 span + `check()` **13 个 Pass**；定位到语言分派的 6 类隐式判据 A~G（含 `Type::Class` 19 处、名字前缀 6 处）；**78 个诊断码** / 253 处引用；**`vitro_lexer` 为幽灵依赖**、crate 无 `[lints]`）；② 16 项语义设计 + 7 项测试 + 4 项文档资产（含移植成本评级；**容器布局 JSON 5 个 POD 类 + Clang golden + U3 红→绿双锚可直接复用**；C++ shadow 22 条**内嵌于 Go 驱动**须先外置）；③ 抛弃清单 R1~R8 / S1~S8 / O1~O11（**`format!("{:?}", expr)` 参与 mangling = 源码位置污染缓存键且不可跨语言复刻**、`instantiated_class_names` 等「已实例化」概念 **5 种实现**、`placeholder_class` 空哑 decl、`char_narrow_suppress` 隐性开关、签名 **4 套真相源**（`printf`/`putchar`/`strcpy` 实测冲突）、**895 行 Rust 硬编码容器合成**致 Phase 41「零硬编码」口径不成立）；④ 在途工作逐条接纳（**D14 应销项而非修**；D16 现 842 仍超 800 且「唯一超标者」已不成立（896/861/842/840）；U3#4 修复后搬但须改语义（现按**收敛轮数**而非实例化深度计数、诊断落 `0:0`）；**U3#5 的归宿是让病灶机制消失**；U3#1/#3/#7 **越界属 codegen 不搬**；U3#6 mangled 单源须做成共享包；U3#9 与函数式构造按目标架构实现；U6#4 布局缓存）+ **7 条本次新发现未登记缺陷**（ctor 同 arity 异类型重载误判 E4031、方法重载同分静默取先声明、`int a[][3]={…}` 首维推断错、`char s[]="…"` 丢边界检查等）；⑤ 9 条架构建议（**用 4 条代码事实论证「单态化改 IR 层纯函数变换 + 显式 `Map[InstKey, InstId]` 缓存」可行，并承认固定点不可取消而须降级为数据结构上的固定点** / `SourceLang` 三层改造点清单（L1 的 46 处不在本模块）/ 诊断结构化 / 数组尺寸单表达 / **沿革保留 `type_mangle_suffix` 并标注「有重载才带后缀」破坏已落库产物之风险**）；⑥ **17 条坑清单**（现象→根因→修复→新语言是否复发，含 **HashSet 迭代序致 Bytecode Libc 产物布局不可重现** 的历史实锤，并据本机 core lib 源码指出 **MoonBit `Hasher` 默认种子 wasm/wasm-gc = 0 而 native/llvm/js 随机**、`HashMap::iter` 文档明写 in unspecified order → **跨宿主对账必红，须改用 `LinkedHashMap` 或显式排序**）；⑦ 8 个 MoonBit spike（S1 Map 迭代确定性**已可从源码预判会红**；S2 递归深度与 S6 AST 就地改写为真未知、S6 决定 §⑤-1 可行性）；⑧ 8 个等价性锚点（**首要缺口 = 全仓无 AST/符号表 dump 出口**，但 AST 已具备 serde 派生故成本低；E1 诊断序列 + E2 符号表 + E3 mangled 名集 + E4 类型化 AST；C++ 用例单独对 Clang++ 一路）；⑨ 8 包切分草案（**新增 `vitro/names` 作为 mangled 名唯一产出口并令 parser 同依赖**、`cpp` 与 `typeck` 反向耦合须解开、容器两路合流为同一数据驱动形态）。**头号工程风险 = 语言分派与魔名契约跨 3 个 crate 散写（26 处裸 `format!` + parser 独立造名），且四个「不报错只错值」类缺陷（嵌套类型静默覆盖 / 容器重复实例化 / 常量折叠溢出 / 数组尺寸半变异）全部落在裸奔期最危险区；头号架构机会 = 用显式实例化缓存把「多轮重跑到收敛」整体删除，从而一并消灭 U3#5、占位 decl 与 5 处查重** |
| [`current/07-质量与裁定/MoonBit迁移_vitro_vm与vitro_runtime模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_vitro_vm与vitro_runtime模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_vm` + `vitro_runtime` 模块勘察（2026-09-18，只读勘察 / 未执行任何改动）**：**唯一同时承载门 1（VM 热循环性能，决定生死）与门 3（快照往返）的模块**九节勘察 + **§0 先纠正三处任务书前提**（"C 有符号溢出 = wrapping"**错误**——解释器六算子全部教学 trap、仅 `UAdd/USub/UMul/UNeg` 是 wrapping；`source_digest` **codegen 完全不参与**，由 Go 脚本注入且 Rust 运行时不校验；**无 `HostFuncId` 枚举**，纯 u32 match、未知 id 静默 no-op）——① 规模实测（40 文件 **10,901 行** = 全仓 14.3%；`OpCode` **132 条**（值域 0–43/50–110/111–129/130–137，空洞 44–49，**最大 137 而 `opcode.rs:18` 注释仍写 128**）；`VitroVM` 36 字段；**`VMSnapshot` 22 字段逐字段列出** + RuntimeSnapshot 11 + MemorySnapshot 8；host funcs **110 个四路计数互证**（常量/分派臂/handler/名字模式）+ **20 个 handler 被 Bytecode Libc 固定索引遮蔽**（同族分裂：`atoi` 走 Bytecode 而 `atof` 走 Host、`iscntrl` 走 Bytecode 而 `isgraph` 走 Host、`memset` 三名同实现）；Bytecode Libc 3,485 指令 / 88 函数 / 索引段 1000–1087 / **用户起点 1089（1088 为空洞）**；1MB 内存 **8 类访问模式逐类列出**（含"每次受检访问 = 边界检查 + BTreeMap 区间查询 + 手工字节装配 + 脏页位图"的固定开销链））；② 可复用资产 11 机制 + 9 测试 + 5 文档（含移植成本评级）；③ 抛弃清单（Rust 特有 9 / 症状治疗 9 / 组织债 5，含**函数指针地址比较判 generic**、`global_count` 恒 0、`OpCode::Strlen` 死路径、逐字节零初始化、错误码字符串协议）；④ 在途工作**已完成 11 项不搬 + 未完成 13 项逐条接纳** + **迁移阻塞点 = C ABI 无 JIT 开关导出 ⇒ parity 的"JIT 关"一路当前不可达**；⑤ 架构建议（**单一 `checked_access` 入口** / 脏页改移位 / 批量走段级前置检查 / **快照不得逐字段枚举** / 穷尽 match 132 / **JIT bulk 必须补 StepEvent 记账** / **输出通道改 `Bytes`** / `freed_logs` 改排序数组+二分）；⑥ **18 条坑清单**（现象→根因→修复→新语言是否复发，含本次新发现 6 条：**JIT `tpl_add/sub/mul` 是 `wrapping_*` 而解释器同三算子教学 trap = 现存未修复未覆盖的静默错值**（附判定性用例）、**JIT 生效区间丢失 StepEvent → 断点失效/热力图漏计**、`strlen(NULL)` 静默返回 0、`sprintf/snprintf/sscanf` 缺栈深守卫、6 个字符串函数"越界当 0"软夹紧、**输出通道非字节保真**（`putchar(200)` 出 2 字节 `C3 88`））；⑦ **10 个 MoonBit spike**（载体 (a)(b)(c) × 8 类访问模式写法表 + 四条判定标准；**spike-4 溢出与 spike-10 字节分工已被同批报告实测，本报告回填并收缩剩余范围**；**packed 表示与每字节 ns/op 吞吐是全批 10 份报告的共同空白 = 门 1 最终空白**）；⑧ 等价性锚点（**三联 diff = stdout 字节级 + 返回码 + 最终 1MB 内存映像**，30 例选取矩阵；**"单步事件流"现版无导出形态必须新建**，附文本行协议草案；指出 `load_golden` 缺失静默跳过的弱化点）；⑨ 6 包切分草案（`core_types → memory → host → vm` + `bytecode`/`jit` 旁支，6 条包规格约束）。**头号结论 = 门 1 成败全在载体（`FixedArray[Byte]` 是否 packed、批量 API 是否存在）、门 3 结构上可行但 `heatmap` 每步更新使"不可变替代 Arc"不成立；两项 JIT 缺陷（溢出语义分歧 / 观测丢失）在 MoonBit 侧必然复发，除非 parity 防线先于 JIT 实现落地** |
| [`current/07-质量与裁定/MoonBit迁移_parser模块勘察报告20260918.md`](current/07-质量与裁定/MoonBit迁移_parser模块勘察报告20260918.md) | **MoonBit 迁移 · `vitro_parser` 模块勘察（2026-09-18，只读勘察 / 只读探针 / 未执行任何改动）**：递归下降语法分析器九节勘察 + **8 条 MoonBit spike 全部实跑**（本机 `moon 0.1.20260915`）——① 规模实测（10 文件 **4,720 行**；**76 个 `parse_*` 定义点**含 7 个深度壳配对 / **21 帧优先级瀑布** / `DeclaratorGuard` 4 保险丝 / `Rollback` 三元快照 / **20 处回滚点** / crate 内零测试）；② 12 项可复用资产 + **21 个语法族逐条实测**（20 通过，唯一失败 **K&R 旧式定义 E2005**；**`stdarg` 全族不在 parser**、实现在 lexer 宏层）；③ 抛弃清单 X1~X8 / Y1~Y6 / Z1~Z4（`mem::take` 子流 hack / `mem::forget` / 7 处人工挂点 / `suffix_count` 死字段）；④ 12 条在途工作逐条接纳（**A1 本次新发现活缺陷** / A2 死字段 / **A5 回滚面缺口实测降级为"不可观测"**）；⑤ 12 条架构建议（深度防护单点收口 / 预算覆盖 `Type` / 不可变解析环境 / 进度单调性组合子 / **字节长度必须走 UTF-8 编码器**）；⑥ **13 条坑清单**（现象→根因→修复→新语言是否复发），含本次实测新发现 **声明符类型通道无深度防护 → 合法 C 代码栈溢出崩溃**（**1200 通过 / 1300 崩**、零诊断；两组反证分别排除下游 typeck 与 `node_cross_count`）；⑦ 8 个 MoonBit spike **实测结果**（**S1 栈深度：wasm-gc 854 / js 538 / native 1005 层 vs Rust 现自限 62 层（余量 8.7×~16×）**，崩溃形态与 Rust 逐位相同 `0xC00000FD` 且不可捕获；**S5 `String::length`=UTF-16 码元（"中文"=2）、`to_bytes`=UTF-16 字节（=4）、`@utf8.encode` 才是真字节（=6）**；**S6 `Int` 静默回绕 + 除零 trap**；S7 GC 无递归释放但递归遍历 1e5 崩；**S8 `moonc v0.10.13` link 阶段 ICE 一手实证**）；⑧ 7 层等价性锚点（**AST dump JSON 为首要锚** + 错误诊断序列 diff + 病态输入"同等拒绝"锚）；⑨ 4+1 包切分草案（含 4 条关键边界裁定）。附录 A 实测原始数据 / 附录 B 未闭合项 6 条 / **附录 C 更正记录 C-1~C-6（含"阶梯扫描读最后一行"的方法论错误自纠：stdout 缓冲使阈值滞后——口径错了数字就是假的）** / 附录 D 与姊妹报告口径交叉核对 |
| [`current/07-质量与裁定/MoonBit迁移蓝图v1_20260918.md`](current/07-质量与裁定/MoonBit迁移蓝图v1_20260918.md) | **MoonBit 迁移蓝图 v1（2026-09-18，决策文件 / 第 14 号汇总代理产出 / 未执行任何改动）**：13 份模块勘察报告的汇总整合——① 模块结论合并表（13 行 × 资产/包袱/spike/最大风险）；② 全量 spike 去重清单（**9 条已实证事实 F1~F9 直接采信** + 16 项待测按序编排，含门 0~3 判定标准与冲突消解项）；③ 在途工作接纳矩阵（**迁移前 Rust 侧必修 7 项 P1~P7** / 按目标架构 30+ 项 / 放弃 12 项 / 待证 6 项）+ **13 处跨报告冲突裁决 C1~C13**（partial_match 级别、StepPayload 字段数、"三分之一量级"纠正为 8.5%、**C9：v1 范围 = C only、C++ 延后 S9 裁定但包边界第一天预留**、**C10：JIT 倾向性默认不搬**（三条依据，最终 S9 复核）、**C12：驱动层 v1 保留 Go、Node 宿主为新增薄层**）；④ 包切分总图（L0~L9 十层依赖 DAG，`.mbti` 取代 ABI 版本化成为对外义务载体）；⑤ 差分对账分级锚点体系（A 字节级/B 结构化/C 端到端/D 三联 diff + 三条冻结 + "排序义务在 Go 侧"显式继承）；⑥ 裸奔期最小防线（24~26 例候选表 + M-0~M-9 里程碑 + 三条防假绿纪律）；⑦ 差异台账 v0（`DIFF-<域>-<序号>` 编号制 + 14 条初始条目 + 19 项 capability_flags 入 JSON 单源）；⑧ 风险登记册（A1~A9 对接 + 新增 R1~R12）；⑨ 里程碑切片 S0~S9 + 全量切换（绞杀者节奏：每片 = 重写进度 + mooncakes 发布 + 押注检查点，锚点未全绿不发版）。**形态裁定（项目所有者已拍板）：同仓绞杀者渐进迁移（Rust 版冻结为差分 oracle 不删）+ 不赶 9 月黑客松按自有节奏迭代**。**同日修订**：项目所有者裁定"实测大于脑测、禁止基于二手摘要下结论"后启动逐份亲读复核（9/13 完成），补入 **0d release 重建前置**（双报告互证：陈旧产物威胁门 1 基线合法性）、**H6★ 宿主 IO 门禁级 spike**、**P7 AST dump 出口新建**、serve 字段冻结测试、C3 裁决更正等 13 处修正——全部带"本轮亲读"标记；复核笔记见配套文件 |
| [`current/07-质量与裁定/MoonBit迁移蓝图v1_第一手复核记录.md`](current/07-质量与裁定/MoonBit迁移蓝图v1_第一手复核记录.md) | **蓝图 v1 第一手复核记录（2026-09-18）**：对 13 份勘察报告的逐份全文亲读复核（**9/13 完成**：VM/codegen/typeck/scripts/lexer/parser/shared+ast/出口层/unified；剩余 4 份下轮）——每份四件事：证据等级分类（现场实测/静态亲读/工具链实测/待证）、蓝图论断逐条核对、关键事实抽查、新发现；**记录了相对初稿的 6 条关键更正**（release 产物陈旧威胁门 1 基线、C3 裁决不成立系初稿臆断、lexer 静默错值计数不准、宿主 IO 门禁级 spike 遗漏、serve 字段冻结测试缺失、AST dump 出口工作项缺失）+ 下轮 4 份报告的复核焦点 |
| [`current/07-质量与裁定/INCIDENTS/README.md`](current/07-质量与裁定/INCIDENTS/README.md) | **事故归档制度与索引**（模板 + 归档规则：任何 GB 级资源事故必须归档，与 CHANGELOG 分工；在档：[seek 重放泄漏](current/07-质量与裁定/INCIDENTS/事故202609_Seek重放泄漏.md)） |

---

### 📁 [spec/](spec/) — 语言中立协议

对外承诺的 wire format 定义，与任何前端实现解耦。当前：

| 文档 | 说明 |
|------|------|
| [`spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](spec/STEP_PAYLOAD_SCHEMA_V0_1.md) | StepPayload v0.1（**已冻结**，2026-09-12，S1–S5 签字回放 61/61）；§9 v0.2 激活轨道、附录 B 受控词汇表 |

---

### 📁 [archive/](archive/) — 历史归档文档

存放**已完成、已废弃或对象已不在本仓库**的历史文档，仅供追溯：

> 命名约定：2026-09-11 起新归档统一加 `ARCHIVE_` 前缀并在标题下写入归档横幅（含归档原因与日期）；
> 2026-09-13 起归档名同样中文化；更早期的归档文件保留原名（如 `FLUTTER_MIGRATION_PLAN.md`、`REVIEW_2026-06-14.md`）。

- 前端时代的迁移与构建（MAUI → Flutter、Flutter 构建脚本、web 部署、前端 UI 设计）
- 历史代码审查报告与事故复盘
- 已完成的实现计划（double / 函数指针 / 多文件编译 / 内存扩容 / 递归类型重构 / 指针复合赋值等）
- 一次性评估报告与工作记录

**2026-09-13 本次归档**（归类翻新，逐个取证后判定）：

| 归档文件 | 原名（docs/current/） | 原因 |
|------|------|------|
| `ARCHIVE_数据结构模板路线图.md` | `DATASTRUCTURE_TEMPLATE_ROADMAP.md` | P0/P1/P2 三批次全部完成，使命耗尽；维护现状由《模板维护指南》承担 |
| `ARCHIVE_零侵入可视化设计.md` | `ZERO_INTRUSIVE_VISUALIZATION.md` | 检测器从未在后端实施、渲染层已随前端切割迁出；缺口记录见路线图 G9 |
| `ARCHIVE_C++容器模板迁移笔记.md` | `STAGE2B_CPP_CONTAINER_TEMPLATE_NOTES.md` | 迁移已完成（Phase 34/41）；两条活约束已回填《C++子集规范》§4.4（G13） |
| `ARCHIVE_BytecodeLibc产品化.md` | `BYTECODE_LIBC_PRODUCTIZATION.md` | §九验收标准 7/7 全部实现，项目完结；已知限制可入内追溯 |
| `ARCHIVE_代码审查复核20260911.md` | `code_review_report_2026-09-11.md` | 外部 PR 12 项复核与批次 A~H 修复全部收口；留痕由 CHANGELOG 承接 |
| `ARCHIVE_工作记录20260911_Shadow提速与Phase1.md` | `WORKLOG_2026-09-11_SHADOW_SPEEDUP_AND_PHASE1.md` | 四项工作全部合入 CHANGELOG / spec / CLI 手册，遗留项闭环 |
| `ARCHIVE_语义单源审计20260912.md` | `R3_MULTI_TRUTH_AUDIT.md` | R3 批次验收线即本清单归档；保留项已归属 CS0/CS5/R4 |

**2026-09-11 归档**（前端切割后）：

| 归档文件 | 原因 |
|------|------|
| `ARCHIVE_BUILD_SCRIPTS.md` | 所描述的 Flutter 构建脚本已全部移除 |
| `ARCHIVE_CI_FAILURES.md` | FRB / Android CI 故障载体已随 CI 收缩消失 |
| `ARCHIVE_code_review_report_2026-06-13.md` | 审阅范围含前端，已被 09-06 / 09-11 报告取代 |
| `ARCHIVE_CIDE_MOBILE_TEACHING_THREE_LANGUAGE_PLAN.md` | "移动端优先"定位已被后端主计划取代 |
| `ARCHIVE_CPP_BUILTIN_LAYOUT_DECOUPLING_PLAN.md` | 布局解耦已完成（Phase 41） |
| `ARCHIVE_DATASTRUCTURE_SYNTAX_ROADMAP.md` | 语法拓展已完成（Phase 27） |
| `ARCHIVE_IMAGE_INPUT_INTEGRATION_PLAN.md` | 依赖已移除的前端与 OCR 能力 |
| `ARCHIVE_LOCAL_PERSISTENCE_PLAN.md` | 方案载体（Dart 运行时）已迁出 |
| `ARCHIVE_M7_BETA_READINESS.md` | 里程碑评估已被 Phase 34~42 超越 |
| `ARCHIVE_PANEL_DRAG_GESTURE_DESIGN.md` | 前端交互设计，宿主已迁出 |
| `ARCHIVE_PHASE_KR_LEETCODE_TEST_PLAN.md` | 计划已达成（K&R 69 绿 / LeetCode 138 通过） |
| `ARCHIVE_POINTER_COMPOUND_ASSIGN_PLAN.md` | 已全链路支持（2026-06-28） |
| `ARCHIVE_RECURSIVE_TYPE_SYSTEM_REFACTOR.md` | 重构已落地于 `vitro_ast` |
| `ARCHIVE_S6_READINESS_ASSESSMENT.md` | 阶段评估已被后续里程碑覆盖 |
| `ARCHIVE_SHADOW_VS_CI.md` | 立论前提（特性缺失期）已消失 |
| `ARCHIVE_WEB_DEPLOYMENT_CLOUDFLARE_AND_WASM_INTEGRATION.md` | Flutter Web 部署路径作废 |
| `ARCHIVE_WEB_DEPLOYMENT_GITHUB_AND_GITEE_PAGES.md` | 双 Pages 部署围绕已删除产物构建 |

> ⚠️ **archive/ 中的文档仅供追溯参考，内容可能已严重过时，且不再维护。**
> 英文文档（`README_EN.md` / `BUILD_EN.md` / `VITRO_CLI_EN.md` / `QUICKSTART_EN.md` 等）已于 2026-09-11 删除，
> 仓库中仅保留 [`AGENTS_EN.md`](../AGENTS_EN.md)；翻译工作后续再议。
