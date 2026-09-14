# Vitro 项目 Agent 指南

> [English Version](AGENTS_EN.md)

## 项目概览

> **定位转型（2026-09-11）**：Vitro 从"跨平台 C 语言 IDE"转型为**教学 C/C++ 子集参考执行引擎（白箱）**——本仓库只做后端（MIT 许可），前端切割给社区，原生移动端放弃。完整决策依据与路线见 [`A-定位与路线/后端定位与白箱计划.md`](A-定位与路线/后端定位与白箱计划.md)。
>
> **项目更名（2026-09-14）**：Cide → **Vitro**（*in vitro*，"在玻璃之中"——白箱观察 + Clang 基线诚实对照）。crate / C ABI（**2.1.0**）/ CLI / DLL 全量 `vitro_*`；`docs/archive/` 与历史标签内路径（`CideFlutter`）保留原名。见 [`项目更名记录.md`](docs/current/01-定位与路线/项目更名记录.md)。
>
> **前端切割已执行（2026-09-11）**：`CideFlutter/`、FRB 桥接（`native/src/api/` + `frb_generated`）、web 部署 workflow 与全部 Flutter 构建脚本已从仓库移除；切割前最后完整状态由标签 `before-frontend-split` 保留（`git checkout before-frontend-split -- CideFlutter` 可取回）。

当前架构（三出口一核心）：

- **核心**：Rust workspace 编译器/VM（`native/`，10 个子 crate），编译管线 Lexer → Parser → TypeChecker → BytecodeGen → VitroVM
- **出口 1**：C ABI（`native/src/capi/`）——`vitro_cli` 与 shadow 防线（ctypes）的第一消费路径
- **出口 2**：wasm32（已冒烟实证：零修改构建 3.75MB，C API 全链路 + 安全检测在 wasm 下工作）——浏览器/白箱形态
- **出口 3**：`vitro_cli serve` JSON-lines 会话模式（已落地：id 关联 / 错误帧同构 / `session.reset`；与 capi 共用 `native/src/session_api.rs`）——headless 交互

**架构纪律**：新能力一律先落语言中立 Rust 层，三个出口只做薄包装且共用同一套入口语义；复杂结构过边界走 JSON 字符串；capi 是公共 API（`vitro_abi_version()` 版本化）。

- **必须中文输出思考以及回答问题**
- **未经允许禁止git提交**
- **`docs/archive/` 下的任何归档文档不具备参考价值**禁止拿里面的任何数据套用到现在的文档里，里面的任何文档也不做任何维护
- **诚实记录**：本项目作为教学c/cpp子集，以clang为标准，任何本项目与标准不符合的，都要进行记录
- **文档体系**：新文档进 `docs/current/` 对应分类子目录（`01-定位与路线` / `02-构建与上手` / `03-语言子集` / `04-标准库与防线` / `05-教学体验` / `06-出口与协议` / `07-质量与裁定`，中文文件名）；被取代或对象已迁出的移入 `docs/archive/`（加 `ARCHIVE_` 前缀与归档横幅），并同步 `docs/README.md` 索引；英文文档只保留根目录 `AGENTS_EN.md`（其余已删除，翻译后续再议）
## 技术栈

| 层级 | 技术 |
|------|------|
| Native | **Rust 1.95.0**, Cargo, cdylib/staticlib/rlib |
| VM | 自定义字节码解释器，1MB 线性内存 |
| 出口 | C ABI（capi）、wasm32（wasm-bindgen）、JSON-lines（serve） |

## 关键目录

```
native/crates/          编译器/运行时子 crate（独立 crate 化进行中）
  vitro_shared/          SourceLoc、ErrorCode 等共享基础类型
  vitro_ast/             AST 节点与类型系统
  vitro_lexer/           词法分析器
  vitro_parser/          语法分析器
  vitro_cpp_frontend/    C++ 前端支持
  vitro_typeck/          类型检查器
  vitro_codegen/         字节码生成器
  vitro_runtime/         VM 运行时共享数据（内存状态、opcode/instruction、符号表）
  vitro_vm/              VitroVM 字节码解释器
native/src/compiler/    剩余本地模块：algorithm_detector、cfg、data_flow、intent (Rust)
native/src/unified/     统一模式 / 时间旅行引擎 (Rust)
native/src/engine/      编译管线与工具 (Rust)
native/src/capi/        C API（出口 1，公共 API，ABI 版本化）(Rust)
native/src/session_api.rs 会话语义中立层（capi / serve / vitro_cli 共用，出口只做薄包装）(Rust)
native/src/diagnostics/ 结构化诊断、自动修复建议、知识图谱、教学推理 (Rust)
templates/              算法模板源（source.c + meta.yaml；前端资产源，暂保留待社区前端认领）
docs/current/           当前有效文档：规范 / 设计 / 活跃计划 / 测试防线
docs/spec/              语言中立协议 schema（对外承诺的 wire format）
docs/archive/           历史归档（ARCHIVE_ 前缀 + 归档横幅；仅供追溯，不再维护）
```

> 历史前端 `CideFlutter/` 与 FRB 桥接已迁出（标签 `before-frontend-split`）。

## Rust 迁移进度（已完成 ✅）

| 阶段 | 模块 | 状态 |
|------|------|------|
| Phase 0 | Rust 骨架 + C API 桩 + Session 类型 | ✅ 完成 |
| Phase 1 | VM 迁移 (VitroVM + host funcs) | ✅ 完成 |
| Phase 2a | Lexer | ✅ 完成 |
| Phase 2b | AST | ✅ 完成 |
| Phase 2c | Parser | ✅ 完成 |
| Phase 2d | TypeChecker | ✅ 完成 |
| Phase 2e | BytecodeGen | ✅ 完成 |
| Phase 2f | C API `vitro_compile_all` 接线 | ✅ 完成 |
| Phase 3 | ~~C# 前端~~ → Flutter 前端端到端测试 | ✅ 完成 |
| Phase 4 | Android 目标构建（cargo-ndk） | ✅ 完成 |
| Phase 5 | 清理遗留 C++ / CMake 文件 | ✅ 完成 |
| Phase 6 | 全面审查：编译警告清理 + 安全加固 + 测试覆盖拓展 | ✅ 完成 |
| Phase 7 | Desktop 内存泄漏修复 + sizeof/scanf 子集拓展 | ✅ 完成 |
| Phase 8 | `float` 类型全管线支持（Lexer→Parser→TypeChecker→BytecodeGen→VM）+ 诊断系统拓展 | ✅ 完成 |
| Phase 9 | Flutter 前端从零搭建：IDE 界面 + 编辑器 + 调试面板 + 算法可视化 | ✅ 完成 |
| Phase 10 | 内存映射 Canvas + 算法可视化事件 FRB 集成 + 交互增强 | ✅ 完成 |
| Phase 11 | 代码审查修复 + 工程规范（`rustfmt.toml`/`CHANGELOG.md`）+ Flutter 前端全面模块化拆分 | ✅ 完成 |
| Phase 12 | `union` 类型全管线支持（Lexer→Parser→TypeChecker→BytecodeGen→VM）+ `sizeof(union U)` | ✅ 完成 |
| Phase 13 | 统一模式 / 时间旅行：VM 快照/恢复、检查点管理器、Seek 进度条、异常回退 | ✅ 完成 |
| Phase 14 | 堆内存可视化增强：malloc 行号追踪、外部碎片可视化、泄漏检测报告 | ✅ 完成 |
| Phase 15 | 指针追踪动画：`PointerSnapshot` 四状态（Valid/Freed/Null/Dangling）实时箭头绘制 | ✅ 完成 |
| Phase 16 | 算法步骤语义标注：27 种算法预定义步骤模板，运行时推断教学描述 | ✅ 完成 |
| Phase 17 | 代码模板参数化 + 交互式教程：参数占位符、`TemplateTutorialPanel` 逐行引导 | ✅ 完成 |
| Phase 18 | 6-04 地毯式审阅：P0 soundness 修复、VM 优化、DRY 重构、clippy 0 警告 | ✅ 完成 |
| Phase 19 | Use-After-Free / Double-Free 运行时检测：`freed_logs` 指令层检查、知识卡片 E3060/E3061 | ✅ 完成 |
| Phase 20 | 认知推理 P0：`TraceAnalyzer` 轨迹切片 + 5 类 Trap 根因推断、`RootCauseHint` | ✅ 完成 |
| Phase 21 | 认知推理 P1：`MisconceptionPattern` 6 种模式检测 + `LearningPath` 推荐引擎 | ✅ 完成 |
| Phase 22 | 认知推理 P2：`KnowledgeGraph` 24 概念节点 + 30+ 关系边、`ConceptGraphView` | ✅ 完成 |
| Phase 23 | 认知推理 P3：`ControlFlowGraph` + `DataFlow` + `IntentInference` 代码意图推断 | ✅ 完成 |
| Phase 24 | 语义智能补全 v2：`CompletionEngine` 五种上下文感知补全 | ✅ 完成 |
| Phase 25 | 模板 JIT（Trace-based Loop Accelerator）：热点循环 trace 录制 + 预优化函数指针序列 | ✅ P0 缺陷已修复 + 加速比实测（2026-09-13）：fast path 录制期禁用（红→绿闭环，`jit_nested_counting_loop*.c` 两条用例锚定）；`vm_bench` 方学校正（真禁用开关 `jit_enabled` + 统一入口）后实测 **9.16x（嵌套）/ 9.43~9.72x（单层热循环）**——旧"0.66x~0.86x 不赚反亏"为方法学假象已撤销；JIT 作用域收缩为"只 JIT 最内层循环"（含内层循环的 trace 录制必然 Abort）；J10 防线落地（`jit_path_parity.rs` 八条三锚差分，突变实测 margin=20）。详见 [`07-质量与裁定/核心资产重构裁定.md`](docs/current/07-质量与裁定/核心资产重构裁定.md) §14 |
| Phase 26 | Flutter Bridge 通信优化：Stream 模式、差分编码 `StepPayloadDelta`、符号表 dedup | ✅ 完成 |
| Phase 27 | 数据结构语法拓展 P0+P1：数组退化、`unsigned` 全链路、`const`、`extern`、VLA 全管线 | ✅ 完成 |
| Phase 28 | CLI 调试工具 `vitro_cli`：`compile`/`run`/`step`/`unified`，支持 stdin 管道快速测试 | ✅ 完成 |
| Phase 29 | Bytecode Libc 产品化：构建期预编译 + 固定索引段 + ctype/abs 走 Bytecode 路径 | ✅ 完成 |
| Phase 30 | P0 语法拓展：通用逗号运算符、Designated Initializer、offsetof + 回归修复 | ✅ 完成 |
| Phase 31 | C++ 扩展 P0：Lexer/Parser/AST 关键字与节点扩展 | ✅ 完成 |
| Phase 32 | C++ 扩展 P1：TypeChecker 类/继承/模板单态化 | ✅ 完成 |
| Phase 33 | C++ 扩展 P2：BytecodeGen 虚函数/this 指针/构造析构 | ✅ 完成 |
| Phase 34 | C++ 扩展 Stage 0.5：容器收口（list<int>/vector<char>/sort_int）、C++ 三 tier 纳入 CI、CPP_FAILURES.md | ✅ 完成 |
| Phase 35 | C++ 扩展 Stage 2：栈对象 RAII（默认构造函数自动调用 / scope exit / return / break / continue 自动析构） | ✅ 完成 |
| Phase 36 | C++ 扩展 Stage 3：`new[]/delete[]` 元素构造析构（base[-4] 存 count、逆序 dtor、temp slot 扩展至 4 个） | ✅ 完成 |
| Phase 37 | C++ 扩展 Stage 4：引用声明与基本语义（`int& r = x` 全链路；`T&` 参数/返回值；引用自动解引用；隐式取地址；返回引用左值识别） | ✅ 完成 |
| Phase 38 | C++ 扩展 Stage 5：隐式移动构造函数自动生成（类含指针/资源字段时自动生成 `__ctor__{Class}__move`；`std::move` 初始化调用移动构造；源指针字段置空防双重释放） | ✅ 完成 |
| Phase 39 | C++ 扩展 Stage 6：`unique_ptr<T>` 简化版 dogfooding + 构造函数初始化语法 `Type name(args);` + 构造函数重载/隐式默认构造 | ✅ 完成 |
| Phase 40 | C++ 扩展 M6：测试防线收尾 — 新增 59 个 C++ E2E 回归用例（核心语言 / 容器算法 / 教学 OJ），`test_vitro_e2e_cpp` 纳入 CI，Golden 由 Clang++ 生成 | ✅ 完成 |
| Phase 41 | C++ 内置容器布局解耦：`.cpp` 接口声明作为唯一真相来源 + JSON 加载器，零 Rust 硬编码 | ✅ 完成 |
| Phase 42 | P0 语法/标准库拓展 + 代码审查报告推进 + 性能优化 + `vitro_vec<T>` 类类型模板实参支持（[Unreleased]） | 🚧 进行中 |

## 测试防线

Vitro 采用**五条分层协作的测试防线**，核心哲学：*测试不是为了标榜通过率，而是为了诚实地发现自己可能存在的问题*。任何失败必须如实记录，禁止通过修改测试预期值来粉饰数据。

```
┌────────────────────────────────────────────────────────────────┐
│  防线 5：CI 集成与一致性监控（Phase F）                          │
│  └─ PR 时自动跑全部防线，*_FAILURES.md 与测试结果交叉验证        │
├────────────────────────────────────────────────────────────────┤
│  防线 4：Fuzz 压力测试（Phase E）                                │
│  └─ 随机内存状态 + 随机标准库调用序列，验证安全检测不泄漏         │
├────────────────────────────────────────────────────────────────┤
│  防线 3：三层契约验证（Phase A~C）                               │
│  ├─ 3a Host Contract：Rust 单元测试直接验证 Host Func 边界行为   │
│  ├─ 3b Bytecode Self-Consistency：C 源码 → Clang vs Vitro 自举   │
│  └─ 3c Differential Stress：同一功能多实现交叉对比               │
├────────────────────────────────────────────────────────────────┤
│  防线 2：K&R 真实程序回归（已有）+ LeetCode（已启动，阶段 4）      │
│  └─ K&R 验证"真实世界代码能不能跑"；LeetCode 简单题逐步填充中     │
├────────────────────────────────────────────────────────────────┤
│  防线 1：Shadow Verification 影子验证（已有）                     │
│  └─ 验证"与 Clang 行为是否一致"                                  │
└────────────────────────────────────────────────────────────────┘
```

### 防线 1：Shadow Verification

将同一 C 源码同时交给 **Clang** 与 **Vitro** 编译执行，对比 stdout 输出是否完全一致。Golden 只能来自 Clang，不能来自 Vitro 自己。

- **门禁**：自 2026-09-06 起为 CI 硬门禁——Clang 预检缺失时 fail fast（exit 2）；存在非预期差异（compile_gap / runtime_gap / output_gap）时 exit 1；match / known_issue / vitro_better 视为通过。`KNOWN_FAILURE_CASES` 与 E2E 防线的 `KNOWN_TEMPLATE_FAILURES` 常量对齐（双向监控：任一防线转绿需同步移除）。
- **覆盖**：321 个 Baseline 用例 + 82 个模板生成用例 + 81 个 K&R 用例 + 138 个 LeetCode 题 + 15 个 gap 用例 + 3 个 JIT 专项用例（`jit_nested_counting_loop` / `_longlong` / `jit_single_hot_loop`，2026-09-13）（C Shadow Verification 合计 667 个用例：match 660、known_issue 3（`function_pointer_sizeof` / `sizeof_array_param`——指针 4 字节模型，已在 C语言子集规范 记载的架构差异 + `bTree_default` 模板 UB）、gap_extension 4（`file_*` ×3 VFS 扩展 + `keyword_compat`——gap 目录语义即"Vitro 扩展"，驱动专用分类，U0#1② 2026-09-13）；**vitro_better = 0**（J2 闭环：16 例逐例审计，12 补头转真 golden、4 归 gap）；`spfa_default` 模板队列溢出已修转 match；统计口径含 match + known_issue + gap_extension；2026-09-13 复测）；100 个 C++ 用例（C++ Shadow Verification，98 个一致 + 2 个已记录的 `clang_compile_fail`：`cpp_vitro_vec_class` / `cpp_vitro_list_class` 使用 Vitro 内置容器无法被 Clang++ 直接编译；2026-06-28 实测，2026-09-11 复测仍一致）
- **标准输入（2026-09-11 新增能力）**：用例可自带同名 `.in` 文件，Clang 与 Vitro 喂**同一份字节**（缓存 key 纳入真实 stdin）。此前防线一律批量运行且不喂 stdin —— K&R 目录里 29 个 `.in` 从未被使用，两侧"都无输入"造成的**虚假 match**；启用后立即暴露"输入注入丢换行"缺陷（`getchar()` 读不到 `'\n'`，19 例 `output_gap`），已随 `RuntimeState::split_stdin` 统一修复
- **输出口径（E-P1-5，2026-09-11）**：比对读的是引擎的**纯程序 stdout 通道**（capi `vitro_get_program_output*`，ABI 1.1.0）——引擎附注（"程序运行完成，返回值：N"、内存泄漏报告、教学警告）与 stderr 各有独立通道。**驱动侧不得再对输出做正则清洗**：此前十余处清洗规则语义互不一致，且在教学程序自己打印同类文本时会误删真实输出（假阳性 `output_gap`），现全部废除。读取入口统一在驱动内置的结构化通道封装（共享包 `scripts/internal/capi` 的 `ReadChannel`/`PtrToGoString`；原 Python 伴生模块 `vitro_output.py` 已随主驱动退役）；DLL 缺新符号时 fail fast，不退回旧清洗。回归用例 `baseline/engine_note_lookalike.c` 固化该口径
- **驱动**：`go run ./scripts/shadow_verify`（C 侧主驱动，2026-09-13 起 D5 语言迁移**最后一站**：与 Python 版双轨对账全维度一致后接管 CI，`shadow_verify.py` 退役删除。形态：两段流水线——Clang 侧并发（`--jobs N`，0=自动 min(CPU,16)），Vitro 侧互斥串行（DLL 非线程安全实证）；实测冷启动全量重算 663 用例 ~26s、缓存热跑 ~5s。`--refresh-clang` 强制全量重算（CI 夜间防漂移）；`--rebuild` 在 release DLL 比引擎源码旧时自动重建；Clang 结果缓存为 Go 自有 schema（`go1`），与历史 Python 缓存同目录共存、key 空间不相交；瞬态环境异常（超时/启动失败/0xC0000005 映像崩溃）自动重试且**不落缓存**）、`go run ./scripts/shadow_verify_cpp`（C++ 侧驱动，2026-09-12 起 D5 第一站：Clang 并发 16 路，**24.75s → 5.2s**；工作目录自管 `.shadow_cpp_tmp/`，已 gitignore）
- **提速设施（2026-09-11 建立，Go 驱动延续）**：Clang Golden 结果缓存（key = 源码 + stdin + clang 版本 + 参数 + 预设文件）+ 并行执行；缓存与运行目录为 `.clang_cache/` / `.shadow_tmp/`（已 gitignore）——**改动用例后无需手动清缓存**（源码哈希变化自动失效）。**⚠️ 并行化对顺序敏感**：用例加载必须确定性（排序 + 按用例序重排结果），否则会出现"结果错配但门禁仍绿"的静默失败。**口径锚点（迁移中实证）**：Python `pathlib` 排序在 Windows 上是 casefold 序、Linux 上是码点序（平台相关缺陷），Go 版统一 casefold；Windows 高并发覆盖同名 `test.exe` 会触发映像加载竞态（确定性 0xC0000005），编译产物必须 per-case 唯一命名
- **报告**：`native/tests/shadow_verification/reports/`

### 防线 2：K&R 真实程序回归（已有）+ LeetCode（计划中）

收集真实教学/竞赛代码作为端到端回归用例，验证"真实世界代码能不能跑"。

- **Baseline**：`native/tests/cases/baseline/`（321 个，全绿；2026-09-06 新增 `codegen_soundness_regression.c` 固化第三批 soundness 修复；2026-09-11 新增 `engine_note_lookalike.c` 固化 E-P1-5 输出通道口径、`scanf_return_value.c` / `scanf_literal_match.c` / `scanf_literal_mismatch.c` 固化 scanf 返回值与普通字符指令）
- **K&R**：《C程序设计语言》课后习题（69 个，69 绿，0 已知失败）
- **Template Generated**：算法模板批量生成（82 个，80 绿，2 已知失败：`bTree_default` / `spfa_default`；G12 对账 2026-09-12：`infixEvaluation_default` 已修复并从失败口径移除）
- **LeetCode**：已全面实施阶段 4 + 阶段 5，当前 138 道题全部通过，详见 `native/tests/LEETCODE_FAILURES.md`
- **报告**：`native/tests/TEST_REPORT.md`、`KR_FAILURES.md`、`E2E_FAILURES.md`、`LEETCODE_FAILURES.md`

### 防线 3：三层契约验证

同一功能可能同时存在 VM Builtin、Rust Host、Bytecode Libc 三种实现，需要独立验证它们之间的一致性。

| 子层 | 目标 | 关键文件 |
|------|------|----------|
| **3a Host Contract** | 验证 Layer B Host Func 的边界条件、安全注入（UAF/Double-Free/Buffer Overflow）| `native/tests/host_contract_tests.rs` |
| **3b Bytecode Self-Consistency** | Vitro 编译器 + VM 能否正确编译并运行"自己的标准库" | `native/tests/bytecode_libc_consistency.rs` + `bytecode_libc_consistency/src/*.c` |
| **3c Differential Stress** | 同一功能的 Host 版与 Bytecode 版交叉验证，结果必须永远一致 | `native/tests/differential_stress.rs` |

- **失败记录**：`HOST_CONTRACT_FAILURES.md`、`BYTECODE_LIBC_FAILURES.md`、`DIFFERENTIAL_FAILURES.md`

### 防线 4：Fuzz 压力测试

使用**确定性 RNG** 生成随机内存状态与随机标准库调用序列，验证安全检测不泄漏、不崩溃。

| 场景 | 覆盖内容 |
|------|----------|
| **Fuzz A** | malloc/free/realloc 随机序列 + UAF/Double-Free 检测验证 |
| **Fuzz B** | strcpy/strcat/strncpy/memcpy/memmove + Buffer Overflow (E3070) |
| **Fuzz C** | printf/scanf/getchar/putchar 随机格式与输入 |
| **Fuzz D** | 混合恶意序列（内存/字符串/IO/rand 交叉） |
| **Fuzz E** | 随机分配 + 部分释放，验证泄漏报告准确性 |

- **驱动**：`cargo test --test fuzz_stress_test`
- **记录**：`native/tests/FUZZ_FAILURES.md`

### 防线 5：CI 集成与一致性监控

`.github/workflows/ci.yml` 在每次 Push/PR 时自动运行以上全部防线，并执行 `scripts/ci_three_tier_check.py` 进行一致性检查（2026-09-06 起"带牙齿"）：

- 若 `*_FAILURES.md` 中标记为 `KNOWN_FAILURE` 的测试现在全部通过 → **CI 失败（hard）**，提示更新文档标记为已修复
- 若失败记录文件本身缺失 → **CI 失败（hard）**
- 若测试失败了但文档中没有对应记录 → **[WARN] 提示添加记录（soft，不阻塞）**——文档为自由文本无法精确匹配失败用例名，硬失败会产生持续误报；精确双向对账由 `vitro_e2e.rs` 的 `KNOWN_*` 常量机制闭环承担
- cargo 编译失败或输出中无 `test result:` 行 → **CI 失败**（此前被误判 PASS）
- 生成 `reports/three_tier_report.md` 作为 CI artifact 上传

---

## 编码约定

### Rust (native)
- AST 使用 enum 替代 C++ 多态类层次：`Expr` / `Stmt` 枚举 + `Box<Expr>` / `Vec<Box<Expr>>`
- `SourceLoc` 已添加 `Copy` derive（两个 `i32`，值传递无开销）
- Parser 零进度保护：`if pos_ == checkpoint { self.advance(); }`
- 错误处理：不 panic，收集到 `Vec<Error>` 后统一返回
- Borrow checker 冲突解决模式：先 clone 数据再调用需要 `&mut self` 的方法

> 前端切割后本仓库无 Dart/Flutter 代码；历史前端约定见标签 `before-frontend-split`。

### 红→绿纪律（U0#6 规约，2026-09-13 成文）

**每个缺陷修复必须先有会失败的用例，再有修复**——杜绝"修完才写必然通过的测试"：

1. **先红**：修复提交之前，先提交（或在同一提交中先落）能暴露该缺陷的用例；本地跑出
   FAIL 输出留痕（测试名 + 失败信息），CI 上表现为该用例的 FAIL 记录。禁止把新用例
   直接标 `known_issue`/跳过来"预防"红。
2. **后绿**：修复提交的 message 引用用例名（如"红→绿锚 `jit_nested_counting_loop.c`"），
   使红→绿链可审计。**禁止修改测试预期值粉饰数据**（防线哲学第 0 条）。
3. **护栏类同理**（保险丝可触发性义务 / J9 脚本可触发性）：新增护栏、上限、判定型
   脚本必须先证明它会红（注入必然违反的输入），再上线。
4. 已实践范例（2026-09-13）：JIT fast path 修复（shadow 红 2 → 绿）、负 argc
   （panic 计数红 → 绿）、serve 边界批（撤钳位 3 FAIL → 复绿）、RSS 护栏
   （BUDGET=5 证红）、三脚本 J9 埋雷（台账：`docs/current/07-质量与裁定/脚本埋雷验证记录.md`）。

### 脚本（`scripts/`、`native/tests/`）

**默认语言：Go**（2026-09-12 起）。唯一例外是**一次性脚本**——复现 bug 的最小样例、临时探针、用完即弃的数据提取。
判据：**它会不会被别人再跑一次 / 是否进 CI / 是否成为防线的一部分**。是 → Go；否 → 随意；介于两者（探针后来转正）→ 转正时用 Go 重写。

理由（每条都有本仓库事故依据，不是风格偏好）：

1. **编码失败模式在 Go 里不存在**——Python 侧现有 **103 处** UTF-8 样板（`TextIOWrapper` / `encoding="utf-8"` / `errors="replace"`），且已付代价：Windows GBK 炸中文诊断、`\S+` 正则吞中文注释污染用例名、BOM 干扰 clang 对照。Go 字符串原生 UTF-8，`os/exec` 出 `[]byte`，**没有隐式编码转换**。
2. **编译器是跨上下文的纪律执行者**——未使用变量/import、类型错误、漏处理 `err` 一律编译失败；动态语言的隐性标准是"能跑就算对"，而 AI 的上下文会丢失，纪律靠记忆维持必然腐坏。
3. **表达空间窄**——只有一种循环、无继承、错误显式，AI 写不出"能跑但没人看得懂"的思路。此点**优于 C#**（后者 LINQ / async / record 等表达空间更大，反而更难审）。
4. 性能**不是**理由——实测 Python 侧总开销 ≈**4.1s** / 门禁全流程 **49.92s** ≈ **8.2%**。

形态约定：

- **仓库根已有 `go.mod`**（`module vitro`，2026-09-13 收尾重构引入；`go run ./scripts/<name>` 包路径形式，零第三方依赖不变）：跨驱动共享的绑定与口径**只允许**落在 `scripts/internal/` 包——`capi`（DLL 绑定 + 字符串读取 + 产物新鲜度）、`pyrandom`（CPython random 逐比特复刻）、`probeutil`（探针 CLI 定位 + psapi 采样）。新驱动默认 import 共享包，禁止再复制 helper（口径分叉是本仓库顽疾，重复即温床）；驱动各占一个子目录（`scripts/shadow_verify/main.go` 等），`go vet ./scripts/...` 必须干净；`pyrandom` 任意改动都可能使既有基线失效，必须附双轨对账证据；
- **零第三方依赖**（只用标准库：`encoding/json` / `os/exec` / `path/filepath` / `sync`），必须能在离线 CI 直接跑；
- **规则、期望值等"资产"外置为 JSON**，代码只做解释器——人审数据，不审代码；
- 自带自检的脚本（如 oracle 的事件类型清单 × 规则表 key 对账）必须 **fail loud**：自检不过直接拒绝给出判定，**禁止静默 default**。

**文档测试数字对账（`scripts/facts`，2026-09-13 起用）**：文档里的测试数字（用例数/断言数/套件数）由机器真值对账，禁止长期人肉同步。判据：CURRENT 文档的裸数字必须等于真值，否则判漂移；带日期或位于历史文档（裁定/决议/工作记录等）的数字视为 as-of 快照冻结；**分解式与实测数字行归"人工维护"**（子项与总数机判不区分，自动替换会改断算式/伪造测量）；CURRENT 文档引用的脚本路径必须真实存在（叙述性"已退役/已迁出"记载豁免）。产物：`reports/facts.json`（真值台账，含溯源与 how_to_get）、`reports/doc_fact_drift.md`。用法（**flag 必须写在子命令之前**——Go flag 在第一个位置参数处停止解析，顺序错了脚本会 fatal 拦截）：

```bash
go run ./scripts/facts check            # CI 门禁：漂移/坏引用/真值超龄任一非零即 exit 1
go run ./scripts/facts                  # 交互式逐条同步（y/n/a/d/q）
go run ./scripts/facts --yes sync       # 自动应用无警告条目（分解式/实测行跳过，人工维护）
go run ./scripts/facts report           # 只生成报告
go run ./scripts/facts --run check      # 补跑 replay/serve_smoke 刷新真值后再判
```

要点：真值采集"不猜不兜底"（取不到记 unavailable 附 how_to_get）；真值超龄（`--max-age` 默认 168h）无条件红（`--allow-stale` 仅限本地调试，CI 不得使用）；新增对账规则时 Lo/Hi 区间须覆盖当前真值（越界后规则静默失配——埋雷方法学见 `07-质量与裁定/脚本埋雷验证记录.md` facts 节）。

既有 Python 脚本的处置：

- **不强制迁移、不冻结修改**；但触碰某脚本时若改动量已接近重写，优先用 Go 重写。**D5 进度（2026-09-13，全部完成 ✅）**：① 试点 `shadow_verify_cpp.py` 完成——`scripts/shadow_verify_cpp.go` 双轨对账一致（94 用例）后接管 CI，Clang 并发 16 路 **24.75s → 5.2s**；② 第二站 `replay_s1_s5.py` 完成——`scripts/replay/replay_s1_s5.go` 双轨对账一致（61 条断言状态与编号逐行一致），并带 `--selftest` 注入自检（J9）；③ 第三站探针集 `random_diff.py` 完成——`scripts/core_asset_verdict/random_diff.go` 以 **MT19937 逐比特复刻**（同 seed 同用例集合）双轨对账一致（1000 例 verdict+expected 逐用例一致），首次建立可复现基线；④⑤ 交互切面探针、资源域长跑探针完成；⑥ **最后一站主驱动 `shadow_verify.py` 完成（2026-09-13）**——`scripts/shadow_verify.go` 与 Python 版双轨对账 **PASS**（663 用例集合/顺序/逐用例 diff_type/expected/summary/category_frequency/clang_version 全一致），CI 已切换，`shadow_verify.py` / `vitro_output.py` / `extract_shadow_cases.py` 退役删除。各站 Go 版均含启动自检 fail loud + 产物新鲜度门禁。**退役收尾（2026-09-13）**：双轨对账基准的使命随 D5 收官而终结，7 个被替代的 Python 版已退役删除（`shadow_verify_cpp.py` / `replay/replay_s1_s5.py` / `core_asset_verdict/{interaction_probe,random_diff,resource_longrun,seek_accumulation,winmem}.py`，git 历史可回取）；**仍在服役的 Python**：CI 活性四件（`ci_three_tier_check.py` / `serve_smoke.py` / `precompile_bytecode_libc.py` / `engineering_health.py`，迁移是后续批次）、一次性取证脚本（裁定 §13.7 明确不迁移）、`mutation_facet_test.py`（J3 测量工具，会再跑，迁移待办）、活性生成器（`extract_cpp_builtin_layout.py` / `sync_templates.py` / `unified_perf_baseline.py`）**⚠️ 实证发现**：DLL 并发调用 → 堆损坏（引擎非线程安全），Vitro 侧调用必须互斥；
- ~~迁移 `shadow_verify.py`（唯一硬门禁、105KB、承载 6 类隐性口径）必须新旧双轨同跑，`663 / match 644 / known_issue 3 / vitro_better 16 / 0 非预期差异` 五项一致才允许切换~~ — **已完成（2026-09-13）**：对账五项全一致后切换；迁移中新挖出三类隐性口径（`pathlib` 排序的平台差异、Go `ExitError` 与 Python 异常模型的结构性错位、同名 exe 映像竞态），均已锚定为 Go 版口径并记录于 `scripts/shadow_verify/main.go` 头注；
- **D5 收尾重构（2026-09-13，PR 评审三项全部落地）**：① `ptrToGoString` 变长窗口扫描的 UB 假设已声明，编译错误改走新增的 `vitro_get_compile_errors_length`（ABI **1.2.0**，加函数 = minor）定长读取根治，剩余调用方（engine_version / runtime_error）均为短而有界串；② scripts 建根 `go.mod` + `internal/{capi,pyrandom,probeutil}` 共享包，六驱动迁入各自子目录，删除 DLL 绑定/字符串读取/pyRandom/psapi 采样等 **~600 行**跨文件重复（pyRandom 整份 ×2、capi helper ×3-4）；③ v0.1 字段白名单外置 `scripts/replay/v01_payload_fields.json`（Go/Py 共读、fail loud，语义快照随 git 版本化——**不从引擎运行时拉取**，保持 S5 断言的"验收快照"检测语义）。剩余已知重复：serve 会话封装 ×3（replay / interaction / seek 各自的 stderr/退出语义有差异），待单独一站统一；
- 保留的 Python **判定型脚本**仍须满足 **J9**：有"注入必然违反 → 必须变红"的埋雷记录（这条是语言无关义务）。

> 完整依据与迁移顺序见 [`G-质量与裁定/核心资产重构裁定.md`](G-质量与裁定/核心资产重构裁定.md) §13（D1 防线自身 / D5 工具链语言）。

## C 教学子集支持概览

本项目支持的 C 语言教学子集覆盖 **Phase 1 ~ Phase 5+** 能力（含逗号运算符、Designated Initializer、`offsetof`、VFS 文件 I/O 等），详细规范见 [`C-语言子集/C语言子集规范.md`](C-语言子集/C语言子集规范.md)。核心支持包括：

**数据类型**：`int`、`char`、`float`、`double`、`unsigned`、`_Bool`/`bool`、`int*`、`char*`、`float*`、`double*`、`int[]`、`char[]`、`double[]`、`struct`（含按值返回）、`union`、`enum`、`typedef`

**数组**：固定大小数组（一维/多维）、**VLA 变长数组**（`int a[n]` / `int a[n][3]`，局部作用域，运行时栈分配）、数组/字符串初始化列表、数组参数退化语义

**指针**：取地址 `&`、解引用 `*`、指针算术（步长自动缩放）、**多级指针**（`int**`、`struct Node**`）、显式类型转换 `(int*)p`、函数指针（含间接调用、结构体成员、typedef）

**语句**：变量声明（含多变量、块作用域）、`if/else`、`while`、`do...while`、`for`（C99 风格变量声明）、`switch/case/default`、`break`、`continue`、`return`

**表达式**：算术、比较、逻辑（短路求值）、位运算 `& | ^ ~ << >>`（含 `long long` 64 位变体）、赋值（含复合赋值；指针支持 `+=` / `-=` 整数，按 pointee 大小缩放，`void*` 按 1 字节扩展）、三目运算符 `?:`、**`_Generic` 泛型选择（C11）**、数组索引、函数调用、`&`、`*`、结构体访问 `.` / `->`、`++` / `--`、`sizeof`、**`_Alignof` / `alignof`（C11/C23）**、**`0b` 二进制字面量与 `'` 数字分隔符（C23）**、**相邻字符串字面量拼接（C89）**、**科学计数法浮点字面量 `1.5e-3`（C89）**

**函数**：定义/调用/递归/前向声明、**函数按值返回结构体**（Hidden Return Pointer ABI）

**内存**：`malloc`/`free`/`realloc`

**I/O**：`printf`/`scanf`/`sprintf`/`snprintf`/`sscanf`/`fprintf`/`fgets`/`fputs`/`puts`/`getchar`/`putchar`/`ungetc`；VFS 沙盒文件 I/O：`fopen`/`fread`/`fwrite`/`fclose`/`feof`/`fgetc`/`fputc`/`fseek`/`ftell`/`rewind`

**字符串**：`strlen`、`strcpy`、`strncpy`、`strcmp`、`strncmp`、`strcat`、`strncat`、`memcpy`、`memmove`、`memcmp`、`strchr`、`strrchr`、`strstr`、`memchr`、`strdup`、`atoi`

**数学**：`sin`/`cos`/`tan`/`sqrt`/`pow`/`atan`/`log`/`log10`/`exp`/`fabs`/`abs`/`ceil`/`floor`/`round`/`fmod`（通过 `libm`，`double` 精度）

**类型系统**：`typedef`、`sizeof`、`const`、`static`（局部+全局+函数）、`extern`、`volatile`、`restrict`、`inline`、`register`、`auto`、**`typeof` / `typeof_unqual`（GCC/C23）**、**`enum E : T` 底层类型（C23）**

**头文件**：`#include <stdio.h>` / `<stdlib.h>` / `<ctype.h>` / `<math.h>` / `<string.h>` 加载存根声明

**其他**：`rand`/`srand`、`memset`、`exit`、`qsort`、`calloc`、`bsearch`、`atof`/`atol`、`#define` 宏（对象宏/参数化宏/嵌套调用）、**模块化预处理器（E2：`#`/`##`、`#if`/`#elif`、`defined`、`__has_include`、`#undef`、include-once、环检测、遮蔽/副作用警告、展开链教学追踪，详见 `C语言子集规范.md` §2.11）**、**C23 语义级（E3：`nullptr`、`static_assert`/`_Static_assert` 真求值、`constexpr`（按 const 口径）、`[[属性]]` 解析忽略、`unreachable()` 教学 trap，详见 §2.12）**、`stdarg.h` 变参函数（`va_list`/`va_start`/`va_arg`/`va_end`/`va_copy`，支持 `int`/`double`/`long long` 等类型）、**`__func__` 预定义标识符（C99）**、**`limits.h` 全宏（含 `ULLONG_MAX`）与 `<float.h>`**

**字符分类**：`isdigit`/`isalpha`/`islower`/`isupper`/`isalnum`/`isspace`/`isprint`/`iscntrl`/`isxdigit`/`tolower`/`toupper`（`ctype.h`，部分走 Bytecode Libc 路径）

**C++ 类与模板（Phase 31+）**：`class`、成员访问控制、`this` 指针、虚函数、模板类单态化、栈对象 RAII（自动构造/析构）、构造函数初始化语法 `Type name(args);`、隐式默认构造/移动构造、`std::move`、简化版 `unique_ptr<T>` dogfooding（构造/`get`/`release`/`reset`/析构/所有权转移）

**明确不支持**：bitfield、全局 VLA。预处理器为 E2 模块化内核（§2.11，覆盖宏全族/条件编译全族/include 解析图）；C23 锚定的完整支持清单与已知差异（浮点字面量 double 语义、IEEE 精确比较、struct packed 布局、指针 4 字节模型）见 `C语言子集规范.md` §2.10~§2.11

**C++ 子集边界（诚实记录）**：`vitro_vec<T>` / `vitro_list<T>` 已支持类类型模板实参；`const T&` 参数已支持绑定到字面量、变量与表达式右值；**默认参数**、**嵌套类 `Outer::Inner` 实例化**、**类模板非类型模板参数（NTTP，如 `Array<int, 5>`）**、**自定义拷贝构造函数（`Class(const Class&)`）** 已支持；函数模板显式 `<>` 调用等特性暂不支持（2026-06-26 记录）。这些限制在 Vitro C++ 教学子集当前 Stage 0~6 范围内尚未覆盖，后续按教学需求逐步扩展。

## 已知限制

### 当前不支持
- ~~**参数化宏调用后带分号**~~ — **已修复（扩展支持，2026-06-25）**。`vitro_lexer` 在参数化宏展开时，若宏体为大括号块且调用位置后紧跟分号，则动态将宏体包装为 `do { ... } while(0)`，使 `SWAP(int,x,y);` 在 `if/else` 等语句中可正确解析。新增 `end_to_end_extra_test::test_e2e_parametric_macro_swap_semicolon` 回归测试。
  - ⚠️ **与 Clang 的行为差异**：Clang 标准模式对 `if (...) { ... }; else ...` 会报"预期表达式"错误；Vitro 通过自动包装支持该教学常见写法。若需严格兼容 Clang，仍应手动使用 `do { ... } while(0)` 宏体。
- ~~**VLA 边界检查**~~ — **已修复（2026-06-25）**。`gen_index` 对 VLA 首维为变量维度的场景生成运行时边界检查：新增 `TrapBoundsVla` opcode，在索引前计算 VLA 维度表达式并压栈，VM 运行时将索引与运行时边界比较；新增 `baseline/vla_bounds.c` 回归用例。参数退化为指针的 VLA 形参（如 `void f(int n, int a[n])`）仍无法在编译期获知边界，保持跳过。
- ~~**`#include` 非标准库路径**~~ — **已修复（2026-06-25；E2 批次进一步增强）**。`#include "header.h"` 可基于源文件所在目录加载自定义头文件；标准库走存根路径。E2 后：嵌套 include 按"包含者目录优先"候选链解析、include-once 内置、依赖环静态检测（E1015）。新增 `baseline/include_custom_header.c` / `include_custom_header.h` 与 `baseline/e2_include_*.c` 系列回归用例。绝对路径与系统 include 搜索路径（`<>` 非标准库目录）仍待扩展。
- ~~**`va_list` / `va_start` / `va_arg` / `va_end`**~~ — **已修复（2026-06-25）**。自定义变参函数现可全链路工作：`va_list` 使用 `char*` 模拟，`va_start`/`va_arg`/`va_end` 通过内部 Host 函数实现，`va_arg` 按目标类型直接解引用读取。支持 `int`、`double`、`long long` 等常见类型（遵循 C 默认实参提升：`float` → `double`，`char` → `int`）。新增 `baseline/variadic.c` 回归用例。
- **全局 VLA** — 全局/静态作用域的变长数组按 C99 标准本身即不允许（Clang 报错 "variable length array declaration not allowed at file scope"），Vitro 保持不支持。
- **VFS 文本模式换行转换（已修复）** — 2026-06-15 已完整实现 Windows 文本模式换行转换：`"r"`/`"w"` 模式下写入时将 `\n` 展开为 `\r\n`，读取时将 `\r\n` 压缩为 `\n`；`fseek`/`ftell` 区分逻辑/物理光标以匹配 Windows CRT 行为。`vfs_io_extensions.c` 与 `file_fread.c` 已恢复匹配。

### 已知 Vitro 与 Clang 的行为差异（诚实记录）

在 LeetCode 防线填充过程中发现以下 Vitro 与 Clang 行为不一致：

- ~~**复合副作用数组索引**~~ — **已修复（2026-06-25）**。根因是 `gen_mem_inc_dec`（自增/自减内存操作）与 `gen_assign` 的 Index 赋值复用了同一个临时槽位（`temp_slot0`），导致右侧索引表达式的副作用覆盖了左侧地址临时变量，最终在赋值表达式返回值读取时触发 NULL 指针陷阱。修复方案为 `gen_mem_inc_dec` 改用 `temp_slot3` 保存新值；新增 `baseline/side_effect_index.c` 回归用例。
- ~~**函数返回 `double` 值异常**~~ — **已修复（2026-06-24）**。根因是 `return` 语句未对返回值表达式插入隐式类型转换，导致 `return 2.5;`（`2.5` 被解析为 `float` 字面量）在函数返回类型为 `double` 时实际生成 `PushConstF` 而非 `PushConstD`。修复后 TypeChecker 在 `return` 语句的 `check_assignable` 成功后调用 `insert_implicit_cast`，并在 `baseline/float_func_return.c` 增加回归用例。
- ~~**`scanf` 的 `%s` 格式符暂不支持**~~ — **已修复（2026-06-25）**。`scanf`/`sscanf`/`fscanf` 中的 `%s` 现在可正确读取空白分隔的字符串并写入目标缓冲区；新增 `baseline/scanf_string.c` 回归用例（基于 `sscanf`，避免 Shadow Verification 用例间输入不可控问题）。
- ~~**`fputs(str, stdout)` 无输出**~~ — **已修复（2026-06-19）**。`fputs` 现在可正确写入 `stdout`/`stderr`（通过 lexer 预定义宏 fd=1/2）并输出到程序 stdout；写入普通 `FILE*` 文件流行为保持不变。
- **`fprintf` 到自定义 `FILE*` 不落盘** — **既有偏差（2026-09-11 记录，E-P1-5 顺带核查）**。`host_fprintf_n` 当前不解析 `stream` 实参：写入 `stderr` 已按 E-P1-5 正确分流到 stderr 通道，但 `fprintf(fp, ...)`（`fp` 来自 `fopen`）不会写入 VFS 文件，而是被当作 stdout 输出（与 Clang 不一致）。教学子集里 `fprintf` 主要配合 `stderr` 使用，既有 633 个 Shadow 用例未触发该差异；需要写文件时请用 `fputs`/`fwrite`/`fputc`。
- ~~**`fclose` 后 VFS `FILE*` 仍被报告为内存泄漏**~~ — **已修复（2026-06-25）**。根因是 `host_fclose` 仅关闭 VFS 文件描述符，未释放 `host_fopen` 在 VM Heap 中为 `FILE*` 结构体分配的 4 字节内存。修复方案为在 `host_fclose` 中调用 `MemoryState::free_region(stream)` 释放该内存；stdout/stderr 等非堆分配 stream 找不到对应 region，安全忽略。新增 `baseline/fclose_leak.c` 回归用例。
- **指针复合赋值 `+=` / `-=`** — **已支持（2026-06-28）**。`int* p; p += n;` 与 `p -= n;` 全链路支持，按 pointee 大小缩放；`void* p; p += n;` 按 GCC/Clang 扩展按 1 字节处理。函数指针算术、指针与指针的 `+=` / `-=`、以及其他复合赋值运算符（`*=`、`/=` 等）保持报错。新增 `baseline/pointer_add_assign*.c` 系列回归用例。
  - ⚠️ **与 Clang 的行为差异**：`void*` 算术属于 GCC/Clang 扩展，严格 C 标准未定义；教学中应引导学生优先使用具体类型指针。复合赋值表达式返回值在 Vitro 中为右值指针，与 C 标准左值语义存在差异，但教学场景通常不依赖此差异。
- **`_Generic` 泛型选择（C11）** — **已支持（2026-06-28）**。`_Generic(expr, type1: expr1, type2: expr2, default: expr3)` 全链路支持，编译期根据控制表达式类型匹配关联表达式并生成选中分支字节码。字符串字面量等数组类型会先执行数组到指针退化再匹配（如 `"hi"` 匹配 `char*`）。新增 `baseline/c11_generic.c` 回归用例，Shadow Verification 与 Clang 输出一致。
  - ⚠️ **与 Clang 的行为差异**：Vitro 当前按精确类型匹配（含数组退化）选择分支，未实现 C11 完整的类型兼容规则（如 `int` 与 `signed int` 的兼容、qualifier 忽略等）。教学场景通常使用明显不同的类型（`int` / `double` / `char*`）做分发，此差异可接受。
- **复合字面量（C99/C11）** — **已支持（2026-06-28）**。`(struct S){1,2}`、`(int[]){10,20,30}`、`(int){5}` 全链路支持，可用于变量初始化、取地址、直接成员访问。新增 `baseline/compound_literal.c` 回归用例，Shadow Verification 与 Clang 输出一致（`1 5 20 7`）。
  - ⚠️ **与 Clang 的行为差异**：复合字面量生命周期简化为当前块结束，教学场景不跨块/函数使用；`int[]` 等未指定大小数组的复合字面量通过初始化列表长度推断大小；复杂嵌套/多级 designated initializer 暂按教学子集处理。
- ~~**全局/静态数据段与堆区共享线性内存（潜在静默损坏）**~~ — **已修复（2026-09-11，重构批次 R1：动态堆起点）**。全局变量、静态变量与字符串字面量仍自 `GLOBAL_START`（`0x1000`）向上分配，但堆起点不再写死 `HEAP_START`（`0x5000` = 20 KB）：运行入口按 `heap_base = max(HEAP_START, align4(global_data_end))` 动态计算（codegen 导出全局数据末端绝对地址）；程序带命令行参数时，argv 改自全局区上界 `GLOBAL_REGION_LIMIT`（`0x10000` = 64 KB，取代 `gen_string_literal` 的 `MEM_SIZE/16` 魔数）向下分配，堆起点相应上移至 64 KB。全局区所有 bump（全局变量 / extern 占位 / vtable / 字符串字面量 / 静态局部变量）统一走 codegen `bump_global_offset` 单一入口，越过 `GLOBAL_REGION_LIMIT` 编译期报错（fail loud，不再静默放行）；全局数据越过 `HEAP_START` 时产生编译 warning 提示堆起点上移与剩余堆空间。`lc_22` / `lc_977` 等"全局区越过 20 KB 但不用堆"的存量用例行为不变。回归：`native/tests/r1_memory_boundary_test.rs`（大全局+malloc 数据完好 / malloc 耗尽明确返回 NULL / 深递归明确 trap / argv 不与全局数据重叠 / 容量上限编译失败 / 大全局 warning）。
  - 历史背景：该风险与 2026-09-11 修复的 `BYTECODE_LIBC_GLOBALS_RESERVED` 自我递增漂移**同源**（都是"全局区边界无单源判据"的结构病）：漂移曾把用户全局区压缩到不足 1 KB，使 `lc_67` 的 `static char res[1000]` 直接溢出到堆区并打印出错乱内容（详见 `CHANGELOG.md [Unreleased] Fixed`）；本批修复后布局判据单源化到 `vitro_runtime`（`GLOBAL_REGION_LIMIT` / `compute_heap_base`）。

- ~~**JIT trace 路径的静默错值（2026-09-13 定位）**~~ — **已修复（2026-09-13 同日，红→绿闭环）**。历史缺陷（与本引擎"解释器路径"不一致，非 C 标准问题）：嵌套纯计数循环下
  ```c
  int main(){int i,j,inner=0;
  for(i=0;i<200;i++){for(j=0;j<200;j++){inner=inner+1;}}
  printf("inner=%d i=%d j=%d\n",inner,i,j);return 0;}
  ```
  `vitro_cli run`（走 executor + JIT）曾输出 `inner=20200 i=200 j=0`（正确：`inner=40000 i=200 j=200`），**静默错值、零诊断**。
  - **归因**：与 `long long` 无关（纯 int 版同样出错）。触发条件：外层回边达 `JIT_THRESHOLD=100` 且内层已 JIT 化 且外层循环体无分支。根因：JIT fast path（`core/executor/mod.rs`）在 trace 录制期间仍生效——外层录制命中内层 trace 被 bulk 跑完，外层 trace 缺失内层指令却被注册。
  - **修复**：fast path 加 `!trace_recorder.is_recording()` 判断。副作用即"作用域收缩"：含内层循环的 trace 录制必然于内层回边 Abort，**JIT 只作用于最内层循环**。回归锚定：baseline `jit_nested_counting_loop.c` / `_longlong.c`（先红后绿）+ `jit_single_hot_loop.c` + `native/tests/jit_path_parity.rs`（八条三锚差分）。
  - **顺带撤销的结论**：`vm_bench.rs` 两处方法学缺陷（`clear()` 不禁用 / 双分支入口不同）曾得出"JIT 0.66x~0.86x 不赚反亏"——校正后实测 **9.16x（嵌套）/ 9.43~9.72x（单层）**，Phase 25 加速比声明更新为实测值。完整复核与 D6 存废裁定见 [`07-质量与裁定/核心资产重构裁定.md`](docs/current/07-质量与裁定/核心资产重构裁定.md) §14。

> 历史特性详情和 Bug 修复记录见 [`CHANGELOG.md`](CHANGELOG.md) 和 [`C-语言子集/C语言子集规范.md`](C-语言子集/C语言子集规范.md)。

## 构建命令

```bash
# 构建 Rust 引擎（Debug / Release）
cd native && cargo build            # Debug
cd native && cargo build --release  # 输出: native/target/release/vitro_native.dll

# 构建 CLI 调试工具
cd native && cargo build --release --bin vitro_cli

# 构建 wasm32 出口（浏览器/白箱形态；冒烟实证 3.75MB）
cd native && cargo build --target wasm32-unknown-unknown --release

# 测试与静态检查
cd native && cargo test --workspace --all-features
cd native && cargo clippy --workspace --all-targets --all-features -- -D warnings

# Shadow 防线（C / C++）
go run ./scripts/shadow_verify
go run ./scripts/shadow_verify_cpp

# serve 协议冒烟
cargo build --bin vitro_cli && python scripts/serve_smoke.py
```

> **磁盘卫生（U0#4，2026-09-13 制度化）**：① 已死交叉 target 不得残留——android
> 三目录（`aarch64-linux-android` / `armv7-linux-androideabi` / `android`，合计
> ~2.6GB）已随前端切割删除，重建它们前先确认确有需要；② CI 在 health 报告后
> 有 **target 体积预算门禁（10GB）**，超限 fail 并回显分布——本地 `target/debug`
> 长期累积超 10GB 时建议 `cargo clean`（重建成本 ≈ 一次全量构建）；③
> `wasm32-unknown-unknown` 是活性出口（出口 2）的构建产物，不属于清理对象。
> **RSS 护栏（U0#2）**：`serve_smoke.py` 第三批在远距 seek 压力形状下监控 serve
> 子进程提交峰值（ctypes psapi，与 `scripts/internal/probeutil` 同口径），默认
> 预算 512MB（当前基线的宽松护栏，U2 完成后按 J5 收紧）；`VITRO_RSS_BUDGET_MB=5`
> 可证红（先证会红义务已履行，2026-09-13 实测 peak=75MB > 5MB → exit 1）。

> 历史前端构建（Flutter / Android / iOS）已随前端迁出，脚本见标签 `before-frontend-split`。

## 调试技巧

### Native 层调试 (Rust)
1. 项目属性 → 调试 → **启用本机代码调试**
2. 在 `native/src/capi/mod.rs` 的 `vitro_compile_all` / `vitro_run` 打断点
3. PDB 警告（`apphost.pdb` 缺失）可以安全忽略
4. 无前端调试：`vitro_cli step <file>` 交互式单步（`p` 打印变量 / `o` 打印输出），或 `vitro_cli serve` JSON-lines 会话

### 内存泄漏定位
- Parser 死循环特征：内存缓慢持续增长（~100MB/秒），AST 节点或错误消息不断累积

## CLI 调试工具

项目提供独立的命令行调试工具 `vitro_cli`，直接操作 Rust 后端编译器/VM（无前端依赖，headless 调试的第一入口）。

### 构建

```bash
cd native && cargo build --release --bin vitro_cli
```

### 命令

| 命令 | 说明 |
|------|------|
| `compile <file>` | 编译并显示诊断信息（错误码 + 修复建议） |
| `run <file>` | 编译并全速运行 |
| `step <file>` | 交互式单步调试（支持 `p` 打印变量、`o` 打印输出、`r` 运行到结束、`q` 退出） |
| `unified <file>` | 统一模式（时间旅行引擎）批量执行并输出摘要（支持 `--max-steps <n>`） |
| `export <file1> [file2 ...] -o <out.json>` | 预编译为字节码产物（多文件 + `--builtin-libc` 选项） |
| `serve` | JSON-lines 会话模式（stdin 请求 / stdout 响应，id 关联 + 错误帧同构 + `session.reset`；与 capi 共用 `session_api` 入口语义） |

### 选项与特殊文件名

- `-i <file>`：从文件读取标准输入（多行输入供 `scanf`/`fgets` 使用）
- `--max-steps <n>`：统一模式下允许的最大执行步数（默认 100_000），用于长程序时间旅行或性能基线测试
- `-`：从标准输入读取源代码，便于快速测试代码片段

### 快速测试示例

```bash
# 管道直接运行
echo '#include <stdio.h>
int main() { printf("hello\n"); return 0; }' | vitro_cli run -

# here-document 编译
vitro_cli compile - <<'EOF'
#include <stdio.h>
int main() {
    int a = 10, b = 20;
    printf("%d\n", a + b);
    return 0;
}
EOF

# 带输入文件运行
vitro_cli run sum.c -i input.txt

# 统一模式执行
vitro_cli unified hello.c

# 统一模式执行并放宽步数限制（用于长程序或性能基线）
vitro_cli unified long_sort.c --max-steps 500000

# 预编译字节码产物（含 Bytecode Libc）
vitro_cli export main.c libc_helper.c -o bundle.json --builtin-libc
```

完整文档见 [`B-构建与上手/CLI使用手册.md`](B-构建与上手/CLI使用手册.md)。

