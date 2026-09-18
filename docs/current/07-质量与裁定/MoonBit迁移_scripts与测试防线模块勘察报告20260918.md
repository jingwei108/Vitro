# MoonBit 迁移 · `scripts/`（Go 驱动集）+ `native/tests/`（测试与防线）模块勘察报告

> **勘察对象**：`scripts/`（golden 与防线驱动链，21 个 Go 文件）、`native/tests/`（全部测试与用例目录）、`reports/facts.json`（真值台账）
> **勘察时间**：2026-09-18
> **性质**：**只读勘察**——未修改任何文件、未执行任何 git 写操作、未切换工具链
> **证据纪律**：每条结论附 `file:line`；全局数字引用 `reports/facts.json` 的 key + `as_of`，不复用评估报告的 AS-OF 冻结数字；不确定项标"待证"并给验证方法；未引用 `docs/archive/` 下任何文档
> **关联文档**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)（§2 事实核查 / §3.3 驱动链与裸奔期 / §7 四道证伪门 / §8 假设 A1–A9）
> **环境事实（本次实测）**：`moon 0.1.20260915 (2e1a46d 2026-09-15)`、`node v25.9.0`、`clang version 22.1.4`（与 `shadow_data_latest.json` 记录的 clang 版本一致，as_of `2026-09-15 00:05:33`）

**与任务书的两处实测偏差（已核实，非估算）**：

1. `scripts/` 下 Go 文件 **21 个 / 9,922 行**（`git ls-files '*.go'` + 逐文件非空行计数实测）。任务书"10,767 行"应为含缓存目录/空行口径差异（`scripts/core_asset_verdict/.randomdiff/` 等构建残渣不计入）。
2. `native/src/` 下**有 7 个本地模块目录**（`bin` / `capi` / `compiler` / `diagnostics` / `engine` / `shared` / `unified`，各有非空行：861 / 978 / 2,681 / 2,222 / 1,988 / 11 / 3,383）+ 3 个根文件（`lib.rs` / `session.rs` / `session_api.rs`），非任务书所称"6 个"。

---

## ① 模块概览与规模（实测数字）

### 1.1 Go 驱动集（21 文件 / 9,922 行，零第三方依赖）

| 驱动 | 行数 | 定位 | 与引擎的通道 |
|---|---|---|---|
| `scripts/shadow_verify/main.go` | 1,319 | 防线 1 主驱动（唯一硬门禁） | `syscall.LazyDLL` → `vitro_native.dll` |
| `scripts/shadow_verify_cpp/main.go` | 712 | C++ 影子验证 | 同上（DLL） |
| `scripts/serve_smoke/main.go` | 834 | 出口 3 JSON-lines 协议冒烟 + RSS 护栏 | 子进程 `vitro_cli serve`，stdin/stdout 行协议 |
| `scripts/replay/replay_s1_s5.go` | 1,055 | S1–S5 回放断言（61 条） | 子进程 serve + DLL 混用 |
| `scripts/core_asset_verdict/random_diff/main.go` | 932 | 三路差分（生成器→clang / Vitro / 语义模型） | DLL + clang 子进程 |
| `scripts/core_asset_verdict/interaction_probe/main.go` | 643 | serve 交互序列 fuzz（固定种子加权） | 子进程 serve |
| `scripts/precompile_bytecode_libc/main.go` | 454 | Bytecode Libc 产物同步门禁（`--check`） | 子进程 `vitro_cli export` |
| `scripts/engineering_health/main.go` | 427 | 工程健康度看板（无红绿判定、无 J9 义务） | 读源码/产物/报告 |
| `scripts/ci_three_tier_check/main.go` | 405 | 防线 5 台账一致性（9 套件） | 子进程 `cargo test` |
| `scripts/facts/main.go` + `facts.go` + `audit.go` | 299 + 514 + 756 | 文档数字机器对账（采集 / 对账 / 同步） | 读产物 + 可选跑防线 |
| `scripts/facts/const_audit_test.go` + `offsets_test.go` | 158 + 91 | 对账层自身的证红锚（J9） | 纯单测 |
| `scripts/core_asset_verdict/seek_accumulation/main.go` | 250 | seek 内存累积判定 | 子进程 serve + psapi |
| `scripts/core_asset_verdict/resource_longrun/main.go` | 231 | 资源长跑三路径 | 子进程 serve + psapi |
| `scripts/core_asset_verdict/regions_growth/main.go` | 173 | regions 条目数四点采样 | 子进程 serve |
| `scripts/internal/capi/capi.go` | 268 | **共享包**：DLL 绑定 / 字符串读取 / 输出归一 | — |
| `scripts/internal/pyrandom/pyrandom.go` | 211 | **共享包**：CPython `random` 逐比特复刻 | — |
| `scripts/internal/probeutil/probeutil.go` | 115 | **共享包**：`vitro_cli` 定位 + psapi 采样 | — |
| `scripts/gosmoke/cabi_smoke.go` | 75 | C ABI 冒烟（版本串往返） | DLL |
| `scripts/wasm_smoke/wasm_smoke.js` | 79 | 出口 2 wasm32 冒烟（Node + Proxy 桩） | wasm 实例 |

共享包单源化与"仓库根 `go.mod`（module vitro）+ 零第三方依赖"纪律见 `AGENTS.md` 脚本章与 `scripts/internal/capi/capi.go:1-16`。

### 1.2 Rust 测试侧（`native/tests/`）

| 维度 | 实测 |
|---|---|
| 顶层 `.rs` | **49 个**；`#[test]` 合计 **900 个** |
| 用例数据 | `cases/baseline` 359 `.c` + 7 `.h` + 5 `.in`；`cases/gap` 15；`cases/knr` 81 `.c` + 29 `.in`；`cases/leetcode` 138；`cases/cpp` 83；`cases_template_generated` 82 |
| golden `.out` | **733 个**：`cases_golden/` 顶层 82（模板）+ `baseline/` 353 + `cpp/` 79 + `knr/` 81 + `leetcode/` 138 |
| 失败台账 | 13 个 md（`KR_FAILURES.md` 597 行最长） |
| 引擎内部耦合 | 108 处 `use vitro_native::`；11 处直引 `vitro_runtime`/`vitro_vm` 内部 crate |

> `#[test]` 总数与评估报告附录 B 的 1,015（`MoonBit迁移方案评估报告20260918.md:339-347`，AS-OF 2026-09-18）**不同口径**：附录 B 统计全 workspace（含 `crates/*/src` 内联单测），本节统计的是 `native/tests/` 集成测试文件——引用时须带口径。**待证**：全 workspace 现值需 `grep -rn "#\[test\]" native --include=*.rs | wc -l` 复测。

### 1.3 真值台账（`reports/facts.json`，generated_at `2026-09-18T13:01:41+08:00`，`rev 4b57191 (dirty)`）

**14 个事实键**，三种 provenance：

| key | 值 | provenance | as_of | 状态 |
|---|---|---|---|---|
| `abi_version` | `"2.1.0"`（svalue） | `read_const`（`native/src/capi/first_batch.rs`） | 2026-09-14T23:15:15 | ok |
| `c_e2e_baseline_cases` | 359 | `count_dir` | 采集时刻 | ok |
| `c_e2e_gap_cases` | 15 | `count_dir` | 采集时刻 | ok |
| `c_e2e_knr_cases` | 81 | `count_dir` | 采集时刻 | ok |
| `c_e2e_leetcode_cases` | 138 | `count_dir` | 采集时刻 | ok |
| `cpp_e2e_cases` | 83 | `count_dir` | 采集时刻 | ok |
| `cpp_failures_active` | 1 | `parse_markdown` | 2026-09-14T00:55:13 | ok |
| `e2e_failures_active` | 1 | `parse_markdown` | 2026-09-14T00:55:13 | ok |
| `replay_assertions` | 61 | `run` | 2026-09-14T01:56:25 | **cached** |
| `serve_smoke_assertions` | 57 | `run` | 2026-09-18T01:19:58 | **cached** |
| `shadow_c_cases` | 675 | `read_report` | 2026-09-15 00:05:33 | ok |
| `shadow_c_match` | 668 | `read_report` | 同上 | ok |
| `shadow_cpp_cases` | 99 | `read_report` | 2026-09-15T00:05:40 | ok |
| `shadow_cpp_match` | 95 | `read_report` | 同上 | ok |
| `cargo_test_passed` / `cargo_test_suites` | null | `none` | — | **unavailable**（需 `--run-slow` 或 `--cargo-log`） |

**注意**：`reports/facts.json` 本身被 `.gitignore:62` 忽略（`reports/` 整目录），只在 CI artifact 与本地存在——迁移后若指望它当"制度"而非"产物"，必须换存放位置。

**影子报告实测明细**（`native/tests/shadow_verification/reports/shadow_data_latest.json`，`timestamp 2026-09-15 00:05:33`，`clang_version clang version 22.1.4`，`config {jobs:16, elapsed_sec:1.5, clang_cache hits:675 misses:0}`）：

- C 侧 `total 675 / match 668 / known_issue 3 / gap_extension 4`，`compile_gap / runtime_gap / output_gap` 全 0，`category_frequency {}`；
- 3 例 `known_issue`：`function_pointer_sizeof`、`sizeof_array_param`（`expected=arch_diff_bug`）、`bTree_default`（`expected=baseline`）；
- 4 例 `gap_extension`：`file_fopen` / `file_fread` / `file_fwrite` / `keyword_compat`；
- C++ 侧 `cpp_shadow_report.json`：99 例 = 95 `match` + 4 `clang_compile_fail`；
- `kr_leetcode_report.json`：`knr 81/81 match`、`leetcode 138/138 match`，`gaps []`。

### 1.4 目录组织债（实测）

- `native/tests/bytecode_libc_consistency/test_force.exe`（142,336 B，孤儿二进制，`drivers/` 无 `test_force.c`，`.gitignore:90` 已忽略）
- `native/tests/test_detect.exe`（143,360 B）+ `test_detect.pdb`（1,282,048 B，内嵌旧路径 `D:\code\c_ide_rust\...`）+ `test_detect.rs`（48 行、**0 个 `#[test]`**，连源码也被 `.gitignore:88` 忽略）
- `native/tests/safe_run.ps1`（56 行，仓库内唯一引用在 `docs/archive/`，已脱线；`:46-47` 存在"退出后才 `ReadToEnd`"死锁风险）
- `native/tests/shadow_verification/{gen_cases.py, test_massive.py, test_more.py, new_cases.txt}`：Python 主驱动退役后的残留，仍被 git 跟踪（`git ls-files` 实证）

---

## ② 可复用资产清单

| # | 资产 | 形态/位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| A1 | **Clang golden `.out` 全量 733 个** | `native/tests/cases_golden/**`（git 跟踪 706 个文件，含 733 `.out`） | **低** | 不依赖实现语言（与评估报告 §3.3 第 2 点、假设 A8 判定一致）；`vitro_e2e.rs:128-137` 的 `load_golden()` 是现成消费范式。⚠️ 有 3 处覆盖缺口与 1 处口径偏差，见 §④/§⑥-P10 |
| A2 | **用例数据（`.c`/`.cpp`/`.in`/`.h`）** | `cases/**`（合计 758 个用例文件） | **低** | 纯文本资产；`.in` 的 stdin 语义已由 `shadow_verify` 固化（同名 `.in` → 两侧同一份字节，`main.go:674-678`） |
| A3 | **判定树与启动自检断言** | `shadow_verify/main.go:147-197`（`analyzeDiff` 12 条 + `classifyCompileError` 3 条）；C++ 版 `shadow_verify_cpp/main.go:393-418`（7 条） | **低** | 全是对纯数据结构的分类逻辑，语言无关；自检本身就是"先证会红"的 J9 范例 |
| A4 | **六类隐性口径（头注契约）** | `shadow_verify/main.go:3-39` 头注 | **低**（口径要抄，实现要重写） | 见 §⑥"口径清单"，本模块最值钱的非代码资产 |
| A5 | **Clang 结果缓存 schema 设计** | `main.go:336-458`（`cacheMaterial` 11 字段 + sha256 + 原子落盘） | **中** | 设计可平移（源码+stdin+clang版本+参数+超时+预设文件+同目录头文件摘要）；Go 结构体序列化的 key 不可复用，**key 空间必须换名**（`go1` → 新 schema），否则跨实现误命中 |
| A6 | **三层契约语义文本** | `host_contract_tests.rs`（102 契约：`malloc(0)`/负/超额 `:48,60,97`、`free(NULL) :108`、越界必须 E3070 `:539,557`）；`differential_stress.rs`（18）；`bytecode_libc_consistency.rs`（12） | **中** | 断言对象是 C 库函数的边界语义，与语言无关；但驱动方式（`host_*` 逆序压栈 196 处）是 Rust 机制特有 |
| A7 | **KNOWN_* 双向对账机制** | `vitro_e2e.rs:209-521`（5 常量 + 4 个反向 panic 测试） | **低** | 本仓库最值钱的测试机制；MoonBit 版可直接用数组常量 + 4 个反向 test 复刻 |
| A8 | **fuzz A–E 场景矩阵 + SplitMix64** | `fuzz_stress_test.rs:43-49`（生成器）+ 5 个固定种子（`:260,509,662,812,902`） | **低** | RNG 是纯算法，可逐位复刻；判据"非法操作必须触发 E3060/E3061/E3070、合法操作不得 trap"是规范文本 |
| A9 | **J10 三锚判据模板** | `jit_path_parity.rs:71-107`（parity / 手算期望 / 生效性自检） | **低** | 即使不做 JIT，"任何快路径必须三锚"可复用（对应"兜底路径 vs 快路径必须等价"的通用形状） |
| A10 | **CPython `random` 逐比特复刻** | `scripts/internal/pyrandom/pyrandom.go`（226 行） | **中** | **它是"测试侧宿主 RNG"，不是引擎侧 `rand()`**（判定见下）；八条语义需在 MoonBit 重写并双轨对账 |
| A11 | **facts 制度（三层判据 + 溯源 + 新鲜度）** | `facts.go:1-9`（三原则）、`audit.go:5-11`（CURRENT/AS-OF/DERIVED 三分）、`facts.go:486-519`（`demoteStale` 超龄降级） | **中** | 制度整体可继承；实现要重写（Go regex + 字节偏移替换 → MoonBit） |
| A12 | **常量对账 + 8 条证红锚单测** | `audit.go` 的 `auditConst` + `const_audit_test.go:45-141` | **低** | 8 条夹具全部取自真实漂移样本，"多版本串同行""迁移箭头豁免"等边界是语义资产 |
| A13 | **`applyHit` 同行多命中偏移契约** | `offsets_test.go:23-99`（3 条） | **中** | "先替换长度不同的数字导致后续偏移失真"是真实缺陷形状；MoonBit 若改字符串模型须重新设计（不可变 String 下天然规避，但要保留测试语义） |
| A14 | **失败台账 md 文本（13 个）** | `native/tests/*_FAILURES.md`、`cases_golden/GOLDEN_FAILURES.md` | **低**（作历史）/ **高**（作依据） | 文本可原样搬；但**已知 8 处文档↔代码数字漂移**（§④-5），搬过去前必须重算 |
| A15 | **J9 埋雷台账** | [`脚本埋雷验证记录.md`](脚本埋雷验证记录.md)（261 行，5 脚本记录） | **低** | "注入→必红→还原→复现命令"四要素格式，是迁移期新脚本的上线门槛模板 |

### 关于 `pyrandom` 的角色判定

**结论：`pyrandom` 复刻的是"测试侧宿主 RNG"，不是引擎侧 `rand()`。**

- `pyrandom` 服务于 `random_diff`（生成随机 C 程序）与 `interaction_probe`（生成随机交互序列）——`random_diff/main.go:3-8` 自述"随机生成器（本脚本）→ C 源码 → clang/Vitro"；导入点 `random_diff/main.go:31-33`、`interaction_probe/main.go:36-38`。
- 引擎侧 `rand()` 是另一套完全不同的算法：LCG `seed*1103515245+12345 & 0x7fff`（`native/crates/vitro_vm/src/host/misc.rs:4-9`，`srand` 在 `:11-14`）。
- 因此**引擎 `rand()` 的语义对齐义务不在 `pyrandom`，而在 golden**：唯一覆盖 `rand` 的用例是 `cases/baseline/srand_rand.c`，其源码刻意只断言 `srand(1);a=rand();srand(1);b=rand(); printf("%d", a==b)`（输出 `1`），**对具体数值序列零依赖**——这是有意的 golden 抗漂移设计，迁移时必须原样保留。
- 需要 MoonBit 重写 `pyrandom` 的真实原因是：`random_diff` 的"同 seed → 同一用例集合"可复现基线（`random_diff/main.go:17-20`）以及 `interaction_probe` 的"同 seed → 同 op 序列"双轨对账口径（`interaction_probe/main.go:24-27`）。若换用 MoonBit 内置 `random` 包，两条基线同时失效。
- ⚠️ **附带发现**：wasm-gc 目标的 `@env.rand_internal` 直接 `return false`（`~/.moon/lib/core/env/env_wasm.mbt:195-198`），即 wasm-gc 下无熵源。**新引擎 `rand()` 默认种子不得依赖系统熵**（现值依赖 `session.runtime.rand_seed` 初值，`misc.rs:5`），否则 wasm-gc 目标下两侧行为分叉。

---

## ③ 抛弃清单

| # | 抛弃对象 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| B1 | **全部 ctypes/DLL 绑定层** | `scripts/internal/capi/capi.go:140-294`（`syscall.LazyDLL` + 16 个 `LazyProc`）、`gosmoke/cabi_smoke.go` | wasm-gc 单出口下 C ABI 消失（评估报告 §3.3 第 1 点指名）。**本模块最大的一块不可移植代码** | 中：`capi.go` 同时承载"输出归一 `Normalize`"与"项目根探测"两个**可移植**能力，抛弃时须先把它们摘出来 |
| B2 | **`syscall`/`unsafe` 贯穿的读取范式** | `capi.go:97-138`（`CBytes`/`ReadChannel`/`PtrToGoString` 残留注释）、`probeutil.go:60-82`（psapi 句柄） | Go 特有；MoonBit 无裸指针/无 `unsafe` | 低：`ReadChannel` 的语义（长度 API + NUL 终止）在新架构下被"子进程 stdout 读取"整体替代 |
| B3 | **psapi 内存采样** | `probeutil.go:41-111`（`OpenProcess`/`GetProcessMemoryInfo`/`TerminateProcess`） | Windows API 直调，wasm-gc 无对应；RSS 护栏需换测量面（Node 宿主侧采样子进程） | **中**：RSS 护栏是 U0#2 验收标准（`serve_smoke/main.go:820-909`，预算 64MB，证红通道 `VITRO_RSS_BUDGET_MB=5`）。**新形态下"进程级 RSS"语义改变**（Node 宿主 + wasm 实例内存混合），判据须重定义——裸奔期必须列为"防线能力降级"项 |
| B4 | **Go flag 解析与顺序敏感陷阱** | `facts/main.go:46-56`（"flag 必须写在子命令之前"+ 残留 flag fail loud） | Go `flag` 在第一个位置参数处停止解析——语言特有缺陷 | 低（陷阱消失），**但"参数顺序错不得静默吞掉"的纪律要留**（`facts/main.go:51-56` 的 fail loud 检查是行为资产） |
| B5 | **Python 退役残留（4 文件）** | `native/tests/shadow_verification/{gen_cases.py, test_massive.py, test_more.py, new_cases.txt}`（仍 git 跟踪） | 消费者 `shadow_verify.py` 已于 2026-09-13 退役删除 | 低：纯死重；`new_cases.txt` 需先确认是否含独有信息（**待证**：与 `cases/**` 基名做差集） |
| B6 | **活跃 Python 生成器 3 个（迁移待办，非立即抛弃）** | `scripts/sync_templates.py`（325 行，**唯一 golden 生成器**）、`extract_cpp_builtin_layout.py`、`unified_perf_baseline.py` | 违反"默认 Go"纪律（`AGENTS.md` 脚本章），且 `sync_templates.py` 是**唯一能重算 `cases_golden/` 的入口** | **高**：迁移期 golden 若需重算（新用例补齐），必须能跑通它；应作为"由 Node 宿主驱动的 golden 生成器"一并重写，而不是丢掉 |
| B7 | **调试残留 / 孤儿产物** | `test_detect.exe`(+`.pdb`)、`test_detect.rs`（0 个 `#[test]`）、`bytecode_libc_consistency/test_force.exe`（孤儿）、`safe_run.ps1`（已脱线） | 均为死重；`safe_run.ps1` 还有死锁风险 | 低（已 gitignore 或无害），**但它们是"产物新鲜度"与"孤儿脚本"两类组织债的活体样本**，应记录而非静默删 |
| B8 | **展示视图输出口径** | `native/src/capi/mod.rs:435-436`（stdout+stderr+附注拼接）被 4 个测试消费（`end_to_end_test.rs:29-32` 等） | 与 E-P1-5（`capi/mod.rs:451-454` 纯 stdout）**同仓库两套口径并存**；实证后果：`end_to_end_test.rs:137-150` 的 C 源没有任何 `printf` 却靠 `l.contains("2")` 命中附注"程序运行完成，返回值：2" | **中**：若新引擎保留"附注通道"，必须一次定死"驱动只读纯程序 stdout"（`shadow_verify` 已是此口径，`main.go:513-514`） |
| B9 | **Rust 特有测试机制（约 240 个白盒单测）** | lexer 57 / parser 17 / typeck 23 / codegen 11 / parser_cpp 33 / typeck_cpp 31 / bytecode_gen_cpp 47 / ast 2 / compile_pipeline 24 / completion 14 等 | 耦合 Rust `enum Expr/Stmt/Type`、`OpCode`、`Instruction`、`CompileOutput` 表示 | 低（价值只在"这些特性必须有测试"的清单），**但须先把清单导出成语言中立的需求表**，否则随代码一起蒸发 |
| B10 | **`sync.Mutex` 并发纪律的 Rust/Go 形态** | `shadow_verify/main.go:460-464`（`var vitroMu sync.Mutex`） | 无 Arc/Goroutine 的原样对应物 | **低（形态）/ 高（纪律）**：DLL 并发堆损坏是实测结论，**"引擎会话非线程安全"这条纪律绝不能随语言一起丢**（见 §⑥-P4） |

---

## ④ 在途工作接纳方案

| # | 在途项 | 证据 | 接纳方案 |
|---|---|---|---|
| ④-1 | **U1#13 packed 布局数值差异防线锚**：新增 shadow 用例 `sizeof_struct_packed.c`，预期 `output_gap → known_issue` 登记 + spec 差异表指向；原则一般化为"**与 Clang 的数值类差异零 golden 覆盖即防线失效**" | [`统一整备路线图.md`](统一整备路线图.md):123（登记未做）；存量唯一覆盖 `sizeof_struct_union.c` 仅单 int 成员，对分歧零敏感 | **直接按目标架构实现**：纯 `.c` 数据 + `known_issue` 分类，与实现语言无关。迁移时**先立 golden（Rust 版跑一次 clang 产 `.out`）再迁用例**，否则裸奔期这条防线等于不存在 |
| ④-2 | **4 个新 C++ 用例缺 golden**：`cpp_copy_ctor` / `cpp_default_args` / `cpp_nested_class_instance` / `cpp_nttp_class` | 实测：`cases/cpp` 83 个 vs `cases_golden/cpp` 79 个，差集恰为这 4 个（`sync_templates.py:310-312` 存在即跳过） | **修复后搬**：迁移前用 Rust 版 `go run ./scripts/shadow_verify_cpp` 确认这 4 例的 live-golden 判定，再补 `.out`。若直接搬，新工程 E2E 层会因缺 golden 而**静默漏检**（**待证**：`vitro_e2e.rs:186` 的 `if let Some(golden)` 是否确为"缺 golden 即跳过比对"；验证法 `read native/tests/vitro_e2e.rs --offset 178 --limit 20`） |
| ④-3 | **6 个 baseline 用例无 golden**（`e2_angle_local_header` / `e2_include_cycle` / `e2_include_guarded` / `e2_include_not_found_angle` / `e2_include_not_found_quote` / `e3_static_assert_fail`） | 实测差集；其中 5 个是 `KNOWN_BASELINE_COMPILE_FAILURES`（`vitro_e2e.rs:209-215`），5 个 `.in` 全在差集内 | **放弃并记录理由**：无 golden 是**语义正确**的——编译失败用例无 stdout，`.in` 用例两侧同字节喂入（golden 生成器 `sync_templates.py:97-115` **不喂 stdin**，才是它们没 golden 的根因）。**但必须新记录**：这 6 例迁移后只能由 shadow 层（live clang）保障，E2E 层天然覆盖不到 |
| ④-4 | **`KNOWN_BASELINE_COMPILE_FAILURES` 缺反向测试** | `vitro_e2e.rs:218-224` 仅单向跳过；对比 template/knr/leetcode/cpp 四路均有反向 panic（`:379-521`） | **直接按目标架构实现（且这是修 bug 的机会）**：新工程五个 `KNOWN_*` 一律成对（跳过 + 反向"转绿即 panic"），并把 md 文件名写进 panic 文案（沿 `:397,428,454,516` 形态） |
| ④-5 | **8 处台账数字漂移**（迁移前必须重算） | `CPP_FAILURES.md:14`(78 vs 83) / `:11`(28 vs 31) / `:12`(38 vs 47)；`DOGFOODING_FAILURES.md:22`(25 vs 28)；`LEETCODE_FAILURES.md:13,109,169`(68/92 vs 138)；`KR_FAILURES.md:14,19`(76/69 vs 81)；`E2E_FAILURES.md:15`(3 已知 vs 常量 1 条 `vitro_e2e.rs:255`) | **修复后搬**：这些正是 `facts` 制度该抓但**未覆盖**的（`facts` 只对账 `shadow_*`/`replay`/`serve`/`cargo`/`abi` 七类，**台账内部数字零对账**）。迁移时扩 `facts` 规则覆盖"台账自述 vs 磁盘真值" |
| ④-6 | **`CORE_ASSET_VERDICT_FAILURES.md` 7 项 OPEN P0 零门禁**（含 R-2026-09-13 JIT 静默错值，实际已修但台账未回填：对照 `jit_path_parity.rs:112-128` + `cases/baseline/jit_nested_counting_loop.c`） | `CORE_ASSET_VERDICT_FAILURES.md:24-156`；不在 `ci_three_tier_check` 白名单（`main.go:46-54`） | **修复后搬**（先回填台账，再决定该文件在新工程是否纳入门禁）。7 项 P0 是否随 MoonBit 重写自动消失需逐条裁定——**禁止默认"重写即修好"** |
| ④-7 | **3 个受扫描台账对门禁不可见** | `HOST_CONTRACT_FAILURES.md`（H2 无关键词）/ `BYTECODE_LIBC_FAILURES.md` / `FUZZ_FAILURES.md`（`KNOWN_LIMITATION` 不在 `ci_three_tier_check/main.go:163-164` 匹配集） | **直接按目标架构实现**：新等价物应把"已知失败状态枚举"显式化（`FIXED/DIVERGENT/KNOWN/LIMITATION/TODO`），而不是靠 H2 标题关键词猜 |
| ④-8 | **golden 生成器 `sync_templates.py` 未随 D5 迁移** | `scripts/sync_templates.py:1-13`（自述职责 1–3）；是唯一 `cases_golden` 写入者（`grep` 实证 Go 侧零命中） | **直接按目标架构实现**：新工程 golden 生成必须由**同一 Node/wasm 宿主驱动**承担（`clang` 子进程不变），否则出现"引擎在 MoonBit、golden 生成在 Python"的双工具链债 |
| ④-9 | **`waiting_input` / `InputMode` 双模式语义** | `vitro_e2e.rs:229-233`（有 `.in` → Batch，无 → Interactive）；`vitro_e2e.rs:416-420`（K&R 另按源码是否含 `getchar()` 选 Batch） | **直接按目标架构实现**：这是"输入耗尽该挂起还是该 EOF"的语义决策，与实现语言无关；新引擎必须一次定死三种输入模式（无输入/批量/交互）的边界并写进规范 |
| ④-10 | **`shadow_verify_cpp` 缺瞬态重试与结果缓存**（对比 C 版 `abnormal`/`Cached`/`clangRetry`） | `shadow_verify_cpp/main.go:422-488` 无 `abnormal` 字段、无缓存（每次全量重算），仅 `:55` 的 `clangRetry=3` 对编译失败重试 | **修复后搬**：新工程 C/C++ 两侧应共用一套"瞬态异常不落缓存"实现（C 版 `main.go:117-121,747-751` 是正确形态），避免 C++ 侧长期裸奔 |

---

## ⑤ 架构优化建议（MoonBit 形态）

### 5.1 〔沿革保留〕必须原样继承的口径（不许"优化掉"）

1. **〔沿革保留〕两段流水线 + 顺序确定性**：Clang 侧并发、引擎侧互斥串行（`shadow_verify/main.go:705-774`），用例加载 casefold 排序（`:645-651`）、结果按用例序回填（`outcomes[i]`）。新形态下"引擎侧互斥"的物理基础变了（不再是同进程 DLL），但**同进程多实例共享可变全局状态的假设仍要显式声明或显式禁止**。
2. **〔沿革保留〕per-case 唯一产物命名**：`test_<name>.exe`（`main.go:237`）+ 每 worker 隔离运行目录（`:735,777-792`）。Windows 映像加载竞态不会因为换语言消失。
3. **〔沿革保留〕输出归一 `strip + CRLF→LF`**：`capi.go:125-129`；对应 12 条自检中的 2 条（`main.go:178-179`）。**MoonBit `String` 是 UTF-16、C 源码语义是字节流**——这条归一必须在**字节层**做，不能在字符串层做（见 §⑦-S3）。
4. **〔沿革保留〕瞬态异常不落缓存**：`main.go:117-121`（`abnormal` 标记）+ `:747-751`。原则："宁重算，不用不可信 Golden"。
5. **〔沿革保留〕J9 启动自检 fail loud（exit 2）**：`main.go:147-197`。判定型脚本的"自检不过即拒绝给判定"必须先于一切。
6. **〔沿革保留〕空用例集不得给门禁通过**：`main.go:689-692`（Python 版硬编码 fallback 已删）。**新工程最危险的一步恰是"驱动跑起来了但用例集是空的"。**
7. **〔沿革保留〕Clang 预检 fail fast（exit 2 vs exit 1 区分）**：`main.go:1190-1214`。
8. **〔沿革保留〕产物新鲜度双闸**：mtime 陈旧告警（`:804-870`）+ 版本串含 HEAD 硬门禁（`capi.go:217-231`）。新形态下"产物新鲜度"要重定义（wasm 产物无版本串的话，改用构建指纹注入）。

### 5.2 〔结构优化〕

9. **〔结构优化〕把"驱动内核"与"宿主能力"分层**：现役每个驱动都自己 `os/exec` + 自己拼 STDIN/STDOUT + 自己实现重试。新工程应有一个 `driver-host` 接口（子进程 spawn / stdin 写字节 / stdout 逐行 / stderr 采集 / 超时 kill / 退出码 / RSS 采样），各防线只写判定逻辑。**这正是 `scripts/internal/capi` 想做的事，但被"只有 DLL 一条通道"锁死了。**
10. **〔结构优化〕缓存 key 的 schema 与序列化格式一次定死**：现役 Go 用结构体 JSON（`go1`），Python 用 `json.dumps(sort_keys)`（schema=1），二者 key 空间不相交**靠不同名字**保证（`main.go:78-80`）。新工程应把 key 材料定义为显式版本化记录（含 `schema` 字段参与 hash），并把"跨实现不共享缓存"写成硬约束。
11. **〔结构优化〕报告产物制度升级**：现役 `--limit` 调试运行不覆盖 `latest`（`main.go:1348-1350,1361-1366,1370-1376`），但 `latest` 无 schema 版本。新工程建议：报告 JSON 带 `schema` + `generated_at` + `engine_build_id`，并让 facts 只认带 schema 的产物（现役 `facts.go:122-155` 靠字段嗅探）。
12. **〔结构优化〕把"台账一致性"从 H2 关键词升级为结构化**：现役三套机制（`ci_three_tier_check` 的 H2 关键词 / `facts.go:184-189` 的 regex / `engineering_health` 的行数）互不一致，且有 3 个文件对门禁不可见。新工程建议台账用**固定 schema 的 front-matter/JSON 块**承载活跃条目，md 只做渲染。

### 5.3 〔新设计〕wasm-gc + Node 宿主专有

13. **〔新设计〕用"一批用例一次宿主进程"替代"每用例一次库调用"**：现役 DLL 调用 ~1.5ms/例（`main.go:710-712` 注释），663 例串行可忽略。换成 Node 宿主子进程后，**进程启动成本会让"每例一进程"不可接受**。建议：宿主进程内跑多例（会话 reset），崩溃/超时才重启进程，并把"重启次数"作为可观测指标。
14. **〔新设计〕错误帧必须显式化**：现役 `runResult` 靠 `CompileSuccess/RunSuccess/RunError` 三字段表达（`main.go:105-121`）。子进程形态下必须新增**通道级错误种类**：宿主启动失败 / wasm 实例化失败 / 引擎 panic（trap）/ 协议帧非法 / 超时。**这五类必须与"程序自身失败"区分**，否则全部退化成 `runtime_gap` 假红或 `match` 假绿。
15. **〔新设计〕`spectest.print_char` 是必接的隐式契约**：本次 spike 实测（§⑦-S1），MoonBit `println` 在 wasm-gc 下被编译为 `spectest.print_char` 导入调用，**每次一个 Unicode 码点**（实测 `abc中` → `0x61 0x62 0x63 0x4e2d`）。宿主必须自己做 UTF-16 码点 → UTF-8 字节编码，**不能直接把码点当字节写 stdout**（否则中文输出全部损坏，且这类损坏在 ASCII 用例上完全看不出来）。**这是裸奔期最容易被漏掉的静默错值来源。**
16. **〔新设计〕引擎的"字节流"载体要早定**：`@env` 在 wasm-gc 下的 `__moonbit_fs_unstable` 导入**没有任何实现**（`~/.moon/lib/core/env/env_wasm.mbt:28-192` 全部是 `#external` 声明，无 wasm-gc 分支实现；只有 `rand_internal` 有 `#cfg(target="wasm-gc")` 分支且 `return false`）。结论：**wasm-gc 产物在裸 Node 下拿不到 args/env/cwd/时间**，必须由宿主通过自定义 import 提供。建议把驱动协议定义成宿主提供的一组 import（读 stdin / 写 stdout / 写 stderr / 时间 / 退出码），而不是让引擎去调 POSIX。

---

## ⑥ 坑清单（本模块事故史）

| # | 现象 | 根因 | 修复 | MoonBit 下是否复发 / 为什么 |
|---|---|---|---|---|
| P1 | **结果错配但门禁仍绿（静默绿）** | 并行化后用例加载顺序不确定，结果按索引对账错位 | 排序 + 按用例序回填（`shadow_verify/main.go:613-615,645-651,705-717`） | **会复发**：只要"多路并发 + 索引回填"的形状还在。语言无关。新工程必须保留"加载确定性 + 结果按序重排"两条，并写成 spike 断言（§⑦-S5） |
| P2 | **`pathlib` 排序在 Windows 是 casefold 序、Linux 是码点序** | Python `pathlib.__lt__` 平台相关 | Go 版统一 casefold（`main.go:641-651`）；自检未覆盖此项 | **不会以原形复发**（Python 消失），**但会以新形复发**：MoonBit `String` 比较是 UTF-16 码元序，与"字节序"在非 ASCII 上不同。用例名当前全 ASCII，风险低但**须定死口径并自检** |
| P3 | **同名 `test.exe` 覆盖 → 确定性 0xC0000005 映像竞态** | Windows 16 路并发快速覆盖并执行同名 exe | per-case 唯一命名 `test_<name>.exe`（`main.go:235-240`，继承 C++ 版第一站实证） | **会复发**：宿主换 Node 不改 Windows 加载器行为。新工程必须 per-case 唯一命名 + 每槽位隔离目录 |
| P4 | **DLL 并发调用 → 堆损坏** | 引擎会话非线程安全 | Vitro 侧全程 `sync.Mutex` 串行（`main.go:460-464`）；`runWithVitro` 内部加锁（`:467-469`）；CI 的 cargo 侧另有 `--test-threads=1`（`ci_three_tier_check/main.go:69-72`） | **形态变了，纪律不变**：wasm 实例仍是可变全局状态（线性/GC 堆、`Session`）。**"一个引擎实例不得并发进入"必须写进新架构的显式契约**——Node 事件循环（异步 I/O + 回调）让"隐式并发"更容易发生 |
| P5 | **瞬态环境异常被写进缓存 → 固化错误 Golden** | Python 版把超时异常也落缓存 | Go 版新增 `abnormal` 标记，异常不落缓存（`main.go:117-121,747-751`）；编译失败重试 3 次（`:214-226`） | **会复发且更严重**：新形态下"宿主启动失败 / wasm 实例化失败"是新的瞬态类别。**必须先把错误分类做完（§5.3-14）再谈缓存** |
| P6 | **`ExitError` 与 Python 异常模型结构性错位** | Go 非零退出是 `*ExitError`，Python `subprocess.run`（无 `check`）正常返回 | 显式分支：程序自身非零退出 = 确定性行为；超时/未启动/`>0xFFF` 退出码 = 环境异常（`main.go:290-307`） | **会复发**：Node `child_process` 语义是第三套（`exitCode` vs `signal` vs `error`）。"程序自己 exit != 0" 与 "宿主/引擎异常" 必须显式区分，否则 `e1_func_identifier`（`return helper()` → exit 1）这类合法用例会变红 |
| P7 | **驱动侧正则清洗误删真实输出（假阳性 output_gap）** | 十余处清洗规则语义互不一致，教学程序打印同类文本时被删 | E-P1-5 全面废除清洗，改走纯程序 stdout 通道（`capi.go:142-148` 必需符号 fail fast；`main.go:513-514`） | **会复发**：新形态下"引擎附注/诊断"若与程序 stdout 混在同一管道，必然有人加清洗。必须一次定死"程序 stdout 是唯一比对面，诊断走独立通道"，并保留 `cases/baseline/engine_note_lookalike.c` 作为锚 |
| P8 | **陈旧产物上全量假绿** | `shadow_verify` 用 release DLL，日常开发跑 debug——改完引擎直接跑门禁，跑的是旧引擎 | 双闸：mtime 陈旧告警 + 版本串必须含 HEAD（`main.go:794-870`、`capi.go:217-231`）；serve_smoke 另有 `resolve_exe` 取 mtime 较新者（`serve_smoke/main.go:13-15` 记录了"固定 debug 优先曾拿假绿"的 J9 埋雷实证） | **会复发**：wasm 产物同样会陈旧。新工程必须能让产物自证身份（构建时注入 commit/时间戳，驱动侧校验）——这是"没有 C ABI 版本串之后"的新义务 |
| P9 | **stdin 未喂导致虚假 match** | K&R 目录 29 个 `.in` 从未被使用，两侧"都无输入" | 2026-09-11 启用 `.in` 注入（`main.go:674-678`），随即暴露"输入注入丢换行"缺陷（`getchar()` 读不到 `'\n'`，19 例 `output_gap`） | **会复发**：`.in` → 引擎 stdin 的链路在新形态下完全重建（宿主写 wasm 引擎）。必须保留"两侧喂同一份字节"的断言（含 `\n` vs `\r\n` 归一，`main.go:132-140`） |
| P10 | **golden 生成口径与消费口径不一致（隐藏缺陷）** | `sync_templates.py:97-115` 生成 golden 时**前置注入** `#include <stdio.h/stdlib.h/string.h>` + `#undef min/max`，且**不喂 stdin**；而 shadow 驱动编译原始 `.c`（无注入、喂 stdin） | 未修（两套 golden 并存、互不引用：`grep` 实证 shadow 驱动零 `cases_golden`/`.out` 命中） | **会复发且必须显式记录**：`.out` 只能保证"在该前置头 + 无 stdin 口径下"的真值。迁移时若把 `.out` 当唯一真值，隐式声明类/带 `.in` 类用例会给出错误期望。建议只把 `.out` 当"交叉校验的第二来源"，live clang 仍是主真值 |
| P11 | **文档数字腐坏（人肉同步必然滞后）** | 文档裸数字与真值脱钩 | `scripts/facts`（M13/M15）：CURRENT 裸数字必须等于真值，超龄即红；常量对账 `abi_version` 以源码为唯一真值 | **会复发**：台账 8 处数字漂移（§④-5）证明**未被 `facts` 覆盖的数字仍在腐坏**。新工程应把 `facts` 覆盖键从 14 个扩到覆盖台账自述数字 |
| P12 | **裸奔期静默错值**（评估报告 §3.3 第 3 点） | 防线 1~5 重建窗口检测能力为 0，而该窗口叠加"跨层语义失配 / 启发式判据误判 / 时序与生命周期"三类"不报错只错值"易错点 | 未发生（预案） | **迁移期头号风险**，处置见 §⑧-3"裸奔期最小防线" |

### 6.1 现役"六类隐性口径"清单（`shadow_verify/main.go:3-39` 头注，逐条落位）

| # | 口径 | 头注行 | 实现行 |
|---|---|---|---|
| 1 | 用例来源五目录 + `@category` ASCII 提取 + 剔除 `// @` 行 + 同名 `.in` 注入 stdin | `:6-7` | `:609-694` |
| 2 | Clang 侧：原目录编译（`#include` 解析）、`-Wno-implicit-function-declaration`、30s/5s 超时、worker 隔离目录 + VFS 预设文件、`<rundir>` 归一 | `:8-10` | `:201-332`、`:777-792` |
| 3 | Vitro 侧：`compile_unit+compile_all` / `set_input_mode+set_input` + E-P1-5 结构化输出通道（禁清洗）+ ABI/新鲜度 fail fast | `:11-12` | `:466-522`、`capi.go:180-231` |
| 4 | 判定树七分支（主驱动口径与 C++ 版不同） | `:13-16` | `:527-562` |
| 5 | 门禁退出码：非预期差异 >0 → 1；Clang 预检失败 / 产物不新鲜 → 2；DLL 缺失 → 1 | `:17-18` | `:1386-1415`、`:1194-1214` |
| 6 | 报告三件套（md + json 各带时间戳 + latest、`kr_leetcode_report.json`）；`--limit` 不覆盖 latest | `:19-20` | `:1339-1376` |

**另有 5 条"与 Python 版的有意差异"**（`:22-35`）：并发形态、缓存 schema `go1`、stdin 口径修正、编译失败重试 3 次、取消硬编码 fallback、启动自检。

---

## ⑦ MoonBit spike 清单

> S1 / S2 为**本次实测已完成**（临时目录 `moonbit_spike_probe`，未触碰 Vitro 仓库）。工具链事实：`moon 0.1.20260915`；`moon.mod.json` 已 deprecated（`moon fmt` 迁移到 `moon.mod`）；产物落 `_build/<target>/<profile>/build/`。

### S1 —— wasm-gc 产物形态与 stdout 通道（**已实测，结论确定**）

- **程序**：`fn main { println("abc\u{4e2d}") }`，`moon build --target wasm-gc`。
- **实测结果**：
  - 产物为 `_build/wasm-gc/debug/build/<pkg>.wasm`，**无 JS 胶水**（对比 `--target js` 产出 `<pkg>.js`，`node` 直跑即打印中文，`String.length()` 为 UTF-16 码元数）；
  - wasm 模块 **imports = `spectest.print_char (function)`，exports = `_start (function)`**，二者唯一；
  - 宿主传入 `spectest.print_char` 回调后 `_start()` 正常执行；回调收到的是 **Unicode 码点**（`abc中\n` → `0x61 0x62 0x63 0x4e2d 0xa`）。
- **判定标准**：① 产物在裸 Node（无 moonrun）可实例化执行 ✅；② 宿主能通过实现 `spectest.print_char` 完整截获程序 stdout ✅；③ 码点 → UTF-8 编码责任在宿主 ✅（实测：`String.fromCharCode` 得到正确串，而 `Buffer.from(codes)` 直转会损坏——**必须显式做码点编码**）。
- **对驱动链的含义**：stdout 捕获通路可控（引擎 `println` 就是 stdout），但**只覆盖 `println`**；`printf` 式格式化输出必须在 MoonBit 侧自己实现并最终汇聚到 `println`（或自定义 import）。

### S2 —— `@env` 在 wasm-gc 下的能力边界（**已实测，结论确定**）

- **程序**：调用 `@env.args()` / `@env.now()` / `@env.current_dir()`。
- **实测结果**：可编译、可实例化，但运行时**持续调用未实现的 import**（`__moonbit_fs_unstable.string_read_char` 等）直到超时——`env` 包在 wasm-gc 目标下**没有任何宿主实现**（源码实证：`env_wasm.mbt` 全部是 `#external` 声明；`rand_internal` 的 `#cfg(target="wasm-gc")` 分支直接 `return false`，`:195-198`）。
- **判定标准**：**驱动不得依赖 `@env`**；args/env/cwd/时间/熵必须由宿主通过自定义 import 提供。
- **附加结论**：wasm-gc 下**无熵源**，引擎 `rand()` 默认种子不得依赖系统熵。
- **待证**：官方运行时（`moon run` / `moon test`，非裸 Node）是否提供这些 import 的实现——验证法 `moon run --target wasm-gc <pkg>` 跑一个 `@env.args()` 程序。

### S3 —— 字节流语义（**最高优先级未做 spike**）

- **要验证的语言特性**：`String`（UTF-16）与 `Bytes`（不可变）在 C 源码字节流语义下的行为；`FixedArray[Byte]` / `Array[Byte]` 的 packed 表示与装箱。
- **程序**：读入含 `\r\n`、`\0`、多字节 UTF-8（中文注释/字符串字面量）、孤立 `\r` 的字节序列；做"剥离 `// @` 行 + universal-newline 归一 + 与 golden 逐字节比较"。
- **判定标准**：① 能无损保存 0x00–0xFF 全字节；② 归一结果与 Go `normalizeNewlines`（`main.go:132-140`）逐字节一致；③ 与 `.out` golden 的比对在**字节层**（非 UTF-16 码元层）成立。

### S4 —— JSON-lines 协议往返（驱动链基础）

- **程序**：用 `moonbitlang/core/json`（本机已装）解析一行 JSON 请求、构造一行响应，含：整型与浮点可区分（对应 `serve_smoke` 的 `UseNumber` 语义 `main.go:95-119`）、非 ASCII 不转义（对应 Go `SetEscapeHTML(false)`，`main.go:1121-1138`）、转义序列、深嵌套、超长行。
- **判定标准**：① 与 Go 版逐字节一致（`id/method/params` 字段序固定，`serve_smoke/main.go:44-49`）；② 错误帧与结果帧"恰一"（`interaction_probe` 不变量）；③ 不因不可变 `String` 造成 O(n²) 拼接（长输出如 `putchar 1M` 场景）。

### S5 —— 并发与顺序确定性（防线正确性 spike）

- **程序**：并发跑 N 个用例，每个返回带自身标识的结果，按用例序回填；注入"故意乱序完成"的休眠。
- **判定标准**：**门禁必须在乱序完成下仍然正确**——构造一个不回填索引的实现，断言门禁报红（P1 的 spike 化）。

### S6 —— 子进程宿主全套错误分类（裸奔期防线基础）

- **程序**：宿主故意制造五类异常并验证分类：① 可执行不存在；② wasm 实例化失败（缺 import）；③ 引擎 trap（`unreachable`）；④ 协议帧非法；⑤ 超时 kill。
- **判定标准**：五类**互不混淆**，且**都不落缓存**；"程序自身 exit != 0"单独一类（P5/P6 的 spike 化）。

### S7 —— 引擎 `rand()` 与 `srand()` 语义（golden 对齐）

- **程序**：`srand(1); a=rand(); srand(1); b=rand();` 两轮一致；再验证 reseed 后的具体序列与现值 LCG（`misc.rs:4-14`）一致。
- **判定标准**：① `srand_rand.c` 的 golden（`1`）通过；② 若新引擎改算法，**必须证明无 golden 依赖具体数值**（当前实证只有 1 个用例且不依赖，风险低但须固化）。

### S8 —— 行协议 + stdin 注入（宿主写引擎 stdin）

- **程序**：宿主向引擎实例喂入 `.in` 字节（34 个 `.in` 之一），引擎按 `scanf`/`getchar` 语义消费。
- **判定标准**：① `\n` 不被吞（P9 原始缺陷不在新形态复发）；② EOF 语义与 `scanf_eof_after_exhaust` 期望一致；③ "输入耗尽挂起 vs EOF"三模式（§④-9）边界正确。

---

## ⑧ 等价性验收锚点

### 8.1 diff 什么、什么格式、用什么工具

| 层 | 中间产物 | 格式 | 比对方式 | 关键口径 |
|---|---|---|---|---|
| **L1 用例集** | 用例清单（名 + category + srcDir + stdin 摘要 + 顺序） | JSON | 逐字段 diff，**顺序逐项相等** | 顺序 = casefold 排序（`.c` 扩展名过滤）；`@category` 提取限 ASCII（`main.go:609,659-664`） |
| **L2 判定** | 每例 `diff_type` + `expected` | 同上 | 逐用例 `diff_type` 一致 | 7 类：`match/known_issue/vitro_better/gap_extension/compile_gap/runtime_gap/output_gap`（`main.go:1397`） |
| **L3 汇总** | `summary`（5 键恒出现含 0）+ `category_frequency` | JSON | 全等 | `known_issue`/`vitro_better`/`gap_extension` **不入 summary**（`main.go:1076-1091`） |
| **L4 引擎输出** | 每例 `clang.stdout` vs `vitro.stdout` 归一后 | 字节串（strip + CRLF→LF） | 字节级相等 | 纯程序 stdout 通道，禁清洗（E-P1-5） |
| **L5 golden 静态** | `.out` 文件 vs 新引擎 stdout | 文本行数组 | 逐行相等（`vitro_e2e.rs:186-194` 形态） | 现存 `.out` 有 P10 口径偏差，仅作第二来源 |
| **L6 缓存** | 缓存命中数 / key 数 | `cacheMaterial` JSON | **key 空间必须不相交**（新 schema 名） | 跨实现共享缓存 = 假绿温床（`main.go:78-80`） |
| **L7 台账** | `facts.json` 14 键 | JSON | 逐键 `value`/`svalue` + `status` | 状态机 `ok/cached/stale/unavailable`（`facts.go:492-519`） |
| **L8 文档** | `doc_fact_drift.md` 摘要表 | Markdown 表 | 漂移 0 / 坏引用 0 / 超龄 0 | `facts check` exit 0（CI 硬门禁，`.github/workflows/ci.yml:164-166`） |

### 8.2 迁移期防线重建里程碑表

| 里程碑 | 内容 | 完成判据（可机判） |
|---|---|---|
| **M-0 基线固化** | 在 Rust 版仍可跑时，冻结一份 `shadow_data.json` + `cpp_shadow_report.json` + `cases_golden/` 快照（含 clang 版本）+ `facts.json` | 产物入版本控制或固定 tag；clang 版本串记录在报告内 |
| **M-1 裸奔期最小防线** | 见 §8.3（24 例 stdout diff） | 该子集在新引擎上跑通并产出 diff 报告 |
| **M-2 驱动骨架** | Node 宿主 + JSON-lines + 错误分类（S6）+ 顺序确定性（S5） | 自检（J9 形态）通过；空用例集拒绝给判定 |
| **M-3 golden 解析层** | `.out` 读取 + 字节层归一（S3） | 与 Rust E2E 层逐用例结论一致 |
| **M-4 全量用例集接入** | 758 用例加载 + casefold 序 + `.in` 注入（S8） | L1/L2 与 M-0 基线逐项一致 |
| **M-5 live-Clang 主真值** | Clang 编译/运行/缓存/重试/不落缓存（P5/P6） | L3/L4 与 M-0 基线逐项一致；`--refresh-clang` 可全量重算 |
| **M-6 三层契约** | host_contract / bytecode_libc / differential 重建 | 三个"对手"定义清晰（见 §4-3 风险） |
| **M-7 fuzz A–E** | SplitMix64 逐位复刻 + 5 种子 | 同 seed 同序列，判定与 Rust 版一致 |
| **M-8 facts 制度** | 14 键采集 + CURRENT/AS-OF/DERIVED 判据 + 超龄门禁 | `facts check` 在注入漂移时 exit 1（J9） |
| **M-9 台账与 CI** | 13 md 纳入结构化对账；CI 接线 | 假 KNOWN 条目 → hard exit 1（J9） |

### 8.3 裸奔期最小防线子集（重写第一周就必须存在）

**目标**：在防线 1~5 全数重建完成之前，用最小代价捕获"静默错值"（评估报告 §3.3 第 3 点机理）。

**规模**：**24 例**（20–30 区间内），全部来自 `cases/baseline`，**全部已有 golden `.out` 且不需要 stdin**。

**选例标准（五条，缺一不可）**：

1. **输出非空且格式敏感**（能暴露 printf 格式化/宽度/符号/进制差异）——排除"仅返回码"的用例；
2. **golden 已存在**（`cases_golden/baseline/<name>.out`）→ 无需 clang 即可比对，裸奔期不依赖外部工具链；
3. **不依赖 stdin**（无 `.in`）→ 避免第一周就背上输入注入链路（S8 未完成前的硬约束）；
4. **每个语言子域至少 1 例**（词法 / 整型 / 浮点 / 字符 / 控制流 / 数组 / 结构 / 指针 / 内存 / 字符串 / 函数 / 位运算 / 溢出 / UB 边界），覆盖"最可能静默错值"的形状；
5. **不依赖未实现的子系统**（无 JIT、无 C++、无 VFS 文件、无 `time()`/`rand()` 具体数值）。

**候选表（附实测 `.out` 字节数与风险点）**：

| 子域 | 用例 | `.out` 字节 | 为什么必须在前 24 例 |
|---|---|---|---|
| 最小冒烟 | `hello_world` | 6 | 通道打通与否的唯一判据 |
| 整型算术 / 除零向负 | `int_arith`、`int_div_neg` | 1 / 2 | 负数整除向零取整（C99 语义，易错） |
| 浮点 | `double_basic`、`float_basic` | 12 / 4 | `%f` 格式化、float/double 提升 |
| 字符 / 转义 | `printf_char`、`escape_tab`、`putchar_function` | 1 / 3 / 1 | UTF-16 码点→字节编码缺陷唯一可见处（§5.3-15） |
| 控制流 | `if_else`、`switch_case`、`for_loop`、`nested_loop` | 3 / 3 / 3 / 4 | 跳转/回填地址错位的直接探针 |
| 函数 / 递归 | `factorial`、`fibonacci`、`recursive_function` | 3 / 2 / 3 | 调用约定 / 栈帧 |
| 数组 | `array_index`、`multi_dim_array`、`reverse_array` | 1 / 1 / 1 | 步长缩放 / 行主序 |
| 结构 / 联合 | `struct_basic`、`union_basic` | 2 / 1 | 布局与成员偏移 |
| 指针 / 内存 | `swap_by_ptr`、`malloc_free`、`realloc` | 3 / 2 / 1 | 堆布局（对应 R1 动态堆起点重构） |
| 字符串 | `string_strcpy`、`string_strcmp`、`strcat_function` | 2 / 2 / 11 | 字节语义与 `\0` 边界 |
| 位运算 / 长整型 | `bitwise`、`long_long` | 1 / 19 | 移位/符号扩展；64 位路径 |
| 无符号格式化 | `unsigned_printf_x` | 2 | `%x` 与无符号提升 |
| 输出通道锚 | `engine_note_lookalike` | 268 | **E-P1-5 口径专用锚**（防清洗/串通道回归，P7） |
| 已知缺陷锚 | `codegen_soundness_regression` | 298 | 固化了三批 soundness 修复 |

（上表 26 例，可裁 2 例；末两例字节数大但属"锚语料"，建议保留。）

**必配的 3 条"防裸奔期自身假绿"纪律**：

1. **空集不得绿**（`main.go:689-692` 口径）；
2. **golden 缺失必须红**（不能像 `vitro_e2e.rs:186` 的 `if let Some(golden)` 那样静默跳过——**这是现存防线的一个真实破口**：4 个新 C++ 用例正因缺 golden 而未被 E2E 覆盖）；
3. **比对必须在字节层**（S3；UTF-16 字符串比较会掩盖编码缺陷）。

---

## ⑨ mooncakes 包切分草案

### 9.1 包划分（8 个包 + 1 个测试包组）

```
vitro/driver-core          # 判定内核（纯逻辑，零宿主依赖）
  ├─ DiffType 枚举 + analyze_diff（12 条自检内嵌）
  ├─ classify_compile_error（关键词表 + expected_category 优先）
  ├─ OutputNorm：bytes strip + CRLF→LF（字节层，S3 门槛）
  ├─ Result/Session 数据结构（runResult 五字段 + 错误分类枚举）
  └─ 自检入口 self_check() -> Unit   # J9：不过则 abort
vitro/case-loader          # 用例加载（文件系统只读）
  ├─ Sources：baseline/gap/template/knr/leetcode/cpp 五目录
  ├─ casefold 排序 + 结果按序重排契约
  ├─ @category 提取（ASCII 限定）+ "// @" 行剔除
  └─ .in 伴生输入 + .h 头文件摘要
vitro/golden-store         # golden 资产访问
  ├─ .out 读取（cases_golden/ 五处）
  ├─ 字节层比对
  └─ 缺 golden 的显式策略（红 / 白名单）
vitro/clang-oracle         # Clang 侧（唯一依赖外部进程的包）
  ├─ 预检 --version（fail fast）
  ├─ 编译/运行参数与超时（30s/5s）+ 重试 3 次
  ├─ 隔离运行目录 + VFS 预设文件 + per-case 唯一产物名
  └─ 缓存 key 材料 + schema 版本 + 原子落盘 + abnormal 不落缓存
vitro/engine-host          # Node 宿主适配层（wasm-gc 专有，唯一碰 I/O 的包）
  ├─ 实例化 + spectest.print_char 接收（码点→UTF-8）
  ├─ 自定义 import：stdin 字节 / 时间 / 退出码
  ├─ 错误分类五类 / 进程重启 / 超时 kill
  └─ JSON-lines 帧编解码
vitro/report               # 报告渲染（md + json）
  ├─ summary / category_frequency / details
  ├─ latest 覆盖纪律（--limit 不覆盖）
  └─ schema 版本 + generated_at + engine_build_id
vitro/pyrandom-compat      # CPython random 逐比特复刻（探针专用）
  ├─ MT19937 + sha512 seeding + init_by_array
  └─ getrandbits / randbelow / randint / choice / random / choices / sample
vitro/probe-host           # 探针宿主能力（RSS 采样等，平台相关，条件编译）
  ├─ 子进程内存采样（替代 psapi）
  └─ 硬超时 + 看门狗击杀
```

**测试包组**（MoonBit 黑盒 / 白盒约定）：

- `*_test.mbt`（黑盒）：判定内核 12 条 + 常量对账 8 条 + `applyHit` 3 条 —— 与实现分离的验收面；
- `*_wbtest.mbt`（白盒）：缓存 key 材料、排序实现、序重排的内部不变量。

### 9.2 依赖方向（严格单向，无环）

```
driver-core ────► (无依赖，纯逻辑)
case-loader ────► driver-core
golden-store ───► driver-core
clang-oracle ───► driver-core, case-loader
engine-host ────► driver-core, case-loader
report ─────────► driver-core
pyrandom-compat ► (无依赖)
probe-host ─────► (无依赖)
        ▲
        └── 顶层驱动包（shadow-verify / shadow-verify-cpp / serve-smoke / …）
            只 import 上面 8 个，自己不写判定
```

**硬纪律（对应现役 `scripts/internal/*` 单源化教训）**：判定逻辑只允许存在于 `driver-core`；**驱动包禁止复制 helper**（现役"口径分叉"顽疾的根因，`capi.go:6-8` 自述）。

### 9.3 对上发布形态

| 包 | 发布形态 | 理由 |
|---|---|---|
| `driver-core` / `case-loader` / `golden-store` / `report` | **mooncakes 公开包**（语义稳定、语言中立，可被第三方用于"C 子集实现的一致性验证"） | Vitro 方法论的可复用出口，"白箱 + 诚实对照"哲学的可传播载体 |
| `clang-oracle` | **公开包**（文档标注"需要 PATH 中有 clang"） | 泛用性高（任何 C 子集实现都能用） |
| `engine-host` | **不发布**（Vitro 专有协议，与引擎内部协议强耦合） | 单出口形态下这是"内部胶水"，对外无承诺 |
| `pyrandom-compat` / `probe-host` | **不发布**（或作为 `driver-core` 的可选子包） | 平台相关（Windows 采样）+ 探针专用 |
| 顶层驱动 | **不发布**，仓库内 `moon run` 入口 | 它们是"防线"，不是"产品" |

**契约文件纪律**：每个公开包的 `.mbti`（公共接口文件）视为**对外承诺**，其变更需版本化记录——这正是"砍掉 C ABI `vitro_abi_version()` 后"留下的那份对外义务的**新载体**。若公开 mooncakes 包，这类义务会以新形式回归，**须在阶段 0 就裁定是否要**。

---

## 附 A：本次勘察的"待证"清单（禁止当结论用）

| # | 待证项 | 验证方法 |
|---|---|---|
| 1 | `native/tests/` 全部 `#[test]` 当前是否全绿 | `cd native && cargo test --workspace --all-features 2>&1 \| tee ci.log`（本次未跑：只读纪律 + 耗时） |
| 2 | 全 workspace `#[test]` 现值（评估报告附录 B 称 1,015） | `grep -rn "#\[test\]" native --include=*.rs \| wc -l` |
| 3 | `vitro_e2e.rs:186` 的 `if let Some(golden)` 是否确为"缺 golden 即静默跳过比对" | `read native/tests/vitro_e2e.rs --offset 178 --limit 20` |
| 4 | `bytecode_libc_consistency/test_force.exe` 的来源 commit | `git log --all --diff-filter=D -- '*test_force*'` |
| 5 | `new_cases.txt` 是否含独有信息 | 与 `cases/**/*.c` 基名做差集 |
| 6 | MoonBit 官方运行时（`moon run` / `moon test`）在 wasm-gc 下是否提供 `@env` 的 import 实现 | `moon run --target wasm-gc <pkg>` 跑一个 `@env.args()` 程序 |
| 7 | `moon check` / `test` / `ide` / `explain` 在 CI 的形态与退出码语义 | 专门 spike（本次只用过 `moon check/build/version`） |
| 8 | `test_detect.exe` 运行时 stdout | 直接运行该 exe（本次未执行任何 exe） |
| 9 | `write_cstring` 越界「拒绝」是静默丢弃还是内部 panic 被吞 | 追 `native/src/capi/` 相关实现 + 构造越界输入 |

## 附 B：与既有勘察报告的接口

本报告与同系列六份模块勘察报告的锚点分层关系（避免重复定义）：

- **lexer 报告**：L1 = token 流序列化 diff；
- **shared 与 ast 报告**：L1 = AST dump JSON；
- **codegen 报告**：产物 JSON 逐字节 diff；
- **unified 报告**：主通道 = serve NDJSON；
- **本报告**：**L1 = 用例集清单（名+category+顺序+stdin 摘要）JSON**，L2 = 逐用例 `diff_type`，L3 = summary/category_frequency，**L4 = 引擎纯程序 stdout 字节比对**（这一层是其他模块报告共同依赖的最终出口）。
- 重叠项：`serve` 会话协议断言（本报告 S4 / unified 报告的 M1–M6）建议合并在同一处实现，避免"会话封装 ×3"的历史重复（`AGENTS.md` D5 收尾重构已登记该残留）。
