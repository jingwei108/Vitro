# Cide 后端 API 评审回复（回应 SharpTutor《后端 API 需求与签名评审》）

> 回应日期：2026-09-11
> 回应对象：SharpTutor《Cide 后端 API 需求与签名评审》（2026-09-11 提交，回应上游 `后端定位与白箱计划.md` §7 风险 4）
> 评审结论：**整体接受，一处分歧（wasm 排序，修订为并行）**。本文档逐条回应其 §1~§9，并直接回答其全部六个开放问题（含三项实证核验）。
> 修订同步：本文档结论已同步修订主计划（§3.2/§5.3/§6），以主计划 + 本文档为准。
> 最后核对日期：2026-09-11
> 修订说明（2026-09-11）：去前端化澄清——`flutter_bridge` 全局会话表由语言中立层 `session_api` 取代；**重构批次 R2（2026-09-11）后 `flutter_bridge.rs` 已整删**，cide_cli 与 serve 直用 `Session` + `session_api`；回放输入表述改为"原生前端 frameCache 消费序列（已切割的历史资产）"。原回应日期与逐条结论保持原样。

---

## 0. 事实核验（评审前置）

对双方文档关键声明做了核验：

| 声明 | 核验结果 |
|---|---|
| SharpTutor 三进程架构 / DiagnosticInfo 含 EndLine/EndCharacter / Runner 组件 | ✅ 属实（`D:\code\SharpTutor` 实地核验：CompilerService.cs / MonacoEditor.cs 消费点、RunnerHost/ControlChannel 组件齐全） |
| 重复编译内存有界（其 §1.2 诉求 2） | ✅ **实测通过**：10000 次交替编译（成功/错误源），RSS 16.6MB → 18.4MB，500 次后进入平台期；destroy 后不回落 ~1.8MB 为分配器缓存（正常，将文档化）。按其要求固化为回归断言 |
| `MemoryRegionData` 是否已含分配点行号（其开放问题 4） | ✅ 已有：`alloc_line` + `alloc_by`（"malloc"/"realloc"/"fopen"）字段现成，UAF/泄漏诊断在用 |
| frameCache 驱逐与越窗行为（其开放问题 3） | ✅ 已查证：窗口 2000 帧、超出丢最早 20%；seek 越窗 = **最近检查点恢复 + 正向重放**（懒重算，非报错） |
| E4001~E4031 NotSupported 段（其 §4 诉求 3） | 现状已具备"超出教学子集"独立码段雏形，需要的是文档化声明而非新建 |

---

## 1. 对其 §1（capi 第一批签名）的回应：全部采纳 + 四点补充

| 项 | 决定 | 说明 |
|---|---|---|
| 指针句柄统一（`Session*`） | ✅ 确认 | `flutter_bridge` 的**全局 `session_id` 表已由语言中立层 `session_api` 取代**（审查 E-P1-2 的问题源随之消解并随前端切割退役）；`flutter_bridge.rs` 已随重构批次 R2 **整删**（cide_cli 改直用本地 `Session` + `session_api`） |
| `cide_abi_version` / `cide_engine_version` / `cide_last_error` | ✅ 全部采纳 | engine_version 用 build script 注入 git hash，成本近零 |
| 诊断 `end_line/end_column` | ✅ schema 先带字段，默认"起点+1"退化值 | 完整跨度需动三处错误结构体（LexerError/ParseError/TypeError）+ 报错点分批补（lexer token 有 span，可行但非低成本）。精确跨度按诊断类别分批：类型不匹配表达式、未声明标识符等高价值跨度优先 |
| warning/hint 进 JSON | ✅ | severity 枚举（error/warning/hint）文档化 |
| `cide_set_max_steps` / `cide_set_call_depth_limit` | ✅ 会话级配置 | 调用栈上限对应审查 V-P1-10（零局部变量递归绕过栈堆碰撞），一并落 |
| `cide_set_deterministic` | ✅ **提前进 Phase 1** | 最小形态成本比预估更低：`host_time`/`host_clock`/`host_rand` 三处模式判断（time 固定 0 + rand 固定种子），半天量级。注意区分：判分确定性（Phase 1 最小形态）≠ 时间旅行重放确定性（Phase 3 完整伪时钟），文档分层说明 |
| `cide_run_json` 字段清单 | ✅ 原样采纳 | return_value / trap（E 码透传）/ waiting_input / steps_executed 全部为现成数据 |
| 输出游标增量 | ✅ | 与 serve 事件流共享底层实现（三出口一套语义） |
| `cide_free_string`（rust-alloc + 调用方释放） | ✅ 完全采纳 | 同时消解 wasm 侧对应问题（JS 调 wasm 导出 free 同模式）；不再为 JSON 函数保留 caller-buffer 双轨 |

**四点补充**（他们未提，定稿需写入契约）：

1. **线程契约**：Session 非线程安全，跨线程访问需调用方外部同步（当前 capi 无锁；防抖单线程 UI 场景安全）；
2. **状态码表**：`0=成功 / 负数=入参错误或会话无效 / 正数=领域状态`（1=trap、2=waiting_input…），逐函数写文档；
3. **所有新入口 `catch_unwind`**（V-P0-3 模式推广），panic 永不跨边界；
4. **编码契约**（对应其 §5.2）：源码输入 UTF-8；程序输出 UTF-8 + `\n`（现状已正确，升级为 API 承诺）。

## 2. 对其 §2（StepPayload schema）的回应：采纳 + 定稿提前

**接受"定稿提前"**，且其论据比上游原计划更自洽：第一批 `step_next_json` 的输出就是 StepPayload，schema 本不可能等到 Phase 3。**修订：schema v0.1 定稿前置于 Phase 1 内**，SharpTutor 参与并带三组回放场景（防抖编译流 / fixtures 判分流 / 单步+seek+内存查询交错流）。

四个具体要求的回应：

1. **display_name**：采纳"StepPayload 直接带 display 形态 + mangled 保留在字段"（消费端按需取，帧体积非轮询场景主要矛盾），`cide_demangle` 作为补充函数；
2. **指针四状态枚举值**（Valid/Freed/Null/Dangling）进 schema 文档：✅ 纯文档工作；
3. **`array_snapshots` + `accessed_vars` 读写枚举**：✅ 文档化；
4. **frameCache 窗口语义**：✅ 已核验（见 §0），随 schema 文档化：窗口 2000 帧 / 驱逐最早 20% / 越窗 seek = 检查点恢复 + 正向重放；检查点间隔作为附加变量一并说明。

字段原则重申：**只增不改语义，废弃走双写过渡期**（与上游既有承诺一致）。

## 3. 对其 §3（内存 API）的回应：全部采纳

1. **`kind` 分类枚举**：诚实回应——现状 `regions` 仅含堆分配记录，全局区/栈帧不在其中；API 层合成三段式（global/stack/heap），由引擎生成 kind 枚举，不让消费端按地址猜（其理由成立：地址布局是实现细节，不泄漏到协议）；
2. **`alloc_line`**：已有（§0 核验），零成本；
3. **字节粒度读取**：✅ 新增 `cide_read_memory_bytes_json`（JSON 数组）；
4. **freed region 保留可见**：现状即如此（`is_freed` 标记不删除，UAF 检测依赖），schema 用 `status: "allocated|freed"` 显式表达。

## 4. 对其 §4（错误码治理）的回应：全部采纳，且比预期更近

1. **码表机器可读导出**：✅ 从 `error_catalog.rs` 生成 JSON（code → 名称/默认 fix_suggestion/严重级别/适用语言 c|cpp）。与审查 §7.5-21"对账权威从 Markdown 迁移到代码常量"同向，实施时把 `*_FAILURES.md` 的统计对账一并收进该机制；
2. **永不复用**：✅ 写入治理承诺（退役语义不回收码号，新码只追加）；
3. **"超出教学子集"独立码段**：✅ E4001~E4031 NotSupported 段即此语义，文档化声明；
4. **已知差异清单机器可读化**（审查报告 §8 的 17 条）：✅ YAML/JSON 导出供其课程红线检查器对账。

## 5. 对其 §5（行为契约）的回应

1. **JIT 与断点/热力图完整性**：✅ **接受从 P1 提级为行为契约**——教学 IDE 的单步断点是核心卖点，不应排在普通修复队列。实现取向按审查 §7.3-14：含断点的循环排除出 trace 编译。验收：任何执行路径（解释器/JIT bulk）下断点命中与热力图计数完整的回归用例；
2. **UTF-8 契约**：✅（并入本文 §1 补充 4）；
3. **杀进程安全**：✅ 可声明——VFS 全内存沙盒、无文件锁/临时目录依赖（注：shadow 驱动脚本写入 CWD 的 out.txt 等是测试脚本行为，非引擎行为，随切割清理）。

## 6. 对其 §6（范围纪律）：照单全收

多文件工程、预处理器完整化、clang 全量对齐、release/semver——均不在验收项内，与上游子集定位一致。

## 7. 对其 §7（路线取舍）的回应：一半接受，一处修订为"并行"

| 其意见 | 上游决定 |
|---|---|
| Phase 1 绝对优先 | ✅ 一致 |
| schema 定稿提前 | ✅ **接受并修订主计划**：schema v0.1 前置于 Phase 1 内 |
| deterministic 最小形态先行 | ✅ 接受，进 Phase 1（见 §1） |
| **wasm 排在第二批之后** | ⚠️ **修订为"与第二批并行"而非后置**。理由：(a) wasm 对 SharpTutor 确无消费价值（学生代码进 WebView2 冻结事件循环的判断正确），但 wasm 是**社区前端生态的冷启动开关**——切割后项目存亡不系于单一消费者，浏览器 demo 是"社区五分钟跑通"承诺的载体；(b) wasm 成本已冒烟实证为确定性一周，与第二批（内存/断点，纯 capi 层）无模块冲突、不抢资源。修订后顺序：**Phase 1（含 schema v0.1 + deterministic）→ 第二批与 wasm 并行 → 第三批时间旅行** |
| serve 为主路径、两出口漂移由其当 Canary | ✅ 欢迎且支持 |
| serve 三小要求（id 关联 / 错误帧同构 / session.reset） | ✅ 全部采纳，成本近零 |

## 8. 对其 §8（对等承诺）：接受，附 review 基线

四项承诺全部欢迎。capi/serve 骨架 PR 的 review 基线约定：

1. 照现有 capi 模式（`cide_` 前缀 + 状态码约定 + 入口 catch_unwind 模板）；
2. 字符串所有权统一 rust-alloc + `cide_free_string`（本文 §1）；
3. 每个新函数带 capi 集成测试（参考 `crash_regression_tests.rs` 的驱动方式）+ serve 等价路径测试（三出口一致性）；
4. 15 用例冒烟集与 byte-level 期望输出随 PR 进 `native/tests/`。

## 9. 对其 §9 开放问题的正式回答

| # | 问题 | 回答 |
|---|---|---|
| 1 | 诊断 end 位置能否低成本给出？ | schema 先带字段（不欠债），值默认"起点+1"退化；精确跨度按诊断类别分批补（高价值优先：类型不匹配表达式、未声明标识符） |
| 2 | deterministic 能否提前 / 最小形态？ | **能**：time 固定 + rand 固定种子进 Phase 1（约半天）；完整 step 派生时钟留 Phase 3（服务于时间旅行重放确定性，与判分确定性分层） |
| 3 | frameCache 驱逐与越窗行为？ | 窗口 2000 帧、驱逐最早 20%、越窗 seek = 检查点恢复 + 正向重放（懒重算）；检查点间隔随 schema 文档化 |
| 4 | region 是否已含分配点行号？ | **是**：`alloc_line` + `alloc_by` 已在 `MemoryRegionData`，零成本直通 |
| 5 | 统一到指针句柄？ | **确认**：`Session*`；`flutter_bridge` 的全局 `session_id` 表已由语言中立层 `session_api` 取代，`flutter_bridge.rs` 已随重构批次 R2 整删 |
| 6 | schema 定稿会议时间？ | 同意参与且接受提前——**Phase 1 内 v0.1 定稿**，请带三组回放场景；上游提供原生前端 frameCache 消费序列（**已切割的历史资产**）与 StepStreamBatch 现有消费序列作为另一方回放输入 |

---

## 10. 落地追踪

### 10.1 capi 第一批实现状态（2026-09-11）

| 函数 | 状态 | 说明 |
|---|---|---|
| `cide_abi_version` | ✅ | 返回 `1.2.0`（**加函数 = minor，改签名/语义 = major**；1.1.0 为 E-P1-5 追加输出通道函数，1.2.0 为 2026-09-13 追加 `cide_get_compile_errors_length`） |
| `cide_engine_version` | ✅ | crate 版本（可选拼接构建期注入的 `CIDE_GIT_HASH`） |
| `cide_free_string` | ✅ | rust-alloc 所有权唯一释放入口；null 安全；不保留 caller-buffer 双轨 |
| `cide_last_error` | ✅ | JSON `{kind: "compile"\|"runtime"\|"none", message}` |
| `cide_compile_json` | ✅ | 诊断 JSON：code / error_code / severity(`error\|warning\|hint\|info`) / line / column / **end_line / end_column**（先给"起点+1"退化值）/ message / fix_suggestion / filename |
| `cide_run_json` | ✅ | ok / status(`finished\|trap\|waiting_input\|not_compiled`) / return_value / trap（E 码透传）/ waiting_input / steps_executed |
| `cide_get_output_delta` | ✅ | 游标增量 + 新游标 + 总字节 + `stream`；游标越界按末尾处理、落在多字节字符中间时前移到字符边界。**展示视图**（含引擎附注），纯净 stdout 用 `cide_get_program_output_delta` |
| `cide_get_program_output_length` / `cide_get_program_output` | ✅ | **E-P1-5（1.1.0）**：纯程序 stdout —— 判分、与 Clang golden 比对、第三方消费的唯一合法来源；不含引擎附注与 stderr |
| `cide_get_engine_notes_length` / `cide_get_engine_notes` | ✅ | **E-P1-5（1.1.0）**：引擎附注单独通道（运行完成提示 / 内存泄漏报告 / 教学安全警告） |
| `cide_get_program_output_delta` | ✅ | **E-P1-5（1.1.0）**：纯 stdout 的游标增量，返回 `stream:"stdout"` |
| `cide_get_compile_errors_length` | ✅ | **1.2.0（2026-09-13）**：编译错误 JSON 字节长度（不含 NUL；无错误 0）——驱动侧定长读取，替代变长窗口扫描（对短于窗口的分配是越界读） |
| `cide_set_max_steps` | ✅ | 会话级保险丝（映射 `CideVM::set_max_steps`；`reset()` 不再清空会话配置） |
| `cide_set_call_depth_limit` | ✅ | V-P1-10：VM 新增 `call_depth_limit` 字段 + `do_call_inner` 深度检查（下限 16 层兜底） |
| `cide_set_deterministic` / `cide_get_deterministic` | ✅ | 最小形态：`time()` / `clock()` 固定返回 0 |
| `cide_set_breakpoints` | ✅ | 入参为 JSON 整数数组（`[]` 清空）；**须在 `cide_step_begin` 之后调用**（step_begin 重建 VM 会清空断点） |
| `cide_step_begin` | ✅ | 新增辅助入口：初始化统一模式会话（装载 VM + 重建运行时 + 初始检查点），未编译返回 -2 |
| `cide_step_next_json` | ✅ | 单步推进并返回 `AutoStepResult` JSON（payloads / finished / trapped / waiting_input / paused / current_line / trap_message / cache_start_step）；命中断点时 `paused=true` |
| `cide_get_step_payloads_json` | ✅ | 按步号区间取 payload 数组 + `cache_start_step` / `max_collected_step`；`StepPayload` 类型链已补 `serde::Serialize` |

**横切契约落实**：全部 JSON 函数为 rust-alloc 所有权（`cide_free_string` 释放）；全部入口 `catch_unwind` 包裹（panic 不跨 C 边界）；状态码 `0/负/正` 约定；Session 非线程安全已声明；输入输出 UTF-8。

**集成测试**：`native/tests/capi_first_batch_tests.rs`（12 用例）——覆盖字符串所有权、severity 与 end 字段、运行三态、游标增量语义、三处保险丝（max_steps / call_depth_limit / deterministic）。

**已知问题（本次实测发现，待修，不属于本批复核范围）**：`#include <time.h>` 会导致"编译失败"且**不给出任何诊断**（runtime_libc 的 `time.h` stub 经 include 展开路径失败；把同样内容手写进源文件则编译成功）。自带原型声明 `long time(long *t);` 可绕过。

### 10.2 Phase 1 收尾（✅ 已全部完成，2026-09-12 核对）

- ~~SharpTutor 用 capi 第一批（含 deterministic 最小形态）+ serve 跑通三进程集成~~ → 仓库侧已就绪；对端 S1~S5 签字回放 61/61 通过（2026-09-12，见 [`下游需求处置回执.md`](.) §1）；
- ~~**StepPayload schema v0.1 文档发布**~~ → 已发布为 [`../spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md 并冻结（2026-09-12，S1–S5 回放 61/61）；
- ~~`cide_cli serve` JSON-lines 会话模式~~ → 已落地并进 CI 冒烟（id 关联 / 错误帧同构 / `session.reset`，见 [`CLI使用手册.md`](../../02-构建与上手）。
