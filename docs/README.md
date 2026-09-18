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
| [`current/01-定位与路线/MoonBit迁移总计划.md`](current/01-定位与路线/MoonBit迁移总计划.md) | **MoonBit 迁移总计划（2026-09-18 定稿）**：唯一存活计划——形态裁定（同仓绞杀者/v1=C only/JIT 倾向不搬/Go 驱动保留/wasm-gc 单出口）；**四门终局 0 红**（门0 弱通过、门1 通过 快于现役解释器 3.3×、门2 有条件 -W gc、门3 通过）；9 条一手语言事实；L0-L9 包切分总图；P1-P7 止血+按目标架构+放弃三清单；A/B/C/D 四级差分锚点；裸奔期最小防线 24 例；差异台账 v0（17 capability_flags）；风险登记册；S0.5-S9+全量切换里程碑；探测档案取回指南（提交 `917251e`，16 份文档全量入 git 历史） |
| [`current/01-定位与路线/MoonBit迁移第一阶段计划.md`](current/01-定位与路线/MoonBit迁移第一阶段计划.md) | **MoonBit 迁移第一阶段计划（S0.5 + S1）**：Rust 止血批 P1-P7+U1/U2 逐项（现象/根因 file:line/修法/验收锚，全部经第八轮亲证：J1 栈溢出、★A 双侧、E 前缀 4 处、string 转义、golden 完整性含手写 golden 循环论证处置、列号口径、AST dump 出口）+ S1 基础片任务分解（source/diag/opcode/ast 四包+码表生成脚本+三大工程约定+五条工具陷阱规则+mooncakes 首发判据） |
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
| [`current/06-出口与协议/wasm多实例并发模型与U2拍板.md`](current/06-出口与协议/wasm多实例并发模型与U2拍板.md) | 宿主并发模型裁定：N 线程 × N 实例构造性隔离（三宿主形态 + 1实例=1线程=1会话铁律）+ U2 拍板（19 声明冻结现状、第二批 capi 裁不做、下游改道 wasm/serve）（2026-09-19） |

#### 质量、裁定与工作记录

| 文档 | 说明 |
|------|------|
| [`current/07-质量与裁定/统一整备路线图.md`](current/07-质量与裁定/统一整备路线图.md) | **统一整备路线图 U0~U7（排期权威）**：三语化 S 系列与重构评估 Phase 系列的合并执行方案（波次总览 / CS 硬门禁 / 防伪绿机制）（原 `VITRO_OVERHAUL_ROADMAP.md`） |
| [`current/07-质量与裁定/核心资产重构裁定.md`](current/07-质量与裁定/核心资产重构裁定.md) | **核心资产重构裁定 v1（独立裁定）+ 重构执行方案**：分区裁定 / 判据 J1~J10 / 候选对比 / 中止条件 / §13 五域执行方案（D1 防线自身、D5 工具链语言 Python→Go 迁移边界与双轨纪律）；**§14 JIT trace 路径 P0 静默错值**（嵌套纯计数循环被外层 trace 穿透；**归因修正为与 `long long` 无关**；根因 = JIT fast path 在录制期间未禁用；含四组双向验证实验与 `vm_bench` 两处方法学缺陷）；**§14.11 对重构范围的影响**：新增子域 **D6（JIT 加速器存废重裁：先校正 `vm_bench` 重测加速比 → 删 JIT 或收缩作用域）**、D1b 形状对抗生成、J10 前置到 W1——**裁定①（核心不重写）维持**。实测脚本与证据 JSON 在 [`scripts/core_asset_verdict/`](../scripts/core_asset_verdict/)（原 `VITRO_CORE_ASSET_RECONSTRUCTION_VERDICT.md`） |
| [`current/07-质量与裁定/列号口径冻结.md`](current/07-质量与裁定/列号口径冻结.md) | 列号现状口径冻结（词法 +1 / 解析非 ASCII −4 / make_token 量纲混算根因）+ MoonBit vitro/source 双坐标契约输入 + 10 形状防漂移锚（2026-09-19） |
| [`current/07-质量与裁定/三语化整备审计计划.md`](current/07-质量与裁定/三语化整备审计计划.md) | 三语化整备计划 v1.1（§1~§4 渗出证据链 / 防伪绿解剖 / 10591ad 事故裁定仍为权威记录；§5 批次表已并入 U 系列路线图）（原 `VITRO_TRILINGUAL_OVERHAUL_PLAN.md`） |
| [`current/07-质量与裁定/重构评估报告20260912.md`](current/07-质量与裁定/重构评估报告20260912.md) | 重构评估（§1~§4 权威证据：泄漏复发洞实锤 / 7 项动态探针 / 分模块风险清单；§5 计划已并入 U 系列路线图）（原 `VITRO_REFACTOR_ASSESSMENT_2026_09_12.md`） |
| [`current/07-质量与裁定/代码审阅与修复追踪20260906.md`](current/07-质量与裁定/代码审阅与修复追踪20260906.md) | 全面代码审阅报告（137 条发现）与四批修复追踪（**修复进度权威追踪**；0911 复核已闭环归档）（原 `code_review_report_2026-09-06.md`） |
| [`current/07-质量与裁定/实测发现登记20260913_性能与头文件.md`](current/07-质量与裁定/实测发现登记20260913_性能与头文件.md) | **实测发现登记（性能 + 头文件语义，2026-09-13）**：P-1~P-6（malloc churn 超线性 / 解释单步 35~41ns / 步数膨胀 18 步 / JIT 覆盖面 / 统一模式 58.5μs 步 / 基准方法学）、H-1~H-5（include 静默跳过零诊断、__has_include 口径不一、<> 宽松、存根遮蔽、wasm 降级）、D-1~D-4 文档漂移、跨平台备查——全部实测登记并经独立复核（§8），已挂接路线图批次（H-1/H-2 入 U1 第三批；P-1 完成 G6 复核一半，解锁 U2#2 开工前置） |
| [`current/07-质量与裁定/脚本埋雷验证记录.md`](current/07-质量与裁定/脚本埋雷验证记录.md) | **J9 台账**：三个判定型脚本（shadow_verify / ci_three_tier_check / serve_smoke）各一条"注入→必须红"埋雷实证——判定型脚本埋雷记录 = 0 时其全绿不得作为结论依据（W0-1 / U0#8 验收达成）；含 serve_smoke 边界批与两处脚本自身缺陷的发现记录 |
| [`current/07-质量与裁定/工作记录20260912_突变测试.md`](current/07-质量与裁定/工作记录20260912_突变测试.md) | 工作记录：影子防线突变测试首次实测（3/3 检出；M3 裕度=1 实证用例形状盲区）（原 `WORKLOG_2026_09_12_MUTATION_TEST.md`） |
| [`current/07-质量与裁定/实测发现登记20260913_性能与头文件.md`](current/07-质量与裁定/实测发现登记20260913_性能与头文件.md) | **实测发现登记（2026-09-13，登记未修复）**：性能域 P-1~P-6（三引擎同源基准量化：JIT 8.80×/7.92×、解释 35~41 ns/步、`malloc` churn 44.6 μs/次超线性、统一模式 58.5 μs/步；含"空载单线程非稳定条件"方法学告警）+ 头文件域 H-1~H-5（**两条 P0 新发现**：找不到头文件静默跳过零诊断、`__has_include` 与 `#include` 口径不一致）+ 文档漂移 D-1~D-4 + 跨平台备查 4 项；15 个 include 探针 × Clang 对照，含复现命令与外推边界 |
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
