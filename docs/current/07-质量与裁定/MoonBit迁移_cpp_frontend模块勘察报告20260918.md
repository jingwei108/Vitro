# MoonBit 迁移 · `vitro_cpp_frontend` 模块勘察报告（2026-09-18）

> **性质**：只读勘察，**未执行任何改动**（未修改文件、未执行 git 操作、未运行会写缓存的 `shadow_verify*`）。
> **勘察对象**：`native/crates/vitro_cpp_frontend/` 全部源码 + 其所承载的 C++ 子集支持面（Phase 31~42）。
> **上游依据**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md) §2 事实核查 / §3.2（C++ 子集"搬或砍"须单独裁定）/ §7 四道证伪门 / §8 假设清单 A1~A9。
> **同批兄弟报告**：`MoonBit迁移_lexer模块勘察报告20260918.md` / `MoonBit迁移_shared与ast模块勘察报告20260918.md` / `MoonBit迁移_codegen模块勘察报告20260918.md`。
> **证据纪律**：每条结论附 `file:line`；只读代码下结论，不从文档倒推代码；全局数字引 `reports/facts.json` 的 key + `as_of`；不确定项标"待证"并给验证方法；不引用 `docs/archive/`。
> **测量方法学更正**：`(Get-Content | Measure-Object -Line).Lines` **不统计空行**，本报告全部行数改用 `(Get-Content).Count`（含空行）并已复测。引用他人文档的旧口径数字时保留原值并标注。

---

## 0. 先修正任务书的四处前提（诚实记录优先于结构）

按防线哲学第 0 条，先记录勘察中发现的、与任务书/上游报告前提不符的事实。这四条直接影响裁定材料怎么读。

| # | 任务书前提 | 实测事实 | 证据 |
|---|---|---|---|
| 0.1 | 勘察对象 `native/crates/vitro_cpp_frontend/` 是"C++ 前端"，C++ 占 11 个 Phase | **该 crate 只有 186 行 Rust + 502 行 JSON，实为"内置容器布局加载器"，不是 C++ 前端。** 真正的 C++ 支持面寄生在 lexer/parser/typeck/codegen 的共享文件里；本 crate 内**无任何** `is_cpp_mode`、无词法、无语法、无类型检查 | `native/crates/vitro_cpp_frontend/src/lib.rs:5-6`（仅两个 `pub mod`）、`builtin_layout.rs:1-179`、`type_map.rs:1`（单行 re-export） |
| 0.2 | "U3#2（类类型模板实参）刚修完" | **U3#2 是"变参 double/long long 实参的 8 字节位模式中转"，与类类型模板实参无关。** 类类型模板实参由 **U3#5**（类实例化与函数实例化同收敛 drain）与 **U3#8**（容器重复实例化查重前置）承载 | `CHANGELOG.md:62`（U3#2 标题）、`native/tests/bytecode_gen_unit_test.rs:142`（"U3#2 红锚"）、`vitro_codegen/src/expr/call.rs:178`（"U3#2：8 字节位模式中转"）；对照 `cpp_monomorph/builtin.rs:23`（U3#8）、`cpp_monomorph/replace.rs:9`（U3#8）、`vitro_typeck/src/lib.rs:381`（U3#5） |
| 0.3 | "shadow_cpp 99 用例中 4 例 `clang_compile_fail` —— golden 缺失" | **4 例 `clang_compile_fail` 的 golden 存在**（内容与 Vitro stdout 逐字节相同，`统一整备路线图.md:816` 自述为"手写 golden"）。**真正缺 golden 的是另外 4 例**：`cpp_copy_ctor` / `cpp_default_args` / `cpp_nested_class_instance` / `cpp_nttp_class` | `.cpp` 83 个 vs `.out` 79 个，差集实测；`cpp_shadow_report.json` 中 4 例 `clang_compile_fail` 的 `.out` 均在 |
| 0.4 | "CPP_FAILURES.md 历史累计约 40 条" | **机械复算不可复现 40**：去表头数据行 23 / 加 `###` 条目 32 / 含表头全表格行 33 / 加 bullet 55。逐条人工归类为 **20 条**（历史 3 + 偏差 1 + dogfooding 1 + M6 表 10 + 待观察 3 + lambda 2） | `native/tests/CPP_FAILURES.md` 全文 146 行；"40"源自 `reports/项目规模与难度评估_2026-09-14.md:195` 的表格行"CPP_FAILURES.md \| 40"，其计数规则无文档（**待证**） |

### 0.5 另外一处必须点明的防线缺口

`native/tests/vitro_e2e.rs:186` 用 `if let Some(golden) = load_golden(...)` 且**无 else 分支**——golden 缺失时整个比对块被跳过，该用例**只校验"退出码为 0"**。上述 0.3 中缺 golden 的 4 例因此在 Rust E2E 下对输出漂移完全免疫；其期望输出仅散见于 CHANGELOG 散文（`CHANGELOG.md:1853` 记 `cpp_nttp_class` 输出 5、`:1858` 记 `cpp_copy_ctor` 输出 `5 5 5`/`5 10 5`），而 `cpp_nested_class_instance` 与 `cpp_default_args` **无任何期望输出记载**。

> 另需澄清一处上游报告的读写口径冲突：评估报告 §3.2 要求 C++ 归宿"**必须在阶段 0 之前拍板**"（`MoonBit迁移方案评估报告20260918.md:181`），而同一报告 §7 把 C++ 放进"**阶段 2**：单独裁定，不默认跟随"（`:303-306`）。两者自相矛盾，裁定排期需先消解。

---

## 1. 模块概览与规模（实测数字）

### 1.1 勘察对象本体

| 文件 | 行数（含空行） | 关键类型 / 内容 |
|---|---|---|
| `src/lib.rs` | 6 | `pub mod builtin_layout; pub mod type_map;`；头注自陈"从 `vitro_native::compiler::cpp_frontend` 拆分而来"（`lib.rs:3`） |
| `src/type_map.rs` | 1 | 纯 re-export：`pub use crate::builtin_layout::{cpp_type_to_vitro, is_builtin_container, map_container_method};` |
| `src/builtin_layout.rs` | 179 | `ClassLayout{size,fields,methods}`（`:45-49`）、`MethodSig{name,params,ret,is_virtual}`（`:51-57`）、JSON schema 结构（`:10-38`）、`LazyLock` 加载（`:96-102`）、公共 API 6 个 |
| `src/builtin_layout_data.json` | 502 | `version:2`、`generated_at:2026-06-13T07:02:27Z`、5 个类、`method_map` 5×10/11 项 |
| `Cargo.toml` | 9 | 依赖仅 `vitro_ast` + `serde` + `serde_json` |
| **Rust 合计** | **186** | |

**关键接口清单**（全部是只读查表，无状态：）
`builtin_class_layout(&str) -> Option<ClassLayout>`（`:113`）、`builtin_method_sig(class, method)`（`:118`）、`builtin_class_mappings() -> Vec<(&str,&str)>`（`:160`）、`cpp_type_to_vitro(&str) -> Option<&'static str>`（`:166`）、`is_builtin_container(&str) -> bool`（`:171`）、`map_container_method(class, method) -> Option<&'static str>`（`:177`）。

### 1.2 真正的 C++ 支持面（跨 crate 实测）

| 层 | C++ 相关行数 | 其中独立文件 | 证据 |
|---|---|---|---|
| `vitro_lexer` / `vitro_parser` / `vitro_ast` | **1 701**（三 crate 合计 9 837 行的 17.3%） | **仅 `parser/src/cpp.rs` 223 行（2.3%）** | 逐文件分布见下表 |
| `vitro_cpp_frontend` | 186 + 502 JSON | 全部独立 | §1.1 |
| `vitro_typeck` | **≈3 534** | `cpp/` 407 + `cpp_monomorph/` 1 675 + `cpp_class_layout.rs` 535 + `cpp_container.rs` 59 + `cpp_auto.rs` 91 + `cpp_overload.rs` 461 + `expr/cpp.rs` 306 | 实测行数 |
| `vitro_codegen` | **483** | `cpp/` 461 + `stmt/cpp.rs` 22 | 实测行数 |
| `vitro_runtime` | **108 手写** + 205 生成 | `bytecode_libc_sig.rs` 手写；`bytecode_libc_index.rs` AUTO-GENERATED | `bytecode_libc_index.rs:1-5` |
| `native/runtime_libc/vitro/*.cpp`（真源） | **359** | list 132 / string 93 / vector 84 / sort_int 50 | 实测行数 |
| **C++ 专属代码合计** | **≈6 514 行**（另 +502 JSON +359 `.cpp` +205 生成物） | — | — |

**占全仓 76 218 行 `.rs` 的约 8.5%**（含 JSON/`.cpp`/生成物约 9.9%）。

前端三 crate 逐文件分布（用于判断"寄生"程度）：

| 文件 | 含空行 | 该文件内 C++ 相关行 | 性质 |
|---|---|---|---|
| `vitro_parser/src/cpp.rs` | 223 | 223 | **C++ 专用文件**（6 个函数） |
| `vitro_parser/src/decl.rs` | 865 | ~458 | 共享文件，含 class/template 全部实现 |
| `vitro_parser/src/type_.rs` | 739 | ~167 | 共享，含模板实例化类型/引用/ctor-init |
| `vitro_parser/src/lib.rs` | 735 | ~105 | 共享，含顶层分派 + 模板实参切流 |
| `vitro_parser/src/stmt.rs` | 718 | ~126 | 共享，含 range-for + ctor-init 语句 |
| `vitro_parser/src/expr/postfix.rs` | 303 | ~152 | 共享，含 new/delete/lambda/member-call |
| `vitro_parser/src/expr/primary.rs` | 299 | ~16 | 共享，this/lambda/限定名 |
| `vitro_parser/src/expr/unary.rs` | 305 | ~6 | 共享，new/delete 入口 |
| `vitro_ast/src/types.rs` | 685 | 145 | C++ 类型变体 + 全部 match 臂 |
| `vitro_ast/src/decl.rs` | 208 | 143 | C++ 声明节点段 |
| `vitro_ast/src/expr.rs` | 295 | 54 | C++ 表达式变体 |
| `vitro_ast/src/stmt.rs` | 98 | 22 | `RangeFor` / `Try` |
| `vitro_ast/src/lib.rs` | 112 | 3 | `compute_type_size` 的 C++ 臂 |
| `vitro_lexer/src/keyword.rs` | 82 | 25 | C++ 关键字表 |
| `vitro_lexer/src/token.rs` | 154 | 23 | C++ token 声明块（`token.rs:44-66`） |
| `vitro_lexer/src/lib.rs` | 451 | ~14 | C++ 模式唯一行为点 |

**结论**：**真正独立的只有 `parser/src/cpp.rs` 223 行；其余 C++ 代码全部寄生在共享文件里**——这正是 `TODO(#D08)`（`vitro_lexer/src/lib.rs:5`、`vitro_parser/src/lib.rs:5`）要治的组织债。

### 1.3 C++ 模式标志的分布（决定"砍"的影响面）

`is_cpp_mode` 全仓 **43 处命中 / 9 个文件**，分布为：lexer 6 处、parser 37 处、**ast / typeck / codegen / runtime / vm 各 0 处**。

- **唯一入口**：`native/src/engine/compile_pipeline.rs:631-634` 按**文件名后缀**判定（`.cpp` / `.cxx` / `.vitrocpp`），随后只传给 Lexer（`:643`）与 Parser（`:657`）。
- **lexer 内唯一行为点**：`vitro_lexer/src/lib.rs:390` —— C 关键字表未命中时回退查 `cpp_keyword_type()`（其余 5 处命中均为字段/构造函数管道：`:31`、`:49-51`、`:57`、`:66`）。
- **typeck/codegen 不接收该标志**：它们从 AST 内容（`ClassDecl` / `TemplateDecl`）发现 C++，是模式无关的。
- **但这些 Pass 是无条件调用的**：`vitro_typeck/src/lib.rs:173`（`register_class_layouts`）、`:371` 与 `:418`（`check_class_methods`）在**每次编译**都执行，纯 C 程序也走这段代码路径（对空集 no-op）。

**这条是"砍"方案的核心论据**：C++ 的代价不只在一个独立模块里，而是**无条件挂在 C 管线的调用图上**；同时它的"模式标志"只在最前端的两个 crate 里。

### 1.4 VM 侧的 C++ 感知度 = 0

`vitro_vm/src` 全目录对 `cpp|Cpp|class|Class|vtable|Vtable|ctor|dtor` 的命中**只有 3 处且全部无关**：一个 errno 字符串（`host/misc.rs:467` 的 `"No such file or directory"`）、一句注释（`core/memory.rs:473` 提到"struct/union/class"）、一个无关词（`core/executor/debug.rs:5`）。

**即：C++ 在到达 VM 前已被完全降解为普通 opcode 与全局数据。** 没有 vtable opcode、没有析构 helper、没有 `new[]` 计数机制（`base[-4]` 只是普通内存算术）。**这是"砍"对 VM 影响 = 0 行、以及 VM 迁移与 C++ 裁定完全解耦的硬证据。**

### 1.5 关键机制速览（供后续各节引用）

| 机制 | 实现位置 | 形态 |
|---|---|---|
| 虚函数 vtable | `codegen/lib.rs:462-476` 分配、`cpp/member.rs:11` vptr 占首 4 字节、`expr/struct_.rs:196-203` 按**名字在 entries 中的位置**定位、`expr/new_delete.rs:151-154` 构造时写 vptr | **编译期全局数据段数组**，4 字节/槽；vptr 占对象首 4 字节 ⇒ **字段偏移随"有无虚函数"整体位移** |
| `this` | 隐式首参（`cpp_class_layout.rs:335-338` 明载"必须用不含 this 的用户参数"） | 普通参数传递，非独立寄存器 |
| 栈对象 RAII | `cpp/raii.rs:39-63` `emit_dtors_for_scope_exit(start_frame_idx)` | scope frame 逆序；四类控制流起点已文档化（return→0、continue→循环体、break→while/do-while 为循环体 / for 为 for-init） |
| `new[]` / `delete[]` | `CPP_FAILURES.md:79` 记 `base[-4]` 存 count、逆序 dtor | 普通内存算术 + 4 个 temp slot（`:81`） |
| 内置容器 AOT | `scripts/precompile_bytecode_libc/main.go`（492 行）→ `vitro_vm/src/bytecode_libc_data.json` + `vitro_runtime/src/bytecode_libc_index.rs` | 容器用引擎自己编译成字节码后**冻结索引段**（`BYTECODE_LIBC_BASE_INDEX=1000`、`FUNC_COUNT=88`、`GLOBALS_RESERVED=1024`，`bytecode_libc_index.rs:7-10`） |

---

## 2. 可复用资产清单（标注移植成本）

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| 2.1 | **C++ AST 枚举设计** | `vitro_ast/src/decl.rs:59-207`（`AccessSpec` 3 变体、`ClassMember` 6 变体、`ClassDecl`、`TemplateParam` 2 变体、`TemplateArg` 3 变体、`Templateable`、`TemplateDecl`、`VTable`）、`types.rs:81-97`（`Type::Class/Reference/RValueRef/Auto/TemplateId`） | **低** | 已是纯 enum + `Box`/`Vec`，**无一等继承、无 trait 对象、无生命周期**，与 MoonBit `enum` + 模式匹配近 1:1。MoonBit 的穷尽性检查还会**强制**所有新增变体被全量处理。`ClassMember::NestedClass{decl: ClassDecl}` 的递归嵌套也天然可用 `enum` 表达 |
| 2.2 | **Phase 41 内置容器布局解耦（`.cpp` 真源 + JSON 加载器）** | 真源 `native/runtime_libc/vitro/{vector,list,string,sort_int}.cpp`（359 行）；产物 `vitro_cpp_frontend/src/builtin_layout_data.json`；加载器 `builtin_layout.rs:96-179`；消费点 `vitro_typeck/src/cpp_container.rs:14-15`、`vitro_class_layout.rs:495-496`、`vitro_codegen/src/lib.rs:307-308`、`cpp/range_for.rs:2` | **低**（加载器）/ **中**（生成链） | **加载器侧零硬编码已验证**：`builtin_layout.rs` 全文无任何容器名/方法名/字段名字面量，全部来自 JSON；`parse_type_str` 只认 6 个基础类型（`:61-77`），连 `vitro_string` 都在 JSON 里。JSON 与三份 `.cpp` 真源**逐字段核对无漂移**（字段、方法集、size 全部吻合）。**但生成链断**：见 §3.1 |
| 2.3 | **C++ 子集规范文档（对外语义契约）** | `docs/current/03-语言子集/C++子集规范.md`（409 行）：§3 明确不支持表含错误码 `:277-289`（E4001 异常 / E4002 运算符重载 / E4003 特化 / E4004 多线程 / E4005 多继承 / E4006 namespace / E4011 using / E4017 constexpr）；§4 差异表 `:293-356` | **低** | 这是**语言中立的语义规格**，正是 MoonBit 版要实现的东西。含指针上/下转型与 Clang++ 的逐项对照表（`:335-347`），可直接当验收清单 |
| 2.4 | **Clang++ 现场对照防线（99 例）** | `scripts/shadow_verify_cpp/main.go`（761 行 Go）；用例 `native/tests/cases/cpp/*.cpp` 83 个 + 内嵌 22 条（`main.go:89-387`，去重 6 → 99） | **低** | 不依赖 Vitro 实现语言：**每次现场跑 `clang++ -std=c++14`**（`main.go:446`），是真正的独立 oracle。J9 启动自检 7 条断言（`main.go:393-418`），自检不过 `exit 2` 拒绝运行。**MoonBit 版可直接复用，只需换 Vitro 侧调用入口** |
| 2.5 | **83 个 C++ E2E 用例 + 79 个锁定 golden** | `native/tests/cases/cpp/`、`native/tests/cases_golden/cpp/` | **低** | 用例是标准 C++14 源（`CPP_FAILURES.md:117` 自陈"无需为 Vitro 做额外规避"），语言中立。分布：核心语言 ~18 / 容器算法 ~15 / 教学 OJ ~28 / G 批 10 / U3 等。**但 golden 有 4 例缺、4 例手写**（见 §3.4 与 §8.3） |
| 2.6 | **C++ 单元/回归测试驱动（142 个 `#[test]`）** | `parser_cpp_unit_test.rs` 33 / `typeck_cpp_unit_test.rs` 31 / `bytecode_gen_cpp_unit_test.rs` 47 / `cpp_dogfooding_test.rs` 28 / `cpp_lambda_test.rs` 3 | **中** | 测试**意图**（用例名 + 内联 C++ 源码字符串）全部可复用，且源码字符串语言中立；但断言写在 Rust API 上（`Lexer::with_mode` / `TypeChecker` / `BytecodeGen` 内部调用），需改为 MoonBit 侧等价断言。结构最干净的是 `bytecode_gen_cpp_unit_test.rs`（47 例，多为 `r#"..."#` 源码 + 输出断言，端到端风味） |
| 2.7 | **类布局算法语义** | `vitro_typeck/src/cpp_class_layout.rs`（535 行）：`register_class_layouts`（`:5`）、`compute_class_has_resource`（`:61`，含环保护）、`register_single_class_layout`（`:86`）——基类字段继承 `:125`、C++ 名字隐藏 `:107,134-136`、虚函数覆写替换 vtable 槽 `:202-206`、静态字段不入实例大小 `:303` | **中** | **语义**（字段顺序、名字隐藏、覆写规则、`has_resource` 决定隐式移动构造）是要保留的设计；但实现重度依赖 `HashMap<String, ...>` 字符串键与 `visiting: HashSet<String>` 环检测，需按 §5.2 重设计 |
| 2.8 | **RAII 析构注入策略（四类控制流）** | `vitro_codegen/src/cpp/raii.rs:39-63` | **低** | **已想清楚并写成文档的策略**，与语言无关（"内到外、LIFO、按 scope frame 逆序"）。MoonBit 版照抄策略即可。对应测试齐备（`cpp_dogfooding_test.rs` 的 LIFO/早 return/break/continue 四例 + 用例 `cpp_raii_break_while`／`cpp_raii_break_dowhile`／`cpp_raii_continue_for`） |
| 2.9 | **AOT 预编译容器产物 + 交叉校验门禁** | `scripts/precompile_bytecode_libc/main.go`（492 行）；`--check` digest 门禁（`main.go:426 checkUpToDate`，漂移 exit 1）；`main.go:242,257-272` 校验"`method_map` 的每个方法必须在预编译产物中可用" | **中** | **真正的工程资产**：容器不是解释执行的，而是**用引擎自己编译成字节码再冻结索引**，带 digest 新鲜度门禁。MoonBit 版若保留字节码 ABI 则可整体复用产物；若改 ABI 则须用 MoonBit 编译器重新预编译（容器是 C++ 源，可重新生成） |
| 2.10 | **失败台账与防线方法学** | `CPP_FAILURES.md`（20 条，1 条 active）、`DOGFOODING_FAILURES.md`（58 行）、`reports/facts.json`（4 个 C++ key）、`scripts/facts`（真值对账）、评估报告 §7 的 J1~J10 判据 | **低** | 全部是**文档与判定规则**，语言中立，可原样带过去当迁移期的验收纪律 |

---

## 3. 抛弃清单

| # | 抛弃对象 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 3.1 | **`extract_cpp_builtin_layout.py` 及其输出路径** | `scripts/extract_cpp_builtin_layout.py`（348 行），`OUTPUT_PATH` 在 `:22` = `native/src/compiler/cpp_frontend/builtin_layout_data.json` | **该目录不存在**（实测 `Test-Path` = False；`native/src/compiler/` 下只剩 `mod.rs`/`ast.rs`/`cfg.rs`/`data_flow.rs`/`intent.rs`/`algorithm_detector/`）。crate 化后产物搬到 `native/crates/vitro_cpp_frontend/src/`，但**生成器路径未同步**。全仓无任何 CI/编排调用它（`select-string` 零命中，仅文档与归档引用）。**跑它会创建一个孤儿文件，且不会更新 `builtin_layout.rs:97` 真正 `include_str!` 的那份**。需区分：`precompile_bytecode_libc/main.go:53,242,257-272` **只读取并交叉校验**该 JSON（校验 `method_map` 的方法在产物中可用），**不生成**它 | **高**。JSON 冻结在 `generated_at:2026-06-13`。目前唯一的漂移防护是上述**方法可用性**交叉校验——它只查"方法名在产物中存在"，**不查字段、不查 size**。若有人改了 `.cpp` 的字段/大小，**两边都不会报错**。已被登记：`统一整备路线图.md:210`（U7#7）明确写"`vitro_cpp_frontend` JSON 解析改 fail-fast + size 与字段累加交叉校验 + **生成脚本路径修正（当前指向 crate 化前老路径，无 drift 门禁）**" |
| 3.2 | **字符串命名合成实体体系** | **83 处命中**，构造点散落在 parser / typeck / codegen **三个 crate 的 11+ 文件** | 命名方案本身有 12 种：`__ctor__{C}`、`__ctor__{C}__{N}`、`__ctor__{C}__copy`、`__ctor__{C}__move`、`__dtor__{C}`、`__lambda_{N}`、`__lambda_{N}__call`、`__anon_struct_{pos}`、`__anon_union_{pos}`、`{C}__{method}`、`{C}__{field}`、`std__move`。**无单一来源**：`cpp_overload.rs:205-214` 的 `constructor_mangled_name` 只覆盖 typeck 内部；parser 自行 `format!("__ctor__{}", ...)`（`expr/postfix.rs:28,38`、`stmt.rs:226,228,269,271`、`cpp.rs:89`、`decl.rs:668`），codegen 自行拼 `__dtor__{}` / `__ctor__{}__move`（`raii.rs:19,30`、`var_decl.rs:46,71,95`、`expr.rs:518`、`new_delete.rs:94,97,161,164,225,327`、`call.rs:31,61,84,247`） | **中**。命名不匹配时 codegen **静默不生成调用**：`raii.rs:20,31` 的符号查找是 `if let Some(&idx) = self.func_index.get(&name)` —— 缺符号 = 无构造/无析构 + **零诊断**。这正是 §6 中多起事故的机械成因。已被登记：`三语化整备审计计划.md:87`（L4"同类病灶不除根…**mangled name 散写**"）、`统一整备路线图.md:162`（U3#6"mangled name 单源审计…**清剩余散写**"） |
| 3.3 | **`placeholder_class` 空壳占位 hack** | `vitro_typeck/src/cpp_monomorph/synth.rs:63-75`；使用点 `builtin.rs:33,41` | 查重命中时返回一个**成员为空、名字正确**的 `ClassDecl`，其正确性依赖**调用方的 push 点纪律**。文档自陈："**不得**进入 `program.classes`（push 点查重会拦截），避免同名空类覆盖真实布局"（`:65-66`）。这是"不变式写在注释里、而非类型里"的典型症状治疗 | **中**。若某条新路径把它 push 进 `program.classes`，会造成**空类静默覆盖真实布局**（与 §6.6 的 U3#9"静默覆盖"同型）。MoonBit 下可用和类型让该状态不可表示（§5.3） |
| 3.4 | **4 例手写 golden 与 4 例无 golden 的防线形态** | `cases_golden/cpp/`：`cpp_vitro_vec_class.out`（mtime 2026-06-26 17:30）、`cpp_vitro_list_class.out`（06-26 18:17）、`cpp_u3_*.out`（09-14 23:29）——**这 4 例 Clang++ 编译不过，内容与 Vitro stdout 逐字节相同**；另 4 例（`cpp_copy_ctor`/`cpp_default_args`/`cpp_nested_class_instance`/`cpp_nttp_class`）无 golden | 前者**违反"Golden 只能来自 Clang"**（`AGENTS.md:142`）；`CPP_FAILURES.md:43` 却称"Golden 全部由 Clang++（`-std=c++14 -O0`）生成"——与事实及驱动实际参数（`main.go:446` 仅 `-std=c++14`，**无 `-O0`**）均不符。后者（无 golden）在 `vitro_e2e.rs:186` 的无-else 分支下**永不比对 stdout** | **高（对迁移对账致命）**。`cpp_vitro_vec_class` 这类 golden 是 **Vitro 自证**，用它验收 MoonBit 版是**循环论证**——两个实现可能一致地错。迁移期必须把这 4 例的对照物换成真正独立的 oracle（见 §8.3.2） |
| 3.5 | **死 token / 死 AST 变体 / 死字段** | lexer：`using`（`token.rs:50`）、`namespace`（`:51`）、`friend`（`:55`）、`static_cast`（`:58`）、`const_cast`（`:59`）、`reinterpret_cast`（`:60`）、`->*`（`:65`）、`.*`（`:66`）全部**无 parser 消费方**；`nullptr` 的 cpp 表条目（`keyword.rs:26`）**不可达**（C 表先命中，`lib.rs:388`）；`protected` **半死**（`decl.rs:107,124-128` 只存不查，全仓无 `AccessSpec::Protected` 消费方）。AST：`Stmt::Try`（`ast/stmt.rs:85-89`）与 `Expr::Move`（`ast/expr.rs:231-235`）**无 parser 生产者**；`TemplateArg::Int` 无 parser 生产者；`ClassDecl.vtable` parser 恒置 `None`（`parser/decl.rs:351`）；`Type::Function.is_const` **无任何生产者置 true** | 占位等待接线，或纯遗留 | **低**。丢弃无功能风险。但**注意**：`Stmt::Try` 在 `typeck/decl.rs:576` 会报 E4001，`typeck_cpp_unit_test.rs:278` 还有一条 try 语句测试——即"死节点 + 活报错 + 活测试"三者并存，迁移时需一并裁定 |
| 3.6 | **`bytecode_libc_sig.rs` 手写签名表** | `vitro_runtime/src/bytecode_libc_sig.rs`（108 行，**无 AUTO-GENERATED 头**） | 把 JSON `methods[].params/ret` 已有的事实**用 Rust 手抄了一遍**：60 条容器方法签名（`:14-72`）逐条 `Some((void.clone(), vec![int_ptr.clone(), int.clone()]))`。这是 Phase 41 声称"零硬编码"的**实际漏点** | **低**（可从 JSON 生成）。但它证明"零 Rust 硬编码"是**限定于布局**的说法，不是全仓事实 |
| 3.7 | **`cpp_overload.rs` 按参数个数 mangling 的重载决议** | `cpp_overload.rs:205-214`、`:220-221` 自陈局限 | 同参数个数、不同参数类型的构造函数**无法区分**，报 E4031 歧义而非正确决议。`C++子集规范.md:102-104` 已把该限制写成对外契约 | **低**（是已记录的子集边界，不是缺陷）。但**迁移时若顺手修好**，会超出"等价迁移"范围，需单独裁定 |
| 3.8 | **`native/src/compiler/mod.rs:7` 的兼容性转发** | `pub use vitro_cpp_frontend as cpp_frontend;` | crate 拆分遗留的别名层 | **低** |
| 3.9 | **文档陈旧数字（组织债）** | `CPP_FAILURES.md:10-15` 记 parser 33 / typeck **28** / bytecode_gen **38**、E2E **78** / `74` / 合计 175 —— 实测 `#[test]` 为 **33/31/47**（合计 111）、`cpp_e2e_cases` 现值 **83**；`CPP_FAILURES.md:19,117` 仍写"61 个用例"；`影子验证框架.md:9` 写"C++ 侧 **97** 个用例（95 + **2** clang_compile_fail）"——真值 99/95/**4**；`C++子集规范.md:176` 写"模板递归深度 ≤ 8"——现行上限 **1024**（`error_codes.rs:31` 的 E1022）；`C++子集规范.md:174` 写"仅类型模板参数"——NTTP 已支持 | 同一文件内 61/74/78 三套并存 | **中**。`影子验证框架.md:9` 的 97/2 被 `reports/doc_fact_drift.md:43-48` 归入"**人工维护**"（分解式/实测数字行）而**未被判为漂移**——即 facts 门禁对这类数字存在盲区 |

---

## 4. 在途工作接纳方案

### 4.1 在修的 bug / 已登记未做的重构（逐条给接纳方式）

| # | 项 | 来源 | 状态 | 新项目接纳方式 |
|---|---|---|---|---|
| 4.1.1 | **U3#9 完整根治：嵌套名 mangled 化（`Outer__Inner`）+ 字段类型引用与访问路径跟随** | `CHANGELOG.md:54-60`（"完整根治…登记下批"）、`统一整备路线图.md:831-834`；现状 `cpp_class_layout.rs:254-260` 自陈"嵌套 struct 直接展平进全局 structs 表（无作用域）…完整根治 = 嵌套名 mangled 化 `Outer__Inner` + 访问路径跟随，登记下批"；止血点 `:271` | **止血已做，根治未做** | **直接按目标架构实现**（不修后搬）。理由：mangled 字符串命名体系在 MoonBit 版应整体替换为**结构化作用域名**（§5.2），修 Rust 版再搬运等于搬一个即将废弃的机制。**必须保留的验收锚**：`cpp_nested_class_instance`（但该例当前无 golden，见 §8.3.1）+ `crash_regression_tests.rs` 的 `test_u3_nested_struct_conflict_diagnosed` |
| 4.1.2 | **函数式构造存量缺陷**：赋值右侧 `p = Point(3,4)` 与调用实参 `f(Point(1,2))` 裸类即失败 E3023 | `统一整备路线图.md:817-819`、`:834`；锚用例用规避写法规避了它（`cases/cpp/cpp_u3_vec_class_twice.cpp:6-7` 注释自陈） | **已登记未做** | **直接按目标架构实现**。理由：这是"类构造在表达式位置"的语义缺口，MoonBit 版重写 typeck 时天然要处理；在 Rust 版修完再搬是重复劳动。**注意**：现有 2 个 U3 用例用规避写法，迁移后应改回自然写法并新增锚 |
| 4.1.3 | **U3#4/T1：模板实例化轮数上限 1024** | `CHANGELOG.md:39-42`、`vitro_typeck/src/lib.rs:384`、`error_codes.rs:31`（E1022） | **已修**（红→绿锚 `test_u3_template_instantiation_round_limit_triggered`，0.59s 内报错） | **修复后搬**：护栏语义（"1024 层内确定性报错，不得 OOM，行为与 clang 一致"）是必须保留的**防线义务**，属 J7"保险丝可触发性"。MoonBit 版必须有等价护栏 + 可触发性证明 |
| 4.1.4 | **U3#5/T2：类实例化与函数实例化同收敛 drain** | `CHANGELOG.md:43-48`、`vitro_typeck/src/lib.rs:381`、`cpp_monomorph/replace.rs:66,210` | **已修** | **修复后搬**（搬的是**语义**：`pending_class_instantiations` 必须与函数实例化同收敛循环 drain，不得只在某一 Pass 后排空一次）。锚：`cpp_u3_class_instantiate_in_template.cpp` |
| 4.1.5 | **U3#8：容器重复实例化查重前置** | `CHANGELOG.md:49-52`、`cpp_monomorph/builtin.rs:23-29`、`replace.rs:9`、`synth.rs:64` | **已修** | **直接按目标架构实现**。理由：其实现载体（`placeholder_class` 空壳 + 字符串查重）正是 §3.3 要抛弃的形态；MoonBit 版用和类型表达（§5.3）后该 bug **不可表示**，而非"被修好" |
| 4.1.6 | **U3#6：C++ 寄生收口 + mangled name 单源审计（清剩余散写）** | `统一整备路线图.md:162`（U3，**CS2/CS3 前硬门禁**）、`三语化整备审计计划.md:215`（S3） | **已登记未做** | **放弃并记录理由**（在 Rust 侧）。理由：MoonBit 重写会把整套字符串 mangling 替换为结构化命名，在 Rust 版做"单源审计"随后丢弃是纯浪费。**但必须把审计产物当迁移输入**：`统一整备路线图.md:162` 已列出的"D1 已做 `method_mangled_name`"单源点（`cpp_overload.rs:205`、`cpp/methods.rs:134,192`）是 MoonBit 版的**命名语义权威**，需先固化成语义表 |
| 4.1.7 | **U3#1/#3：temp_slot 固定 4 槽手术 / `next_local_offset` 作用域回收** | `统一整备路线图.md:157,159`；遗迹 `CPP_FAILURES.md:80-81`（3 槽→4 槽）、`项目规模与难度评估_2026-09-14.md:208`（"`next_local_offset` 只增不减"） | **已登记未做**（U3，硬门禁） | **直接按目标架构实现**。理由：这是 codegen 槽位分配器设计，MoonBit 版从零写分配器时天然解决。**不能丢的是语义约束**：`call.rs:178` 注释的"8 字节位模式必须走 8 字节专用槽"是**正确性要求**（U3#2 红锚 `bytecode_gen_unit_test.rs:142`） |
| 4.1.8 | **U3#7：变参实参区溢出（预留 64 字节，>16 words 静默覆写）** | `统一整备路线图.md:163` | **已登记未做** | **直接按目标架构实现**：新 codegen 应做动态预留或超限诊断（"静默覆写"违反防线哲学第 0 条） |
| 4.1.9 | **U7#7：`vitro_cpp_frontend` JSON 解析改 fail-fast + size/字段累加交叉校验 + 生成脚本路径修正** | `统一整备路线图.md:210` | **已登记未做** | **分三段接纳**：① fail-fast —— 搬（`builtin_layout.rs:98-101` 现在是 `panic!`，需改成结构化错误）；② **size 与字段累加交叉校验** —— 搬，且这正是本次实测发现的真实缺口（§3.1）；③ **生成脚本路径修正** —— **放弃**（脚本本身要重写）。MoonBit 版应把生成器做成 MoonBit 自身，彻底消除 Python 依赖 |
| 4.1.10 | **`TODO(#D08)`：lexer/parser 的 C++ 专属词法/语法下沉到 `lexer/cpp.rs` / `parser/cpp.rs`** | `vitro_lexer/src/lib.rs:5`、`vitro_parser/src/lib.rs:5`、`vitro_codegen/src/expr.rs:1`、`codegen/lib.rs:5`、`expr/assign.rs:31`、`expr/binary.rs:3` | **已登记未做** | **直接按目标架构实现**（模块切分债，新语言下按 §9 的包切分自然解决）。**但 `#D08` 的诉求本身应保留为设计原则**：C++ 代码不应寄生在共享文件里 |
| 4.1.11 | **`TODO(#D10)`：class/template 顶层分派与 C++ 引用判断继续下沉到 `parser/cpp.rs`；typeck 的 `cpp_*.rs` 收进 `cpp/`** | `vitro_parser/src/cpp.rs:5-6`、`vitro_codegen/src/cpp/mod.rs:5`、`vitro_typeck/src/cpp/mod.rs:5` | **已登记未做** | **直接按目标架构实现**（同 4.1.10）。注意 `codegen/cpp/mod.rs:5` 的 TODO 写在 codegen 里却描述 parser 的事——**TODO 放错文件**，属组织债 |
| 4.1.12 | **lambda 双 E3014 重复声明** | `统一整备路线图.md:285`（"未采信：来源方复测只见一条，登记待查"） | **登记待查** | **先复测再定**。C++ lambda 属"可最后搬"（§10.2），可在迁移后期裁定；若届时未复现则按"不成立"关闭 |
| 4.1.13 | **`CPP_FAILURES.md` 是否应登记 U3 批** | `CPP_FAILURES.md` 全文 `U3` 词频 **0** | 事实缺口 | **迁移期一并治理**：搬迁台账时把 U3#4/#5/#8/#9 补为"已修复"历史条目，否则新项目的坑史起点是残缺的 |

### 4.2 计划文档条目 → 接纳方式汇总

`C++拓展实施计划.md`（1 614 行）是 Phase 31~42 的**活进度载体**（`docs/README.md:43`），其 §15.3"仍然有效的约束与当前写法"（`:1583-1591`，5 行）是**必须逐条裁定**的清单：

| 约束（`C++拓展实施计划.md:1583-1591`） | 接纳方式 |
|---|---|
| `T()` 值初始化不支持（"模板活约束 G13"） | 直接按目标架构实现 |
| 函数模板显式实例化不支持（仅类模板支持） | 直接按目标架构实现或明确放弃并记录 |
| 同一模板类不能跨文件重复定义 | 直接按目标架构实现（新包的模块系统天然解决） |
| 嵌套模板类需显式实例化 | 直接按目标架构实现 |
| `method_map` 必须指向 mangled 方法名 | **修复后搬**（搬语义：JSON 的 `method_map` 值必须是最终函数名，`bytecode_libc_sig.rs` 已同步） |

> **⚠️ 结构性证据缺口（必须上报）**：`C++拓展实施计划.md:5` 把 Phase 42 lambda 批次的最细粒度进展指向 `docs/archive/ARCHIVE_代码审查复核20260911.md`；`:9`、`:1552`、`:1609` 另指向 3 份归档文档。按引用纪律这些**不可引用**。因此 **Phase 42 在途状态的可引用证据只剩两条通道**：`CHANGELOG.md:37-66` 与 `CPP_FAILURES.md`——而两者都带 §3.9 与 4.1.13 记录的完整性缺陷。**裁定前建议先补一份非归档的 Phase 42 状态固化文档。**

---

## 5. 架构优化建议（MoonBit 形态）

### 5.1 〔沿革保留〕保留的既有决策

| 建议 | 依据 |
|---|---|
| **保留 `.cpp` 作为容器布局唯一真相来源 + JSON 中间产物** | `builtin_layout.rs` 全文零硬编码（实测）；JSON 与三份 `.cpp` 逐字段核对无漂移；这是本项目少数"数据与代码分离"的正确决策。**只修生成链，不推翻设计** |
| **保留字节码 ABI 的固定索引段思想** | `BYTECODE_LIBC_BASE_INDEX=1000` / `GLOBALS_RESERVED=1024` / `FUNC_COUNT=88`（`bytecode_libc_index.rs:7-10`）+ digest 门禁（`main.go:426`）。这是"预算可机检"的正面先例 |
| **保留 RAII 析构的"scope frame 逆序"策略** | `raii.rs:39-46` 的四类控制流语义已文档化且测试齐备，与语言无关 |
| **保留"编译期全局 vtable + vptr 首 4 字节"的布局方案** | `codegen/lib.rs:462-476`、`cpp/member.rs:11`。白箱友好（vtable 是可直接观察的全局数据），无运行期元数据 |
| **保留 `is_cpp_mode` 只作用于最前端两层的边界** | 实测：typeck/codegen/vm 零 `is_cpp_mode`。这是干净的"模式只影响解析"设计 |

### 5.2 〔结构优化〕用类型替换字符串（核心建议）

**问题**：`Type::Class { name: String }`（`types.rs:81`）、`Type::TemplateId { base: String, args }`（`:93`）、`ClassDecl { name: String, base: Option<String> }`（`decl.rs:111-114`）、`VTable { entries: Vec<(String, Type)> }`（`:106-108`）——**类身份、模板身份、继承关系、vtable 槽、方法、静态字段全部以字符串为键**，mangled 名在 11+ 文件里各自拼接（§3.2）。

**MoonBit 形态**：

```
/// 名字驻留：字符串 → 稳定 ID（一次映射，全流程用 ID）
enum ClassId { ... }          // 或 newtype over Int，比较/哈希廉价
enum SymId   { ... }

/// 模板身份是 (基准, 实参表) 的结构，不是拼出来的字符串
enum TemplateArg { TType(Type); TInt(Int); TExpr(Expr) }
struct TemplateId { base : SymId; args : Array[TemplateArg] }

/// 合成实体是数据，不是"靠命名约定互相找"
enum Entity {
  UserCtor(ClassId, Arity)
  CopyCtor(ClassId)
  MoveCtor(ClassId)
  Dtor(ClassId)
  Method(ClassId, SymId, Arity)
  Lambda(Int)
}
```

收益：① §3.2 的"命名不匹配 → 静默不生成调用"**不可表示**；② 4.1.1 的 `Outer__Inner` 展平问题变为"作用域链"的自然表达；③ 4.1.5 的重复实例化查重变成集合成员判断，而非字符串前缀比较。

### 5.3 〔结构优化〕让"不可注册的空壳"不可表示

`placeholder_class`（`synth.rs:63-75`）的正确性依赖调用方纪律。MoonBit 和类型表达：

```
enum SynthOutcome {
  Fresh(ClassDecl)            // 需要注册
  AlreadyRegistered(SymId)    // 只需要名字，构造上无法被注册
}
```

把"不得进入 `program.classes`"的注释**升级为类型约束**。同类收益适用于 4.1.5。

### 5.4 〔结构优化〕把无条件调用的 C++ Pass 变成显式阶段

现状：`register_class_layouts`（`typeck/lib.rs:173`）与 `check_class_methods`（`:371`、`:418`）**无条件在每个编译里跑**。
MoonBit 形态：做成**显式的可组合阶段**，输入是"有无类/模板"的判据，空输入时**短路**，而非进入循环。收益：C 程序编译路径不再经过 C++ 代码（同时正面回答"砍"的收益问题——可用短路取得同等收益而不必砍）。

### 5.5 〔结构优化〕把"静默缺符号"改为 fail-loud

`raii.rs:20,31` 与 codegen 各处的 `if let Some(&idx) = self.func_index.get(&name)` 在符号缺失时**静默不生成**。MoonBit 版应改为返回结构化错误（缺 `__dtor__` 符号 = 编译期错误，而非"析构没生成"）。这与 `AGENTS.md:142` 的诚实记录原则同向。

### 5.6 〔新设计〕诊断与错误类型化

Rust 版用 `Vec<String>` + `ErrorCode` 收集错误（`cpp_container.rs:19-28` 用 `report_error(&format!(...), loc, ErrorCode)`）。MoonBit 有类型化错误（`error` 类型 + `raise`），建议定义 `enum CppDiag { ... }` 而非字符串，使 §6 中"错误消息格式"成为可断言的稳定契约（对 golden 对账也更有用）。

### 5.7 〔新设计〕模板单态化的收敛循环作为一等结构

U3#5（`vitro_typeck/src/lib.rs:381`）与 U3#4（`:384`）的教训是"pending 队列 + 轮数上限 + 同收敛 drain"三者必须同时成立。MoonBit 版建议显式建模为一个**工作列表 + 预算**的结构（而非散在 Pass 3/3.5/3.6/4 之间），让"新发现的实例化必须被同轮 drain"成为结构性事实。

---

## 6. 坑清单（现象 → 根因 → 修复 → 新语言下是否复发）

数据源：`CPP_FAILURES.md`（146 行，20 条）+ `DOGFOODING_FAILURES.md`（58 行）+ `CHANGELOG.md:37-66`。

**两个口径已分清**：历史累计 **20 条**（人工归类）；当前 active **1 条**（`facts.json` key `cpp_failures_active`，`as_of 2026-09-14`），其判据是 `CPP_FAILURES.md:88` 的 `KNOWN_DIVERGENCE`（**不是缺陷，是实现方式差异**：C++ 版 `push_back` 用 `new[]/delete[]` + 循环复制，C 版用 `realloc`，以 stdout 一致性为验收标准）。

| # | 现象 | 根因 | 修复 | MoonBit 下是否复发 | 为什么 |
|---|---|---|---|---|---|
| 6.1 | `new A[n]` 的 `i_temp` 与 `user_ptr_temp` 冲突（`CPP_FAILURES.md:77-82`） | `get_temp_slot` 只有 **3 个**独立槽位 | 扩展到 `temp_slot0..3` 共 4 槽 | **结构上不复发，语义上会复发** | 结构：MoonBit 版若用显式槽位分配器（作用域化 bump + 释放）则无固定槽位数。语义"临时槽必须在嵌套下不冲突"仍须测——这是 U3#1（`统一整备路线图.md:157`，登记未做）要根治的 |
| 6.2 | 字节码比较测试**永远 SKIP、从未执行**（`CPP_FAILURES.md:62-70`） | 测试里硬编码 mangled 名 `"get__vector__int"`，实际规则是 `{class}__{method}` → `"vector__int__get"`，`contains_key` 恒 false | 改名后重跑通过 | **不复发** | 这是"字符串命名 + 测试侧硬编码"的组合病。§5.2 用 ID 替换字符串后，测试侧无法拼错名字（拼不出来）。**这是 §5.2 最有力的单点收益证据** |
| 6.3 | 模板类 `Class<T>&` 自引用参数不工作（`CPP_FAILURES.md:110`） | 单态化未替换裸 `Type::Class` | Parser 把类模板名加入 `template_names`；单态化替换裸 `Type::Class` | **取决于新单态化实现**，非语言决定 | 属单态化替换的完备性问题。MoonBit 版若按 §5.2 用结构化 `TemplateId` 表达，替换变成显式的类型变换，可用穷尽 match 保证不漏（Rust 版靠 `HashMap` 遍历，天然易漏） |
| 6.4 | 全局 `auto` 推断出 `auto` 而非 lambda 类型（`CPP_FAILURES.md:132,142`） | Pass 2.5 的 `declare_var` 登记的是**替换前**的 `auto`（类型替换发生在登记之后） | 改为**先定型再登记**，解析结果缓存复用 | **可能复发（同型）** | 这是"多 Pass 之间同一事实有两个版本"的经典问题（`项目规模与难度评估_2026-09-14.md:206` 列为 A 类跨层语义失配）。MoonBit 无借用检查器，**不会**帮你发现这类顺序错误。需靠"单源 + 顺序断言"或 §5.6 的类型化中间表示 |
| 6.5 | lambda 返回类型被当 int → printf 报 E3062（`CPP_FAILURES.md:133,141`） | `__call` 的返回类型在 `resolve_lambda`（typeck）与 Pass 4 生成的 `FuncDecl` 中**各自硬编码 `Type::int()`** | 新增 `infer_lambda_return_type`，结果存 `LambdaInfo::return_type`，**两处共用同一来源** | **可能复发（同型）** | 与 6.4 同族："同一语义在两处各自算"。MoonBit 版如把 lambda 的返回类型放进单一数据结构（而非两处计算），则不复发 |
| 6.6 | 嵌套 struct 同名静默覆盖 → 后者覆盖前者布局，`a.i.x` 报 E3042（`cpp_class_layout.rs:254-260`） | 嵌套类被**展平进全局 structs 表（无作用域）**，直接 `insert` | U3#9 止血：保留首个 + 同名冲突**显式报错**（"显式拒绝优于静默错布局"）。完整根治登记下批 | **不复发** | §5.2 的"作用域链"命名让同名嵌套类天然共存，`insert` 覆盖在结构上不成立。**注意**：这是本项目"静默错值"类中最危险的一档（不报错、只错布局） |
| 6.7 | 第二个 `vitro_vec<Foo>` 报 E3002"类重复定义"（**合法代码被拒**，外部审查实锤）（`cpp_monomorph/builtin.rs:23-25`、`cases/cpp/cpp_u3_vec_class_twice.cpp:2-7`） | 快路径无条件合成 + `register_single_class_layout`，未先查重 | U3#8 查重前置（`builtin.rs:28-29`） | **不复发** | §5.3 的和类型让"已注册"与"新建"成为不同构造路径，重复注册不可表示 |
| 6.8 | 函数模板体内的类实例化被静默丢弃（**合法 C++ 误拒** E3042）（`cases/cpp/cpp_u3_class_instantiate_in_template.cpp:2-4`） | `pending_class_instantiations` 只在 Pass 3 后排空一次，Pass 3.6 期间新发现的类被丢弃 | U3#5/T2：类与函数实例化**同收敛 drain** | **可能复发（同型）** | "轮询队列 + 单点排空"的顺序病。§5.7 建议把工作列表建模为一等结构 |
| 6.9 | `template<class T> int f(T t){ return f(&t); }` **一行代码让编译器 OOM/挂死**（`重构评估报告20260912.md:96`） | 实例化循环无深度上限；去重靠 mangled 名，对类型无限增长链无效 | U3#4：上限 1024 + 超限报 E1022 | **可能复发** | 递归实例化是语义本身的性质，与语言无关。**护栏必须重建并用 J7 证可触发**（`统一整备路线图.md:807-810` 用 0.59s 有限完成证明） |
| 6.10 | `cpp_ctor_overload` / `cpp_member_out_of_line` / `cpp_template_struct` 三条被标 `category: gap`（`main.go:273,335,364`） | — | — | **不适用（不是语言问题）** | **但这是本清单里最该上报的防线缺陷**：`main.go:704-705,758-760` 只对**非 gap** 用例的差异 `exit 1`。3 条内嵌 gap 用例当前是 `match`，**未来回归成 `output_gap` 也不会变红**。J9 只保证判定函数没坏，不保证这 7 条 gap 用例有逐例审计（`AGENTS.md` 的 J2 闭环做过 C 侧 16 例审计，C++ 侧未做） |
| 6.11 | `cpp_move_semantics` 探索用例因触发内存泄漏报告未纳入（`工程债务维护方案.md:503`） | 移动构造未把源指针字段置空前存在泄漏 | Phase 38 声明已修（源指针字段置空防双重释放） | **待证** | 该用例从未纳入防线（`工程债务维护方案.md:503` 明载"因触发内存泄漏报告未纳入"）。**迁移前应补该用例**，否则移动构造的内存语义无回归锚 |
| 6.12 | 消元测试时临时禁用查重短路（`false &&`）后忘记恢复，被 C++ e2e 当场抓回（`统一整备路线图.md:819-821`） | 人工调试残留 | 防线抓到 | **不复发（防线使然）** | 这条是**正面案例**：证明 C++ e2e 防线有效。迁移期必须优先建立等价防线，否则同类残留会静默通过 |

**跨条目归纳**（按根因分类，20 条基数）：命名冲突/mangling **2**、临时槽位 **1**、类型替换不彻底/类型硬编码 **3**、解析能力缺口 **2**、初始化列表语义 **1**、类型检查过窄 **1**、引用基底/左值语义 **2**、lexer/VM 能力缺口 **2**、内置容器布局/类型映射 **2**、降级路径未经过 **1**、实现算法差异（接受）**2**。

**共同特征**（与 `项目规模与难度评估_2026-09-14.md:230` 一致）：**A/B/C 三类都不会让编译失败、不会让测试崩溃，只会让输出"看起来对但实际错"**。其中 **6.1/6.4/6.5/6.8 四条同属"多 Pass 顺序 / A 类跨层语义失配"**，是 MoonBit 版最需要结构性防治的一族。

---

## 7. MoonBit spike 清单（依赖的语言特性 → 最小验证程序 + 判定标准）

**本节语言事实已实证**，来源为本机工具链（`moon 0.1.20260915 (2e1a46d)`，`C:\Users\liangjingwei\.moon\`）自带的核心库源码与 `.mbti` 接口——**本次勘察未写任何文件**，通过直读 `~/.moon/lib/core/` 取证。

### 7.0 三条已实证的、直接改变设计的事实

| # | 事实 | 证据 |
|---|---|---|
| F1 | **MoonBit `Int` 的 `+`/`-`/`*` 是二补数静默回绕，不 trap；全 core 无任何 checked 运算 API** | `~/.moon/lib/core/builtin/intrinsics.mbt:236`（`inspect(2147483647 + 1, content="-2147483648") // Overflow wraps around`）、`:239`（`Add for Int = "%i32_add"`）、`:264`（Sub）、`:288`（Mul）；`Int64` 同（`core/builtin/int64.mbt:216,220`）。全 core 扫 `pub fn .*(checked\|overflow\|wrapping)` 仅命中一个无关的 `Bytes::to_unchecked_string` |
| F2 | **`String` 是 UTF-16 LE**，`code_units()` 返回 `ArrayView[UInt16]`，`length()` 数的是 code unit 不是字符 | `core/builtin/string.mbt:244`（"`String` holds a sequence of UTF-16 code units encoded in little endian format"）、`:271-272`、`:582`（"MoonBit strings are UTF-16 encoded (like Java)"） |
| F3 | **`Bytes` 不可变；可变载体是 `FixedArray[Byte]`，`Bytes` 只是它的视图** | `core/builtin/bytes.mbt:16-25`：`FixedArray::unsafe_reinterpret_as_bytes(self : FixedArray[Byte]) -> Bytes = "%identity"`，注释明载"any modification to the original byte sequence will be reflected in the `Bytes` object" |

**F1 的后果（本次勘察最重要的技术结论之一）**：Vitro 解释器对 int32 的加/减/乘/除/模/取反**全部 trap 并给中文教学诊断**（`vitro_vm/src/core/executor/arithmetic.rs:11,26,41,58,81,99`；JIT 侧 `jit_templates.rs:272,287,297`）。而 MoonBit 的默认算术**静默回绕**。**若移植时直接写 `+`，溢出会从"教学 trap"变成"静默错值"——正好落进本项目防线哲学最忌讳的那一档，且现有 golden 若不含溢出用例则不会抓到。** 这是必须先 spike 的头号项。

> 与同批 `MoonBit迁移_codegen模块勘察报告20260918.md` 的 S2 结论（"已核实 `String::length` 是 UTF-16 code unit，直接照抄必错"）互为独立佐证。

### 7.1 Spike 清单

| # | 依赖的语言特性 | 最小验证程序 | 判定标准 |
|---|---|---|---|
| **S1** | **整数算术溢出语义**（F1） | 实现 `checked_add_i32`/`sub`/`mul`/`div`/`mod`/`neg`：用 `Int::to_int64`（`core/builtin/pkg.generated.mbti:637`）或 `Int64::from_int`（`:657`）加宽到 `Int64`，运算后与 `@int.MIN_VALUE`/`MAX_VALUE`（`core/int/pkg.generated.mbti:5,7`）比较，越界则返回错误。测 `2147483647 + 1`、`-2147483648 - 1`、`-2147483648 / -1`（除法溢出）、`-2147483648 % -1`（Rust 会 panic 的那个）、`-(-2147483648)` | **必须**：六个算子全部检出溢出并产生与 `arithmetic.rs:11-99` 同构的诊断；`2147483647 + 1` **不得**得到 `-2147483648`。**附带测量**：加宽路径的每指令开销（决定门 1 的 VM 热循环预算） |
| **S2** | **1MB 可变字节内存载体**（F3，对应评估报告 A3 / 门 1） | `FixedArray[Byte]` 分配 1MB，跑读写循环 + 一个 20 opcode 的最小 VM 解释 `vm_bench` 那条嵌套计数循环 | **定量**：与 Rust 版同口径对比，**慢 3× 以上即出局**（`MoonBit迁移方案评估报告20260918.md:289` 门 1）。**定性**：确认 `FixedArray[Byte]` 是否 packed（无装箱）——这是 A3 的核心未知 |
| **S3** | **字节流 ↔ 源码文本的边界**（F2） | 把 C 源按字节遍历，对含中文注释 / UTF-8 字面量的源文件跑 Lexer 的字符分类路径；再对同一份源用 `String` 的 `code_units()` 遍历对比 | **必须**：字节视角下每个字节独立可见（C 语义是字节流）；确认 `String` → `FixedArray[Byte]` 的正确转换点唯一。**风险**：若误用 `String` 做词法，中文注释会变成 1 个 code unit 而非 3 字节——`vitro_lexer` 的列号/偏移语义会整体错位 |
| **S4** | **`enum` + `match` 穷尽性**（迁移安全网） | 用 MoonBit `enum` 复刻 `ClassMember`（6 变体，`ast/decl.rs:66-103`）与 `Type` 的 C++ 变体（`:81-97`），写一个消费函数并**故意漏掉一个变体** | **必须**：编译器拒绝（穷尽性错误）。**加分项**：新增一个变体后，所有 match 点**编译失败**而非静默走 default——这是 Rust 版已有、MoonBit 需确认同等的迁移安全网 |
| **S5** | **无 `Arc` 下的结构共享**（对应 A5；本模块的具体场景） | `ClassSymbol { methods : Map[String, Array[MethodSig]] }`（`vitro_typeck/src/symbols.rs:50` 附近）在 `cpp_class_layout` 与 `cpp_overload` 之间被多处读写；用不可变 Map + 重建替代 `Arc` 式共享，测模板实例化 1000 次时的拷贝量 | **可接受**：单次编译的总分配量与 Rust 版同量级；**不可接受**：实例化轮数增长导致 O(n²) 拷贝（对应 U3#4 的 1024 轮上限场景） |
| **S6** | **无借用检查器下的递归类型替换**（`cpp_monomorph/replace.rs` 372 行） | 实现"把 `Type` 中的 `TemplateId` 递归替换为实例化后的 `Class`"，含 `:66`（保留 TemplateId 的变体）与 `:210` 两个特例 | **必须**：与 U3#5 语义一致——函数模板实例化体内的 `VarDecl` 类型替换要用保留变体。**风险**：Rust 的借用检查器曾强制作者把逻辑拆成"先 clone 再调"，MoonBit 不会给这类提示，**顺序错误不会被编译器抓到**（见 §6.4/6.5 同族） |
| **S7** | **诊断收集（不 panic、不抛）** | 用 MoonBit `error` 类型 + `raise` 收集 C++ 语义诊断（`cpp_container.rs:19-28` 现在是 `report_error(&format!(...), loc, ErrorCode)`），验证错误不逃逸出编译入口 | **必须**：诊断累积为列表且编译入口不 panic（对齐 J8"panic 零容忍"）；错误消息格式稳定可断言（§8 的对账需要） |
| **S8** | **wasm-gc 单出口 + `moon check`/`moon test` 工具链** | 把 S2 的最小 VM 编到 `wasm-gc`，在浏览器 / Node 各跑一次 | **必须**：无外部运行时依赖可运行（A1 已证于 `pptx-svg`，此处验证**本项目的**产物）。**注意**：A2（Wasmtime）**未证**，属门 2，本 spike 不含 |
| **S9** | **`.mbti` 公共接口文件形态**（§9 的发布形态） | 建一个含 2 个包的 module，用 `moon info` 生成 `.mbti`，确认公共 API 边界 | **观察**：本机工具链自带的 `.mbti` 首行是 `// Generated using moon info, DON'T EDIT IT`（`core/int/pkg.generated.mbti:1`）——即 **`.mbti` 是机器生成的接口快照，不是手写契约**。这决定 §9 的"发布形态"应产出生成物而非手写文件 |

**（待证）**：S1 的"加宽法"性能开销、S2 的 `FixedArray[Byte]` packed 行为、S5 的拷贝量——三者均需真实跑测。**S1 / S2 是阻断性的**（不合格则整个重写方案回退，与评估报告门 1 同命）。

---

## 8. 等价性验收锚点

### 8.1 现有两条 C++ 防线（**性质完全不同，必须分开用**）

| 防线 | 驱动 | 对照物 | 规模 | 是否依赖 Vitro 实现语言 |
|---|---|---|---|---|
| **防线 A：现场 Clang++ 对照** | `scripts/shadow_verify_cpp/main.go`（761 行 Go），CI `ci.yml:129` | **每次现场跑 `clang++ -std=c++14`**（`main.go:446`） | 99 例（83 目录 + 22 内嵌 − 6 重名）；`facts.json` `shadow_cpp_cases` = 99 / `shadow_cpp_match` = 95（`as_of` 2026-09-15） | **否** —— 真正的独立 oracle |
| **防线 B：锁定 golden** | `native/tests/vitro_e2e.rs:462` `test_vitro_e2e_cpp` | `cases_golden/cpp/*.out`（79 个） | 83 例（`facts.json` `cpp_e2e_cases` = 83，`as_of` 2026-09-18） | **否**（但 golden 来源有瑕疵，见 §8.3） |

**关键事实**：**shadow 驱动不读 `.out`**（`main.go` 内 `golden` / `.out` / `cases_golden` 零命中）；`.out` 的唯一消费者是 Rust E2E（`vitro_e2e.rs:128-137`）。**任务书设想的"Clang++ golden（现有 .out 全量复用）"实际只对应防线 B，而防线 B 有 8 例瑕疵。**

### 8.2 对账口径（迁移期应 diff 什么、什么格式、什么工具）

| 层 | diff 对象 | 格式 | 工具 | 判据 |
|---|---|---|---|---|
| L0 前端 | **token 流** | JSON 数组（`{type, text, line, col}`） | MoonBit `moon test` 白盒（`*_wbtest.mbt`）+ Rust 侧导出 dump | 逐 token 相等；这是**最早能定位差异**的层（与 lexer 报告的 L1 锚点衔接） |
| L1 前端 | **AST dump** | JSON（`ProgramNode` 现有 `serde::Serialize`，`ast/decl.rs:58` 等已 derive） | 两侧各 dump 后 `diff` | 结构相等；`SourceLoc` 只比 line/col 不比 file_id |
| L2 中端 | **模板实例化产物 dump diff**（任务书指定项） | JSON：`{实例化名, 模板基准, 实参表, 字段布局[], 方法表[], vtable 槽[]}` | 从 `program.classes` / `program.templates` dump（Rust 侧已有 `serde` derive） | **U3#5/#8 的回归锚**：`vitro_vec<Point>` 的字段/大小/方法集必须逐字段相等；**实例化集合（名字集合）必须相等**——这条能直接抓 §6.7/6.8 两类"误拒 / 漏实例化" |
| L3 后端 | **字节码**（含索引段） | 现有 `vitro_cli export -o bundle.json` 产物 | 两侧字节码 dump + `bytecode_libc_index` 的 digest | 指令序列相等；`BYTECODE_LIBC_BASE_INDEX` / `FUNC_COUNT` 相等。**这是最严格也最脆的锚**（若新 codegen 有意改进，需显式裁定放弃该锚）。与 codegen 报告的"产物 JSON 逐字节 diff"锚点衔接 |
| L4 端到端 | **程序 stdout** | 纯文本 | 防线 A（现场 clang++）+ 防线 B（golden） | 逐字节相等。**注意 E-P1-5 口径**：只读纯程序 stdout 通道，禁止文本清洗（`main.go:533`） |
| L5 运行期 | **vtable 全局数据段** | 内存 dump | 两侧各 dump 全局区 | vtable 槽位顺序相等（`struct_.rs:203` 靠位置定位，**顺序即 ABI**） |
| L6 失败面 | **错误码 + 诊断文本** | 现有 `ErrorCode` 枚举（`vitro_shared/src/error_codes.rs`） | 对 §2.3 的 E4001~E4017 逐码回归 | 错误码相等；文本可放宽 |

### 8.3 现有锚点的**四处必须修补**（否则对账不成立）

| # | 问题 | 影响 | 处置建议 |
|---|---|---|---|
| 8.3.1 | **4 例无 golden**：`cpp_copy_ctor` / `cpp_default_args` / `cpp_nested_class_instance` / `cpp_nttp_class`（83 `.cpp` vs 79 `.out` 差集；反向差 0） | Rust E2E 下**只校验退出码**（`vitro_e2e.rs:186` 无 else 分支），stdout 不比对。其中 2 例连期望输出都无任何记载 | **迁移前补齐**：用防线 A 现场 clang++ 生成真 golden，并给 `vitro_e2e.rs` 加"golden 缺失即 fail-loud"（对应 J7 保险丝义务） |
| 8.3.2 | **4 例 golden 系手写 / Vitro 自证**：`cpp_vitro_vec_class` / `cpp_vitro_list_class` / `cpp_u3_class_instantiate_in_template` / `cpp_u3_vec_class_twice` —— 内容与 Vitro stdout 逐字节相同，而 Clang++ 编译不过（report 内 `error: no template named 'vitro_vec'`） | 用它们验收 MoonBit 版是**循环论证** | 这 4 例依赖 Vitro 专有容器名，**Clang 原理上无法对照**。建议：① 在 C++ 侧改写成标准 `std::vector`/`std::list` 等价物以取得真 Clang 对照；或 ② 显式登记为"无独立 oracle"，仅作**回归锚**（防行为漂移）而非**等价锚**（证与标准一致） |
| 8.3.3 | `CPP_FAILURES.md:43` 称"Golden 全部由 Clang++（`-std=c++14 -O0`）生成"——与 8.3.2 矛盾，且驱动实际参数**无 `-O0`**（`main.go:446`） | 文档失真，掩盖 8.3.2 | 按 8.3.2 处置后同步修订该句 |
| 8.3.4 | **`category: gap` 使差异不触红**：`main.go:704-705,758-760` 只对非 gap 用例 `exit 1`。现有 **7 条** gap 用例（内嵌 3：`main.go:273,335,364`；目录 4） | 3 条内嵌 gap 用例当前是 `match`，**未来回归不可见** | 迁移期把"gap"语义收敛为**显式豁免清单 + 逐例理由 + 到期复核**，而非首行注释；并对现有 7 条做逐例审计（对齐 `AGENTS.md` 的 J2 闭环对 C 侧 16 例做过的审计） |

### 8.4 锚点复用结论

- **防线 A（99 例现场 clang++）可整体复用**，只需把 Vitro 侧调用从 `capi` 换成 MoonBit 导出的等价入口。**这是最有价值的可复用资产（§2.4）**。
- **防线 B 的 79 个 golden 可复用**（语言中立），但**必须先修 8.3.1 与 8.3.2**。
- **迁移期新增的 L2"模板实例化产物 dump diff"是任务书指定的锚**，也是唯一能直接覆盖 U3#5/#8 的锚——Rust 侧现有 `serde` derive 使 dump 成本很低。

---

## 9. mooncakes 包切分草案

### 9.1 建议切分（5 个包，依赖单向）

```
vitro/cpp_layout          ← 无依赖（除 std）     容器布局 JSON 加载 + 类型映射
      ↑
vitro/cpp_ast             ← 依赖 vitro/ast        C++ AST 节点（ClassMember/Template…）
      ↑
vitro/cpp_parser          ← 依赖 ast, lexer        C++ 词法关键字 + 语法（class/template/引用/new）
      ↑
vitro/cpp_typeck          ← 依赖 ast, cpp_layout   类布局 / 重载决议 / 模板单态化
      ↑
vitro/cpp_codegen         ← 依赖 ast, cpp_typeck   字段偏移 / new-delete / RAII / vtable
```

| 包 | 内容对应现状 | 建议依赖 | 备注 |
|---|---|---|---|
| `vitro/cpp_layout` | `vitro_cpp_frontend`（186 + 502 JSON）**整包平移** | 无（只依赖 `vitro/ast` 的 `Type`） | **本模块唯一可"整包平移"的部分**。JSON 作为包内资源（`include_str!` 等价物）；`.cpp` 真源留在仓库但**不进包** |
| `vitro/cpp_ast` | `vitro_ast` 的 C++ 变体 | `vitro/ast` | **建议不单独成包**，而是把 C++ 变体并入 `vitro/ast`（与现状一致）。理由：`Type::Class/TemplateId` 与 C 类型同处一个 `enum`，拆开会造成循环依赖。**若坚持拆包**，需先把 `Type` 做成可扩展（MoonBit 无开放 enum，代价高） |
| `vitro/cpp_parser` | `vitro_lexer` 的 C++ 关键字 + `vitro_parser/src/cpp.rs` + 各共享文件里的 C++ 分支 | `vitro/ast`、`vitro/lexer` | 对应 `TODO(#D08)`（`lexer/lib.rs:5`、`parser/lib.rs:5`）的诉求，一次到位 |
| `vitro/cpp_typeck` | `vitro_typeck` 的 `cpp/` + `cpp_monomorph/` + `cpp_class_layout.rs` + `cpp_container.rs` + `cpp_auto.rs` + `cpp_overload.rs` + `expr/cpp.rs`（≈3 534 行） | `vitro/ast`、`vitro/typeck`、`vitro/cpp_layout` | 对应 `TODO(#D10)`（`typeck/cpp/mod.rs:5`） |
| `vitro/cpp_codegen` | `vitro_codegen` 的 `cpp/` + `stmt/cpp.rs`（483 行）+ `vitro_runtime/bytecode_libc_sig.rs` | `vitro/codegen`、`vitro/ast`、`vitro/cpp_typeck` | 对应 `codegen/cpp/mod.rs:5` |

### 9.2 依赖方向原则

- **单向、无环**：`layout → ast → parser → typeck → codegen`。现状**基本符合**（实测消费点 `cpp_container.rs:14-15`、`cpp_class_layout.rs:495-496`、`codegen/lib.rs:307-308`、`cpp/range_for.rs:2` 全部指向 `vitro_cpp_frontend`，方向正确）。
- **`cpp_layout` 必须保持零业务依赖**（现状成立：`Cargo.toml` 只依赖 `vitro_ast` + serde）。
- **`cpp_ast` 建议不独立**（见上表），以 `vitro/ast` 内分区形式存在。

### 9.3 对上发布形态

| 项 | 建议 |
|---|---|
| 公共接口 | 用 **`moon info` 生成的 `.mbti`**（本机实证：首行是 `// Generated using moon info, DON'T EDIT IT`，`core/int/pkg.generated.mbti:1`）——即**发布生成物，不手写契约** |
| 对外可见面 | **只有 `vitro/cpp_layout` 需要公开**（数据层，无内部实现细节）。`cpp_parser` / `cpp_typeck` / `cpp_codegen` 建议**不单独发布**，作为引擎内部包（对上游只暴露"编译一个 C++ 子集源"这一个入口） |
| 数据集 | `builtin_layout_data.json` 与 `runtime_libc/vitro/*.cpp` 是**成套事实**，发布时须同版本绑定；建议 `.cpp` 随包发布（人可读的真相源），JSON 作为生成物 |
| 版本策略 | 与引擎主版本联动；`.mbti` 变更即视为 breaking（它是被消费的接口快照） |

---

## 10. 特别要求：两种方案的规模估算与裁定材料

> **定位声明**：本节只提供裁定材料，**不做裁定**（评估报告 `MoonBit迁移方案评估报告20260918.md:180-181` 将决定权归项目）。

### 10.1 砍方案（砍掉 C++ 子集）的影响面

**收益（可量化的耦合删除）**

| 项 | 量 | 证据 |
|---|---|---|
| 删除 C++ 专属 crate | 186 行 Rust + 502 行 JSON | §1.1 |
| 删除前端 C++ 面 | ≈1 701 行（lexer/parser/ast） | §1.2 |
| **`is_cpp_mode` 标志彻底消失** | **43 处 / 9 文件**，全部在 lexer+parser+engine 入口 | §1.3 |
| 删除中后端 C++ 面 | typeck ≈3 534 行 + codegen 483 行 + runtime 108 行手写 | §1.2 |
| **C 管线变干净** | **成立**：`register_class_layouts`（`typeck/lib.rs:173`）与 `check_class_methods`（`:371`、`:418`）是**无条件调用**的，砍掉后 C 编译路径不再进入 C++ 代码 | §1.3 |
| 删除 AST 变体后的连带收益 | `Type::Class/Reference/RValueRef/Auto/TemplateId`、`ClassMember` 6 变体、`TemplateParam/Arg/Templateable/TemplateDecl/TemplateInstantiation/VTable`、`Expr::{This,MemberCall,New,Delete,Lambda,Move}`、`Stmt::{RangeFor,Try}` —— 所有下游 match 少若干臂 | `ast/decl.rs:59-207`、`types.rs:81-97`、`expr.rs:194-235`、`stmt.rs:77-89` |
| **VM 收益 = 0 行** | VM 零 C++ 感知（3 处偶然命中且无关） | §1.4 |
| 删除测试与防线 | 5 个驱动 3 565 行 / 142 `#[test]`；83 用例 + 79 golden；`shadow_verify_cpp` 761 行 Go；`extract_cpp_builtin_layout.py` 348 行 | §2.5、§2.6 |

**净减少量估算**：Rust **≈6 500 行（8.5%）** + JSON 502 + C++ 真源 359 + 测试驱动 3 565 + 文档（`CPP_FAILURES.md` 146 + `DOGFOODING_FAILURES.md` 58 + `C++子集规范.md` 409 + `C++拓展实施计划.md` 1 614 ≈ 2 227 行）≈ **合计约 1.3 万行**。

**成本与风险（这是裁定的关键，不是收益）**

| # | 风险 | 证据 | 量级 |
|---|---|---|---|
| R1 | **砍掉的是 C# 系列（CS2/CS3）的地基**。项目已裁定"**CS2 直接复用 C++ 高速铺路区（类布局/this 注入/单态化/raii 插桩）**"，且 U3 的验收标准明确含"**CS2 复用路径（类布局/this 注入/`raii.rs`/容器加载器）专项审查报告归档**" | `统一整备路线图.md:153`、`:167` | **决定性** |
| R2 | 丢失 1 614 行的 C++ 设计文档与 409 行的对外语义规范 | `C++拓展实施计划.md`、`C++子集规范.md` | 高（重新设计成本） |
| R3 | 丢失 99 例现场 clang++ 对照 + 83 例 E2E + 142 单元测试的**语义回归网** | §8 | 高 |
| R4 | `is_cpp_mode` 虽是 43 处，但**集中在 lexer/parser**，删除收益有限（1 701 行）；而 typeck/codegen 的 4 017 行是**独立模块**，删除对 C 路径的实际收益仅是"3 个无条件 Pass 调用" | §1.3 | 中（收益被高估的风险） |

**砍方案的结论性事实**：**收益 ≈ 8.5% 代码量 + 3 个无条件 Pass 调用；代价 = 让出 CS2/CS3 的复用地基（已被项目自身裁定为复用路径）。** VM 侧无收益。

### 10.2 搬方案的最小依赖序

**拓扑序（自底向上，6 层）**

| 层 | 内容 | 可独立验证？ | 备注 |
|---|---|---|---|
| **L1** | `vitro_cpp_frontend` 布局加载器（186 行 + JSON） | ✅ 独立（零业务依赖） | **最先搬**。JSON 与 `.cpp` 真源已核对无漂移；只需修生成链（4.1.9） |
| **L2** | C++ AST 节点（`ast/decl.rs:59-207`、`types.rs:81-97`） | ✅ 可（结构断言） | 已实测无漂移，纯 enum |
| **L3** | lexer C++ 关键字 + parser C++ 语法（`parser/cpp.rs` + 37 处 `is_cpp_mode`） | ✅ 可（token / AST dump diff，§8 L0/L1） | 依赖 L2 |
| **L4** | typeck：类布局 → 重载决议/mangling → 模板单态化 | ⚠️ 部分（L2 dump diff） | **内部还有序**：`cpp_class_layout` → `cpp_overload` → `cpp_monomorph` |
| **L5** | codegen：字段偏移 → new/delete → RAII → ctor 语法 → range-for | ⚠️ 部分（L3 字节码 diff） | 依赖 L4；RAII 依赖类布局的 `has_resource` |
| **L6** | runtime：容器签名与索引（`bytecode_libc_sig.rs` 108 手写 + 生成 205）+ AOT 预编译链 | ⚠️ 需 L5 产出的字节码 | **最后搬** |

**"哪些 C++ 能力可以最后搬"**（按依赖深度排序，尾部可独立延后）

| 顺序 | 能力 | 依赖 | 延后风险 |
|---|---|---|---|
| 1（最先） | 布局加载器、AST 节点、关键字/基本类语法 | 无 | 低 |
| 2 | 类布局 + 字段/成员访问 + `this` | L2 | 低 |
| 3 | 构造/析构 + RAII（四类控制流） | 类布局 | 中（内存语义无锚，见 §6.11） |
| 4 | 重载决议 + mangling | 类布局 | 中（E4031 限制已是对外契约） |
| 5 | 模板单态化（含 U3#4/#5/#8 三护栏） | 重载决议 | **高**（OOM 护栏是安全项） |
| 6 | 内置容器（`vector`/`list`/`string`/`sort_int`）+ 类类型模板实参 | 单态化 + 布局加载器 | **高**（4 例 gap 用例与 U3 锚所在） |
| 7（最后） | **虚函数 vtable 动态分派** | 类布局 + 全局数据段分配（`codegen/lib.rs:462-476`） | 中（槽位顺序即 ABI，§8 L5） |
| 7（最后） | **lambda（含闭包类合成）** | 类布局 + 引用捕获 | 中（`cpp_lambda_test.rs` 仅 3 例；另见 4.1.12） |
| 7（最后） | **NTTP / 嵌套类实例化 / 自定义拷贝构造 / 默认参数**（最新 4 项特性） | 单态化 | **高**（这 4 项**恰好就是无 golden 的 4 例**，见 §8.3.1） |
| 7（最后） | `unique_ptr<T>` 简版、范围 for、引用/移动语义 | 上述 | 低 |

**"可最后搬"的收敛答案**：**虚函数 vtable、lambda、以及"最新 4 项特性（NTTP / 嵌套类 / 拷贝构造 / 默认参数）"可以最后搬**——但**最后一项恰恰是防线最薄的地方**（无 golden）。这构成一个**顺序陷阱**：越晚搬的能力，验收锚越弱；因此**建议把"补齐这 4 例的 golden"提前到 L1 阶段**（它与 C++ 代码无关，纯防线工作）。

### 10.3 两方案规模估算对照

| 维度 | 砍 | 搬 |
|---|---|---|
| Rust 代码量变化 | −6 500 行（−8.5%） | 0（保留参考实现） |
| 新增 MoonBit 代码量估算 | 0 | **≈5 500–7 000 行**（按 Rust 6 514 行折算；含 JSON 加载器约 200 行，不含测试） |
| 需重建的测试/防线 | 删除（−3 565 行驱动 + 162 用例） | **复用为主**：99 例 shadow（改调用入口）+ 83 E2E + 79 golden（补 4 例）；142 个 `#[test]` 需按 MoonBit 重写（估 **2 500–3 500 行**） |
| C 管线清洁度 | **提升**（3 个无条件 Pass 去除 + 43 处模式标志消失） | 不变（但可用 §5.4 把 Pass 短路，取得同等收益而不砍） |
| VM 影响 | 0 | 0（C++ 与 VM 完全解耦，§1.4） |
| 对 CS2/CS3 的影响 | **失去已裁定的复用地基**（`统一整备路线图.md:153,167`） | **保留并可用 MoonBit 形态加固**（§5.2/§5.3） |
| 主要技术风险 | 语义回归网消失；1 614 行设计资产作废 | 字符串命名体系重构（§3.2）、溢出语义反转（§7 F1）、1MB 内存载体性能（§7 S2） |
| 关键阻断项 | 需重新裁定 CS2 复用路径 | S1/S2 两个 spike 不合格则整体回退（与评估报告门 1 同命） |

### 10.4 供裁定使用的三条硬事实（不带倾向）

1. **C++ 与 VM 完全解耦**（VM 零 C++ 感知）：**C++ 的去留不影响 VM 迁移的规模与风险**——这条把两个决定解开了。
2. **`is_cpp_mode` 只在 lexer/parser**（43 处），而 **C++ 的 typeck/codegen 部分（4 017 行）是无条件挂在 C 管线上的独立模块**：砍它主要省"3 个 Pass 调用"，不省"C 管线的复杂度"。
3. **项目已用文字裁定 CS2 复用 C++ 铺路区**（`统一整备路线图.md:153`、`:167`）：**砍 C++ 与 CS2 的既定复用策略直接冲突**，这一条的影响面超出本模块，需与 CS 系列计划合并裁定。

---

## 附录 A · 待证清单（8 项，各附验证方法）

| # | 待证事项 | 验证方法 |
|---|---|---|
| A-1 | 评估报告"三分之一量级"（`:180`）用的是**行数**口径还是 **Phase 数**口径？本次实测两者相差 3 倍（8.5% vs 26%） | 向报告作者确认；或按 `:166-173` 的规模表反推其言据 |
| A-2 | "`CPP_FAILURES.md` 历史累计约 40 条"（`reports/项目规模与难度评估_2026-09-14.md:195`）的**计数规则** | 该文件未给算法；本次机械复算得 23/32/33/55 四组，均非 40。需向作者确认或改记 20（人工归类） |
| A-3 | 4 例手写 golden（`cpp_vitro_vec_class` 等）的**实际生成者** | `git log -p --follow -- native/tests/cases_golden/cpp/cpp_u3_vec_class_twice.out`（本次勘察禁止 git 操作） |
| A-4 | 4 例缺 golden 是**有意还是遗漏** | `git log --diff-filter=D -- native/tests/cases_golden/cpp/`；`git log --stat -- native/tests/cases/cpp/cpp_nttp_class.cpp` |
| A-5 | `Type::Function.is_const`（`ast/types.rs:69`）是否为**死字段**（无任何生产者置 true） | 全仓 grep 该字段的写点；若确认无写点则登记为死字段 |
| A-6 | `C++拓展实施计划.md:57` 声明的简化版 `shared_ptr` **是否有实现或用例** | 83 例与 5 驱动中未见；`grep -rn "shared_ptr" native/ scripts/`（排除 target） |
| A-7 | `cpp_move_semantics` 的内存泄漏是否已随 Phase 38 消除 | 取回该用例（`工程债务维护方案.md:503` 记其因泄漏未纳入）复跑 |
| A-8 | `统一整备路线图.md:285` 的"lambda 双 E3014 重复声明"能否复现 | 按该行记述的来源方复测条件重跑 |

## 附录 B · 勘察边界与子勘察状态（诚实记录）

- **本次勘察为纯只读**：未创建或修改任何文件，未执行 git 操作，未运行会写缓存的 `shadow_verify*`。全部"当前值"引 `reports/facts.json` 的 key + `as_of`；文档内 AS-OF 冻结数字均已标注来源；`docs/archive/` 未被引用。
- **4 份子勘察的状态**：5 个后台子代理中 **3 个交付**（文档裁定框架与在途工作、parser/lexer/ast 全量清单、测试防线与坑史），**2 个在产出报告前被中止**（typeck 语义面、codegen/VM 面）。这两块内容已在本文中以第一手读码覆盖（`cpp_monomorph/builtin.rs`、`synth.rs`、`cpp_overload.rs`、`cpp_class_layout.rs`、`cpp/raii.rs`、vtable 机制、`bytecode_libc_*`），但**未做全量逐文件清点**——若裁定需要 typeck/codegen 的逐文件行级清单，需补一次定向勘察。
- **与同批报告的重叠与互补**：本文的 §1.3/§1.4（`is_cpp_mode` 分布、VM 零 C++ 感知）为独立实测，与 `MoonBit迁移_codegen模块勘察报告20260918.md` 的槽位机制结论互补；§7 F2（String 为 UTF-16）与该报告 S2 互为独立佐证；§8 L0/L1/L3 锚点分别与 lexer 报告、shared/ast 报告、codegen 报告的锚点分层衔接。
