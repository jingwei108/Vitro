# MoonBit 迁移 · `vitro_lexer` 模块勘察报告

> 勘察时间：2026-09-18
> 勘察对象：`native/crates/vitro_lexer/`（14 个 `.rs`，**3,523 行**）含 E2 模块化预处理器内核
> 性质：**只读勘察**。未修改任何文件、未执行任何 git 写操作。
> 上游依据：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)（§2 事实核查 / §7 四道证伪门 / §8 假设清单 A1–A9）、[`C语言子集规范.md`](../03-语言子集/C语言子集规范.md) §2.11（E2 预处理器规范）
> 数字口径：全局数字一律引用 `reports/facts.json` 的 key 与 `as_of`；模块内数字（文件/行数/测试数）为本次实测。本文件为 as-of 快照，裸数字不参与 `facts` 回填。
> 证据标记：**【实测】**= 本次现场跑命令所得；**【亲读】**= 代码 file:line；**【文档】**= 文档 file:line；**【待证】**= 未验证，附验证方法。
>
> **实测环境（可复现性声明）**
> - 仓库状态：`0b243ca`（已核实 `4b57191..HEAD` 之间 `native/crates/vitro_lexer/` **零差异**，lexer 无未提交改动）
> - 引擎：`native/target/debug/vitro_cli.exe`（2026-09-18 13:01:42 构建，**debug 构建**）
> - 对照编译器：clang 22.1.4（`C:\clang+llvm-22.1.4-x86_64-pc-windows-msvc\bin\clang.exe`，与既有 shadow 对照同版本）
> - MoonBit 工具链：`moon 0.1.20260915 (2e1a46d)` 已安装（`C:\Users\liangjingwei\.moon\bin\moon.exe`），但 `moon run -e` 在本次沙箱下失败（`Error: spawn node.exe ENOENT`）→ MoonBit 语言事实取自官方 skill 的已验证示例，需实测项一律标【待证】
> - 全部实测经 **stdin 管道**完成（`clang -E -x c -` / `vitro_cli compile|run -`），未在仓库内产生任何文件

---

## 0. 一页速览（结论先行）

**三条核心判断：**

1. **头号工程风险 = H-5（wasm 无文件系统）**。`resolver.rs:170-183` 用 `include_str!` 编译期嵌入 14 个存根、`directives.rs:403` 用 `std::fs::read_to_string(...).ok()` 读自定义头文件。MoonBit 单出口是 **wasm-gc**——若不设计宿主 IO 抽象，"自定义 include"这一 E2 核心能力在目标形态下**整体不存在**，且当前实现是**静默降级**（`实测发现登记20260913_性能与头文件.md:199-202`，未修）。
2. **头号架构机会 = 项目自己已裁定的"预处理器独立成 pass + LineMap"**（`代码审阅与修复追踪20260906.md:270`：当前 splice 寄生架构是"`#if` 缺失、行号偏移、递归 include、宏体续行 4 类问题的共同根因"）。Rust 版因 `Vec::splice` 原地插入架构**改不动**；MoonBit 的 `String`/`Bytes` 不可变使这件事从"可选优化"变成**物理上必须做**——这是重写最大的结构性红利。
3. **头号语言风险不是 String 的 UTF-16，而是"坐标口径"**。Rust 版已在"扫描按 `char` × 列号按 `text.len()`（字节）反推 × token 文本被规范化"三者上产出 **3 类实测偏差**（见 ⑥-1/2/3）；MoonBit 的 UTF-16 索引（`s[i]` 给 code unit、`for c in s` 给 `Char`、`.length()` 给 code unit 数）会给同一病根提供**新的发作形态**。spike S1 必须先定契约再写扫描器。

**规模事实**：预处理器（`preprocessor/` 7 文件 **1,786 行**）比词法核心（顶层 7 文件 1,737 行）**更大**——本模块早已不是"lexer 带个预处理"。

---

## ① 模块概览与规模（实测数字）

| 文件 | 行数 | 职责 |
|---|---:|---|
| `src/preprocessor/directives.rs` | 570 | 指令消费骨架（`impl Lexer` 方法），**最大文件** |
| `src/macros.rs` | 521 | 内置宏表（预定义常量宏 + `va_*` 宏展开为 `__vitro_va_*` 调用） |
| `src/lib.rs` | 451 | `Lexer` 状态机 + token 主分发（`next_token`） |
| `src/preprocessor/cond.rs` | 396 | `#if`/`#elif` 求值器（文本级 `__has_include` 替换 + `defined` 提取 + i64 递归下降） |
| `src/number.rs` | 299 | 数值字面量（hex/oct/bin/dec + 分隔符 + 后缀 + 值域分派） |
| `src/preprocessor/expander.rs` | 295 | token 树转录展开器（深度/字节双保险丝 + trace + W1019 + H01 包装） |
| `src/preprocessor/resolver.rs` | 207 | include 解析（include-once + 静态环检测 DFS + 14 个存根白名单） |
| `src/string.rs` | 197 | 字符串/字符字面量与转义 |
| `src/token.rs` | 154 | `TokenType`(116 变体) / `Token` / `LexerError` / `LexerWarning` |
| `src/preprocessor/splice.rs` | 134 | `#` 字符串化 / `##` 拼接 |
| `src/preprocessor/mod.rs` | 100 | 保险丝常量 + `ExpandCtx` + 教学 trace 封顶推送 |
| `src/preprocessor/macro_table.rs` | 84 | 宏表 + W1018 遮蔽诊断 + 预定义宏族 |
| `src/keyword.rs` | 82 | C 关键字 43 条 + C++ 19 条映射 |
| `src/comment.rs` | 33 | `//` 与 `/* */` 跳过 |
| **合计** | **3,523** | 顶层 7 文件 1,737 行 / `preprocessor/` 7 文件 **1,786 行** |

**依赖**：仅 `vitro_shared`（`Cargo.toml:6-7`），是本 workspace 依赖最轻的 crate 之一。

**关键类型**（全部为本模块自持）：

| 类型 | 位置 | 说明 |
|---|---|---|
| `Lexer` | `lib.rs:20-42` | 14 个字段：`chars: Vec<char>` / `pos` / `line` / `column` / `macros: MacroTable` / `conditional_stack` / `include_resolver` / `include_cond_boundary` / `preprocessor_trace` / `macro_body_mode` / `is_cpp_mode` / `errors` / `warnings` |
| `TokenType` | `token.rs:5-126` | **116 个变体**（本次实测计数） |
| `Token` | `token.rs:129-135` | `ty` / `text` / `line` / `column`——**无 span、无 file_id、无 end 位置** |
| `LexerError` / `LexerWarning` | `token.rs:138-144` / `:148-154` | 致命/非致命分流（警告经管线进 diagnostics，severity=1） |
| `MacroDef` | `preprocessor/mod.rs:34-38` | `params: Vec<String>` + `body: Vec<Token>`——**无"函数式宏"标志位**（见 ⑥-9） |
| `ConditionalState` | `preprocessor/mod.rs:44-49` | `active` / `has_else` / `taken`——**不记录开启位置**（见 ⑥-23） |
| `IncludeResolver` | `preprocessor/resolver.rs:27-36` | `processed: HashSet` / `base_path` / `dir_stack` |
| `ExpandCtx` | `preprocessor/mod.rs:75-89` | 借用聚合：macros / errors / warnings / trace / `expr_mode` / `emitted` / 两个熔断标志 |
| `CondOutcome` | `preprocessor/cond.rs:19-24` | `value: bool` + `reason: String` |

**保险丝常量**：展开深度 64（`mod.rs:57`）、展开字节预算 16MB（`mod.rs:65`）、教学 trace 64 条（`mod.rs:68`）+ 单条 2KB（`mod.rs:72`）、include 深度 64（`resolver.rs:20`）、环检测节点 512（`resolver.rs:24`）、**宏表 4,096（硬编码内联于 `macro_table.rs:55`，不在常量区）**。

**消费方**：`native/src/engine/compile_pipeline.rs:412`（单文件）/ `:643`（多文件）为唯一生产入口；`into_warnings()`（`lib.rs:98`）/ `into_expansion_trace()`（`lib.rs:103`）→ `session.compile.preprocessor_trace`（`compile_pipeline.rs:416`）→ serve JSON `"preprocessor_trace"`（`native/src/session_api.rs:68`）。另有 10+ 测试文件直接构造 `Lexer::new`。

### 1.1 token 体系全量清单（116 变体，`token.rs:5-126`）

- **C 关键字 38 类**（`keyword.rs:32-81`，43 条映射含同义拼写）：`Int Void Char If Else While Do For Return Break Continue Struct Union Sizeof Offsetof Alignof Generic Switch Case Default Typedef Enum Unsigned Long Short Signed Const Extern Float Double Volatile Inline Restrict Register Auto Bool Goto Null`
- **C23 锚定**：`alignof`/`_Alignof`（`keyword.rs:72-73`）、`nullptr`→`Null`（`:77`）、`constexpr`→`Const`（`:79`）、`null`/`NULL`→`Null`（`:74-75`）、`bool`/`true`/`false`（`:67` + 内置宏 `macros.rs:225-248`）
- **C++ 关键字 18 + 标点 3**（`keyword.rs:6-29`；`ColonColon`/`ArrowStar`/`DotStar` 于 `lib.rs:348`、`:186-189`、`:329-331`）
- **字面量 7**：`Identifier Number UnsignedLiteral FloatLiteral LongLiteral CharLiteral String`
- **运算符 33 + 分隔符 13 + `Eof`/`Unknown` + `Hash`/`HashHash`**（E2 新增，`token.rs:122-125`）

### 1.2 C23 特性落地核查

| 特性 | 状态 | 证据 |
|---|---|---|
| `0b` 二进制字面量 | ✅ | `number.rs:55-92` |
| `'` 数字分隔符（hex/bin/dec） | ✅ | `number.rs:25,64,139`（分隔符不进值） |
| 科学计数法 `1.5e-3` | ✅ | `number.rs:153-176` |
| 相邻字符串拼接 | ✅ **但在 parser** | `native/crates/vitro_parser/src/expr/primary.rs:135-147`——**lexer 产出两个独立 `String` token**，拼接不是本模块职责 |
| `[[属性]]` 解析忽略 | ✅ **但在 parser** | token 层无特殊处理，`[` 走 `LBracket`（`lib.rs:320-323`） |
| `u8"..."` 前缀 | ✅ 简化 | `lib.rs:385-387`（按普通字符串处理，无 `char8_t`） |

---

## ② 可复用资产清单

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| 1 | **116 变体 token 分类体系** | `token.rs:5-126` | **低** | 纯 enum 数据，可逐条转 MoonBit `enum`；但需借机重新分层（见 ⑤-4） |
| 2 | **C/C++ 关键字表（43+19 条）** | `keyword.rs` | **低** | 纯映射表，建议外置为数据结构而非 `match`（Go 版脚本"资产外置"纪律同样适用） |
| 3 | **内置宏表语义**（14 组 stdio/stdlib/limits/stdbool 常量 + 4 个 `va_*` 函数式宏） | `macros.rs:9-520` | **低** | 语义可完整复用；实现需从 521 行手写 token 构造改为表驱动（`macros.rs:198-223` 已有紧凑形态可循） |
| 4 | **数值字面量的值域分派规则** | `number.rs:231-297` | **中** | 规则本身（`u32→UInt`、`i64→Long`、`(i64::MAX,u64::MAX]→Unsigned`、无后缀十进制不自动 unsigned）是有价值的 C 语义资产；但实现依赖 `u64::from_str_radix` + `i32/u32/i64::try_from`，MoonBit 侧需重写并实测溢出语义（⑦-S2） |
| 5 | **转义序列表** | `string.rs:25-53`（string）、`:93-159`（char） | **中** | 语义可复用，但**两处实现已分叉**（char 侧 U1#7 已修、string 侧未修，见 ⑥-5）——移植时必须单点收口而非照搬两份 |
| 6 | **条件编译求值语义**（`defined` 宏展开前提取、`__has_include` 文本级替换、`&&`/`\|\|` 短路不触发除零、残余标识符→0） | `cond.rs:63-103,148-194` | **中** | 语义符合 C99 §6.10.1；但求值器有 2 个实测缺陷（`\|\|` 不归一、无位运算）必须在目标架构重写而非搬运 |
| 7 | **宏展开的 C99 §6.10.3.1 语义**（实参先行完整展开、体含 `#`/`##` 时用原始实参、自引用停止、蓝漆等价物） | `expander.rs:64-178` | **中** | **本模块最有价值的知识资产**：140 行代码里沉淀了 E2 批踩过的全部展开陷阱 |
| 8 | **include 解析语义**（include-once、14 存根白名单、quote 候选链"包含者目录优先"、静态环检测、深度保险丝） | `resolver.rs:38-187` | **中** | 语义层可完整复用（U1#11 已修至与 Clang 对齐）；实现层依赖 `std::fs` 必须换（见 ③-12） |
| 9 | **教学追踪（展开链 + 分支原因）的数据设计** | `expander.rs:209-233`、`cond.rs:98-102` | **低** | 格式 `"NAME(args) ⇒ 结果拼写"` / `"#if \`expr\` → 真"` 已被出口协议消费，应作为契约保留；建议结构化（⑤-6） |
| 10 | **W1018/W1019 教学警告文案与触发条件** | `macro_table.rs:41-58`、`expander.rs:236-278` | **低** | 文案是教学资产（中文 + 根因 + 建议）；触发判据清晰 |
| 11 | **错误码与诊断语义**（E1001~E1010 词法 / E1011~E1017 预处理 / E1021 include / W1018/W1019） | 全模块 | **低** | 错误码是跨模块契约（parser/typeck 复用同一码表），迁移必须原样保留 |
| 12 | **测试资产：57 个单元测试**（含 8 条 U1#11 红→绿锚、4 条保险丝红锚/反向锚） | `native/tests/lexer_unit_test.rs`（837 行，`#[test]` 实测 57） | **低** | **可直接改写为 MoonBit `*_test.mbt`**；断言形态（token 文本序列 / 错误码 / 警告码 / trace 含子串）语言无关 |
| 13 | **测试资产：baseline 用例**（`e1_c23_*` 7 个、`e2_*` 12 个 `.c` + `e2_has_include_nest/` 目录、`e3_*` 4 个、`parametric_macro_*` 4 个、`include_custom_header.c`） | `native/tests/cases/baseline/` | **低** | Clang golden 可直接复用（评估报告 A8 假设） |
| 14 | **Shadow golden 真值** | `native/tests/shadow_verification/reports/` | **低** | C 侧 675 用例 / 668 match（`reports/facts.json` → `shadow_c_cases` / `shadow_c_match`，as_of `2026-09-15 00:05:33`）；评估报告 A8 已证"golden 不依赖实现语言" |
| 15 | **规范 §2.11 的口径表**（能力矩阵 + 诚实放弃清单 7 条 + 诊断对照） | `C语言子集规范.md:525-568` | **低** | 可直接作为 MoonBit 版规格输入；**但其中 2 处与代码不符**（见 ⑥-13/14） |
| 16 | **`runtime_libc/include/*.h` 14 个存根** | `native/runtime_libc/include/` | **中** | 内容是纯资产可复用；载体 `include_str!`（`resolver.rs:170-183`）是 Rust 编译期嵌入，MoonBit 需另择机制（代码生成 / 包内资源 / 构建期生成） |

**不可复用（虽在②但在源头上就带缺陷，须先修）**：`make_token` 列号计算（③-4）、string 转义（③-7）、`cond.rs` 求值器（③-5/6）、宏表静默上限（③-9）。

---

## ③ 抛弃清单

| # | 抛弃对象 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 1 | **`chars: Vec<char>` + `chars.splice()` 原地插入架构** | `lib.rs:21,59`；`directives.rs:442` | Rust 特有的"可变 `Vec` 原地拼接 + 单一可变扫描指针"设计。MoonBit `String`/`Bytes` **不可变**，`Array[Byte]` 虽可变但语义不同；照搬等于用错语言 | **高**：这是 include 机制的地基，必须整体重设计（⑤-1） |
| 2 | **`#__vitro_push_dir` / `#__vitro_pop_dir` 哨兵指令** | `directives.rs:124-145,431-435` | 把目录栈维护**编码成合成指令注入源码流**——`mod.rs:1-17` 自称"皮肤与内核分离、不做文本变换黑魔法"，此处正是黑魔法本体 | 中：改为显式 include 栈（⑤-1）后自然消失；但需保留"嵌套 include 相对路径按包含者目录解析"的 C 语义 |
| 3 | **`self.line -= inserted_newlines` 行号补偿** | `directives.rs:445` | 症状治疗：用无条件减法对冲 splice 造成的行号漂移，**无下界钳制**（`重构评估报告20260912.md:106` 记为 M5，未修）；且头文件内诊断的行号本就归属错误 | 高：正是项目自裁"独立 pass + LineMap"要根治的 4 类问题之一 |
| 4 | **`column: self.column - text.len() as i32`** | `lib.rs:448` | 字符列号 − **字节长度**，且与 `number.rs`/`string.rs` 用规范化文本作 `token.text` 相互放大。【实测】3 组偏差（见 ⑥-1/2） | 低：重写位置模型时自然消除；但**必须同时固化新口径**，否则诊断锚点漂移 |
| 5 | **`cond.rs` 的 `\|\|` 短路不归一实现** | `cond.rs:218-228` | 【实测】`#if (2 \|\| 0) == 1` → Vitro 选 `#else`、clang 选 `#if`。`parse_or` 在左真短路时 `Ok(left)` 直接返回原值而非 `1`，与 `parse_and`（`:231-241` 归一到 0/1）不对称 | **高（静默错值）**：条件编译分支选错且零诊断 |
| 6 | **`cond.rs` 手写递归下降求值器** | `cond.rs:198-374` | 三重问题：① 不支持位运算 `& \| ^ ~ << >>` 与三目（【实测】全部报 E1014）；② 递归无深度保护（`重构评估报告20260912.md:106` 记 M4，未修）；③ 自造 `parse_c_int`（`:377-396`）与 `number.rs` 双份字面量解析 | 中：`#if FLAGS & MASK` 是**真实教学代码形态**；规范 §2.11 表格未列此限制 |
| 7 | **string 侧转义实现** | `string.rs:36-52` | 【实测】`sizeof("\x4")`=3（clang 2）、`sizeof("A\012B")`=6（clang 4）——**静默产出错误字节**；char 侧已修（`:104-149`）形成两处口径分叉 | **高（静默错值）**；且 `:146` 仍留 `unwrap_or(0)` |
| 8 | **展开后强制改写 token 位置** | `expander.rs:81-85,171-175` | `mt.line = tok.line; mt.column = tok.column;` 把全部展开产物钉在宏调用点——教学上丢失"这行来自宏体第几个 token"的信息 | 低-中：简化了位置一致性；重设计时可选择保留宏体内坐标（诊断精度提升） |
| 9 | **宏表 4,096 静默丢弃** | `macro_table.rs:55-57` | `if self.macros.len() < 4096 { insert }` **无 else 分支**：超限后 `#define` 被静默忽略，后续引用报"未声明标识符"，零诊断指向真因（`重构评估报告20260912.md:106` 记为 [agent] 项，未修） | 中：与"保险丝可触发性义务"（`统一整备路线图.md:246`）直接冲突 |
| 10 | **fail-soft 静默路径**（宏 arity 不匹配 `expander.rs:124-129`；`##` 失败保留左操作数 `splice.rs:101-109`） | 同左 | 宏 arity 不匹配 → 【实测】报 `E3023 未声明的变量 'MAX'` + `E3066`（真因在宏定义处） | 中：应改为"预处理阶段显式诊断 + fail-soft 继续" |
| 11 | **`macro_body_mode` + 子 `Lexer` 重词法** | `directives.rs:561-563` | 宏体用**新建的完整 Lexer** 再跑一遍 `tokenize()`（含内置宏表、错误通道）；且 `let (body_tokens, _)` **丢弃子词法器的全部错误** | 中：这是"多行宏反斜杠静默进宏体"的成因之一（⑥-4） |
| 12 | **宿主文件系统直连**（`std::fs::read_to_string` / `PathBuf::exists` / `canonicalize` / `include_str!`） | `directives.rs:403`；`resolver.rs:52,57,69,145`；`resolver.rs:170-183` | wasm32 下 `std::fs` 不可用且 `.ok()` 吞错 → **H-5**：自定义 include 静默失效（`实测发现登记20260913_性能与头文件.md:199-202`，排期 `统一整备路线图.md:194`，未修） | **高**：MoonBit 单出口是 **wasm-gc**，这是迁移中**必然**面对的头号工程问题（⑤-3） |
| 13 | **`HashMap` 宏表** | `macro_table.rs:16` | 无迭代故当前无害；但顺序不确定性与"确定性重放"的长期纪律相悖 | 低：MoonBit `Map` 保持插入序，迁移后**天然更好** |
| 14 | **预处理诊断 `column: 0`**（11 处 + W1018） | `directives.rs:139,267,278,307,319,334,344,364,381,392,415`；`macro_table.rs:50` | 列定位能力存在但预处理路径全部填 0 | 低：重设计位置模型时一并解决 |
| 15 | **`let _ = call_line;` 死参数** | `splice.rs:132` | 形参 `call_line` 完全未用 | 低：清理 |
| 16 | **`TODO(#D08)` 组织债** | `lib.rs:5` | "未来应将 C++ 专属词法拆分到 lexer/cpp.rs"——所引 `#D08` 台账编号已失效（`工程债务维护方案.md:52` 标完成） | 低：MoonBit 版按包边界一次性划清（⑨） |

---

## ④ 在途工作接纳方案

### 4.1 已修复待搬（**直接按目标架构实现，红锚测试随迁**）

| 工作项 | 内容 | 接纳方案 |
|---|---|---|
| **U1#6**（2026-09-13） | 展开深度保险丝从死代码复活（检查点移入 `expand_inner` 每层入口 `expander.rs:32-49`）；预算口径 token→**字节**（`mod.rs:65`、`expander.rs:181-183`）；trace 单条封顶（`expander.rs:228-231`） | **修复后搬**：语义与常量一并搬运。4 条红锚直接改写为 MoonBit 测试：`lexer_unit_test.rs:514`（5000 层不同名链）、`:535`（4KB×2^14=67MB）、`:552`（反向锚）、`:489`（深度） |
| **U1#7**（2026-09-13） | `08` 消费残段+报错（`number.rs:101-115`）；十进制/八进制溢出报 E1006（`:186-202`、`:118-130`）；`.5` 前导点浮点（`lib.rs:341-343`）；char 侧 hex 转义 1~2 位+超范围诊断（`string.rs:104-149`） | **修复后搬**；**但必须补 string 侧同口径**（③-7）后一起搬，否则把分叉搬进新代码 |
| **U1#11**（2026-09-14） | E1021 定位报错（`directives.rs:405-419`）；`__has_include` 与 `#include` 候选链单源（`resolver.rs:100-114`）；`<>` 只查存根（`directives.rs:370-385`）；跨文件条件栈边界（`directives.rs:124-145,325-348`）；环检测封顶 64/512（`resolver.rs:20,24`）；动态嵌套深度保险丝（`directives.rs:352-368`） | **直接按目标架构实现**：这些语义是 U1#11 用 8 条红锚换来的，**必须原样保留**；但实现形态改为显式 include 栈（不再用哨兵指令）。8 条红锚（`lexer_unit_test.rs:675,697,711,725,743,757,777,794,814`）全部随迁 |

### 4.2 已登记未做 → 逐条接纳

| 工作项 | 登记位置 | 接纳方案 |
|---|---|---|
| **预处理器独立成 pass + LineMap + 启用 `file_id`** | `代码审阅与修复追踪20260906.md:270` | **直接按目标架构实现**——本模块最重要的一条。项目自判它是 4 类问题共同根因；MoonBit 版没有历史包袱，正是唯一窗口。落地形态见 ⑤-1 |
| **宏体 `\` 续行（多行宏）** | `代码审阅与修复追踪20260906.md:228`；`C语言子集规范.md:880` | **直接按目标架构实现**。【实测】clang 编译通过、Vitro 报 3 条 `无法识别的字符: '\'`。独立 Preprocessor pass 天然解决（续行是"行拼接"阶段职责）。**风险**：会改变 `#define` 行号记账，需 LineMap 同步 |
| **`make_token` 列号字节/字符混用** | `代码审阅与修复追踪20260906.md:228` | **直接按目标架构实现**（⑤-2 双坐标），不搬运旧算法 |
| **string 侧 `\x`/八进制转义** | `代码审阅与修复追踪20260906.md:158`、`统一整备路线图.md:117` | **修复后搬**（须先与 char 侧收口为单一实现） |
| **`#include <` 未闭合** | `代码审阅与修复追踪20260906.md:228` | **修复后搬**。【实测】现状：`parse_include_path`（`directives.rs:459-461`）一路 advance 到 EOF，把余下全文当路径名；报 E1021 但**消息内嵌多行源码**（实测消息里出现 `int after_include = 2; int main(){...}`），且后续代码全部丢失。**修正登记表述**："静默吞掉整个文件"→"有诊断但路径解析失控 + 消息污染" |
| **零参数函数式宏 `#define FOO() 42`** | `代码审阅与修复追踪20260906.md:156` | **直接按目标架构实现**。【实测】报 `E3066 不能对非函数类型进行调用`。根因是 `MacroDef.params: Vec<String>` 无法区分"对象宏"与"零参数函数式宏"（`expander.rs:71`）——目标架构改为 `enum MacroDef { Object, Function(params) }` |
| **宏参数数量不匹配** | `代码审阅与修复追踪20260906.md:162` | **直接按目标架构实现**（加显式预处理诊断） |
| **环检测 `visited` 不回溯（菱形依赖漏报）** | `重构评估报告20260912.md:106`（M2） | **直接按目标架构实现**：`resolver.rs:142` 的 `visited.insert` 从不移除，非路径局部访问集。MoonBit 版用不可变 Set 自然写成路径集 |
| **`cond.rs` `/` `%` 未用 checked** | `统一整备路线图.md:119` | **直接按目标架构实现**（MoonBit 整数溢出语义需先实测，⑦-S2） |
| **`cond.rs` 递归深度保护** | `重构评估报告20260912.md:106`（M4） | **直接按目标架构实现**（U1#8 只覆盖 parser 五入口，`#if` 迷你解析器漏网） |
| **H-4 存根硬遮蔽** | `实测发现登记20260913_性能与头文件.md:197-198`（P3，承诺入 spec 差异表但**未落**） | **放弃并记录理由**：教学价值低、`统一整备路线图.md` 未排期。目标架构可顺手改为"自定义 `"stdio.h"` 优先于存根"——但**需先确认不破坏 shadow 用例**（存量语料 655 处 `<>` 全为标准头名，`directives.rs:372` 注释） |
| **H-5 wasm 下自定义 include 静默失效** | `实测发现登记20260913_性能与头文件.md:199-202`；`统一整备路线图.md:194`（U6#4） | **直接按目标架构实现，且升级为 P0**：MoonBit 单出口就是 wasm-gc，若不设计宿主 IO 抽象，"自定义 include"在目标形态下**整体不存在**。方案见 ⑤-3 |
| **`-I` / 系统 include 搜索路径** | `工程债务维护方案.md:947` | **放弃并记录理由**：与"单文件教学场景"不符；`<>` 已收紧为存根白名单（U1#11 H-3） |
| **`#warning` 指令** | `结构重构与C23锚定决议.md:46`（称"已支持"）；代码 `directives.rs:203-206` 落 `_ =>` | **修复后搬**。【实测】含 `#warning "..."` 的源**编译成功且零诊断**——决议记载与代码不符（⑥-12） |
| **`#elifdef` / `#elifndef` / `__VA_OPT__`** | `结构重构与C23锚定决议.md:71-72`（E2 范围声明） | **直接按目标架构实现**（`#elifdef`/`#elifndef` 在 C23 且 clang 22 支持）。【实测】`#elifdef A` 被静默当未知指令跳过 → **选错分支且零诊断**（输出 `PICK_ELSE`，clang 应 `PICK_ELIFDEF`）；`__VA_OPT__` 需先补变参宏支持 |
| **诊断 `end_line/end_column` 退化值** | `CAPI评审回复与实现状态.md:32` | **直接按目标架构实现**：`Token` 现有 `line/column` 已具备 span 条件（`token.rs:129-135`），MoonBit 版直接产出 `(start,end)` |
| **lexer"不动"裁定的反证义务**（随机词法差分 ≥2000 例） | `核心资产重构裁定.md:184`、`:404` | **迁移期承接**：用"Rust 版 vs MoonBit 版 token 流差分"（⑧）**超额完成**该义务——现仅 100 例零分歧，而 token 级全量差分可覆盖每个 baseline 用例的全部 token |

### 4.3 范围外（明确不接纳）

- `native/src/engine/completion/context.rs:96` 的 UTF-8 字节切片（`统一整备路线图.md:194`）——**间接相关**（同"列号语义"缺陷族），不在 lexer 模块；本报告仅登记"目标架构的列号口径统一后该缺陷自然消失"。
- `#embed`（`结构重构与C23锚定决议.md:111-112` 已降级为 ROADMAP 观察项）——随裁定放弃。

---

## ⑤ 架构优化建议（MoonBit 形态）

### 1）〔结构优化〕预处理独立成 pass + 显式 LineMap
**目标形态**：`Lexer` 只做"字符流 → 原始 token 流"；`Preprocessor` 独立消费 token 流（含独立的 directive 行识别），产出"展开后 token 流 + LineMap"。
**为什么现在做**：项目已自裁这是 4 类问题共同根因（`代码审阅与修复追踪20260906.md:270`）；且 MoonBit 无 `Vec::splice` 原地插入，**照搬旧架构物理上做不到**。
**收益**：宏体续行、include 行号归属、`#if` 表达式解析、诊断定位四类问题一次性消解；`SourceLoc.file_id`（现全程写 0，`compile_pipeline.rs:485`）得以启用。
**注意**：LineMap 是"教学诊断可信度"的地基，必须与 ⑧ 的等价性锚点同步固化。

### 2）〔新设计〕双坐标：字节偏移 + Unicode 标量列号
C 源码语义是字节流，但教学编辑器按字符显示列号。建议位置类型携带多坐标：`Pos { byte_off: Int, line: Int, col_scalar: Int, col_utf16: Int }`（按需取二），在 **lexer 出口一次算定**，下游不再换算。
**理由**：现在的缺陷全部来自"扫描按 char、列号按 byte、MoonBit `String` 按 UTF-16"三方混算（`lib.rs:429` vs `:448`）。MoonBit `String` 索引 `s[i]` 返回 **UTF-16 code unit**、`s.get_char(i)` 返回 `Char?`（官方 skill 的 `mbt check` 示例已验证），若沿用"Char 迭代 + 长度反推"，**同类缺陷会以新形态复发**。

### 3）〔新设计〕源码承载与宿主 IO 抽象（单出口前提）
- **源码承载**：`Bytes`（构造一次，不可变）+ `BytesView`（**零拷贝切片**，官方 skill 已验证）承载源码；扫描器持 `BytesView` + 游标。这同时让"输入契约"明确为**字节序列**，把 GBK/BOM 问题从"Rust String 类型系统兜底"变成显式契约（可支持 GBK，也可显式拒绝并给清晰诊断）。
- **宿主 IO**：定义 `trait SourceProvider { fn read(self, path) -> Result[Bytes, SourceError] }`，wasm-gc 形态注入"仅存根/虚拟文件系统"实现，native/Node 形态注入真实文件系统。**关键**：不可用能力必须**显式报错**，不得静默降级（`统一整备路线图.md:194` U6#4 的硬要求）。
- **存根载体**：14 个 `.h` 从 `include_str!`（`resolver.rs:170-183`）改为构建期生成的 MoonBit 数据表或包内资源。

### 4）〔结构优化〕`TokenType` 分层
116 个变体的单一 flat enum 会让每个 `match` 都承担穷尽性负担。建议：

```
enum Tok { Keyword(Kw), Punct(P), Literal(Lit), Ident(String), Eof, Unknown }
```

**收益**：主分发 `match` 按层分流（MoonBit `enum` + 穷尽 `match` 是编译期强制，分层后每层手臂数从 116 降到十余）；`Kw`/`P` 可 derive `Eq` 直接做表查找。

### 5）〔新设计〕宏表与展开栈的函数式形态
MoonBit 无 `Arc`，但**不可变结构天然可共享**（评估报告 §2.1 结论）。把 `MacroTable` 做成不可变持久映射（每次 `#define` 返回新表），展开栈用 `List[String]` 而非 `HashSet`（`expander.rs:20,27`）：
- 天然线程安全、天然支持"快照回滚"（教学时间旅行用得上）；
- 自引用查重语义等价（`List` 查找 O(n)，但展开栈深度 ≤64，成本可忽略）；
- 消除"宏表 4096 静默上限"（③-9）——不可变结构无此必要，若保留则**必须可触发 + 有诊断**。

### 6）〔新设计〕教学 trace 结构化
现状 `Vec[String]`（`lib.rs:41`），元素为拼好的中文串（`expander.rs:221`）。建议改为：

```
enum TraceEntry { Expand { name, args, result, depth }, CondBranch { expr, value, reason } }
```

出口层再渲染成现有中文串（保持 `session_api.rs:68` 的 wire format 不变）。**收益**：教学前端可按需渲染（如只显示顶层展开）、可断言、可 diff；同时天然消除"单条 trace 2KB 截断"这类补丁（`mod.rs:72`）。

### 7）〔沿革保留〕双保险丝 + 红锚制度
深度 64 + 字节预算 16MB 双保险丝（`mod.rs:57,65`）是 U1#6 用 67MB 实测换来的，**必须原样保留常量语义**；「保险丝可触发性义务」（`统一整备路线图.md:246`）在 MoonBit 版继续执行——每条保险丝必须有能证红的测试。

### 8）〔结构优化〕关键字/内置宏/错误码外置为数据
`macros.rs` 521 行手写 token 构造（每个 `MacroDef` 展开 6~11 行）应改为表驱动（`macros.rs:198-223` 已有紧凑形态可循）。理由与本仓库脚本纪律同源："规则、期望值等资产外置，代码只做解释器"（`AGENTS.md` 脚本节）。

---

## ⑥ 坑清单（现象 → 根因 → 修复 → 新语言下是否复发）

| # | 现象 | 根因（file:line） | 修复状态 | MoonBit 下是否复发 | 为什么 |
|---|---|---|---|---|---|
| 1 | 【实测】`int main(){ int "中文"; }` 报列 **13**，应 17（偏差 −4） | `advance()` 按 char 递增列（`lib.rs:425-430`）× `column - text.len()` 按字节（`:448`）；string token 的 `text` 还是源拼写（`string.rs:72-74`） | ❌ 未修（`代码审阅与修复追踪20260906.md:228` 登记） | **会复发且更隐蔽** | MoonBit `String` 是 UTF-16：`s[i]` 给 code unit、`for c in s` 给 `Char`、`.length()` 给 code unit 数——三方任一混用即产生新的"偏移倍数"。且 emoji（代理对）会让 UTF-16 计数与 Unicode 标量计数再度分叉 |
| 2 | 【实测】`0xFFFF` 列报 18（应 17，+1）；`1'000'000` 列报 19（应 17，+2） | `number.rs:238,268` 等用 `val.to_string()`（规范化的十进制值）作 `token.text`，与源拼写长度不等；`lib.rs:448` 反推列号 | ❌ 未修（**本次新发现**，未见登记） | **会复发** | 只要"token 文本 = 规范化值 + 位置由文本长度反推"这两个设计同时存在，与语言无关；MoonBit 版应**位置由扫描器记录而非反推** |
| 3 | 【实测】`#if (2 \|\| 0) == 1` → Vitro 输出 `FALSE`，clang 输出 `TRUE` | `cond.rs:218-228`：`parse_or` 左真短路时 `Ok(left)` 返回原值，未归一到 `0/1`（`parse_and` `:231-241` 做了归一，**两者不对称**） | ❌ 未修（**本次新发现，未见任何登记**） | **会复发**（若照搬求值器） | 语言无关的逻辑错误；但 MoonBit `match` + 枚举化 AST 求值器更易一次性写对 |
| 4 | 【实测】多行宏 `#define SWAP(a,b) do { \ … } while(0)` → clang 编译通过，Vitro 报 3 条 `无法识别的字符: '\'` | `parse_define_directive` body 只读到 `\n`（`directives.rs:554-557`），无续行处理；且第 2 行的 `\` 进了宏体、**子词法器错误被 `let (…, _)` 丢弃**（`:563`） | ❌ 未修（`代码审阅与修复追踪20260906.md:228` 登记） | **会复发**（若照搬"宏体=单行文本"模型） | 独立 Preprocessor pass 的"行拼接"阶段天然解决（⑤-1）；但**必须配红锚**：clang 通过 + 展开结果逐 token 一致 |
| 5 | 【实测】`sizeof("\x4")`=3（clang 2）；`sizeof("A\012B")`=6（clang 4）——**静默错值** | string 侧转义只认"恰好 2 位 hex"否则把 `x` 当普通字符（`string.rs:36-52`）；只认 `\0` 单字符不认八进制（`:35`）；char 侧已修（`:104-149`）→ 两处分叉 | ❌ 部分修（`统一整备路线图.md:117` 记为"顺带词法保真三处"，实只修 char 侧） | **会复发** | 与语言无关；但 MoonBit 的 `Bytes` 不可变意味着"往结果里 push 字节"要换成"构造 `Array[Byte]` 后转 `Bytes`"，**重写时正是收口的机会** |
| 6 | 【实测】`#if (6 & 3) == 2`、`#if (1 << 3) == 8`、`#if (1 ? 2 : 3) == 2`、`#if ~0 == -1` → 全部 E1014 | `cond.rs` 的 `Parser` 只实现 `\|\| && == != < <= > >= + - * / % ! unary± 括号`（`:218-374`），无位运算/三目 | ❌ 未修（规范 §2.11 表格只列支持集，**放弃清单未列此项**） | **会复发**（若照搬求值器） | 教学代码 `#if FLAGS & MASK` 真实存在；建议目标架构用完整 C 常量表达式求值器（可复用规范 §2.12 的 `static_assert` 常量求值器） |
| 7 | 【实测】`#include <` 未闭合 → E1021 消息把余下全文当路径（消息内嵌 2 行源码），后续代码全部丢失 | `parse_include_path` 扫描循环无换行边界（`directives.rs:459-461`），未命中定界符仍返回 `Some(...)`（`:466`） | ❌ 未修（登记 `代码审阅与修复追踪20260906.md:228` 表述为"静默吞掉整个文件"——**本次实测修正**：有诊断，但路径失控 + 消息污染） | **会复发** | 与语言无关；MoonBit 版应在**行内**解析 include 路径（行是预处理的基本单位） |
| 8 | 【实测】UTF-8 BOM → E1001"无法识别的字符: '\u{FEFF}'"（clang 跳过 BOM 正常编译） | `skip_whitespace` 用 `is_ascii_whitespace`（`lib.rs:403`），BOM 不是 ASCII 空白；`Vec<char>` 把它当普通字符 | ❌ 未修（D5 第 ③ 类事故"BOM 干扰 clang 对照"（`核心资产重构裁定.md:606`）的**引擎侧**表现） | **会复发** | MoonBit 侧若按 `Bytes` 承载则 BOM 是 3 个字节、若按 `String` 则是 1 个 code point——**必须先定契约**。建议在"字节预处理"阶段显式剥离 BOM 并记账 |
| 9 | 【实测】`#define FOO() 42` + `FOO()` → E3066"不能对非函数类型进行调用" | `MacroDef` 无函数式标志，`expander.rs:71` 用 `params.is_empty()` 判对象宏 | ❌ 未修（`代码审阅与修复追踪20260906.md:156`） | **会复发**（若照搬 `params: Vec`） | 目标架构用 `enum MacroDef { Object, Function(params) }` 一次性根除 |
| 10 | 【实测】`MAX(1)`（arity 不匹配）→ E3023"未声明的变量 'MAX'" + E3066 | `expander.rs:124-129` 原样保留 token，无诊断 | ❌ 未修（`代码审阅与修复追踪20260906.md:162`） | **会复发**（若照搬 fail-soft） | 应在预处理阶段报"宏参数个数不匹配（期望 2，实得 1）" |
| 11 | 【实测】`#elifdef A` → 输出 `PICK_ELSE`（clang 选 `PICK_ELIFDEF`）——**静默选错分支** | `directives.rs:203-206` 未知指令一律 `skip_to_line_end()`，**且不改变条件栈状态** | ❌ 未修（`结构重构与C23锚定决议.md:71-72` 把它归入 E2 范围，代码 grep 0 命中） | **会复发**（若照搬 `_ =>` 兜底） | 未知指令应**报错或至少警告**；教学引擎在"静默选错分支"上零容忍 |
| 12 | 【实测】`#warning "..."` → 编译成功、零诊断（决议 `结构重构与C23锚定决议.md:46` 记载"✅ 已支持"） | 同 #11 | ❌ 未修（**文档与代码不符**） | **会复发** | 同 #11；另需修文档 |
| 13 | 【文档 vs 代码】规范 §2.11 放弃清单第 7 条称"`#if` 中的字符常量…不支持 `'A'` 求值" | 【实测】`#if 'A' == 65` → `PICK_A`、`#if 'A' == 66` → `PICK_B`——**实际支持**（`string.rs:191-194` 把 `CharLiteral` 的 text 换成数值，`cond.rs:86-89` 直接取 text） | —（**反向漂移**：把已实现写成不支持） | — | 迁移前必须修正规格，否则 MoonBit 版会按错误规格实现（丢掉已有能力） |
| 14 | 【文档 vs 代码】规范 §3.1（`:880`）与 §9（`:932`）把"完整预处理器（`#`/`##`、多行宏、条件宏表达式）"列为**不支持** | E2 已实现 `#`/`##`（`splice.rs`）与条件宏表达式（`cond.rs`）；真不支持的只有**多行宏** | —（漂移，与 U1#11 修的 D-1/D-2 同类但未清） | — | 同上；"多行宏不支持"这一真实限制被裹在错误陈述里，缺独立准确记录 |
| 15 | 栈溢出：2 万行 `//c` 注释（80KB）→ `has overflowed its stack`，`catch_unwind` 不可捕获，IDE 直接崩 | `next_token` 自递归（旧 `lib.rs:122-135`） | ✅ 已修（`lib.rs:112-115` 改 `loop` 重派发 + 代码注释固化；F-P0-1，2026-09-06） | **会复发** | MoonBit 有 GC 但**调用栈深度限制同样存在**；`next_token` 的重派发必须仍是循环——好消息是 MoonBit 循环/尾调用更自然 |
| 16 | 展开深度保险丝是**死代码** → 5000 层不同名对象宏链栈溢出崩溃（另测 5 万层 `ID(ID(…))` → exit 127） | 检查点只在 depth=0 公开入口（`expander.rs:32-35` 注释记载旧实现） | ✅ 已修（U1#6，2026-09-13） | **会复发风险高** | 经典"护栏写了但照不到"：MoonBit 版重写时若只在公开入口检查，同样失效。**唯一防线是红锚测试**（`lexer_unit_test.rs:514`）随迁 |
| 17 | 展开预算口径数 token 不数字节 → 4KB 字面量 ×14 层 = 16384 token（低于 26 万预算）但实际 67MB、**零诊断** | `expander.rs:181-183` 注释记载旧口径 | ✅ 已修（U1#6，改字节计费 16MB） | **会复发风险中** | MoonBit 的 `Bytes`/`String` 长度语义同样可按"个"或"字节"计——**口径必须写进常量注释 + 红锚**（`lexer_unit_test.rs:535`） |
| 18 | trace 条数封顶但**单条**无封顶 → 67MB 展开结果拼成一条 trace | `mod.rs:68` vs `:72` | ✅ 已修（U1#6 补 `TRACE_ENTRY_MAX_CHARS`） | **会复发风险中** | 封顶必须"条数 × 单条"双维度，与语言无关 |
| 19 | 宏表 4096 上限静默丢弃；`key_for` 失败静默放行；include 找不到静默跳过（H-1）；`std::fs` 在 wasm 静默降级（H-5） | `macro_table.rs:55`；`resolver.rs:83-86`；旧 `directives.rs:340`；`directives.rs:403` | ❌ H-1 已修（E1021）；**H-5 / 宏表上限未修** | **H-5 必然复发** | MoonBit 是 **checked error** 语言（官方 skill：错误必须在签名中声明或就地处理）——**这是迁移的技术收益**：`Result`/`raise` 强制显式，`.ok()` 式静默吞错写不出来。但前提是**不把旧代码的 `.ok()` 逐字译成 `catch { _ => None }`** |
| 20 | 【文档】跨 crate 不变量："预处理器 splice 后必须 `self.line -= inserted_newlines`"，不在任何单一文件可读，违反即行号系统性错位 | `reports/项目规模与难度评估_2026-09-14.md:154`（列为不变量第 4 条） | —（不变量登记） | **不该复发** | 独立 pass + LineMap（⑤-1）把这个隐式不变量变成**类型化的映射结构**——迁移的第二个技术收益 |
| 21 | 条件栈跨 include 边界污染（头内多余 `#endif` 弹掉包含者条件组；头内未闭合 `#if` 吞掉后续代码） | `directives.rs:124-145`（U1#11 修复注释） | ✅ 已修（U1#11，2026-09-14，`include_cond_boundary` 记账） | **会复发**（若照搬"单线性扫描 + 全局条件栈"） | 显式 include 栈 + pass 边界让"条件组必须在本文件闭合"成为结构性约束而非补丁 |
| 22 | `#include <local.h>` 同目录加载成功而 clang 拒绝（"Vitro 能编、Clang 编不过"——**破坏 golden 前提**） | 旧实现 `<>` 与 `""` 共用候选链 | ✅ 已修（U1#11 H-3 收紧为存根白名单；存量 655 处 `<>` 零迁移） | **不该复发** | 目标是"与 Clang 行为一致"，这条纪律必须在 MoonBit 版单源化 |
| 23 | 同文件内未闭合条件组，E1013 报在 **EOF 行**而非未闭合的 `#if` 行 | `tokenize()` 收尾时条件栈非空则用当前 `self.line` 报错（`lib.rs:85-92`）；`ConditionalState`（`mod.rs:44-49`）**不记录开启行号** | ⚠️ 部分修：跨文件边界已由 `include_cond_boundary` 处理（`directives.rs:134`），同文件内仍报 EOF 行 | **会复发**（若照搬三字段状态） | 目标架构应在 `ConditionalState` 里存 `open_pos: Pos`——与 ⑤-2 的位置模型是同一件事 |
| 24 | `short` / 单 `long` 类型修饰符**静默丢弃**；`null`/`bool`/`NULL` 被关键字化 | `keyword.rs:56-58,66-67,74-75`（关键字表）→ 语义丢弃发生在 parser 类型修饰符合成 | ❌ 未修（`代码审阅与修复追踪20260906.md:228` P2 清单） | **不因换语言而改变** | **归属裁定**：parser/typeck 的语义债，lexer 只是"关键字化"提供方。**迁移时应在 parser 勘察中处理**；lexer 侧需配合决策"保留独立 token 还是折叠"——现状是保留 token 但下游丢弃，属半成品契约 |
| 25 | 前端对畸形源码**无覆盖引导式 fuzz**（Fuzz 防线只模糊 VM host 函数） | `reports/Cide引擎多维度评估报告_2026-09-13.md:100,126,165`（建议第 9 条，未修） | ❌ 未修 | **会复发** | 已完成的三次 lexer 大规模止血（F-P0-1 栈溢出、U1#6 双保险丝、U1#7 词法保真）**全部来自人工构造用例**——没有 fuzz 就没有新形状输入。**建议**：⑦-S1/S2 的 spike 扩成"随机字节序列 × 1 万例不崩不挂"的语料生成器（用 `.mbtx` 脚本写，符合仓库"脚本用目标语言"纪律） |

---

## ⑦ MoonBit spike 清单

**前置说明**：本次勘察**未能执行** MoonBit 运行（`moon run -e` → `Error: spawn node.exe ENOENT`，疑似沙箱/运行时限制）。以下 spike 均为**设计 + 判定标准**；语言事实凡带"官方 skill 已验证"标记者取自 `moonbit-orientation` / `moonbit-agent-guide` 技能的 `mbt check` 示例，其余标【待证】。已安装工具链：`moon 0.1.20260915`。

| # | 依赖的语言特性 | 最小验证程序 | 判定标准 | 失败后果 |
|---|---|---|---|---|
| **S1** ⭐ | `Bytes`（不可变）× `BytesView`（零拷贝）× `Array[Byte]`（可变）；`String`(UTF-16) 与 `Char` 迭代 | 构造 `let src : Bytes = b"int x; // 中文注释\n"`；用 `BytesView` 逐字节扫描产出 3 个 token（`int`/`x`/`;`），每个 token 记录**字节偏移**；再对 `"中文"` 字面量源计算列号 | ① 中文注释/字符串字面量源码全部 token 正确；② 字节偏移无损（可还原源）；③ 行列号与"字节偏移→行列"换算表一致；④ 同一源分别用 `Bytes` 与 `String` 入口跑，token 流**必须一致**（否则契约分叉） | **单出口方案不成立**：无法承载 C 的字节流语义 → 回退到"仅支持 ASCII 源码"的受限形态 |
| **S2** ⭐ | 整数类型（`Int`/`UInt`/`Int64`/`UInt64`/`Byte`）与**算术溢出语义** | 逐条断言：`(2147483647 : Int) + 1`、`(0 : Int) - 1`、`(-9223372036854775808 : Int64) / -1`、`(1 : Int) << 31`、`UInt64` 的 `18446744073709551615` 字面量、`u64::from_str_radix` 等价 API 的存在性与越界行为 | 每条明确落到"环回 / panic / 类型错误 / 大整数"之一，并记录在案 | **`number.rs` 的值域分派无法移植**（③-5 依赖 `u32::try_from`/`i64::try_from` 的精确语义）；`cond.rs` 的 `wrapping_*` 语义待定 |
| **S3** | `enum` + 穷尽 `match`；116 变体的工程性 | 把 `TokenType` 全量 116 变体转成 MoonBit `enum`，写一个 `fn describe(t : TokenType) -> String` 用 `match` 覆盖全部；再故意删一个分支看编译错误 | ① 编译器**确实**报非穷尽；② 补一行新变体后，全部 `match` 点都被编译器点出（迁移核心收益）；③ 编译耗时与代码体积可接受 | 116 臂 `match` 维护成本过高 → 需先做 ⑤-4 的分层设计 |
| **S4** | 不可变结构共享（无 `Arc`）；`Map` 插入序 | 用不可变 `Map[String, MacroDef]` 实现 `define/undefine/get`；模拟 `#define` ×5000 后取表；再对展开栈用 `List[String]` 实现自引用查重 | ① `#define` 5000 次后内存/耗时线性可接受；② 展开栈查重语义与 `HashSet` 等价（跑 `#define A A`、`MAX(MAX(1,5),3)`、5000 层链三个用例）；③ `Map` 迭代顺序确定 | 宏表退化为可变 `Map` + 手动管理 → 教学快照/回滚能力丧失 |
| **S5** ⭐ | checked error（`raise` / `Result`）；宿主能力抽象（wasm-gc 无 fs） | 定义 `trait SourceProvider`，两个实现：① 存根表（内存）；② 真实文件系统（native target）。在 **wasm-gc target** 下构建只带 ①的实现，尝试 `#include "local.h"` | ① wasm-gc 下构建通过且自定义 include **给出显式诊断**（"当前宿主不支持文件系统 include"），**不得静默失效**；② native target 下同一用例正常；③ 错误类型可区分"文件不存在"与"宿主不支持" | H-5 在目标形态下复现且无法修复 → 与"单出口"目标直接冲突 |
| **S6** | 扫描吞吐 / 分配行为 | 对 ≥100KB 源码（含中文注释、数百宏定义、include 链）做完整 tokenize ×100 次计时；与 Rust 版同口径对比 | 慢 **3× 以内**可接受（对齐评估报告门 1 口径）；若慢 >10× 需重评"逐字节扫描 vs 批量切分"策略 | 教学大型用例体验受损（非致命，可优化） |
| **S7** | 诊断收集（不中断的多错误） | 一个含 10 处错误的源（未知字符/未闭合字符串/未闭合注释/宏 arity 错/`#endif` 多余/…）走完整 lexer | 10 条诊断**全部收集**（不是遇错即返），顺序确定，每条带行/列/码 | Rust 版"收集到 Vec 后统一返回"契约无法保持 → 错误恢复策略需重设计 |
| **S8** | 测试形态（黑盒/白盒 + 快照） | 把 `lexer_unit_test.rs` 中 10 个代表用例改写为 `*_test.mbt`；token 流用 `debug_inspect` 做快照 | ① `moon test --update` 能生成快照；② 快照 diff 可读（含 ty/text/line/column）；③ 57 个用例全部可迁 | 测试资产迁移成本上升（影响 ②-#12 的"低"评级） |

**S1 / S2 / S5 是门禁级**：任一条失败，本模块的 MoonBit 重写方案需重新裁定。

---

## ⑧ 等价性验收锚点

### 8.1 锚点分层

| 层 | 产物 | 格式 | 比对工具 | 说明 |
|---|---|---|---|---|
| **L1 原始 token 流**（最强） | `Lexer::tokenize()` 的**预处理前**输出：`(TokenType, text, line, column)` 序列 | 每行一条 TSV：`<index>\t<Ty>\t<text 转义>\t<line>\t<col>`；末尾写 `count=<n>` | Rust 侧加 `--dump-tokens-raw` 出口，MoonBit 侧对等出口；`diff` 逐字节比较 | **必须在 Rust 版仍存活时建立**（评估报告阶段 1 第 3 条）。text 需转义（`\n`/`\t`/`\\`）避免歧义 |
| **L2 展开后 token 流**（教学语义主体） | `expand_macros` 之后的 token 序列 | 同 L1 | 同 L1 | 覆盖宏展开、`#`/`##`、`do{}while(0)` 包装 |
| **L3 诊断与教学层** | errors / warnings（code,line,column,message）/ `preprocessor_trace` | JSON 数组（顺序敏感） | 结构化 diff（逐字段） | **中文文案是教学资产**，必须逐字一致；trace 是 serve 协议字段（`session_api.rs:68`） |
| **L4 端到端** | Clang golden（`.out`） | 现有 shadow 驱动格式 | `go run ./scripts/shadow_verify` | 复用 675 用例（`reports/facts.json` → `shadow_c_cases`，as_of `2026-09-15 00:05:33`）与 359 个 baseline（`c_e2e_baseline_cases`，as_of `2026-09-18T13:01:41+08:00`） |

### 8.2 必须先固化的 3 个口径（否则 diff 全是噪声）

1. **`token.text` 是规范化后的值，不是源拼写**：整数是十进制值（`number.rs:238` 等 `val.to_string()`）、字符串是解码值（`string.rs:74`）、字符是数值（`:193`）。锚点必须**显式固化这个口径**——重写时若改成保留源拼写，L1 diff 会全红而语义其实等价。
2. **列号口径**：现状 = Unicode 标量（含上述两个已知偏移）。新设计若改双坐标（⑤-2），**L1 必须同时输出旧口径与新口径**，或在锚点中显式声明"列号字段不参与比对、只比对字节偏移"。
3. **宏展开产物的位置**：现状全部被改写为宏调用点（`expander.rs:81-85`）。新设计若保留宏体内坐标，L2 的 line/column 列必须**可配置关闭**。

### 8.3 弱断言风险（硬约束）

`native/tests/typeck_u1_p0_regression.rs:143,145,152` 出现 `let _ = &parse_errors;` 形态——先跑 `Lexer::new(src).tokenize()`，再把 tokens 交给 `Parser::new(tokens).parse()`，而**词法错误从未与解析结果合并断言**。后果：`Lexer` 返回的 `Err` 被丢弃后，该用例对词法层失败完全不敏感（"词法静默错值 + 语法恰好通过"可长期不被发现）。

**迁移硬约束**：
1. MoonBit 侧 `tokenize` 的返回类型**必须同时携带 token 流与诊断**（建议 `struct LexResult { tokens, errors, warnings, trace }` 单值返回），且 **errors 非空时锚点强制红**——不允许"只看 tokens 不看 errors"的调用形态存在；
2. 对账驱动自身遵「保险丝可触发性义务」（`统一整备路线图.md:246`）：**先证会红**——人为注入一个 token 差异，确认驱动报红，再跑真对账。

### 8.4 执行方式

- **语料**：`native/tests/cases/baseline/` 全量（含 E1/E2/E3/parametric_macro/include 相关用例）+ `e2_has_include_nest/` 目录 + K&R 81 例（真实代码词法形态，`reports/facts.json` → `c_e2e_knr_cases`）。
- **工具链**：Rust 版与 MoonBit 版各产出一份 L1/L2/L3 快照 → Go 驱动比对（遵仓库"脚本默认 Go"纪律），**fail loud**：任一层 diff 非空即红，并打印首条差异的完整上下文。
- **反证义务承接**：本方案可**超额完成** `核心资产重构裁定.md:184` 要求的"随机词法差分 ≥2000 例"（现仅 100 例零分歧）。

---

## ⑨ mooncakes 包切分草案

### 9.1 包划分（4 包）

```
vitro_lexer/                     # facade 包：对外唯一入口
  moon.pkg                       # 无额外依赖
  lexer.mbt                      # pub fn tokenize(source : SourceInput, opts~) -> LexResult
  types.mbt                      # pub(all) enum TokenType / Token / Pos  ← 公共类型归属 facade
  errors.mbt                     # pub(all) suberror LexError { ... } / LexWarning
  *_test.mbt                     # 黑盒测试（57 用例迁移主战场）

internal/source/                 # 源码承载与坐标（不对外暴露类型）
  moon.pkg
  bytes_source.mbt               # Bytes + BytesView 扫描器、行/列/字节偏移三坐标换算
  line_map.mbt                   # LineMap（include 拼接后的行号映射）

internal/pp/                     # 预处理器（独立 pass）
  moon.pkg                       # 内部依赖 internal/source
  directives.mbt                 # 行拼接（含 \ 续行）→ 指令识别
  macro_table.mbt                # 不可变宏表 + W1018
  expander.mbt                   # 展开 + 双保险丝 + W1019 + H01 包装
  cond.mbt                       # #if 求值器（**完整** C 常量表达式）
  resolver.mbt                   # include 栈 + include-once + 环检测
  splice.mbt                     # # / ##
  stubs.mbt                      # 14 个标准库存根（构建期生成）

internal/host/                   # 宿主能力抽象（单出口的关键）
  moon.pkg
  source_provider.mbt            # pub trait SourceProvider（fs 实现只在 native 包）
  wasm_stub.mbt                  # wasm-gc 下的显式"不支持"实现（报错，不静默）
```

**依赖方向（单向，无环）**：

```
vitro_lexer (facade)
   ├─> internal/pp ──> internal/source
   │                      ↑
   └─> internal/host ─────┘   (SourceProvider 注入 pp 的 resolver)
```

**类型归属纪律**（依据官方 skill 的包组织指南）：`TokenType`/`Token`/`Pos`/错误类型是**用户会构造和模式匹配的公共类型**，必须定义在 **facade 包**（或 facade 转出的非 internal 公共包）——**不得放在 `internal/*` 再 `pub using` 出来**（技能文档明确：external 用户对 internal 包的类型不做隐式方法归属加载，`x.method()` 会失败）。

### 9.2 与测试约定的映射

| Rust 现状 | MoonBit 形态 |
|---|---|
| `native/tests/lexer_unit_test.rs`（57 用例，黑盒调 `vitro_lexer::Lexer`） | `vitro_lexer/*_test.mbt`（黑盒，只走 public API）——**首选** |
| 需要触碰内部状态（如 `macro_table` 直测） | `vitro_lexer/*_wbtest.mbt`（白盒）——**应尽量避免**，改为通过公共 API 断言 |
| `native/tests/cases/baseline/e2_*.c`（端到端） | 保持为**语言无关语料**，由 shadow 驱动消费，不进 MoonBit 包 |

### 9.3 对上发布形态

- **模块名**：`vitro/lexer`（`moon.mod` 位置取决于与 Rust workspace 的目录共存策略——**待定，需与构建方案一并裁定**）。
- **对外接口**：`moon info` 生成的 `pkg.generated.mbti` 纳入版本控制，作为公共 API 变更信号（官方 skill 明确要求）。**规则：只允许新增，不允许破坏性修改**——`TokenType` 会被 parser 包消费，`preprocessor_trace` 的字符串格式会被 serve 出口消费（`session_api.rs:68`）。
- **发布粒度**：建议**只发布 facade 包** `vitro/lexer`，`internal/*` 随模块分发但不承诺 API（与 Cargo 的 `pub(crate)` 语义对齐）。
- **消费者**：`vitro/parser`（消费 token 流）、`vitro/diagnostics`（消费 warnings/errors）。**迁移期建议先冻结 `TokenType` 的 116 变体清单**（现状已在 `token.rs:5-126` 冻结），避免 parser 侧同步抖动。

---

## 附录 A · 核验说明（专项搜集 vs 本次实测）

对两条并行专项搜集（事故史 / 在途工作）提交的条目，逐条做"可实证者必实测"的核验：

**实测确认（4 条）**：零参数函数式宏 `#define FOO() 42` → E3066 ✓；宏 arity 不匹配 → E3023+E3066 ✓；`#warning` → 编译成功零诊断 ✓；`#elifdef` → 静默选错分支（`PICK_ELSE`）✓。

**实测修正（1 条）**：`#include <` 未闭合。专项记为"未修（至少会报 E1021，但'吞文件'行为仍在）"——**实测更严重**：E1021 消息把余下全文当作路径名并**内嵌进诊断文本**（多行污染），且后续 `after_include` / `main` **全部丢失**（编译失败）。已在 ⑥-7 精确表述。

**未复核（如实标注）**：`cond.rs` 递归深度崩溃、`i64::MIN / -1` debug panic、include 行号补偿负行号、环检测菱形漏报——均为**代码亲读可得、但未构造实测**，一律标【待证】并附验证方法（多数需要写临时头文件目录，本次勘察严守只读纪律未做）。

**本次独立新发现（3 条，此前未见任何登记）**：⑥-2 number token 列号偏移、⑥-3 `#if` 的 `\|\|` 短路不归一、⑥-6 `#if` 不支持位运算/三目。

---

## 附录 B · 明确"未命中"的检索面（避免下游重复劳动）

- `native/tests/` 下 13 份 `*FAILURES.md`（CPP / E2E / KR / LEETCODE / BYTECODE_LIBC / DIFFERENTIAL / FUZZ / HOST_CONTRACT / CORE_ASSET_VERDICT / DOGFOODING 等）与 `TEST_REPORT.md`：对 lexer / 词法 / 预处理 / 宏 / include / 编码 **零命中**。
- `native/tests/E2E_FAILURES.md`：零命中。
- `docs/current/03-语言子集/C++拓展实施计划.md:1530`：仅错误码归属行，无事故内容。
- `reports/` 中除已引用者外（`three_tier_report.md`、`doc_fact_drift.md`、`facts.json`、`unified_perf_baseline.md` 等）对 lexer 缺陷零命中。
- `MoonBit迁移方案评估报告20260918.md`：**无 lexer 专项缺陷清单**（仅 `:298` 给出 `shared → ast → lexer → parser → …` 逐 crate 平移顺序）——本报告即该迁移的 lexer 侧坑清单输入。

---

## 附录 C · 诚实记录（防线哲学第 0 条）

1. **本报告新发现的 3 个缺陷，此前未见任何登记**：`#if` 的 `\|\|` 短路不归一（⑥-3）、`#if` 不支持位运算/三目（⑥-6）、number token 列号因规范化文本而偏移（⑥-2）。三条均有实测证据。
2. **修正了 2 条既有登记的表述**：`#include <` 未闭合不是"静默吞文件"而是"有诊断但路径解析失控 + 消息污染"（⑥-7）；string 转义的"顺带修复"实际只覆盖了 char 侧（⑥-5）。
3. **发现 2 处文档与代码反向漂移**：规范称 `#if` 不支持字符常量（**实际支持**）、称完整预处理器不支持（`#`/`##` 与条件宏表达式**已实现**，只有多行宏真不支持）。
4. **MoonBit 侧未能实测**：`moon run -e` 在本次沙箱下 `spawn node.exe ENOENT`。所有 MoonBit 语言事实取自官方 skill 的 `mbt check` 已验证示例（`String` 为 UTF-16、`s[i]` 返回 code unit、`Bytes` 不可变、`BytesView` 零拷贝、checked error、`moon.mod`/`moon.pkg` 非 JSON），需实测项已在 ⑦ 明确标注并给出验证程序。
5. **本次勘察未修改任何文件、未执行 git 写操作**；所有实测经 stdin 管道完成，MoonBit 探测在系统临时目录进行且未产生项目内文件。
6. **实测结论的适用范围已核实**：当前 HEAD `0b243ca` 与 `facts.json` 记录的 `4b57191` 之间，`native/crates/vitro_lexer/` **零差异**（`git diff --stat 4b57191..HEAD -- native/crates/vitro_lexer/` 无输出），且 lexer 无未提交改动——本报告全部实测结论适用于当前 HEAD。实测二进制为 **debug 构建**（`native/target/debug/vitro_cli.exe`，2026-09-18 13:01:42）；实测涉及的代码路径（`wrapping_*` 显式使用、字面量解析、转义处理）无隐式整数溢出，故 debug/release 差异不影响结论。
