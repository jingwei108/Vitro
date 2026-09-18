# MoonBit 迁移总计划

> **定稿**：2026-09-18（探测阶段收官版）｜ **性质**：唯一存活计划文档——本文件浓缩并取代探测阶段的 16 份文档（评估报告 + 13 份模块勘察 + 蓝图 v1.1 + 八轮第一手复核记录，全部经提交 `917251e` 保存在 git 历史，取回方法见 §11）
> **证据基线**：四道证伪门全部实测关闭（0 红）+ 27 项丢/继承/改进决定逐项亲证（0 推翻）+ 9 条 MoonBit 语言事实一手实证。所有论断可溯三层之一：勘察报告原文亲读、复核实测、代码亲验。
> **执行文档**：第一阶段（S0 收尾 + S0.5 Rust 止血批 + S1 基础片）的逐项任务清单见 [`MoonBit迁移第一阶段计划.md`](MoonBit迁移第一阶段计划.md)。

---

## 1. 形态与范围裁定

| # | 裁定 | 依据摘要 |
|---|---|---|
| F-1 | **同仓绞杀者渐进迁移**：Rust 版冻结不删、降级为差分对照 oracle；MoonBit workspace 与 `native/` 并列。冻结纪律：Rust 侧只收安全修复，新特性一律 MoonBit 侧 | 项目所有者拍板；"每搬一包趁 Rust 版仍在做差分扫描"是唯一不重踩坑路径 |
| F-2 | **v1 范围 = C only**：C++ 子集（≈6,514 行 / 8.5%）延后 S9 单独裁定；包边界第一天预留（cpp 5 包草案）；砍 C++ 与 CS2 复用策略冲突须合并裁定；空输入短路可取得"砍"的主要收益 | VM 零 C++ 感知（实测 3 处偶然命中）；is_cpp_mode 全在 lexer+parser 最前两层 |
| F-3 | **JIT 倾向不搬**（S9 复核）：宿主 V8 自带 JIT 边际缩水；JIT 与解释器溢出语义分歧现存未修；统一模式下 JIT 录制纯浪费。`jit_path_parity` 八形状外置 JSON 作"复活必全绿"遗产 | 门 1 实测：放弃 JIT 代价收敛为热循环 ~2.8× 且仍快于现役解释器 |
| F-4 | **驱动层 v1 保留 Go**（10,767 行清白资产）；Node 宿主为新增薄层（engine-host 接口：spawn/stdin 字节/stdout 逐行/stderr/超时 kill/退出码/RSS 采样）；golden 生成器由 Node 宿主驱动承担 | D5 刚收官；绞杀者策略新语言只承担引擎本体 |
| F-5 | **wasm-gc 单出口、多宿主**：浏览器（主交付）/ Node 22+（CI 主力）/ Wasmtime（需 `-W gc`，部署文档写明）；宿主接口 4 函数（invoke/reset/protocol_version/engine_version）+ 21 方法表；`memory.regions` 字段当场定型；砍 capi 45 导出 | 门 2 实测 2.34s 真实运行；45 导出中 28 无消费者（亲证） |
| F-6 | **不赶比赛按自有节奏**：以国家级/国际级赛事与 mooncakes 分片发布为可见性节点；每片 = 重写进度 + 发布 + 押注检查点 | 项目所有者拍板 |

## 2. 四道证伪门终局（全部实测，0 红）

| 门 | 判定 | 关键数字 | 随迁条件/遗留 |
|---|---|---|---|
| 门 0 LLM 效率 | **通过（弱）** | 干净上下文 6 真实函数：首过 1/6、每函数 ≤2 轮收敛、6/6 正确（含 quirk 保真与饱和边界断言） | 无同协议 Rust 对照组；三大语言差异进 S1 工程约定 |
| 门 1 VM 吞吐 | **通过** | 真实引擎锚（vm_bench 1k×1k）：现役解释器 512ms / JIT 55ms；MoonBit 迷你 VM（含 NULL/UAF 16,384 条二分/脏页检查链，校验和同口径）native 81ms / wasm-gc·V8 156ms = **快于解释器 3.3×、慢于 JIT 2.8×** | 条件 A：S6 片起 1k×1k 等价基准对照 512/55ms 双基线；条件 B：LeetCode 全量 ≤3s |
| 门 2 Wasmtime | **有条件通过** | 零 import 产物 `-W gc` 下 2.34s 真实运行（哨兵双证红）；v33 CLI 默认拒 GC 模块 | 部署须显式开特性 |
| 门 3 快照往返 | **通过（模块级）** | PASS 5/5 双目标（1MB 逐字节 + freed 全表 + 隔离区三件套 + seek 确定性 + UAF 无假阴性） | S6 集成版验收 |

**门 1 方法论教训（入档）**：首轮"名义红 7×"系对照物失真（理想化 Rust 孪生比现役解释器快 23×）——**比值结论必须声明分母引擎**。

## 3. 已实证 MoonBit 语言事实（F1–F9，全部一手取证）

| # | 事实 | 后果 |
|---|---|---|
| F1 | `Int` 四则**静默回绕**（`2147483647+1→-2147483648`），全 core **无 checked API**；`1<<1000=256`（位移 mod 32）；`x/0`→RuntimeError | 教学溢出 trap 必须应用层显式检测（`to_int64` 中转 + 范围判定，与 Rust `arithmetic.rs` 同构）；检测成本占每算术指令比已入门 1 数据 |
| F2 | `String` 内部 UTF-16（`"中文".length()=2`，`@utf8.encode().length()=6`）；`to_bytes()` 已弃用 | C 字节流一律 `Bytes`/`FixedArray[Byte]`；坐标单位契约进 `vitro/source` 包 |
| F3 | `Bytes` 不可变且是 `FixedArray[Byte]` 的 `%identity` 视图（core 源码取证）；`FixedArray[Byte]` 可写、**packed**（100M 元素 106.9MB vs Int 406.9MB = 1:3.8，实测） | 1MB 内存载体定案；门 1a 通过 |
| F4 | 21 帧瀑布递归栈深：wasm-gc 854 / js 538 / native 1005（崩溃 0xC00000FD 不可捕获，与 Rust 同形态） | 深度上限必须显式做；`MAX_PARSE_DEPTH` 语义可沿用但阈值重标定 |
| F5 | wasm-gc 产物 imports 仅 `spectest.print_char`（宿主收 Unicode 码点，须自做 UTF-8 编码）；`@env` 无宿主实现、无熵源 | 驱动协议 = 宿主提供的 import 组；引擎 rand 默认种子不得依赖系统熵 |
| F6 | `Hasher` 种子 wasm/wasm-gc=0、native/llvm/js=随机；`HashMap::iter` 序 unspecified | **一切产物输出显式排序/LinkedHashMap**；"同输入两次产物哈希相同"入 CI |
| F7 | `moon check`/`moon build` 下缺臂 enum match **默认即 error**（删臂实验 exit 127）；诊断**列出**缺失变体名 | 穷尽收益成立；CI 用 `moon check` 即可（`-d` 非必需） |
| F8 | moonc v0.10.13 存在跨文件顶层 `pub let` link-core ICE（常量内联即消失） | 工具链不稳信号，A6 持续监控 |
| F9 | `@json` 浮点序列化 `1.0→1`、`-0.0→0`（与 serde_json 不同）；libc 产物 f64/i64/string_data 全空（实测）故暂不触发 | **bundle 禁逐字节比对，解析后结构化比对** |

## 4. 包切分总图（L0–L9，`.mbti` 取代 ABI 版本化成为对外义务载体）

```
L0 零依赖   vitro/source(SourceLoc+坐标契约)   vitro/opcode(132+Instruction+operand 校验)
L1 诊断契约  vitro/diag(ErrorCode 137+Severity+SourceLang+Diagnostic+catalog JSON+覆盖率断言)
L2 抽象语法  vitro/ast(Type 17/Expr 26/Stmt 16+depth+判等渲染单源；不含 compute_type_size)
L3 名字单源  vitro/names(InstKey→InstId→mangled Name 唯一产出口；parser/typeck 共依赖)
L4 前端     vitro/lexer(facade tokenize→LexResult；internal/{source,pp,host})  vitro/parser
            〔预留〕vitro/parser/cpp
L5 语义     vitro/typeck ─ vitro/containers(JSON 数据驱动) ─ vitro/libc(单表签名)
            〔预留〕vitro/typeck/cpp
L6 发射     vitro/codegen(internal/{Layout Planner, frame LIFO 池, c, cpp})  vitro/bytecode(产物 schema+libc 固定索引)
L7 执行     vitro/memory(载体+MemoryMap+checked_access 单入口+bump/隔离堆+freed_logs 有序结构)
            vitro/host(110 路由表单源+vfs 入快照)  vitro/vm(executor 穷尽 match+snapshot 不可变派生)
            〔S9 裁定〕vitro/jit(必须可整体移除)
L8 会话/协议  vitro/session(SessionConfig 值对象)  vitro/protocol(帧+schema 版本+StepPayload/词汇/契约)
            vitro/gateway(wasm-gc 4 函数导出+NDJSON)
L9 教学智能  vitro/time_travel  vitro/teaching/steps  vitro/analysis(cfg/algorithms)  vitro/diagnostics
            —— 经 VmObserver/SourceProvider/AlgorithmContext 三接口依赖反转，不依赖 session
仓库外      Go 驱动层(保留) + Node engine-host(新增薄层) + spike 目录
```

硬约束：依赖严格单向无环；`.mbti` 只暴露 `protocol` 全量 / `lexer.tokenize` / `typeck.check` 三面；跨包不变量做成可执行断言包；版本承诺锚 `protocol_version` 编译期常量。

## 5. 在途工作接纳（摘要）

- **Rust 侧必修（P1–P7，S0.5 执行，详见第一阶段计划）**：J1 声明符栈溢出、★A 全局字符串指针双侧修复、E 前缀 4 处、string 转义收口、golden 补齐与 fail-loud（含 4 例手写 golden 循环论证处置）、列号口径冻结、AST dump 出口新建；外加 U1（认知链二/三/四层 Rust 侧补最小导出——差分退路现在不存在）与 U2（`vitro_capi.h` 19 声明是 SharpTutor 当前阻塞项，与开工同批拍板）。
- **直接按目标架构实现（要点）**：单态化纯函数化+实例化缓存（1024 上限三处改法：按栈深/带真实 SourceLoc/点名模板）；SourceLang 单源（is_cpp_mode 46 处亲证）；预处理独立 pass+LineMap+双坐标；Layout Planner；LIFO 槽位分配器（8 条事故回归必挂）；统一写路径校验器；freed_logs 有序数组+二分；OutputLog 载体 Bytes 化；VFS 入快照；诊断结构化；note 有界；U6#4；Trap 回退改"检查点+正向重放"（消每步 1MB 快照——engine.rs:169 亲证）；stream 升格窗口表示；语义标注改执行事件；签名真相源单表（printf/putchar/strcpy 三处实测冲突）；golden 生成器换 Node 宿主；memory.regions 定型；"喂入不重置运行态"与"会话级配置不可被初始化覆盖"两条不变量；发布缓冲显式状态类型；`Stmt::Try` 保留标 reserved-for-csharp。
- **放弃并记录**：capi 全部后续批次；双轨管线（生产零调用亲证）；unified/stream 死码（外部引用零亲证）；`OpCode::Strlen`/`TrapBoundsVla` 死 opcode（codegen 零发射亲证）；`compiler/ast.rs` 死文件；`extract_cpp_builtin_layout.py`（OUTPUT_PATH 指向不存在目录亲证）；K&R 语法；`-I` 搜索路径；H-4 存根硬遮蔽；D14（已清零销项）/D16 再拆；诊断切面 38/810（差分锚替代）；vm_benchmark.rs。

## 6. 差分对账分级锚点体系

| 级 | 锚点 | 前提 |
|---|---|---|
| **A 字节级** | ①字节码产物 code 段 ②stdout ③最终 1MB 内存映像 | Go canonicalizer（键排序/转义/缩进固定，fail loud）；**三条冻结**：槽位策略版本化 / 绝对 IP 跳转编码 / libc 固定索引（1000/1024/1089 按名→索引比对）；**排序义务显式继承**（现版产物确定性完全依赖 Go 侧 sort_keys，MoonBit 侧原生有序）；bundle 禁逐字节（F9） |
| **B 结构化** | token TSV / AST dump（依赖 P7 出口）/ 符号表 / 诊断序列 / mangled 名集合 / 实例化产物 / 协议帧 NDJSON / error_catalog JSON / 标注首现序列 | 两侧**显式 emitter**（禁一侧 serde 一侧 ToJson）；serve 补字段级冻结测试（protocol_frames.jsonl 双宿主对拍）；白名单补"缺失即红"；含非 ASCII/\xHH 用例（现覆盖 0） |
| **C 端到端** | Clang golden 733 全量 / replay 61 / serve_smoke 57 / JIT parity 八形状（若复活） | golden 缺失必红；`.out` 只作第二来源，live clang 为主真值 |
| **D 三联 diff** | stdout+返回码+1MB 映像 × 30 例矩阵（JIT 形状+UAF/隔离区+快照往返+字节通道+浮点+调用栈+VFS+路由分叉） | 内存 dump 出口新增 |

## 7. 裸奔期最小防线与重建里程碑

最小防线 = **24~26 例**（全部 baseline、已有 golden、无 stdin；五条选例标准；必含 `engine_note_lookalike.c` 与 `codegen_soundness_regression.c`）；三条防假绿纪律（空集不得绿 / golden 缺失必红 / 字节层比对）；M-0 基线冻结（shadow 快照+facts 入版本控制）；重建里程碑 M-1~M-9（驱动骨架→golden 解析→全量→live-Clang→三层契约→fuzz→facts→台账 CI）。

## 8. 差异台账 v0（机器单源，S8 片落地）

格式：`DIFF-<域>-<序号>`；`class ∈ {architectural, implementable, pedagogical}`；`carry_over ∈ {inherit, fix, drop, retest}`；JSON schema 含 `anchors`（与 shadow KNOWN 常量双向对账）与 `detectable_by_defense` 诚实字段；capability_flags **17 项**；落地三步（单源→CI 对账→J9 埋雷）。初始条目 14 条代表项（DIFF-PTR-4BYTE-01 / DIFF-LAYOUT-PACKED-01（防线零覆盖，先立 golden）/ DIFF-LIB-PRINTF-01 / DIFF-LIB-PUTCHAR-01（golden 缺口）/ DIFF-PREPROC-MULTILINE-01（规范反向漂移，先修规格）/ DIFF-TYPE-SHORT-01 等）。守门规则：引擎行为与台账冲突时要么改行为要么改台账，禁止沉默漂移。

## 9. 风险登记册（终态）

A2 实测关闭（有条件）；A3 实测关闭（方向有利）；A7 实测关闭（弱）；A8 持续监控（F8 ICE）；R1 门 1 条件 A/B；R2 HashMap 种子（排序义务）；R3 UTF-16 三单位（坐标契约）；R5 双头维护（冻结纪律）；R6 裸奔期（最小防线）；R7 生成物门禁；R9 拖延（分片可停可续）；R11 wasm 冒烟证据链（重建：真 E3070/E3061 断言+体积断言+产物更名，每条护栏先证红）；R12 rand 种子（wasm-gc 无熵源）。

## 10. 里程碑切片

| 片 | 内容 | 验收（锚点级） | 发布 |
|---|---|---|---|
| S0.5 | Rust 止血批 P1–P7 + U1/U2 | 每条红→绿留痕；M-0 基线冻结 | — |
| S1 | `vitro/{source,diag,opcode,ast}` | E1 AST dump（B）+ error_catalog JSON（B）+ 码表生成幂等 | `vitro/diag` 首发 |
| S2 | `vitro/lexer`（独立 pass+LineMap+宿主 IO） | L1/L2 token TSV + 随机词法差分 ≥2000（.mbtx 语料生成器） | `vitro/lexer` |
| S3 | `vitro/parser`（深度统一入口；J1 语义不复刻） | E1–E4 + 病态输入"同等拒绝"12 样本 + 活性内部断言 | — |
| S4 | `vitro/{names,typeck,containers,libc}` | E1–E4 + mangled 名集合相等 | `vitro/names` |
| S5 | `vitro/{codegen,bytecode}` | A 级产物 code 段 + libc 自举 + LIFO 八条事故回归 + r1 7 道 + `--dump-compile-output` 工具 + codegen 自建单测 | — |
| S6 | `vitro/{memory,host,vm}` | D 级 30 例三联 diff + 门 3 集成版 + 条件 A 性能锚 | `vitro/vm` |
| S7 | `vitro/{session,protocol,gateway}` + Node 宿主 | 协议帧双宿主对拍 + replay/serve_smoke 重建 | `vitro/protocol` + wasm-gc 产物 |
| S8 | `vitro/{time_travel,teaching,analysis,diagnostics}` + 差异台账 | seek 往返五类相等 + 标注 golden 311 三方 diff + 台账 CI | 认知链切片 |
| S9 | 裁定批：JIT 复核 / C++ 搬或砍（与 CS2 合并）/ libc 机制形态 / Wasmtime 形态 | 各自判定书 | — |
| 全量切换 | 758 用例 + golden 733 全绿 + facts 双轨收口 | shadow 逐项一致（match/known_issue/gap 三口径） | 1.0 |

节奏纪律：锚点未全绿不发版；每片回填 facts（新键空间独立）；任一时刻可停。

## 11. 探测档案指南（git 历史）

全部 16 份探测文档保存在提交 `917251e`：

```bash
git show 917251e --stat                          # 文件清单
git show 917251e:"docs/current/07-质量与裁定/MoonBit迁移_<模块>模块勘察报告20260918.md"
git log --oneline --follow -- "docs/current/07-质量与裁定/MoonBit迁移蓝图v1_第一手复核记录.md"
```

要点：13 份勘察报告的九节结构（资产/包袱/在途/架构/坑/spike/锚点/包切分/交叉声明）是 S2–S8 各片的执行输入；八轮复核记录含全部实测原始数据（探针输出、vm_bench 数字、门 0 迭代表、wasmtime 哨兵验证、五类工具陷阱）；评估报告含 A1–A9 假设原始表述。**本计划只保留结论与判据，细节以档案为准。**

## 12. 制度随迁

保留直接搬：红→绿纪律、诚实记录（防线哲学第 0 条）、J9 埋雷（升级为脚本上线硬门禁）、事故归档、同类清查义务、指标自报行、预留位 tripwire、KNOWN_* 双向对齐、单源登记义务。保留改写：facts 对账（机制重写，制度保留；覆盖键扩到台账自述数字）、文档数字三分法、RSS 护栏（口径重标定）、磁盘卫生（对象换 MoonBit target/包缓存）、发布节奏（ABI→包版本+.mbti）、产物新鲜度（版本串含 git 短哈希）。作废：ABI 版本化（随 capi）、`#![forbid(unsafe_code)]`、clippy 四规则（找 MoonBit 等价，覆盖度待证）、C ABI 契约测试、双轨驱动制度。新增：wasm-gc 宿主契约、包版本+.mbti 兼容性、每函数预期 1~2 轮编译迭代的排期口径、坐标单位契约、五类工具陷阱规则（SIGPIPE / 管道退出码 / 哨兵证红 / 基准同口径校验和 / 反斜杠 grep）。
