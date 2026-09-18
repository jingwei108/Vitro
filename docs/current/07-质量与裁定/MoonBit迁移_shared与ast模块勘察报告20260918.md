# MoonBit 迁移 · `vitro_shared` + `vitro_ast` 模块勘察报告（2026-09-18）

> **性质**：MoonBit 重写前置的**模块级只读勘察** + **实测发现登记**。不修改引擎任何文件、不做 git 操作。
> **对象**：`native/crates/vitro_shared/`（SourceLoc、ErrorCode 等共享基础类型）与 `native/crates/vitro_ast/`（AST 节点与类型系统）。
> **上游依据**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)（§2 事实核查 / §7 四道证伪门 / §8 假设清单 A1~A9）。
> **同批兄弟报告**：[`MoonBit迁移_lexer模块勘察报告20260918.md`](MoonBit迁移_lexer模块勘察报告20260918.md)（`vitro_lexer`，含 token 流 L1 锚点与词法侧实测）。
>
> **数字口径（AS-OF 冻结）**：文件名与标题含日期，本文所有裸数字均为 **2026-09-18 现场实测/亲读快照**，
> **不参与 `reports/facts.json` 回填**；引用全仓真值一律标 key 与 as_of。
>
> **证据等级（沿用 [`实测发现登记20260913_性能与头文件.md`](实测发现登记20260913_性能与头文件.md) 头注纪律）**：
> 每条结论标注「**实测**」（本机命令行复现，命令与原始输出留档）/「**亲读**」（代码逐行读取 = 本项目复核纪律中的"代码亲读"，
> 是合法证据但**不是实测**）/「**未复现**」（机制成立但造不出可观测实例 → **不得占缺陷位**）。
> 凡推断一律显式标注，禁止以亲读冒充实测。复核方式与 [`实测发现登记20260913_性能与头文件.md`](实测发现登记20260913_性能与头文件.md) §8.1 同源：
> **实跑复现 / 代码亲读 / grep 核对，不采信未验证条目**，且**复核命令本身必须随结论留档**
> （[`统一整备路线图.md`](统一整备路线图.md):307 活例：曾以错误拼写 grep 得出"Layer A 全域零引用"并已口头输出，二轮复验修正）。
>
> **证据适用范围**：全部代码证据读自**当前工作区**（2026-09-18 盘上状态）。**未做 git 校验**（遵守只读勘察纪律，禁 git 操作）；
> 兄弟报告记录当前 HEAD 为 `0b243ca`，与 `reports/facts.json` 的 `git.rev` = `4b57191` 存在位移，
> 故本文引用全仓真值处一律标注 as_of，不声称"与某 commit 对齐"。

---

## 0. 前提更正与记录对账

### 0.1 对任务书/文档表述的更正（五条）

| # | 任务书/文档表述 | 实测或亲读结论 | 证据 |
|---|---|---|---|
| 0-1 | 错误码分段「E1xxx 语法 / E2xxx 预处理 / E3xxx 运行时 / E4xxx C++ / E5xxx 预留」 | **标签错**。实测分段：**E1xxx = 词法（预处理器在 E1014~E1022 内，不独立成段）/ E2xxx = 语法 / E3xxx = 语义（typeck 为主，混入运行时检测码）/ E4xxx = C++ / E5xxx = 空段（0 变体）** | 亲读 `native/src/diagnostics/error_catalog.rs:470-497`（`lang_of_code`/`category_of_code` 段注释与映射）+ `native/crates/vitro_shared/src/error_codes.rs:10-157` 全量枚举 |
| 0-2 | `SourceLoc` = 「两个 `i32` + `Copy`」 | **三个 `i32`**（`line`/`column`/`file_id`），`Copy` 成立。根 `AGENTS.md` 编码约定仍写"两个 i32"= **文档漂移** | 亲读 `native/crates/vitro_shared/src/source_loc.rs:2-9`；`file_id` 引入记录见 [`模板维护指南.md`](../05-教学体验/模板维护指南.md):160 |
| 0-3 | [`下游需求处置回执.md`](../06-出口与协议/下游需求处置回执.md):22 记「B1 C# 码段 … ✅ 已落地」 | **仅范围映射落地，枚举内 0 个 E5xxx 变体**（block5 count = 0）；`lang_of_code(5000..=5999)` 返回 `"csharp"` 但无码可落 | 亲读 `error_codes.rs`（无 E5xxx 行）+ `error_catalog.rs:476-483`。与 [`三语化整备审计计划.md`](三语化整备审计计划.md):51 自记的"B1（E4xxx 码段误判）证明文档级理解会漂移"同族 |
| 0-4 | 「MoonBit 穷尽 match 让加码漏改变编译期红」 | **工具链实测：不穷尽是 `partial_match` 警告（E0011），默认不是错误**；须 `moon check -d` / `--warn-list` 才升级为失败。**Rust 的 `match` 漏臂是硬错误 ⇒ 这一维度上直译会变松，不会变紧** | 实测 `moon explain --diagnostic E0011` → "Warning name: `partial_match`"；`moon check --help` → `-d, --deny-warn  Treat all warnings as errors`。工具链 `moon 0.1.20260915 (2e1a46d)` / `moonc v0.10.13+cbb11c36f` |
| 0-5 | 「Expr 枚举实测 26 变体」 | **成立**（逐条数出 26） | 亲读 `native/crates/vitro_ast/src/expr.rs:72-236` |

### 0.2 记录对账：以下条目**项目早已登记**，不得复述为"新发现"

| 本报告涉及条目 | 项目既有登记（file:line） | 记录原文要点 |
|---|---|---|
| 非 ASCII 列号失真 | [`代码审阅与修复追踪20260906.md`](代码审阅与修复追踪20260906.md):228；**同批兄弟报告 §⑥-1 已给实测实例** | 「`make_token` 列号用字节长度（中文行偏移 2 倍）」 |
| string 侧转义（`\x`/八进制）静默错值 | [`代码审阅与修复追踪20260906.md`](代码审阅与修复追踪20260906.md):158；[`统一整备路线图.md`](统一整备路线图.md):117；**兄弟报告 §⑥-5 已给实测** | 「八进制转义 `\012` 与一位十六进制转义 `\x4` 静默产生错误字节（实测）…`"A\012B"` → sizeof=6（clang 4）；char 与 string 两处行为还不一致」 |
| `Stmt::Try`/`CatchClause` 无生产者 | 20260906:228 | 「Try/Catch 死 AST 节点」 |
| `TemplateArg` 相等性 | 20260906:228 | 「`TemplateArg::Expr` 无相等性恒 false」 |
| `Type::set_const` 兜底漏臂 | 20260906:228 | 「`set_const` 漏 3 分支」 |
| `error_catalog` 表无一致性校验 / 无直接测试 | 20260906:232 | 「`ERROR_INFO_MAP` 重复错误码静默覆盖无校验」「`error_catalog.rs` 455 行 0 直接测试」 |
| 诊断码 W/H 被打成 `E` 前缀 | [`C++子集规范.md`](../03-语言子集/C++子集规范.md):354-356；[`工程债务维护方案.md`](../01-定位与路线/工程债务维护方案.md):513 | 「**已知显示瑕疵（未修，如实记录）**…修复需调整 `session_api::compile` 的 code 前缀生成规则（按 severity 输出 `E`/`W`/`H`），属独立小项」；2026-09-11 已实测 `(E3053)` 误标 |
| `Type::PartialEq` 非自反 / `Display` 丢修饰符 | [`统一整备路线图.md`](统一整备路线图.md):210（U7#7） | 「`Type::PartialEq` 对 Typeof 非自反却 `impl Eq` 修复 + `Display` 补 unsigned/const…error_catalog ~36 个零引用变体清理 + E4105/E4106 接线裁定」 |
| 尺寸链 checked / 巨数组溢出 | [`统一整备路线图.md`](统一整备路线图.md):179（U5#4）；20260906:164 | 「`ast/codegen` 统一 checked_mul + 超限编译期错误（`int a[50000][50000]` 当前 debug panic/release 负尺寸倒退帧布局）」；「超大数组维数 i32 乘法溢出无检查（实测 `int a[100000][100000]` 无诊断挂死）」 |
| 递归深度保险丝 | [`统一整备路线图.md`](统一整备路线图.md):426-438（U1#8） | 「`vitro_ast::depth` 迭代测深后置预算（512 上限，超限 `mem::forget`）」 |
| 自含 struct 栈溢出（T-P0-8） | 20260906:118 | 已修（`visiting` 环保护 + E3072） |
| `void*` 算术步长不一致 | 20260906:173（§4.2#5） | 「`void*` 算术 `+`/`++` 步长为 0，与 `+=`（步长 1）不一致」 |

**本次复核的三处自我更正（诚实记录）**：

1. 初稿把「E3060/E3061 无知识卡条目」列为缺陷 —— **实测证伪**：二者**都有**条目（M-6），真正缺的是 `E3070_BufferOverflow`。
2. 初稿把「5 处伪造 `E` 前缀」全部列为缺陷 —— 亲读 `native/src/engine/compile_pipeline.rs:176-198` 后修正：
   `push_warnings`/`push_hints` 只调 `push_one`，**不构造** `"错误 E{}"` 富文本 ⇒ 该拼接只在 `push_diagnostics`（error 级）生效，
   对 W/H 码**不可达**。**有效伪造点 3 处**（M-3/M-4/M-5/M-7 覆盖）。
3. 初稿把「非 ASCII 列号失真」标为**未复现**并准备移出缺陷位 —— **自己复现成功**（M-13，
   输入形态 `int main(){ int "中文"; }`，实报列 13、应 17）。初稿的四次尝试失败是**输入形态没选对**
   （需让**解析错误落在含非 ASCII 的 token 上**），不是缺陷不存在。兄弟报告 §⑥-1 已记录同一现象，本报告为**独立复现**而非首报。

---

## 1. 实测发现登记（本模块域，接续 `实测发现登记20260913` 编号体系）

> 工具：`native/target/debug/vitro_cli.exe`（2026-09-18 13:01:42）。
> **M-0 产物新鲜度**：`target/release/vitro_cli.exe` = **2026-09-15 00:05:31**，引擎源码最新改动 = **2026-09-18 13:00:34**
> ⇒ **release 产物陈旧 3 天**。本仓有产物新鲜度门禁制度（[`三语化整备审计计划.md`](三语化整备审计计划.md):73 **原记**"影子验证/回放可能在陈旧二进制上**假绿**"——该行号是**原文出处引用**，非断言计数），
> 故本次一律用 debug（源码 13:00:34 → 产物 13:01:42，新鲜）。

### 1.1 摘要表

| 编号 | 域 | 问题 | 证据等级 | 状态 |
|---|---|---|---|---|
| M-0 | 工程 | release 产物比源码旧 3 天 | 实测（时间戳） | 新登记 |
| M-1 | 词法/字符串 | `"\xff"` 落成 **2 字节** `C3 BF`，`sizeof` 5（clang 4） | **实测** | **新登记**（对兄弟报告 ⑥-5 的**形态补充**：那条覆盖 `\x4`/`\012`，未含"编码成 UTF-8 双字节"这一支） |
| M-2 | 词法/字符串 | `"A\012B"` → `sizeof=6`；`"\x4"` → `sizeof=3`（clang 4 / 2） | **实测** | 既有**未修**（20260906:158）；与兄弟报告 ⑥-5 独立复现一致 |
| M-3 | 诊断协议 | **警告**打 `E` 前缀：`[警告] 2:12 … (E3053)` | **实测** | 既有**未修**（`C++子集规范.md:354-356`） |
| M-4 | 诊断协议 | **提示**级同样打 `E`：`[提示] 2:45 … (E3057)` | **实测** | **新登记**（既有记录只提 W 级） |
| M-5 | 诊断协议 | serve JSON **同一帧内自相矛盾**：`code=E3053` + `error_code=3053` + `severity=warning` | **实测** | **新登记**（把 M-3 从"CLI 显示"升格为**机器可读契约**问题） |
| M-6 | 诊断协议 | `error_catalog` 实测 **77** 条 / 枚举有码 **136** / **缺 59** | **实测** | **新登记**（既有"~36 个零引用变体"是**另一口径**） |
| M-7 | 诊断协议 | catalog 导出对 W/H 码统一伪造 E：`3053 → "E3053"` | **实测** | **新登记** |
| M-8 | codegen | `void*`：`p+1` 位移 **0**，`p+=1` 位移 **1** | **实测** | 既有**未修**（20260906:173） |
| M-9 | parser/types | `int a[100000][100000]` → **debug panic** `attempt to multiply with overflow` @ `type_.rs:540` | **实测** | 既有，**形态修正**（20260906:164 记"挂死"；与 U5#4"debug panic"一致） |
| M-10 | AST | 自含 struct → `E3072` 明确诊断，无崩溃 | **实测** | 既有**已修成立**（T-P0-8） |
| M-11 | 词法 | 中文标识符 → `[错误] 1:18 无法识别的字符: '中' (E1001)` | **实测** | 现状登记（该路径列号 = 字符位，**自洽**） |
| M-12 | 词法/字符串 | 中文字符串 `sizeof("中文")` = **7** | **实测** | 与 clang 一致（UTF-8 源下无分歧） |
| M-13 | 词法/位置 | 解析错误落在含非 ASCII 的 token 上 → **列号偏差 −4**：`int main(){ int "中文"; }` 报 `1:13`（应 17） | **实测** | 既有**未修**（20260906:228 登记；兄弟报告 ⑥-1 同现象，本报告**独立复现**） |

### 1.2 实测原始输出

```powershell
$exe = "D:\code\Vitro\native\target\debug\vitro_cli.exe"   # 2026-09-18 13:01:42
```

**M-1 `\xff` 字节流（新登记）**

```powershell
'#include <stdio.h>
int main(){char s[]="A\xffB";printf("sizeof=%d bytes=%d,%d,%d,%d\n",(int)sizeof(s),s[0]&255,s[1]&255,s[2]&255,s[3]&255);return 0;}' | & $exe run -
```

```
sizeof=5 bytes=65,195,191,66
```

判读：`s[1..2] = 195,191 = C3 BF` ⇒ **`\xff` 被编码成 U+00FF 的 UTF-8 两字节**；clang 为 `sizeof=4 / 65,255,66`。
根因链与亲读吻合：`vitro_lexer/src/string.rs:43`（`value.push(byte as char)`）→ `vitro_codegen/src/expr/literal.rs:23`（按 UTF-8 长度分配）。
**这是 AST 字段 `Expr::StringLiteral.value: String` 的载体选择所造成的、有实例的字节流语义分歧。**

**M-2 转义（既有未修）**

```powershell
'…char s[]="A\012B"…' | & $exe run -        # → sizeof=6
'…char s[]="\x4"…'    | & $exe run -        # → sizeof=3
```

与 20260906:158 登记的实测数字**逐字一致**（clang 4 / 2）⇒ 登记项至今未修（U1#7 只修了 char 侧，string 侧未同步，见 `vitro_lexer/src/string.rs:105-109` 与 `:36-52` 的对照）。
`native/tests/**/*.c` 对 `\xHH`/八进制转义的覆盖为 **0**（grep 零命中）—— 按 [`统一整备路线图.md`](统一整备路线图.md):123（M14）的一般化原则
「**与 Clang 的数值类差异零 golden 覆盖即防线失效**」，这两类差异目前处于防线盲区。

**M-3 / M-4 前缀伪造（警告级 + 提示级）**

```powershell
'int main(){int i = 1.5;printf("%d\n",i);return 0;}' | & $exe run -
```

```
[警告] 2:12  double 被隐式转换为 int，可能会丢失精度。 (E3053)
```

```
[提示] 2:45  具体指针类型被隐式转换为 void*。 (E3057)
```

判读：`W3053`、`H3057` 均被打印为 `E…`。既有记录只登记 W 级；**hint 级为本次新实测**。

**M-5 / M-7 serve 通道（机器可读契约）**

```powershell
'{"id":1,"method":"compile","params":{"filename":"main.c","source":"int main(){int i = 1.5;return 0;}"}}
{"id":2,"method":"error_catalog"}
{"id":9,"method":"shutdown"}' | & $exe serve
```

```
id=1: code=E3053  error_code=3053  severity=warning  line=2 col=12
id=2: 条目数 = 77；3053 → code_str=E3053  lang=c  category=语义
```

判读：**同一帧内 `code` 前缀与 `severity` 相矛盾** —— 上游若按 `code` 字符串判 severity 即错。
下游判分契约依赖 `error_code` 数值（[`CSharp前端引入计划.md`](../03-语言子集/CSharp前端引入计划.md):322），故尚未受伤，
但这是**对外协议层**的瑕疵，比"CLI 显示问题"严重一级。`code_str` 对所有 W/H 码统一伪造 E。

**M-6 覆盖率（实测锚定，取代静态计数）**

```
实测 error_catalog 条目数 = 77   范围 = 1002 ~ 4104
枚举有码变体 = 136              实测无知识卡条目的码 = 59
```

缺失清单（**由实测导出与枚举定义在同一次运行内求差集**，非跨时点拼接）：

```
E1001, E1011~E1022(含 W1018/W1019), E2001, E3001, E3049, W3050, E3058, E3059,
E3062, E3063, W3064, E3065, E3070, E3071, E3072,
E4001~E4031（31 条 C++ 段全缺）, E4105, E4106
```

要点：**`E3070_BufferOverflow` 缺**（UAF/DoubleFree 两张卡有，缓冲区溢出这张没有）；
**`E4001~E4031` 全段 31 条缺** —— 与 [`CSharp前端引入计划.md`](../03-语言子集/CSharp前端引入计划.md):50（D4 裁决）登记的
「E4001~E4031 目前在 catalog 无条目——存量缺口一并补」一致；
**实验证伪**：`E3060_UseAfterFree` / `E3061_DoubleFree` **有条目**。

**M-8 `void*` 步长（既有未修）**

```
plus=0 plusassign=1
```

与 20260906:173 登记的"`+`/`++` 步长为 0，与 `+=`（步长 1）不一致"逐字吻合 ⇒ 未修。

**M-9 巨数组（形态修正）**

```powershell
'int main(){ int a[100000][100000]; a[0][0]=1; return a[0][0]; }' | & $exe compile -   # 20s 超时保护
```

```
thread 'main' panicked at crates\vitro_parser\src\type_.rs:540:29:
attempt to multiply with overflow
```

判读：形态是 **panic**（不是登记里的"挂死"），与 U5#4 的"debug panic"一侧吻合。
附带观察：**该 panic 直接击穿 CLI**（`vitro_cli compile` 无 `catch_unwind`；capi 侧有，见 `native/src/capi/first_batch.rs:6,64`），
属"panic 击穿出口"家族，登记备查。

**M-10 自含 struct（已修成立）**

```
[错误] 1:8  结构体 'S' 的值成员存在循环包含（经 'S' 回到自身）：结构体不能按值包含自己，
            链表/树节点请改用指针成员，如 `struct S* next;` (E3072)
```

**M-11 / M-12 非 ASCII（自洽路径）**

```
[错误] 1:18  无法识别的字符: '中' (E1001)      # '中' 是第 17 个字符 → 列 = 字符位（post-char），自洽
sizeof=7                                        # "中文" = 3+3+1，与 clang 一致
```

**M-13 非 ASCII 列号偏差（本次独立复现）**

```powershell
'int main(){ int "中文"; }' | & $exe compile -
```

```
[错误] 1:13  预期标识符名称 (E2005)
```

列号核算（同一行）：

```
i=1 n=2 t=3 ␠=4 m=5 a=6 i=7 n=8 (=9 )=10 {=11 ␠=12 i=13 n=14 t=15 ␠=16 "=17
```

判读：字符串 token 起始应为**列 17**，实报 **13**，**偏差 −4** = 该 token 的 **8 字节 − 4 字符**。
根因（亲读）：`vitro_lexer/src/lib.rs:419-432` 的 `advance()` 按 **char** 递增 `column`，
而 `:443-449` 的 `make_token` 用 `self.column - text.len()`（**byte** 长度）反推列号；
`string.rs:72-74` 又把源拼写（含引号）作为 token text 传入 ⇒ 反推量偏大 4。
**注意 M-11 揭示的关键区分**：同一份中文输入，**词法错误路径（E1001 用 `self.column`）列号自洽，解析错误路径（用 token 列号）偏差 −4** ——
"列号是否可信"取决于诊断来自哪条路径。这解释了初稿为何四次都没复现：**必须让解析错误落在含非 ASCII 的 token 上**。

### 1.3 与兄弟报告的边界与交叉引用

| 事项 | 归属 | 处置 |
|---|---|---|
| string 侧转义实现（`string.rs:36-52`）、`make_token` 列号、`Vec<char>`/splice 架构、116 变体 token | **lexer 模块报告**（②③⑥ 各节） | 本文只登记**与本模块字段类型相关**的那一面：`Expr::StringLiteral.value: String` 的载体选择（§4-B5/§6-4） |
| M-1（`\xff` → UTF-8 双字节） | lexer 报告 ⑥-5 的**形态补充** | 该条覆盖 `\x4`/`\012`，未含"≥0x80 字节被 UTF-8 重编码"这一支；本文补实测，**不主张首报** |
| M-13（列号偏差） | lexer 报告 ⑥-1 的**独立复现** | 同一现象、同一数字（13 vs 17）；本文的作用是把它与"`SourceLoc` 列单位契约"（§6-5）和"AST dump 锚点列字段"（§9-E1）连起来 |
| 等价性锚点层次 | lexer 报告 §⑧ **L1 = 预处理前原始 token 流 TSV**；本文 §⑨ **E1 = AST dump JSON** | **两层互补，不重叠**：L1 锚"词法边界"，E1 锚"语法/类型形状"。**E1 依赖 L1 先定列号口径**（L1 的列号字段若参与比对，须先修 M-13，否则两侧"一致地错"） |

### 1.4 前一轮勘察中被本次实测改变的三处判断

| 上轮判断 | 实测后 |
|---|---|
| 「E3060/E3061 无知识卡条目」 | **撤回**（事实错误）：两者都有条目；真正缺的是 `E3070` |
| 「5 处伪造 E 前缀」 | **改为 3 处有效 + 2 处同形不可达**：`compile_pipeline.rs:158,167` 的富文本只在 error 级路径构造 |
| 「非 ASCII 列号失真 → 未复现，移出缺陷位」 | **恢复为实测缺陷（M-13）**：自己复现成功（列 13 vs 应 17）；初稿的四次尝试失败属**输入形态设计失误** |
| 「`\xHH` 两字节化为亲读推导」 | **升级为实测**（M-1，有数字）；定位为兄弟报告 ⑥-5 的形态补充 |

---

## 2. ① 模块概览与规模（实测）

### 2.1 规模（逐文件行数，`[System.IO.File]::ReadAllLines().Length`）

| crate | 文件 | 行数 |
|---|---|---|
| `vitro_shared` | `src/lib.rs` 9 / `src/source_loc.rs` 21 / `src/error_codes.rs` 157 | **187** |
| `vitro_ast` | `src/lib.rs` 112 / `src/decl.rs` 208 / `src/depth.rs` 196 / `src/expr.rs` 295 / `src/stmt.rs` 98 / `src/types.rs` 685 | **1594** |
| **合计** | **9 个 `.rs`** | **1781 行**（空行 80，非空 1701） |

对照：`native/` 全量 257 文件 / 76,218 行 / 1,015 个 `#[test]`（`MoonBit迁移方案评估报告20260918.md` 附录 B，AS-OF 2026-09-18 冻结）
⇒ 本模块占 **2.3%** 代码量。

依赖：`vitro_shared` 仅依赖 `serde`（`crates/vitro_shared/Cargo.toml:7`）；`vitro_ast` 仅依赖 `vitro_shared` + `serde`（`crates/vitro_ast/Cargo.toml:7-8`）。
**零第三方运行时依赖**，是全工程依赖最干净的两个 crate。

### 2.2 关键类型清单（实测枚举计数）

| 类型 | 位置 | 变体/字段数 |
|---|---|---|
| `SourceLoc` | `source_loc.rs:1-9` | 3 字段 `line`/`column`/`file_id`，`Copy`+`Default`+serde，**无 `PartialEq`/`Eq`/`Ord`/`Hash`** |
| `ErrorCode` | `error_codes.rs:8-157` | **137 变体**（`Unknown` 1 + E 段 124 + W 段 12 + H 段 1）；`#[repr(i32)]`，**无 serde** |
| `TypeKind` | `types.rs:9-28` | 16 变体 |
| `Type` | `types.rs:32-104` | **17 变体**（手写 `PartialEq` + `impl Eq`，手写 `Display`，`mangle_name_into`，约 20 个形状查询存取器） |
| `Expr` | `expr.rs:72-236` | **26 变体**（`MemberCall` 单变体 9 字段） |
| `BinaryOp`/`UnaryOp`/`AssignOp` | `expr.rs:9-57` | 19 / 9 / 11 |
| `Designator`/`InitElement` | `expr.rs:60-69` | 2 变体 / 2 字段 |
| `Stmt` | `stmt.rs:8-90` | **16 变体** |
| `CatchClause` | `stmt.rs:93-98` | 4 字段 |
| `Param`/`FuncDecl`/`StructField`/`StructDecl`/`GlobalDecl` | `decl.rs:9-52` | — |
| `AccessSpec`/`ClassMember` | `decl.rs:59-103` | 3 / **6 变体**（`Field`/`Method`/`Constructor`/`Destructor`/`NestedStruct`/`NestedClass`） |
| `VTable`/`ClassDecl` | `decl.rs:106-117` | — |
| `TemplateParam`/`TemplateArg`/`Templateable`/`TemplateDecl`/`TemplateInstantiation` | `decl.rs:120-185` | 2 / 3 / 2 / — / — |
| `CaptureMode` | `decl.rs:188-192` | 3 变体 |
| `ProgramNode` | `decl.rs:199-208` | 7 个 `Vec`（C 4 + C++ 3） |
| 自由函数 | `lib.rs:24-112` | `base_element_type` / `compute_type_size`（带环保护） |
| 深度遍历 | `depth.rs:20-196` | `expr_depth` / `stmt_depth`（显式栈迭代，非递归） |

### 2.3 错误码分段实测（137 变体精确分布）

| 段 | 数量 | 码位（含空洞） |
|---|---|---|
| 1xxx 词法 | 20 | 1001-1007、**1010**-1022（1008/1009 空洞）；1018/1019 是 `W` 前缀 |
| 2xxx 语法 | 8 | 2001-2008 |
| 3xxx 语义 | 70 | 3001-3067 连续、**3068/3069 空洞**、3070-3072；其中 12 个是 `W`/`H` 前缀（3050-3057、3064、3067） |
| 4xxx C++ | 38 | 4001-4031（错误码）+ 4100-4106（知识卡） |
| 5xxx C# | **0** | 无变体，仅范围映射 |
| `Unknown` | 1 | = 0 |

**协议层关键结论**：**前缀（E/W/H）不是码段的函数，码号本身才是身份** —— `W1018`/`W1019` 占 1xxx 段，`W3050`~`H3057` 占 3xxx 段。
这直接导致 §1.2（M-3~M-7）的身份字符串伪造缺陷。

### 2.4 诊断骨架作为「语言中立协议」的完整度评估

**结论：骨架可用，但身份字符串不单源；且缺陷面是"对外协议层"而非仅"CLI 显示"。**

实测确认的**有效伪造点 3 处**（均把 `i32` 拼成 `"E{n}"`，无一查询 `ErrorCode` 变体名——而变体名里才带真实前缀）：

| # | 位置 | 形态 | 对 `W1018_MacroShadowing`（实存码，`error_codes.rs:26`）的实际输出 |
|---|---|---|---|
| 1 | `native/src/session_api.rs:50` | JSON `"code": format!("E{}", d.error_code)` | `"code":"E1018"` + `"severity":"warning"`（**同帧自相矛盾**，severity 由 `severity_name`（`:26-33,52`）独立给出）—— M-5 已实测 |
| 2 | `native/src/diagnostics/error_catalog.rs:536` | `"code_str":"E{}"` | `"code_str":"E1018"` —— M-7 已实测 |
| 3 | `native/src/bin/vitro_cli.rs:94` | CLI `(E{})` | `E1018` —— M-3/M-4 已实测 |

**同形但当前不可达的 2 处**（亲读修正）：`native/src/engine/compile_pipeline.rs:158,167` 构造 `"错误 E{}"` 富文本，
但 `push_warnings`/`push_hints`（`:176-198`）只调 `push_one`，该字符串**只在 error 级（`push_diagnostics`）生成** ⇒ 对 W/H 码不可达。
其消费者为 `native/src/capi/first_batch.rs:191-192`（`{"kind":"compile","message": …}`）。

另一侧完整度缺口：**知识卡覆盖率 77/136 = 57%**（M-6），且
`lookup_error_info` 返回 `Option`（`error_catalog.rs:41-43`），调用方 `compile_pipeline.rs:112,132,155` 全部走
「有就用、没有就空」的静默降级 ⇒ **加码漏加知识卡零信号**。

**「加码漏改」的真实风险面**：

- **全仓 `ErrorCode` 的穷尽 match = 0 处**（`grep 'ErrorCode::\w+ =>'` 零命中）；**59 处** `ErrorCode::X as i32` 立即脱类型
  （`vitro_lexer/src/preprocessor/mod.rs:92` 是唯一封装点）。
- 消费侧全部是 **i32 键表 + 兜底**：`error_catalog.rs:65`/`:477`/`:489`/`:503`（含 `_ => "unknown"`、`_ => "其它"`）；
  `diagnostics/knowledge_graph.rs:426`（数字键）、`:460`（`activate_from_error(error_code: i32)`）；
  `diagnostics/misconception_patterns.rs:16,28,54-94`（`error_codes: Vec<i32>` 硬编码 `vec![3021, 3051]` 等）。
- ⇒ **今天加一个码，Rust 编译器一句话都不说**；MoonBit 侧若照搬 i32 键表，`partial_match`（且默认只是警告）同样不说话。
  **收益必须"设计进去"，不是"语言送的"**（§6-1、§8-S2）。

---

## 3. ② 可复用资产清单

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| A1 | **26 变体 `Expr` / 16 变体 `Stmt` 的形态划分**（哪些是节点、哪些是 op 分类、哪些字段挂 `loc`/`ty`） | `expr.rs:72-236`、`stmt.rs:8-90` | **低** | 纯数据形态，是 C23/C++ 十二个 Phase 的沉淀。**唯一成本**：Rust 的 struct-variant 命名域（`Expr::Call { name, args, loc, ty }`）+ `..` 剩余模式在 MoonBit 无直接对应（工具链实测枚举构造为**位置式**：`RGB(Int,Int,Int)`、`RGB(r,g,b) =>`；见 [Tour·枚举](https://tour.moonbitlang.com/zh/custom-types/enum/index.html)），需改造为位置参数或 payload struct |
| A2 | **`Type` 17 变体 + `TypeKind` 16 变体的双层结构** | `types.rs:9-104`、`types.rs:382-403`（`kind()` 投影） | **低** | 形状判别与负载分离：73 处只关心形状的调用点不必展开负载；且 `kind()` 的 match **已是穷尽**（无 `_` 臂）⇒ MoonBit 下天然获得"加 `Type` 变体必须表态 kind"的编译期强制 |
| A3 | **`mangle_name_into` 的 mangling 规则表**（模板单态化 / 方法重载命名契约） | `types.rs:287-379`、`decl.rs:145-155` | **低** | 纯字符串拼接，逐分支可搬；双入口已被 `ast_unit_test.rs` 锚定（A8） |
| A4 | **`depth.rs` 的显式栈迭代测深**（26+16 变体子节点枚举表） | `depth.rs:45-196` | **低** | "递归测深自身会先溢出"的教训产物（`depth.rs:3-4` 注释）；逻辑与语言无关，且是唯一不受 GC/Box 影响的纯遍历 |
| A5 | **`compute_type_size` 的 packed 布局语义 + 环保护** | `lib.rs:33-111` | **中** | 语义资产但**存在三副本**（§4-B2）；搬迁取一份，另两份的"差异"须逐条确认（`Reference/RValueRef → 4`、`Auto/TemplateId → 0`、VLA → 4） |
| A6 | **`ErrorCode` 的 137 个码位分配与命名规范**（W/H 前缀语义、E4001~E4031 NotSupported 段、E4100~E4106 教学卡段） | `error_codes.rs:8-157` | **中** | **码位是外部契约**（[`CAPI评审回复与实现状态.md`](../06-出口与协议/CAPI评审回复与实现状态.md):22,71 记下游按码段理解能力边界；[`下游需求处置回执.md`](../06-出口与协议/下游需求处置回执.md):22 记 B1 已交付）。**必须脚本化生成，不得手抄 137 行** |
| A7 | **知识卡语料**（标题/emoji/解释/常见成因）77 条 | `native/src/diagnostics/error_catalog/{lexer,parser,semantic,cpp}.rs` | **低** | 纯数据、语言无关；但**覆盖率 57%**（M-6），搬迁前须决定"补齐再搬"或"带着 57% 搬" |
| A8 | **唯一的直接测试资产**：`ast_unit_test.rs`（2 例）—— `mangle_name_into` ≡ `mangle_name` + 追加语义（黄金串 `"prefix_p_a2_3_int"`） | `native/tests/ast_unit_test.rs:33-75` | **低** | 短小、断言精确，可直接转写为 MoonBit 白盒测试 |
| A9 | **间接覆盖**：AST 被 `parser_unit_test`(17)/`parser_cpp_unit_test`(33)/`typeck_cpp_unit_test`(31)/`completion_unit_test`(14) 消费；端到端由 `end_to_end_extra_test`(5152 行) 等集成套件承载 | `native/tests/` | **中** | 断言多为端到端 stdout，**不直接锚 AST 形状，不能替代 AST 差分**（§9） |
| A10 | **语言无关的 `SourceLoc` 三字段模型**（`file_id=0` 表示用户主文件） | `source_loc.rs:5-9` | **低** | 语义已被下游消费：`vitro_runtime/src/instruction.rs:9` 每条指令挂 `loc`、`bytecode_libc_loader.rs:50` 用 `file_id` 标记库指令、`executor/mod.rs:295` 用 `file_id==0` 过滤覆盖率统计。**搬它等于搬全引擎定位口径** |

**测试资产实测（重要）**：`vitro_shared` 与 `vitro_ast` **各 0 个 `#[test]`、0 个 `#[cfg(test)]`**（逐文件统计）。
本模块的"测试资产"实质只有 A8 的 2 例。对照：兄弟模块 `vitro_lexer` 有 **57 个** 单元测试（`native/tests/lexer_unit_test.rs`）。

---

## 4. ③ 抛弃清单

| # | 抛弃项 | 证据 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| B1 | **`Box<Expr>`/`Box<Type>`/`Box<Stmt>` 全族间接层**（`expr.rs` 内约 20 处、`types.rs:55,59,66,86,90,99`、`decl.rs:169-170`、`stmt.rs:27-88`） | §2.2 逐字段列举 | Rust 值语义需要显式间接表达递归；MoonBit 为 GC 语言，递归 enum payload 天然是引用 | 低。**要点**：`Vec<Box<Expr>>`（仅 `types.rs:64` 的 `vla_dims`）在 Rust 里本身即**冗余间接**（`Vec` 已间接），MoonBit 下自然写成 `Array[Expr]` |
| B2 | **`compute_type_size` 的三副本** | ① `vitro_ast/src/lib.rs:33-111`（活）② `native/src/compiler/ast.rs:37-116`（**死文件**）③ `vitro_codegen/src/lib.rs:847-848` + `vitro_typeck/src/context.rs:8-41`（薄转发到 ①） | `native/src/compiler/mod.rs:4` 是 `pub use vitro_ast as ast;`，全仓无 `mod ast;` 声明（grep 零命中）⇒ `compiler/ast.rs` **从未被编译**（其 `pub mod decl;` 还指向不存在的目录）。注释里的"保持同一语义"义务已是空转（`lib.rs:32` 与 `ast.rs:36,44` 互相指认） | **中**。风险不在删除而在**误信**：调用方（`typeck/context.rs:41`、`codegen/lib.rs:848`、`session_api.rs:543`、`parser/decl.rs:820`、`compile_pipeline.rs:532,771`）全走 ①。验证法：`grep -rn "mod ast" native/src`（应为空） |
| B3 | **`Type::PartialEq` 手写实现 + `impl Eq`** | `types.rs:106-183` | 三点必须**抛弃设计而非仅抛弃代码**：(a) `Typeof` 对自身返回 `false`（`:165`）却 `impl Eq`（`:183`）⇒ 违反 `Eq` 自反性；(b) `Array` 比较用 `..` **忽略 `vla_dims`**（`:124-141`）⇒ 两个 VLA 维度表达式不同的类型判等；(c) 手写 17 臂维护成本。**注**：`TemplateArg::Expr` 无相等性恒 false 已被 20260906:228 登记 | **高（本模块最实质的语义缺陷）**。MoonBit 侧不能靠 `derive(Eq)`：`Expr::FloatLiteral{value: f64}` 使 `Expr` 无法 `Eq`，而 `Type::Array` 含 `Vec<Box<Expr>>` ⇒ **必须显式设计"类型相等"判据**（§6-2、§8-S4） |
| B4 | **`ErrorCode` 的 `as i32` 脱类型范式（59 处）+ `#[repr(i32)]`** | `preprocessor/mod.rs:92` 等 59 处 | Rust 下 enum→int 只能 `as`，码号在创建点即丢失类型与 severity 前缀信息（M-3~M-7 的实测后果） | **高**。MoonBit 无 `as Int`（需穷尽 match）；**若不设计，就退化为"再写 137 臂 match 然后照样脱类型"**。必须与"码号单源"一起做（§6-1） |
| B5 | **AST 上未被消费的 `serde::Serialize/Deserialize` 派生** | `expr.rs:8,31,44,59,65,71`、`stmt.rs:7,92`、`types.rs:8,31`、`decl.rs` 多处、`source_loc.rs:1` | **亲读**：全仓 **无任何一处序列化 AST**（22 个 `serde_json::from_str` 调用点无一消费 `ProgramNode`/`Expr`/`Type`；唯一相关消费者为 `BytecodeLibcArtifact` 与 `builtin_layout`）。唯一被真正序列化的本模块类型是 **`SourceLoc`（经 `Instruction`）** | **中**。派生本身无害，但**搬迁时若默认"衍生了就等价"，会把"没有格式契约"误当"有格式契约"**。§9 的 AST dump 必须**新写显式 emitter** |
| B6 | **`Stmt::Try`/`CatchClause` 的"无生产者"脚手架** | AST 定义 `stmt.rs:85-98`；typeck 消费 `vitro_typeck/src/decl.rs:576-577`（报 E4001）；codegen 消费 `vitro_codegen/src/stmt/mod.rs:55` → `stmt/cpp.rs:19-21`（只 `report_error("尚未实现")`）；**lexer 无 `try`/`catch` 关键字**（`vitro_lexer/src/keyword.rs` grep 零命中）⇒ parser 永不产出。项目已登记为「Try/Catch 死 AST 节点」（20260906:228） | 三处脚手架 + 一个 AST 节点，唯一构造点在测试手工构造（`native/tests/typeck_cpp_unit_test.rs:280`，该文件 `:273-274` 自注"Parser does not support try/catch syntax yet"） | **必须与 C# 计划联动**：[`CSharp前端引入计划.md`](../03-语言子集/CSharp前端引入计划.md):272-273（CS3a/CS3b）**正是要 try/catch/finally** ⇒ **不要删**，标 `reserved-for-csharp`（§5-6、§6-6） |
| B7 | **存量台账中已漂移的 file:line**（本模块相关） | 20260906:163,164,173 指向 `vitro_ast/src/lib.rs:55-62`（现为 `compute_type_size` 内部注释）、`vitro_ast/src/types.rs:229-243,555-571,591`（现为 `array_of`/`total_elements`/`subscript_type` 附近） | 行号漂移使"已登记未修"条目无法直接定位 | **低但持续腐蚀**。搬迁时按**符号名**重新索引，不照抄台账行号 |

---

## 5. ④ 在途工作接纳方案

| # | 在途条目（既有登记） | 现状证据 | 新项目接纳方式 |
|---|---|---|---|
| 1 | **U7#7 前半：`Type::PartialEq` 对 `Typeof` 非自反却 `impl Eq`** | `统一整备路线图.md:210`；代码 `types.rs:165,183` | **直接按目标架构实现**：MoonBit 无 `Eq` 派生可用（`f64` payload），落地 = 显式 `Type::structural_eq` + 显式 `vla_dims` 策略（比较 or 明确不比较并写文档），配白盒测试锚定 `Typeof == Typeof` 为真。**不搬手写 17 臂** |
| 2 | **U7#7 中段：`Type: Display` 补 `unsigned`/`const`** | `统一整备路线图.md:210`；证据 `types.rs:613-682` 全部丢修饰符（`Int{..} => "int"`），而 `Reference` 却打 `const `（`:661-666`）⇒ 同函数内部口径不一致 | **直接按目标架构实现**，但**必须先裁定唯一渲染函数**：现存**两个**渲染器 —— `types.rs:613-682`（`Display`，丢修饰符）与 `vitro_runtime/src/type_utils.rs:28-113`（`type_display_name`，保留修饰符，且注释自称"类型名的**单一来源**"`:25`）。**该"单一来源"声称为假** |
| 3 | **U7#7 后段：error_catalog 零引用变体清理 + E4105/E4106 接线** | `统一整备路线图.md:210`；实测：**35 个零引用变体**、**E4100~E4106 全部 0 引用**、**59 个码无 catalog 条目**（M-6） | **清理部分放弃并记录理由**：35 个零引用变体里 21 个属 E4001~E4031/E4100~E4106 **预留段**，删除等于拆掉 C# 接入挂点。**改为**：搬迁时显式标 `reserved`（与 `unified/vocabulary.rs` 的 `reserved` 词汇先例同构，见 `CSharp前端引入计划.md`:229-231），并让"码已定义但无知识卡"成为**可断言清单**进 CI，而非靠人记 |
| 4 | **CS5：E5xxx + catalog 语言字段** | `CSharp前端引入计划.md:50,275`；实测 E5xxx 空段、`lang_of_code` 已就位（`error_catalog.rs:476-483`） | **直接按目标架构实现**：语言字段从"由码段推断"升级为**码自带字段**——码段推断在 `W1018` 这类"前缀与段不符"的码上已证明脆弱。**不搬** `lang_of_code(code: i32) -> &str` 推断式 |
| 5 | **CS0：`SourceLang` enum 单源化** | `CSharp前端引入计划.md:29,269`（"现状唯一检测点 `compile_pipeline.rs:603`"） | 本模块**间接相关**：`is_cpp_mode` 不在 AST（AST 由节点自感知，`:47`），但 `ErrorCode` 的 `lang` 与 catalog 导出吃这个轴。**迁移时一并做**：`SourceLang` 与 `ErrorCode` 同包，`lang` 由码的构造方式直接携带 |
| 6 | **CS3a/CS3b：try/catch/finally + 3 个新 opcode** | `CSharp前端引入计划.md:90-119,272-273`；AST 已备 `Stmt::Try`/`CatchClause`（`stmt.rs:85-98`），lexer 无关键字 | **修复后搬**：`Stmt::Try`/`CatchClause` **原样搬进 MoonBit AST**（不删），但在 §9 的 AST dump 中显式标注"无生产者节点"，避免被误读为"已实现异常支持"。C# 前端落地时即现成挂点 |
| 7 | **U5#4：尺寸链全 checked + 多维不定长数组 `array_size` 修正** | `统一整备路线图.md:179`；热点 `types.rs:229-245`（`array_of` 的 `product()`）、`:557-573`（`total_elements`）、`:593`（`subscript_type`）、`lib.rs:68`（`elem_count * elem_size`）。**M-9 实测现状 = debug panic** | **直接按目标架构实现**。MoonBit 数值溢出语义**未实测**（§8-S1）⇒ 不能假设"换语言就不溢出"。落地 = 全链路 `Int` 乘法显式上界检查 + 超限报编译错误 |
| 8 | **U7#4：`compute_type_size` 布局缓存 + 深递归迭代化评估** | `统一整备路线图.md:207` | **直接按目标架构实现**：MoonBit 侧做成"纯函数 + 显式 memo 表"，环保护沿用 `visiting` 语义（`lib.rs:42,72,91`） |
| 9 | **D14/D16：`vitro_typeck/src/decl.rs` 3 处 `unwrap` 与 871 行超标** | [`工程债务维护方案.md`](../01-定位与路线/工程债务维护方案.md) D14/D16 行 | **不属本模块**（typeck），但**搬迁时天然消解**：MoonBit `Option` 需显式匹配，`unwrap` 无处可写。**在 U 批次不必再投入**，标"由重写消解"（诚实记录：这是重写少数的顺带收益之一，**不作为主要理由**） |
| 10 | **T-P0-8（已修）：自含 struct 栈溢出** | 20260906:118；修复在 `lib.rs:39-43,72-74,90-92`；**M-10 实测 E3072 生效** | **修复后搬**：语义（环 → 返回 0 防崩 + typeck Pass 1 报 E3072）必须保留，且须在新语言下重验（§8-S5） |
| 11 | **M-1/M-2 字符串字节流缺陷**（本次实测；词法侧归属见兄弟报告 ⑥-5） | 20260906:158 + M-1 补充实测 | **放弃修复 Rust 版，直接按目标架构实现**：载体改 `Bytes`（§6-4）后该缺陷**结构性消失**；但**必须先补 golden**（当前覆盖 0），否则重写后无法证明"新实现对了" |
| 12 | **M-13 列号偏差 + `SourceLoc` 列单位未定义** | M-13 实测（列 13 vs 应 17）；兄弟报告 ⑥-1/⑥-2 | **直接按目标架构实现**（不搬旧算法）：位置由**扫描器记录**而非"用文本长度反推"；`SourceLoc` 显式声明列单位（§6-5）。**这是本模块与 lexer 模块唯一的强耦合点**，须与 L1 锚点同批定契约 |

---

## 6. ⑤ 架构优化建议（MoonBit 形态）

### 1.〔新设计〕错误码 = 穷尽 match 单源 + `-d` 门禁，把"加码漏改"真正变成编译失败

- **做什么**：`ErrorCode` 保留 137 臂；**唯一**的码号/前缀/severity/语言/知识卡查表全部改为**对 enum 的穷尽 match**（无 `_` 臂）：
  `code_of` / `name_of`（`"E1018"`、`"W3053"` 由变体名推出）/ `severity_of` / `lang_of` / `catalog_of`；
  M-3~M-7 的 3 处有效伪造点收敛为 1 处 `name_of`。
- **为什么真有效**：MoonBit **会**报 `partial_match`（工具链实测 E0011），但**默认是警告** ⇒ 必须同时把 `moon check -d`
  （或 `--warn-list` 精确配置）钉进 CI，与现仓 `cargo clippy -- -D warnings` 同强度。**这一步不做，本条收益为零。**
- **支撑证据**：Rust 侧今天 **0 处穷尽 match**、59 处 `as i32`、**3 处有效伪造前缀（M-3/M-4/M-5/M-7 实测）**、
  59/136 无知识卡（M-6 实测）——"加码漏改"在编译期与测试期**均无信号**。

### 2.〔结构优化〕类型相等与类型渲染各自单源

- `Type` 判等收敛为一个显式函数（含 `Typeof` 自反、`vla_dims` 策略、`is_const` 是否参与逐条裁定）；
  渲染收敛为一个 `to_c_string`（保留 `unsigned`/`const`），删除 `Display` 与 `type_display_name` 双轨。
- 证据：`types.rs:106-183`（自相矛盾的手写 Eq）× `vitro_runtime/src/type_utils.rs:25`（自称单一来源）× `types.rs:613-682`（第二个来源）。

### 3.〔沿革保留〕`TypeKind` 投影层与 `kind()` 分离

73 处只查形状的调用点不必展开负载；`kind()` 的 match 已穷尽（`types.rs:382-403` 无 `_` 臂）⇒ MoonBit 下天然获得
"加 `Type` 变体必须表态 kind"的编译期强制（**这一条是真实的**）。

### 4.〔结构优化〕`StringLiteral` / `Identifier` 的载体从 `String` 改为 `Bytes`

- **做什么**：`Expr::StringLiteral.value`、`Expr::Identifier.name`、字段/成员/方法名统一用 `Bytes`（不可变字节串）；
  **源码**本身亦以 `Bytes` 进 lexer（与兄弟报告 ⑤-1 的 `Bytes`+`BytesView` 方案同一契约）。
- **实测证据**：M-1（`"\xff"` → 2 字节，`sizeof` 5）、M-2（`\012` → 6；`\x4` → 3）；`native/tests/**/*.c` 覆盖 **0**。
- **MoonBit 侧硬事实（工具链实测）**：`String::at(String, Int) -> UInt16`（别名 `code_unit_at`）、
  `String::code_units(String) -> ArrayView[UInt16]`、`String::char_length` **是另算的** ⇒ **`String` 以 UTF-16 码元索引**
  （另见 [core 提交：为 String 文档补 UTF-16 code unit 说明](https://github.com/moonbitlang/core/commit/84fb2da0d776d93752bba4fa84d1492e6b023b0d)）。
  C 的字符串/源码语义是**字节流**，用 `String` 承载 = 语义错配（Rust 版已把"UTF-8 字节 ≡ C 字节"这一错误假设固化进 4 处 `.as_bytes()`：
  `codegen/src/stmt/var_decl.rs:156,384`、`codegen/src/lib.rs:394`、`codegen/src/expr/literal.rs:23`）。

### 5.〔新设计〕`SourceLoc` 保持 3×`Int`，但把长度单位写进类型契约

- 现状是三套单位混算（M-13 实测偏差 −4；M-11 显示词法路径自洽而解析路径不自洽）。
  新设计必须**选一套单位并写进文档**（建议：**字节偏移 + 1**，与 `error_catalog.rs:99` 的 `line_text.as_bytes()`
  及 `generate_fix` 的字节坐标一致；兄弟报告 ⑤-2 进一步建议双坐标 `Pos{byte_off, col_scalar, col_utf16}`，两者不冲突——
  **本模块只要求 `SourceLoc` 携带的列单位被显式声明**）。
- 证据：`vitro_lexer/src/lib.rs:419-432`（char 递增）vs `:443-449`（减字节长度）vs `error_catalog.rs:56-63,99`（按字节解释）+ M-13 实测。

### 6.〔沿革保留〕`Stmt::Try`/`CatchClause` 原样保留并标 `reserved-for-csharp`

它是"已铺路"而非技术债（§4-B6、§5-6）。

### 7.〔结构优化〕AST 与"类型大小"的边界收紧

把 `compute_type_size` 从 AST 包移出（放到布局/codegen 包），AST 只留形状查询。
证据：AST 包当前的 `compute_type_size` 需 `struct_defs`/`union_defs`/`class_size_map` 三张外部表（`lib.rs:33-38`）
—— **它已越过"纯 AST"边界**，是布局计算器寄居在 AST 包内；这也是 §4-B2 三副本的成因。

---

## 7. ⑥ 坑清单（现象 → 根因 → 修复 → 新语言下是否复发）

| # | 现象 | 根因（file:line） | 修复 | 新语言下是否复发 / 为什么 |
|---|---|---|---|---|
| P1 | **解析错误落在含非 ASCII 的 token 上 → 列号偏差**（`int main(){ int "中文"; }` 报 13、应 17，M-13 实测） | `vitro_lexer/src/lib.rs:419-432`（`column` 按 **char** 递增）vs `:443-449`（减 `text.len()` = **byte**）；`string.rs:72-74` 用源拼写作 token text | **未修**（20260906:228 登记） | **会复发且更隐蔽**：MoonBit `String` 为 UTF-16 ⇒ 三套长度单位（char / byte / UTF-16 code unit）并存，代理对（emoji）会让 UTF-16 与 Unicode 标量计数再分叉。**须用 §8-S6 把单位钉死并让位置由扫描器记录，而非用文本长度反推** |
| P2 | **`\xHH`/八进制转义产生错误字节**（M-1/M-2 实测：`\xff`→2 字节、`\012`→sizeof 6、`\x4`→sizeof 3） | `vitro_lexer/src/string.rs:42-43`（`u8` → `as char` → 存 Rust `String`）→ `vitro_codegen/src/expr/literal.rs:23`（按 UTF-8 长度分配并整串存入） | **未修**（U1#7 的 `\x` 1~2 位修复只落在 `char_literal`，见 `string.rs:105-109` 注释；**string_literal 未同步**） | **会复发且加重**：MoonBit `String` 为 UTF-16，**不存在"把任意字节塞进 String"的合法路径** ⇒ 只要 `value` 仍是 `String`，本 bug 从"编码侥幸"升级为"编译期无法表达"。**最应被结构性消灭的坑**（载体改 `Bytes`） |
| P3 | **自含 struct 令编译器栈溢出崩溃** | `vitro_ast/src/lib.rs` 的 `compute_type_size` 无环保护 + typeck Pass 1 无自含检查（20260906:118，T-P0-8） | **已修**（`lib.rs:39-43` 注释 + `:42,72,90` 的 `visiting` 集合；诊断移交 typeck E3072）。**M-10 实测生效** | 语义须保留，但崩溃形态不同。**Rust 的 `visiting.insert/remove` 成对语义易在提前 return 路径漏 `remove`**（现有三处分支均成对）⇒ 搬迁时逐条复刻或改不可变 `Set` 传递。验证 §8-S5 |
| P4 | **超大数组维数溢出无检查** | `types.rs:229-245`、`:557-573`、`:593` 三处 `product()`；`lib.rs:68`（`elem_count * elem_size`） | **未修**（U5#4 已登记） | **会复发，语言帮不上**：MoonBit `Int` 溢出语义**未实测**（§8-S1）。**M-9 实测当前形态 = debug panic**（`type_.rs:540`），与 U5#4 描述一致 |
| P5 | **AST 深度导致 typeck / AST Drop 栈溢出** | 递归遍历 + 递归 Drop（U1#8） | **已修**：`depth.rs` 迭代测深 + `vitro_parser/src/lib.rs:103` `MAX_AST_DEPTH=512`，超限 `mem::forget` 防 Drop 溢出 | **部分不复发**：MoonBit 为 GC 语言，**无递归 Drop** ⇒ "Drop 溢出"结构性消失。但**递归遍历溢出仍在**（`depth.rs:3-4` 已证明必须用显式栈），且 `mem::forget` 这个补丁在 MoonBit 下**无对应物也不需要** —— **不要照抄** |
| P6 | **三副本 `compute_type_size`** | §4-B2 | **未修**（第二份是死文件） | **会复发**：MoonBit 包系统（目录即包）不阻止复制粘贴。对策 = 布局计算单包 + `.mbti` 只暴露一个入口 + 包依赖单向检查（§10） |
| P7 | **`ErrorCode` 加码无编译期信号** | §2.4 全表 | **未修**（U7#7 部分登记） | **默认会复发**（`partial_match` 只是警告，§0-4 工具链实测）。唯一对策 = §6-1 |
| P8 | **AST serde 派生无消费者，易被误当格式契约** | §4-B5（**亲读**：22 个 `from_str` 点无一消费 AST） | **未修**（也无人受伤） | **会以另一种形态复发**：MoonBit 有 `ToJson`/`FromJson`（工具链实测 `moon ide doc 'Json'` 确认 `moonbitlang/core/json` 与两个 trait 存在，`type Json` 在 prelude），一旦有人 `derive(ToJson)` 到 AST 上，就产生**第二套、与 Rust serde 形状不同**的 JSON，污染 §9 差分锚点 |

---

## 8. ⑦ MoonBit spike 清单（本模块依赖的语言特性）

> 每项给出最小验证程序与判定标准。工具链已就位（`moon 0.1.20260915` / `moonc v0.10.13+cbb11c36f`），可直接执行。

| # | 依赖特性 | 最小验证程序 | 判定标准 |
|---|---|---|---|
| **S1** | **整数溢出语义**（`Int` 乘法 / `Int32`） | `int_overflow.mbt`：`let a:Int = 100000; let b:Int = 100000; println((a*b).to_string())`；另测 `1/0`、`Int::min_value() / -1`、`(-1).lsl(64)` | 三种结果分别记录：(a) 是否 wrap；(b) 是否 panic/trap；(c) debug 与 `--release` 是否同语义。**判据：若 wrap 且与 Rust release 一致 ⇒ P4 沿用 `checked_mul` 思路；若 trap ⇒ 必须在 AST 层先检查再运算**（对齐 U5#4） |
| **S2** | **大 enum（137 臂）穷尽 match 与"加臂"成本** | `codec.mbt`：脚本生成 137 臂 `ErrorCode`，写 `code_of`/`name_of`/`severity_of`/`lang_of`/`catalog_of` 五个**无 `_` 臂**的穷尽 match；**故意删一条臂**，分别跑 `moon check` 与 `moon check -d` | (a) 不带 `-d`：必须复现 `partial_match` **警告**（已工具链实测）；(b) 带 `-d`：必须**失败**（exit ≠ 0）；(c) 137 臂 match 的编译时间与可读性（记录行数）。**判据：(b) 不失败 ⇒ §6-1 收益不成立，须改用生成脚本 + 校验测试** |
| **S3** | **加臂后的"漏改"可获得性**（诊断码过 JSON 边界） | 承接 S2：断言 `name_of(W1018)` = `"W1018"`（非 `"E1018"`），并 `stringify` | 判据：前缀由**变体名**推出；3 处伪造点在新代码里**剩 1 处**。附带：验证 `@json` 对 137 臂 enum 的**派生**形状是否可控（不可控 ⇒ AST dump 用显式 emitter，见 §9） |
| **S4** | **enum 嵌套变体带 payload 的穷尽 match + 递归 payload** | `ast_shape.mbt`：定义 `Expr` **缩微版**（`Binary(BinaryOp, Expr, Expr, SourceLoc, Type)` / `Literal(Int, SourceLoc, Type)` / 递归 `Type`），写 `expr_depth`（显式栈）与 `type_kind`（穷尽 match），**故意漏一臂** | (a) 递归 enum payload 是否需显式间接（应天然支持，需证实）；(b) 漏臂在 `-d` 下必须失败；(c) **命名域模式的替代方案实测**：位置构造在 9 字段变体（对应 `Expr::MemberCall`）下的可读性 —— Rust 的 `Expr::Call { args, .. }` 剩余模式在 MoonBit 的等价写法是什么（位置通配？payload struct？）。**判据：能用一种可读且不脆弱（加字段不炸全部模式）的形态表达全部 26 变体** |
| **S5** | **递归深度上限与栈行为**（对应 P3/P5） | `deep.mbt`：构造 10 万层嵌套递归 enum，分别用递归与显式栈遍历；另测自含类型下的类型大小计算 | (a) 递归遍历在多少层崩；(b) 显式栈版本是否稳定；(c) **是否存在不可捕获的栈溢出**（若是 ⇒ `MAX_AST_DEPTH` 保险丝仍必需，且数值须**重标定**，不得照抄 512）；(d) 有无线程栈大小配置 |
| **S6** | **UTF-16 `String` 对源码定位的影响**（对应 P1/P2/M-13） | `utf16_loc.mbt`：`let s = "中a"; s.length(); s.at(0); s.get_char(0)`；再测 `Bytes` 版本；最后写"按字节扫描源码"的最小 lexer，用 `int main(){ int "中文"; }` 这一真实形态对比三种列号口径 | (a) 证实 `length()` 与索引单位；(b) `Bytes` 能否承载任意 `0x00-0xFF`（含 `\xff`）—— **P2 根治前提**；(c) 三种口径在**该实测形态**上各报什么列号（Rust 版基准 = 13，语义应为 17）。**判据：能把"列单位"以一行文档钉死；且 `\xff` 无损往返** |
| **S7** | **`Bytes`（不可变）与 `FixedArray[Byte]`（可变）的定位** | `carrier.mbt`：`Bytes` 上尝试写入（应失败）；`FixedArray[Byte]` 上 `blit_from_bytes` / 逐元素写 | 判据：确认 `Bytes` 只读、`FixedArray[Byte]` 可写（工具链实测 `moon ide doc 'FixedArray'` 已见 `FixedArray::blit_from_bytes`）。**注意**：本模块只需"不可变字节串"（StringLiteral）；1MB 程序内存载体属 VM 模块（评估报告 A3），**本模块不替 VM 决策** |
| **S8** | **137 臂代码生成的工程可行性** | 用 Go 脚本从 `error_codes.rs` 生成 `.mbt`（符合仓库脚本语言纪律），生成后跑 `moon check -d` | 判据：生成物 `moon check -d` 干净；重复运行幂等；生成脚本自带"源变则产物变"检测（对齐"生成代码与手写源码混管"的 D 系列纪律） |

---

## 9. ⑧ 等价性验收锚点

**总原则**：本模块是**纯数据结构 + 纯函数**，不产生 stdout ⇒ **不能用 shadow/stdout 差分验收**，必须锚**中间产物**。
与兄弟报告的分工：**lexer 报告 §⑧ 的 L1（预处理前 token 流 TSV）锚词法边界；本文 E1（AST dump JSON）锚语法/类型形状**，两层互补不重叠。
**L1 的列号字段与本文 E1 的 `loc` 字段同源**（都来自 `SourceLoc`），故 M-13 未修前，**两侧锚点都会"一致地错"** —— 必须先用 §9 的 canonicalizer 显式声明列号字段是否参与比对。

| # | 锚点 | Rust 侧现状 | 格式 | 比对工具 | 覆盖 |
|---|---|---|---|---|---|
| **E1** | **AST dump JSON 逐字节 diff**（首要锚点） | **需新增**（AST 只有 serde 派生、无 dump 入口；建议写在测试内，不动生产代码） | serde **默认外部标签**（`types.rs:684` 注释确认"自动生成嵌套 JSON"）：`{"Binary":{"op":"Add","left":{…},"right":{…},"loc":{"line":1,"column":1,"file_id":0},"ty":{"Int":{"is_unsigned":false,"is_const":false}}}}` | **自写 canonicalizer（Go，符合 `scripts/` 语言纪律）**：两侧 dump → 规范化（键排序、转义统一、缩进固定）→ 逐字节 diff。**禁止"结构等价的模糊比较"**（会放走键名/标签差异） | 26 `Expr` × 16 `Stmt` × 17 `Type` 的**形状与字段名**全量；parser 产出的每个节点 |
| **E2** | **`SourceLoc` 逐指令往返** | **已存在**：`vitro_cli export -o bundle.json` 的 `code: Vec<Instruction>`，每条含 `loc`（`vitro_cli.rs:304-316` + `vitro_runtime/src/instruction.rs:5-10`） | `BytecodeLibcExport` JSON（`version:1`、`code_len`/`code`/`func_table`/`func_index`/`globals_init_32|64`/`string_data`/`f64_constants`/`i64_constants`/`globals_size`） | 同 E1 canonicalizer + 逐字段 diff | **`SourceLoc(line,column,file_id)` 在真实编译产物上的全量取值** + 整个 codegen 输出（一个锚点两用） |
| **E3** | **诊断帧 JSON 逐字段 diff** | **已存在**：`vitro_compile_json` / serve `compile`（`session_api.rs:38-70`） | `{code,error_code,severity,line,column,end_line,end_column,message,fix_suggestion,filename}` | 逐字段 diff（**必须先修 M-3/M-5 的 `code` 前缀，否则两侧会"一致地错"**） | 137 个码的触发路径、severity 映射、`fix_suggestion`（依赖 catalog 77 条 + `generate_fix`） |
| **E4** | **`error_catalog` 导出 diff** | **已存在**：`vitro_get_error_catalog_json` / serve `error_catalog`（`session_api.rs:160-163` → `error_catalog.rs:524-550`），按 code 升序 ⇒ 天然可差分 | `{"catalog":[{code,code_str,lang,category,emoji,title,explanation,common_causes[]}]}` | 逐条 diff **+ 覆盖率断言**（`catalog_len()`，`:553`；基线 = M-6 实测 77） | 77 条知识卡 + 段映射回归 |
| **E5** | **`mangle_name` 黄金串** | **已存在**：`ast_unit_test.rs:33-75`（2 例，含 `"prefix_p_a2_3_int"`） | 字符串相等 | MoonBit 白盒测试（同断言搬过去） | A3 mangling 规则逐分支 |
| **E6** | **类型大小 / 深度**（属性型锚点） | **需新增**（`compute_type_size`/`expr_depth`/`stmt_depth` 无出口） | 表格（源码 → 期望 size / depth） | 两侧各跑**穷举表**（`Type` 17 变体 × 嵌套形态；26 `Expr` 变体的深度贡献）逐行 diff | §4-B2 三副本语义、`depth.rs` 的 26+16 臂子节点表 |

**关键要求（否则锚点会假绿）**：

1. **canonicalizer 必须 fail loud** —— 遇未知键/未知枚举标签直接报错，禁止静默丢弃（仓库"禁止静默 default"纪律）。
2. **E1 的 dump 必须在两侧都显式实现**，不得一侧 serde 派生、另一侧 `derive(ToJson)` —— **两种派生形状必然不同**（§7-P8），
   会把"格式差异"伪装成"实现差异"。
3. **用例集必须覆盖非 ASCII 与 `\xHH`/八进制转义**（§7-P1/P2、M-1/M-2/M-13）。当前 `native/tests/**/*.c` 对 `\xHH` 的覆盖是 **0**
   —— 按 [`统一整备路线图.md`](统一整备路线图.md):123（M14）的一般化原则「**与 Clang 的数值类差异零 golden 覆盖即防线失效**」，
   这两类必须强插 E1/E2 用例集；M-13 的形态（`int main(){ int "中文"; }`）应作为**位置锚用例**固化。

---

## 10. ⑨ mooncakes 包切分草案

**建议 3 个包**（非 2 个）。理由：诊断码是**对外协议**（下游按码段消费），AST 是**对内结构**，两者变更节奏与消费者不同；
混在一个包会让 AST 形状改动带来诊断协议包的版本抖动。

```
vitro/source-loc            ← 包 1：定位原语（零依赖）
  ├─ SourceLoc { line, column, file_id }
  ├─ 列单位契约（字节 or UTF-16 码元，S6 后钉死）+ 文档
  └─ 显式 Eq/Ord/Hash（Rust 现状缺，§2.2）

vitro/diag-protocol         ← 包 2：诊断骨架（对外协议，零依赖）
  ├─ ErrorCode（137 臂）
  ├─ code_of / name_of / severity_of / lang_of（全部穷尽 match，§6-1）
  ├─ Severity / SourceLang 枚举
  ├─ ErrorInfo 目录（77 条）+ 覆盖率断言（M-6 基线）
  └─ wire 形状：{code,error_code,severity,line,column,end_line,end_column,message,fix_suggestion,filename}
       ↑ 只此一处产出 "E1018"/"W3053"，消灭 M-3/M-4/M-7 三处伪造

vitro/ast                   ← 包 3：AST + 类型系统（依赖 1、2）
  ├─ Type / TypeKind / Expr / Stmt / decl 全族
  ├─ depth（显式栈迭代）
  ├─ 类型相等（显式函数，§6-2）与类型渲染（单源 to_c_string）
  └─ 不含 compute_type_size（移出，§6-7）
```

**依赖方向**：`source-loc ← diag-protocol ← ast`（严格单向无环）。
与 Rust 现状的差异：现 `SourceLoc` 被 `vitro_ast/src/lib.rs:11` 再导出、又被 `vitro_runtime/src/instruction.rs:3` 再导出、
再被 `native/src/shared/mod.rs:8` 与 `native/src/compiler/ast.rs:16` 再导出 ⇒ 同一类型有 **5 条 import 路径**
（`vitro_shared::SourceLoc` / `vitro_ast::SourceLoc` / `vitro_runtime::instruction::SourceLoc` /
`vitro_native::shared::SourceLoc` / `vitro_native::compiler::ast::SourceLoc`）。**MoonBit 侧只允许一条**。
与兄弟报告的接口：lexer 包产出 token（含位置），**依赖包 1 而不依赖包 3**；`ast` 包消费 lexer/parser 的输出。

**对上发布形态**：

- **包 3（ast）不进 mooncakes 公开面**：它是引擎内部结构，公开会形成"AST 形状 = 契约"的伪承诺
  （Rust 侧的 serde 派生已是该陷阱雏形，§4-B5）。仅在 `.mbti` 公开最小面（`Type` 形状查询 + `Expr`/`Stmt` 定义），
  **不公开任何 JSON 形状**。
- **包 2（diag-protocol）可公开**：它已是下游契约（`docs/spec/` + `vitro_get_error_catalog_json`）。
  公开时把 **137 个码位作为 versioned 常量**（加码只增不改，与 `error_catalog.rs:519` 的"只增不改即可消费"承诺一致）。
- **包 1（source-loc）可公开但价值低**：亦可内联进包 2；**先做 3 包，若发布粒度带来痛苦再合并**（可逆决定）。
- **`vitro_csharp_frontend` 挂点**（[`CSharp前端引入计划.md`](../03-语言子集/CSharp前端引入计划.md):32 要求"依赖只到 `vitro_shared`/`vitro_ast`"）：
  MoonBit 形态下即"只依赖包 1+2+3"，该约束**天然满足**（AST/C++ 扩展信息全在包 3 内：
  `TypeKind::Class/Reference/RValueRef/Auto/TemplateId`、`Expr::{This,MemberCall,New,Delete,Lambda,Move}`、`Stmt::Try` 等）。

---

## 附录 A · 复现方法

```powershell
# 【第一步】产物新鲜度检查（本仓纪律：陈旧二进制会造成假绿）
Get-Item native\target\debug\vitro_cli.exe, native\target\release\vitro_cli.exe | Select LastWriteTime,FullName
Get-ChildItem native\crates,native\src -Recurse -Filter *.rs | Sort LastWriteTime -Desc | Select -First 1

# M-1 / M-2（源码走 stdin，不落文件）
'<source>' | & native\target\debug\vitro_cli.exe run -
# M-3 / M-4（看 "[警告]/[提示] …(E…)"）
'<source>' | & native\target\debug\vitro_cli.exe run -
# M-13（解析错误落在含非 ASCII 的 token 上）
'int main(){ int "中文"; }' | & native\target\debug\vitro_cli.exe compile -
# M-5 / M-6 / M-7（JSON-lines 走 stdin）
'{"id":1,"method":"compile","params":{"filename":"main.c","source":"<src>"}}
{"id":2,"method":"error_catalog"}
{"id":9,"method":"shutdown"}' | & native\target\debug\vitro_cli.exe serve
# M-9 需超时保护（20s 内不返回即判挂死；本次为 panic）
```

## 附录 B · 测量口径与已知偏差

1. **构建配置**：本次为 **debug**（`opt-level=1`，`native/Cargo.toml [profile.dev]`）。M-9 的 panic 是 debug 行为；
   release 侧按 U5#4 登记为"负尺寸倒退帧布局"（静默错值），**本次未测 release**（产物陈旧，见 M-0）。
2. **CLI vs capi/serve 通道**：M-3/M-4/M-13 取自 CLI；M-5~M-7 取自 serve。二者共用 `session_api`，
   但 CLI 打印格式是**第三处独立实现**（`vitro_cli.rs:88-94`），故分别登记。
3. **未做 Clang 对照**：M-1/M-2 的 clang 期望值引自既有登记（20260906:158）与 C 语义，**本次未实跑 clang**；
   若要进 shadow，须补 clang 侧 golden。
4. **M-6 的"枚举有码变体 = 136"**：由 `error_codes.rs` 静态解析（`Unknown = 0` 不计），与实测导出的 77
   在同一次运行内求差得 59，非跨时点拼接。
5. **M-13 的"应 17"**：由同源行逐字符核算得出（该行前 16 字符为纯 ASCII，字节位与字符位一致，故无歧义）；
   未用 clang 复核列号（clang 的诊断列号口径本身也需另行校正）。
6. **未做 git 校验**（只读纪律）：证据读自当前工作区；兄弟报告记录 HEAD = `0b243ca`，与 `facts.json` 的 `4b57191` 有位移。

## 附录 C · 引用来源对照

| 数字/结论 | 来源 | 证据等级 |
|---|---|---|
| 1781 行 / 9 文件 / 137 变体 / 26 Expr / 16 Stmt / 17 Type | §2，逐文件 `ReadAllLines().Length` 与逐行解析 | 实测 |
| `error_catalog` 77 条 / 缺 59 / 覆盖 57% | serve `error_catalog` 导出 vs 枚举定义求差（M-6） | **实测** |
| `\xff` 2 字节；`\012` sizeof=6；`\x4` sizeof=3 | `vitro_cli run -`（M-1/M-2） | **实测** |
| 列号偏差 13 vs 应 17 | `vitro_cli compile -`（M-13） | **实测（独立复现）** |
| W/H 码打 E 前缀（CLI + serve + catalog） | M-3/M-4/M-5/M-7 | **实测** |
| `void*` 步长 0 vs 1 | M-8 | **实测** |
| 巨数组 debug panic @ `type_.rs:540` | M-9 | **实测** |
| 自含 struct → E3072 | M-10 | **实测** |
| 中文标识符列号 18 / 中文字符串 sizeof 7 | M-11/M-12 | **实测** |
| `make_token` 列号单位混血（机制） | `vitro_lexer/src/lib.rs:419-449`、`error_catalog.rs:56-63,99` | **亲读**（现象有实测，机制为亲读） |
| AST serde 无消费者 | 全仓 22 处 `serde_json::from_str` 亲读 | **亲读** |
| `compiler/ast.rs` 为死文件 | 全仓 `mod ast` grep 零命中 + `compiler/mod.rs:4` 亲读 | **亲读** |
| MoonBit `partial_match` 是警告 / `-d` 可升级 | `moon explain --diagnostic E0011`、`moon check --help` | **工具链实测** |
| MoonBit `String` 为 UTF-16 码元索引 | `moon ide doc 'String'`（`at -> UInt16` / `code_units` / `char_length` 另算） | **工具链实测** |
| 全仓真值（用例数等） | `reports/facts.json`（`generated_at`=2026-09-18T13:01:41+08:00，`git.rev`=`4b57191`） | 引用，标 as_of |

## 变更记录

| 日期 | 变更 |
|---|---|
| 2026-09-18 | 建档：`vitro_shared` + `vitro_ast` 模块勘察（九节模板 ①~⑨）+ 实测发现登记 M-0~M-13 |
| 2026-09-18（同日） | **二稿修正**（记录对账后）：① 文件名改为与同批兄弟报告一致的 `MoonBit迁移_<模块>模块勘察报告20260918.md` 形态；② 补 §0.2 记录对账（12 条既有登记 + 三处自我更正）；③ M-13 由"未复现"改为**独立复现的实测缺陷**（列 13 vs 应 17）；④ M-1 重新定位为兄弟报告 ⑥-5 的形态补充、不主张首报；⑤ 新增 §1.3 兄弟报告边界与交叉引用、§9 锚点层次说明（L1 ↔ E1） |
