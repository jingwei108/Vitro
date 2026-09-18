# MoonBit 迁移 · `vitro_typeck` 模块勘察报告

> 勘察时间：2026-09-18
> 对象：`D:\code\Vitro` @ `4b57191`（master，工作区 dirty）—— `native/crates/vitro_typeck/` 全部源码（35 个 `.rs`，含 `decl.rs`——工程债务 D14 / D16 所在）
> 性质：**只读勘察**。未修改任何文件、未执行任何 git 操作、未编译（无 cargo 调用）。
> 数字口径（**本文件为 AS-OF 冻结**，文件名含日期）：本文所有裸数字均为勘察时点快照，不参与 `facts` 回填；引用全局真值一律标 `reports/facts.json` 的 key 与 `as_of`（该文件 `generated_at` = 2026-09-18T13:01:41+08:00，`git.rev` = `4b57191`）。标注「本次实测」（2026-09-18）的为本轮现场计数。
> 未标「待证」的行为学结论均为静态代码路径读出，已在附录给出可复现验证法。

---

## 0. 前置校正：三条任务前提与代码事实不符（先于一切结论）

按防线哲学第 0 条（禁止粉饰），先纠正三条硬前提——它们直接决定后续工作量估算。

| # | 任务书前提 | 代码事实 | 影响 |
|---|---|---|---|
| **P-1** | 「grep `is_cpp_mode` **在本 crate 的分布**，全仓库 46 处」 | `is_cpp_mode` 在 `vitro_typeck` **出现 0 次**；全仓 **46 次**（本次实测精确计数）**全部不在本 crate**：`vitro_lexer/src/lib.rs` 6、`vitro_parser`（`lib.rs` 11 / `decl.rs` 8 / `type_.rs` 6 / `stmt.rs` 4 / `expr/primary.rs` 3 / `cpp.rs` 2 / `expr/unary.rs` 2 / `expr/postfix.rs` 1）、`native/src/engine/compile_pipeline.rs` 3 | 「typeck 承担语言分派」的定性**不成立于布尔标志维度**；typeck 的语言性由 **AST 节点形状 + `Type` 形态 + 名字前缀**隐含携带（见 §①-3） |
| **P-2** | 「职责**四合一**：命名解析 / 类型检查 / C++ 模板单态化 / 语言分派」 | 前三项成立；**语言分派不成立**。真正的第四项职责是 **AST→AST 改写（lowering）**：本 crate 有 **88 处 AST 字段就地赋值**（`*ty =` 47、`*expr =` 11、`*var_type =` 9、`g.ty =` 4、`dims[0] =` 3、`*array_size =` 3，其余 11）+ `set_ty()` 1 处 + `insert_implicit_cast` 插入 `Expr::Cast` 节点 | 迁移目标不是「移植一个检查器」，而是**移植一个 lowering pass**——这决定 §⑤ 与 §⑨ 的形态 |
| **P-3** | 「`decl.rs` D14：3 处 `unwrap/expect`」 | **代码侧已修复，台账未销项**。全 crate `\.unwrap\(\)` 命中 **0**；原 L44 / L65 已是 `let Some(source) = init.take() else { return false; };`（`decl.rs:46`、`decl.rs:69`），原 L648 已是 `let Some(d) = default else { continue };`（`decl.rs:706`），并带 D14 注释（`decl.rs:44-45`、`decl.rs:704-705`）。旁证：`reports/engineering_health.md`（生成时间 2026-09-18T01:35:04）「Rust unwrap/expect（生产代码）」= **1**（全项目，且不在 typeck） | §④ 中 D14 的处置是**销项 + 补留痕**，不是「修」 |

**顺带纠正（任务书重点勘察项的归属）**：**U3#2（变参 8 字节槽）不在 typeck**。`vitro_typeck` 内 grep `temp_slot|slot0|get_temp_slot` = **0 命中**（本次实测）；实修点在 `vitro_codegen/src/expr/call.rs:172-201`。typeck 侧只负责变参**默认实参提升**（`convert.rs:67-74`）与个数放宽（`decl.rs:752-753`、`decl.rs:790-791`）。

---

## ① 模块概览与规模

### 1.1 文件与行数（2026-09-18 实测，`Get-Content .Count`）

35 个 `.rs`，合计 **8,816 行**。行数口径：本表为**总行数**；`reports/engineering_health.md`（2026-09-18）用**非空行**口径，两者对 `decl.rs` 分别为 862 / 842，可交叉核对。

| 文件 | 总行 | 非空行（health） | 职责 |
|---|---|---|---|
| `decl.rs` | **862** | **842**（全仓 rank 3） | 函数/语句 visitor、构造函数初始化改写、`check_user_func` 四级回退 |
| `lib.rs` | 711 | 663（rank 14） | `TypeChecker` 结构 + `check()` 13 个 Pass 编排 |
| `cpp_monomorph/builtin_list.rs` | 572 | 556（rank 19） | **Rust 硬编码合成 `vitro_list<T>`（T = 类类型）** |
| `cpp_class_layout.rs` | 535 | — | 类布局 / vtable / 嵌套类 / mangled 符号注册 |
| `convert.rs` | 511 | — | 隐式转换表、`check_assignable`、左值判定 |
| `expr/call.rs` | 483 | — | 调用 / 成员调用 / lambda / builtin 分派 |
| `cpp_overload.rs` | 461 | — | 重载决议、ctor mangling、隐式移动构造生成 |
| `init.rs` | 388 | — | 初始化器检查、**数组尺寸推断（改写 AST）** |
| `expr/ops.rs` | 373 | — | 二元 / 三目 / 一元 / 赋值 / `_Generic` |
| `cpp_monomorph/replace.rs` | 372 | — | 模板参数替换（类型 / 语句 / 表达式） |
| `builtin/mod.rs` | 336 | — | `visit_call` 硬编码 62-arm 分派 + printf/scanf 格式 |
| `cpp_monomorph/builtin_vec.rs` | 323 | — | **Rust 硬编码合成 `vitro_vec<T>`（T = 类类型）** |
| `expr/cpp.rs` | 306 | — | `new` / `delete` / `move` / lambda |
| `cpp/methods.rs` | 252 | — | `method_mangled_name` 单源 + 重载评分 |
| `expr/mod.rs` | 217 | — | `resolve_expr_type` 总入口 + 类方法名隐藏 |
| `builtin/io.rs` | 213 | — | printf / scanf / fprintf / sprintf… |
| `expr/var.rs` | 200 | — | 标识符 / 索引 / 成员 / `this` |
| `builtin/file.rs` | 181 | — | VFS 文件 I/O |
| `cpp_monomorph/func.rs` | 158 | — | 函数模板单态化 + `mangle_template_name` |
| `cpp/lambda.rs` | 146 | — | lambda capture 改写为 `this->field` |
| `builtin/string.rs` | 135 | — | str* / mem* |
| `context.rs` | 122 | — | 作用域、`declare_var`、`compute_type_size` |
| `decl_types.rs` | 114 | — | auto / typeof / 限定符（**D16 拆出的文件**） |
| `builtin/math.rs` | 107 | — | 8 个数学函数 |
| `cpp_monomorph/class.rs` | 106 | — | 类模板单态化 |
| `expr/literal.rs` | 106 | — | 字面量 / 初始化列表 / 复合字面量 |
| `cpp_auto.rs` | 91 | — | `auto` 推导 |
| `builtin/memory.rs` | 86 | — | malloc / free / realloc / calloc / memset / exit |
| `cpp_monomorph/synth.rs` | 76 | — | 合成 AST 小工具 + U3#8 占位 decl |
| `symbols.rs` | 75 | — | 7 个符号结构（纯数据） |
| `expr/cast.rs` | 62 | — | sizeof / alignof / cast |
| `cpp_container.rs` | 59 | — | 内置容器方法 → 宿主函数降解 |
| `cpp_monomorph/builtin.rs` | 50 | — | 容器合成入口 + U3#8 查重前置 |
| `cpp_monomorph/mod.rs` | 18 | — | 子模块声明 |
| `cpp/mod.rs` | 9 | — | 子模块声明 |
| **合计** | **8,816** | | |

**规模定性**：`decl.rs`（842 非空行）超 800 行阈值；`cpp_monomorph/builtin_list.rs`（556）+ `builtin_vec.rs`（323）合计 **895 行**占本 crate 的 10.2%，且是**手写硬编码的容器实现**（见 §③-3.3 与 §⑤-5，与 AGENTS.md Phase 41「零 Rust 硬编码」存在口径差）。

### 1.2 关键类型（定义行号）

- **`TypeChecker`**（`lib.rs:26-64`）—— **27 个字段**，全部为可变状态：
  - 符号表 8 张：`funcs`@32、`static_func_sigs`@33、`static_func_files`@34、`static_global_files`@35、`structs`@36、`unions`@37、`classes`@38、`templates`@39
  - 作用域：`scopes: Vec<HashMap<String, VarSymbol>>`@40
  - 上下文标量：`current_func_return`@41、`current_file`@42、`loop_depth`@43、`switch_depth`@44、`current_func_params`@45、`func_labels`@46、`pending_gotos`@47、`current_class: Option<String>`@48、`current_method_is_const`@49
  - **模板实例化四件套**：`pending_instantiations`@51、`pending_class_instantiations`@53、`instantiated_class_names`@56、`pending_lambdas`@58
  - 诊断 3 个 `Vec`：`errors`@29、`warnings`@30、`hints`@31
  - **可变抑制开关**：`char_narrow_suppress: bool`@63（跨函数置位/复位，见 §③-2.2）
  - 库模式：`is_library_mode: bool`@28（唯一使用点 `cpp_class_layout.rs:492`）
- **`TypeError`**（`lib.rs:18-24`）：`{ message: String, line: i32, column: i32, code: i32 }` —— **无 `file_id`、无 span 长度、无 severity、无结构化参数**。
- **符号结构**（`symbols.rs`）：`VarSymbol`@5、`FuncSymbol`@14、`StructSymbol`@23（**只有 `fields`，无 size / offset**）、`MethodSig`@28、`ClassSymbol`@42、`TemplateSymbol`@58、`LambdaInfo`@64。
- **入口**：`TypeChecker::new(is_library_mode: bool)`（`lib.rs:101`）、`check(mut self, &mut ProgramNode) -> (Vec<TypeError>, Vec<TypeError>, Vec<TypeError>)`（`lib.rs:108`）。

### 1.3 `check()` 的 13 个 Pass（`lib.rs:108-491`）

```
Pass 1      struct/union 注册 + T-P0-8 值成员循环包含检测      lib.rs:110-170
Pass 1.5    类布局注册（register_class_layouts）                lib.rs:173 → cpp_class_layout.rs:5
Pass 1.6    类外方法定义合并                                     lib.rs:179
Pass 2      函数签名注册                                         lib.rs:182-218
Pass 2.4    类静态字段 → 全局变量（mangled `Class__field`）      lib.rs:225-250
Pass 2.6    模板注册                                             lib.rs:253-267
Pass 2.65   显式模板类实例化（`template class X<int>;`）         lib.rs:271-283
Pass 2.5    全局变量注册 + 初始化检查（auto/typeof 先定型）      lib.rs:286-354
Pass 3      函数体检查                                           lib.rs:357-361
（drain）   Pass 3 期间发现的类实例化入 program.classes          lib.rs:365-368
Pass 3.5    类方法 / 构造 / 析构体检查                           lib.rs:371
Pass 3.55   隐式移动构造生成                                     lib.rs:374
Pass 3.6    ★实例化收敛循环（含 1024 轮上限）                    lib.rs:387-421
Pass 4      lambda 提升为 ClassDecl + FuncDecl                   lib.rs:425-488
```

### 1.4 依赖与对外契约

- **Cargo 依赖**（`Cargo.toml:7-10`）：`vitro_shared`、`vitro_ast`、`vitro_lexer`、`vitro_runtime`、`vitro_cpp_frontend`。
  - ⚠️ **`vitro_lexer` 是幽灵依赖**：全 crate grep `vitro_lexer|Lexer` = **0 命中**（本次实测）。迁移时可从依赖图砍掉。
  - **无 `[lints]` 段**：`unwrap_used = "deny"` 只在根包 `native/Cargo.toml:19`，工作区无 `[workspace.lints]` → 该纪律对被拆出的子 crate 不生效。
- **诊断契约**：本 crate 引用 **78 个不同的 `ErrorCode`**（253 处引用，本次实测）；`vitro_shared/src/error_codes.rs` 共 136 个变体。这 78 个码是三个出口共享的 wire 契约（`error_catalog` 消费）。
- **AST 依赖**：`vitro_ast::Type` **17 个变体**（`types.rs:32-99`：Void / Int / Char / Float / Double / LongLong / Pointer / Array / Function / Struct / Union / Class / Reference / RValueRef / Auto / TemplateId / Typeof），`TypeKind` 16 个（`types.rs:9-28`）。

### 1.5 测试资产（2026-09-18 实测）

| 资产 | 数量 | 位置 |
|---|---|---|
| crate 内联测试（纯 `implicit_cast_target` 单测） | **10** | `lib.rs:622-710` |
| `type_checker_unit_test.rs` | 23 test / 29 assert | `native/tests/` |
| `typeck_cpp_unit_test.rs` | 31 test / 36 assert | 同上 |
| `typeck_e3053_regression_test.rs` | 4 test / 7 assert | 同上 |
| `typeck_u1_p0_regression.rs` | 12 test / 22 assert | 同上 |
| `crash_regression_tests.rs`（共享，含 2 个 U3 锚） | 48 test / 72 assert | 同上；U3 锚在 `:795`、`:814` |
| C++ E2E 用例 | **83**（`reports/facts.json` → `cpp_e2e_cases`，as_of 2026-09-18） | `native/tests/cases/cpp/*.cpp` |
| C++ Shadow 用例 | **99**（`shadow_cpp_cases`，as_of 2026-09-15；match 95 = `shadow_cpp_match`） | 22 条内嵌于 Go 驱动 + 83 目录文件；去重并集 = 99（本次实测核对一致） |
| C Shadow 用例 | **675**（`shadow_c_cases`，as_of 2026-09-15；match 668） | C E2E 合计 359 + 15 + 81 + 138 = 593（`c_e2e_*`） |

---

## ② 可复用资产清单

> 成本口径：**低** = 语义可 1:1 表达为 MoonBit，照抄判据表即可；**中** = 需重新设计数据结构但语义不变；**高** = 依赖 Rust 特有能力或需重建机制。

### 2.1 语义设计（可直接搬的「判据」）

| # | 资产 | 位置 | 成本 | 理由 |
|---|---|---|---|---|
| A1 | **隐式标量转换规则表**（12 条臂，纯函数 `(from,to) → Option<Type>`） | `convert.rs:6-45` | **低** | 纯查表 + `match`，MoonBit `enum + match` 更自然；穷尽检查还能自动发现漏臂 |
| A2 | `insert_implicit_cast` 的 **FloatLiteral→double 重定型特例** | `convert.rs:47-63` | **低** | 「不插 Cast 只改节点类型」是 12 行逻辑；但依赖 AST 可变（见 §⑤-3） |
| A3 | **C 默认实参提升**（变参：float→double、char→int） | `convert.rs:67-74` | **低** | 12 行，纯语义 |
| A4 | **T-P0-8 值成员循环包含检测**（沿值语义成员展开，指针/引用不构成环） | `lib.rs:495-517` | **低** | 递归 + 路径 `Vec[String]`，可直译；这是「学生写链表节点漏 `*`」的核心教学防线 |
| A5 | **W0-4 char 窄化豁免判据**（字符常量 / char 值域内整常量） | `lib.rs:591-599` + 6 处置位点 | **中** | 判据本体 9 行；但**承载方式是 `self` 上的可变开关**（`lib.rs:63`），MoonBit 无隐式 `&mut self` 状态 → 必须改显式上下文参数（§⑤-8） |
| A6 | **U1#12 数组尺寸三形状判据**（`< -1` 负尺寸 / `-1` 无初始化器 / `== 0` 显式零） | `lib.rs:556-585` | **低** | 判据清晰；但**同族判据在 `init.rs` 有第二份**（§③-3.3），迁移前必须先合流 |
| A7 | **`type_mangle_suffix` 类型编码方案**（ASCII、稳定、可读：`v/i/u/l/c/f/d/P/R/S/A/T/F`） | `cpp/methods.rs:96-130` | **低** | 34 行纯递归；**本模块最值得原样保留的机制**。注意 `Type::Typeof` 落到 `"y"`（`:128`）是无损的，比 `mangle_name_into` 好（后者用 Rust `Debug`，见 §③-1.1 R6） |
| A8 | **重载可行性筛 + 评分档位**（3 = 精确 / 引用基类型精确 / 类名同；2 = 数值同族 / 引用标量同 kind / 指针兼容 / rvalue-ref→类；0 = 不可行） | `cpp/methods.rs:200-251` | **低** | 52 行纯函数；但**同分取先声明者且无歧义诊断**（`methods.rs:186` 严格 `>`），迁移时须补（§④-4.3 N2） |
| A9 | **类布局与 vtable 计算**（继承字段拼接、C++ name hiding、虚函数槽位） | `cpp_class_layout.rs:86-320` | **中** | 逻辑清晰但**副作用密集**（同时写 `classes`/`funcs`/`program.globals`）；MoonBit 下应改为「输入 AST → 输出不可变 Layout 表」 |
| A10 | **mangled 名单源三函数**：`method_mangled_name`（`cpp/methods.rs:138-149`）、`constructor_mangled_name`（`cpp_overload.rs:205-213`）、`mangle_template_name`（`cpp_monomorph/func.rs:77-105`） | 三处 | **低** | 函数本体简单；**问题在调用面**：26 处裸 `format!` 绕过它们（§③-3.3） |
| A11 | **`evaluate_constexpr`（NTTP 常量求值）** | `decl.rs:648-684` | **低** | 支持字面量 / `sizeof` / 模板参数值 / 四则 / 一元负号；32 行 |
| A12 | **lambda capture 改写为 `this->field`** | `cpp/lambda.rs:7-146` + `lib.rs:425-488` | **中** | 遍历改写可直译；但依赖 `__lambda_N` 命名 + `Class` 复用作闭包（见 A13 风险） |
| A13 | **类方法体内非限定调用 → `this->method`（C++ name hiding）** | `expr/mod.rs:39-126` | **中** | 语义正确且带 D1 加固（`:58-76`）；但**报错后仍 `return None`**（`:74`）会落到兜底路径再报一条（§④-4.3） |
| A14 | **`auto` 推导规则表** | `cpp_auto.rs:5-90` | **低** | 16 臂纯 `match`，含 E1 字面量后缀定型 |
| A15 | **printf/scanf 格式串解析与实参匹配** | `builtin/mod.rs:91-242` | **低** | 手工 parser + 类型匹配表，纯函数式；**注意 `_ => true` 一律放行**（`:183`、`:232`） |
| A16 | **内置容器布局 JSON（数据驱动唯一真相源）** | `vitro_cpp_frontend/src/builtin_layout_data.json`（v2，generated 2026-06-13） | **低** | 覆盖 **5 个类名**：`vitro_list_int` / `vitro_string` / `vitro_vec_int` / `vitro_vec_float` / `vitro_vec_char` + `method_map` 5 项；加载器 `builtin_layout.rs:96-115`（`include_str!` + `LazyLock`）。**JSON 是语言中立资产，可直接搬** |

### 2.2 测试资产（防线，可整体复用）

| # | 资产 | 位置 | 成本 | 理由 |
|---|---|---|---|---|
| B1 | **Clang / Clang++ golden 真值**（`.out` / stdout） | `native/tests/shadow_verification/reports/`（653 文件） | **低** | 与实现语言无关（迁移评估报告 §8 A8 已判成立）；C++ 侧 99 例、C 侧 675 例 |
| B2 | **C++ E2E 用例 83 个 `.cpp`** | `native/tests/cases/cpp/` | **低** | 源码用例语言中立 |
| B3 | **C++ Shadow 22 条内嵌用例** | `scripts/shadow_verify_cpp/main.go` 第 89 行起（引用为源码行号，2026-09-18 实测） | **中** | ⚠️ **用例源码硬编码在 Go 驱动里**（不是数据文件）；迁移期建议先做一次「Go 脚本 → 数据文件」提取，否则 MoonBit 侧对账要先逆向 |
| B4 | **typeck 定向单测 70 个 / 94 assert** | 4 个 `native/tests/typeck_*.rs` + `type_checker_unit_test.rs` | **中** | 用例是「C/C++ 源码字符串 → 诊断码断言」，**语言中立**；但驱动代码调 `vitro_native::compiler::typeck::TypeChecker`（Rust API），需重写 driver 层 |
| B5 | **U3 红→绿锚**（`test_u3_template_instantiation_round_limit_triggered`@`crash_regression_tests.rs:795`；`test_u3_nested_struct_conflict_diagnosed`@`:814`） | 同上 | **低** | 两例源码各 2~3 行，是**验收语义最强的锚**（断言具体 `ErrorCode` + 消息子串），建议原样搬 |
| B6 | **10 个 `implicit_cast_target` 内联单测** | `lib.rs:622-710` | **极低** | 纯函数单测，可逐行翻译 |
| B7 | **字节码导出产物**（`func_table` mangled 名 + globals 布局） | `native/src/bin/vitro_cli.rs:272-395` | **中** | 可作 mangled 名等价性锚；⚠️ 但**产物 JSON 非字节稳定**（`func_table: HashMap` 直接 `to_string_pretty`，`vitro_cli.rs:308` / `:382`），稳定性目前靠消费侧排序（`scripts/precompile_bytecode_libc/main.go:84/191/279/303/349`） |

### 2.3 文档资产

| # | 资产 | 位置 | 成本 | 理由 |
|---|---|---|---|---|
| C1 | **U3 批次表 + 验收标准 + 完成判据**（9 项，逐项含来源） | `docs/current/07-质量与裁定/统一整备路线图.md:151-168` | **低** | 迁移期可直接当 backlog |
| C2 | **债务台账 D01–D16**（本模块涉 D14 / D16） | `docs/current/01-定位与路线/工程债务维护方案.md:54-60` | **低** | 但**已与代码漂移**（D14 已修未销项） |
| C3 | **C++ 子集边界与设计片段**（含 typeck 各文件设计稿） | `docs/current/03-语言子集/C++拓展实施计划.md:705/724/744/830/942` | **中** | 设计意图可复用，但含**已过期行号**（如「`typeck/mod.rs:50-51`」早已拆散） |
| C4 | **78 个诊断码 + `error_catalog` 教学文案** | `vitro_shared/src/error_codes.rs` + `native/src/diagnostics/error_catalog/` | **中** | 文案可复用；但存在**码语义漂移**（§③-3.3），迁移前需一次码审计 |

---

## ③ 抛弃清单

> 每项：抛弃理由 + 风险。分三类：Rust 特有机制 / 症状治疗代码 / 组织债。

### 3.1 Rust 特有机制（MoonBit 无对应或语义不同）

| # | 机制 | 位置（逐处行号） | 抛弃理由 | 风险 |
|---|---|---|---|---|
| R1 | **`&mut self` + 先 clone 再改** | `expr/call.rs:292-332`（注释自陈 drop borrow 后 reborrow）、`expr/call.rs:139-141`、`expr/var.rs:34-36`、`expr/mod.rs:45` | MoonBit 无借用检查器，也没有「借用冲突」这一约束；这套模式是 Rust 所有权模型的产物，搬过去纯属噪声 | **低**（模式消失，语义不变） |
| R2 | **`std::mem::take`（5 处）** | `expr/mod.rs:86`、`expr/mod.rs:103`、`expr/call.rs:76`、`expr/call.rs:461`、`convert.rs:55` | 为绕借用而生的「取出占位」惯用法；MoonBit 值语义下直接构造新值 | **低** |
| R3 | **`unreachable!()` 23 处** | `expr/var.rs:8`；`expr/cpp.rs:15/117/127/135/231`；`expr/call.rs:8/16/27/42/61/85/126/162/224/308/319/327/331/338/361/374/478` | 根因是「拿 `&mut Expr` 却按变体假设拆解」——Rust 无法从 `&mut` 收窄类型，只能 `.clone()` 再匹配再 `unreachable!()`。**`builtin/`、`init.rs`、`context.rs`、`symbols.rs`、`decl_types.rs` 等 12 个文件 0 处**（panic 家族仅这些 + `lib.rs` 测试内 `assert!`） | **中**：MoonBit 下应改为 `match` 返回 `Result`/`Option` 收敛，是**净改进**但需改签名，非机械替换 |
| R4 | **`.clone()` 深拷贝 111 处（仅 `expr/` 子树）** | `expr/` 全目录（合并篮子 `mem::take`/`clone`/`matches!`/`if let`/`iter_mut`/`HashMap`/`Box::new`/`as_ref`/`as_mut` 共 279 处） | 多数克隆是借用绕行的副产物；MoonBit 不可变结构天然可共享（迁移评估报告 §2.1 判 A5「不可变替代 Arc」方向一致） | **中高**：每个 clone 都要显式决定「共享 or 复制」。若一律照抄 `clone()`，会把「深拷贝 AST」写进热路径（`expr/call.rs:140` `(**callee).clone()` 整棵 Lambda 子树、`expr/cpp.rs:223` 整棵 lambda 体） |
| R5 | **`Box<Expr>` 所有权搬移** | `convert.rs:55-61`、`expr/mod.rs:86/104-109`、`expr/call.rs:76-77/461-467`、`expr/cpp.rs:104/138/201`、`expr/var.rs:45-49` | Rust 用 `Box` 表达递归枚举；MoonBit 递归 enum 直接内联 | **低** |
| R6 | **`format!("{:?}", expr)` 参与 mangling** | `vitro_ast/src/decl.rs:149-152`（`TemplateArg::Expr` 分支）、`vitro_ast/src/types.rs:364-367`（`Type::Typeof` 分支） | **最危险的一条**：mangled 名依赖 **Rust `Debug` 派生格式**，而 `SourceLoc` 派生 `Debug` 且含 `line`/`column`/`file_id`（`vitro_shared/source_loc.rs:1-9`），`Expr` 各变体携带 `loc`（`vitro_ast/src/expr.rs:71-72`）→ **同一份未求值 NTTP 表达式出现在不同行会得到不同 mangled 名**（缓存失效 / 重复实例化），且 MoonBit **无法逐字节复刻** `{:?}` 输出 | **高**：必须在设计阶段就替换为语言中立的规范化编码（形如 A7 的 `type_mangle_suffix`）。验证法：`template<int N> struct A{}; A<1+1> a; A<1+1> b;`（不同行）观察是否生成两个实例 |
| R7 | **`HashMap` 迭代序泄漏到产物（历史实锤）** | `cpp_overload.rs:302-309`（**含修复**） | 注释原文：「按类名排序后遍历：HashSet 的迭代顺序随进程随机种子变化，会让隐式移动构造函数的生成顺序（进而整段字节码布局）不可重现——表现为每次重新生成 Bytecode Libc 产物都得到不同的 code 布局（2026-09-11 由产物重生成 diff 定位）」 | **本模块命中「确定性」问题的最硬实锤**；修复方式是**显式排序**（`cpp_overload.rs:308-309`），不是依赖语言保证。**全 crate 仅此 1 处 `sort()`**（本次实测） |
| R8 | `#[allow(dead_code)]` 死字段 4 个 | `symbols.rs:7`（`VarSymbol::is_global`）、`:33`（`MethodSig::is_static`）、`:35`（`MethodSig::is_explicit`）、`:48`（`ClassSymbol::base`） | 死字段是组织债；迁移时要么接线要么删 | **低**：`ClassSymbol::base` 实际被 `cpp_class_layout.rs:92-99`、`:126-127` 使用（`allow` 属误标，**待证**：删 `allow` 跑 clippy 确认） |

### 3.2 症状治疗代码（治标不治本，新项目应换形态）

| # | 代码 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| S1 | **`MAX_TEMPLATE_INSTANTIATION_ROUNDS = 1024` 轮数上限** | `lib.rs:387-403` | 上限本身**方向正确**（clang 也有此防线：本次实测 clang 22.1.4 默认 `-ftemplate-depth` 报 `recursive template instantiation exceeded maximum depth of 1024`，数值对齐）。但形态是**「收敛循环的迭代轮数」**而非「实例化栈深度」：计数把函数实例化与类实例化混在一起（`lib.rs:389`），且**超限诊断用 `SourceLoc::default()`**（`lib.rs:397`）→ 报在 `0:0`，教学场景无定位；文案不点名**哪个模板**发散 | **中**：语义近似（每轮排空整个 frontier ⇒ 轮数 ≈ 实例化链深度），但资源消耗按「1024 轮 × 每轮全量类型检查」计 |
| S2 | **`char_narrow_suppress` 可变抑制开关** | `lib.rs:63` + 置位对 `decl.rs:264-268/297-301/357-361/389-393`、`init.rs:293-297/345-349` + 消费点 `convert.rs:218` | 跨函数的隐式状态，「记得成对复位」是隐性契约；且**覆盖不全**（`validate_nested_init_list` 与结构体路径未置位，见 §⑥-13） | **中高**：MoonBit 无 `&mut self`，照抄会把隐性契约变成显式 `Ref` 状态，**更难审计**；必须改为显式上下文参数 |
| S3 | **`is_upcast` 的 `for _ in 0..32` 防环上限** | `convert.rs:284-302` | 继承环本应在布局注册期拒绝；用步数上限兜底是防守位置错位 | **低** |
| S4 | **`placeholder_class` 空哑 decl** | `cpp_monomorph/synth.rs:64-75` + `builtin.rs:32-34/40-42` | 「查重命中时返回成员为空的 `ClassDecl`，靠调用方 push 点拦截」——这是 U3#8 在**缺少显式实例化缓存**下的补丁：返回一个**故意不完整**的值，指望下游不检查 | **高**：迁移期若下游少一个查重点，就会用空类覆盖真实布局。新设计应改为 `Result[InstanceId, AlreadyInstantiated]` 显式区分（§⑤-1） |
| S5 | **`instantiated_class_names` + 3 处 probe + pending 扫描（同一「已实例化」概念 5 种实现）** | `lib.rs:56/273`、`cpp_monomorph/class.rs:47-49`、`cpp_monomorph/builtin.rs:28-29`、`cpp_monomorph/replace.rs:11-15` | 五个位置各自用不同判据回答同一个问题；`builtin_list.rs:79` 更是 **push 进 `pending_class_instantiations` 却没同步插入 `instantiated_class_names`**（与 `replace.rs:14`、`lib.rs:273` 不一致） | **中**：当前被 `builtin.rs:29` 的 `already` 探针掩盖，**未证有可触发路径**（待证，见附录 #9） |
| S6 | **`try_monomorphize_class` 用 `None` 同时表示「不是模板」与「已实例化」** | `cpp_monomorph/class.rs:18-22`（非模板）、`:47-49`（已实例化） | 两种语义合流，逼出 `replace.rs:18-24` 的「`else if !templates.contains_key` → 否则重算 mangled 名」绕行 | **中**：新设计用 `enum MonomorphResult { New(..), Cached(id), NotATemplate }` |
| S7 | **`lookup_var` + 8 张表按字符串查名** | `context.rs:114-121` | 无预解析 / 无名字绑定，每次按字符串查到底；**类字段隐式 `this->`**（`expr/var.rs:34-57`）是二次查表补丁 | **低**：教学子集够用；但迁移期若要支持嵌套类（U3#9 完整根治）必须先有作用域化的名字解析 |
| S8 | **`__ctor__` / `__dtor__` / `__lambda_N` / `std__move` 字符串前缀分派** | `expr/cpp.rs:32`、`expr/call.rs:50/100`、`decl.rs:21/245/249`、`builtin/mod.rs:78`、`expr/cpp.rs:156/193`、`expr/call.rs:244` | 用字符串前缀代替 enum tag 做语言/语义分派，是移植期最容易漏的判据 | **中**：`std__move` 改名需同步 `builtin/mod.rs:78`、`decl.rs:97-98`、`vitro_codegen/src/expr.rs:428`、`vitro_codegen/src/expr/call.rs:267` |

### 3.3 组织债

| # | 债 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| O1 | **`decl.rs` 842 非空行（D16）** | `decl.rs` | 单文件超阈值；`decl_types.rs`（114 行）已拆出 5 个函数（`decl_types.rs:1-4` 自述「D16 拆分」），但**仍超标 42 行**；且台账口径的「唯一超标者」已不成立（`engineering_health.md` rank 1–4：`vitro_codegen/src/lib.rs` 896、`vitro_cli.rs` 861、`typeck/decl.rs` 842、**`vitro_parser/src/decl.rs` 840**） | **低**（可再拆：`Stmt::VarDecl` 巨型 arm `decl.rs:183-410`；`check_user_func` 四级回退 `decl.rs:747-858`） |
| O2 | **`compute_type_size` 每次调用重建 3 张 HashMap** | `context.rs:8-42` | 每次把 `self.structs`/`self.unions`/`self.classes` **全量 clone** 成新 HashMap；调用点含**逐字段循环内**调用（`expr/mod.rs:142` 的 `offsetof` 字段循环、`cpp_class_layout.rs:304` 的 `total_field_size`）→ O(定义数 × 字段数) 重复分配。台账已登记未做：路线图 `:207`（U6#4「`compute_type_size` 布局缓存」） | **低**：MoonBit 侧应做成不可变 Layout 表 + 记忆化（一次构建，查表 O(1)） |
| O3 | **`declare_var` 重复声明诊断位置写死零** | `context.rs:96-100`（`SourceLoc { line: 0, column: 0, file_id: 0 }`） | 报错无行号，前端 / CLI 无法定位；同因 `lib.rs:397`（E1022）、`cpp_overload.rs:325`（合成 move ctor 的 loc） | **低** |
| O4 | **`TypeError` 不带 `file_id`** | `lib.rs:18-24` | `line`/`column` 是 `i32` 但**无文件维度**，多文件（`#include` 展开后 `SourceLoc.file_id` 非 0）诊断无法归属文件 | **中**：这是**对外契约**（capi / serve 错误帧），迁移期改需同步协议 |
| O5 | **诊断码语义漂移** | `expr/cpp.rs:49` 用 `E3003_FuncRedeclared` 报「类没有接受 N 个参数的构造函数」；`expr/ops.rs:131-135` 用 `E3020_UnaryTypeError` 报三目条件类型；`expr/literal.rs:68/86` 用 `E3009_InvalidArrayInit` 报标量复合字面量；`init.rs:56/61/100/270/278/313` 用 `E3005_ArrayInitTooMany` 报 designator 语义错误（而 `error_catalog/semantic.rs:26-32` 对 3005 的讲解是「初始化值数量超过数组大小」→ **给学生无关解释**） | 码表是 wire 契约，语义与 catalog 文案不符会误导教学 | **中**：迁移前应先做码审计（78 个码逐条对 catalog） |
| O6 | **签名真相源重复（4~5 套）** | 头文件存根 `runtime_libc/include/*.h`（`stdio.h:4 int printf(const char* fmt);`、`string.h:4 char* strcpy(...)`、`stdlib.h:13 void qsort(...)`）+ `visit_call` 硬编码检查器（`builtin/mod.rs:16-88`）+ `bytecode_libc_sig`（`decl.rs:827` 唯一调用点）+ codegen 两张表（`host_func_id`、`bytecode_libc_index`） | 同一函数签名多处独立维护，**已实测出冲突**：`printf` 在 `io.rs:51` 是 `Type::void()` 而 `stdio.h:4` 是 `int`；`strcpy` 在 `string.rs:16` 是 `char*` 而 `bytecode_libc_sig.rs`（strcpy 分支）是 `void`；`putchar` 在 `io.rs:109` 是 `void` 而 `stdio.h:11` 是 `int`。另 `stdlib.h:19-20` 把 `long long llabs(long long n);` 连写两遍 | **高**：迁移期若不先单源化，会把 4 套不一致搬到 MoonBit 变成 4 套不一致 |
| O7 | **`qsort` 专用检查器在含 `<stdlib.h>` 时是死代码** | `builtin/mod.rs:44-50` 判据 `self.funcs.contains_key("qsort")`；而 `stdlib.h:13` 的存根声明必进 `self.funcs`（`lib.rs:182-217`）→ 走 `check_user_func`，`check_builtin_qsort`（`mod.rs:286-312`）不执行 | 代码无法区分「用户自定义」与「存根声明」 | **低**（行为差异 **待证**，见附录 #6） |
| O8 | **诊断静默吞没点** | `expr/call.rs:198-217`（实参个数不匹配时**完全不 resolve 实参**，实参内部的未声明变量错误被吞；而 `:175-180`、`:188-193` 同类分支都会 resolve） | 诊断完备性缺口 | **低** |
| O9 | **`decl.rs:661-663` 常量折叠无溢出检查**（裸 `l + r` / `l * r`） | `decl.rs:657-677` | 路线图 U1#9 的常量折叠清单只点 `cond.rs` 与 codegen 的 `decl.rs:830/845/846`，**未含此处** | **中**（**待证**，见附录 #8）：新项目须先 spike `Int` 溢出语义（§⑦-S4） |
| O10 | **`vitro_lexer` 幽灵依赖** | `Cargo.toml:9` | 零引用 | **极低** |
| O11 | **内置容器的「零硬编码」口径不成立** | `cpp_monomorph/builtin_vec.rs`（323 行）+ `builtin_list.rs`（572 行） | Phase 41 的「零 Rust 硬编码」只覆盖 **POD 元素**路径（JSON + 预编译 Bytecode Libc）；**类类型模板实参**路径（Phase 42，2026-06-26）在 Rust 里手写合成整个容器类（`builtin_vec.rs:15-34` 硬编码字段 `n`/`m`/`a`；`builtin_list.rs:29-79` 硬编码 node 类） | **中高**：这 895 行不是「可复用语义设计」而是**必须重写的实现**；新项目应把两条路径合流为同一数据驱动形态 |

---

## ④ 在途工作接纳方案

> 三条处置路线：**修复后搬** / **直接按目标架构实现** / **放弃并记录理由**。

### 4.1 债务台账（D14 / D16）

| 编号 | 台账原文 | 代码实测 | 处置 | 理由 |
|---|---|---|---|---|
| **D14** | 「`decl.rs` 3 处：L44 / L65 `init.take().unwrap()`、L648 `default.clone().unwrap()`，⚠️ 待收敛」（`工程债务维护方案.md:58`、`:431`） | **已修**：0 处 `unwrap()`；`decl.rs:46/69` 已 `let-else`，`decl.rs:706` 已 `let-else`；health 报告「生产代码 unwrap/expect = 1」（非 typeck） | **放弃（无需搬）+ 补销项留痕** | 代码侧已归零；但**必须先在台账销项并修掉过期行号**，否则新项目的审计基线带着一条假未决项 |
| **D16** | 「`decl.rs` **871 非空行**；199 个 `.rs` 文件中**唯一超标者**」（`工程债务维护方案.md:60`、`:433`） | **部分缓解**：现 **842 非空行**（862 总行），较 871 降 29 行；`decl_types.rs`（114 行）已拆出 5 个函数。**仍超 800 阈值 42 行**；「唯一超标者」已不成立（现有 4 个文件 > 800：896 / 861 / 842 / 840） | **分歧点：不走「再拆」路线** | 新项目的文件大小约束应由 MoonBit 包边界（§⑨）承担，而非继续切 Rust 文件。**但** `decl.rs:183-410`（`Stmt::VarDecl` 单 arm 228 行，内含 4 组状态开关配对）与 `decl.rs:747-858`（四级签名回退）是**语义上真正该切的接缝**，其切法应作为 MoonBit 包内模块划分的输入 |

### 4.2 U3 剩余 5 项 + 函数式构造存量缺陷

（登记原文：路线图 `:821-822`、`:833-834`）

| 项 | 登记原文（路线图行号） | 现状实测 | 处置 | 新项目怎么做 |
|---|---|---|---|---|
| **U3#1 temp_slot 手术**（固定 4 槽 → 作用域化分配器） | `:157` | **不在 typeck**：全 crate `temp_slot`/`slot0`/`get_temp_slot` = **0 命中**；属主是 `vitro_codegen`（`lib.rs:63-66` 四槽、`:69` `temp_slot_64`、`:76` `init_base_slot`、`:686`/`:703` 分配器、`func.rs:64-70` 按函数重置） | **不搬（越界）** | typeck 侧无动作。参考价值仅在「typeck 只做 AST 改写、槽位全属 codegen」这一分工**要在新架构里保留**（typeck 输出带类型的 AST，codegen 独占槽位分配） |
| **U3#3 `next_local_offset` 作用域回收** | `:159` | 同上，属 codegen（`vitro_codegen/src/func.rs:21` 每函数入口重置 `= 0`；路线图 §7A M9 已定性为「编译期帧容量放大，非正确性缺陷」） | **不搬（越界）** | 同上 |
| **U3#4 T1 模板实例化上限** | `:160`；修复记录 `:806-809` | **已修**：`lib.rs:387-403` + 新码 `E1022`；红锚 `crash_regression_tests.rs:795` | **修复后搬，但改语义** | 保留「有上限 + 确定性诊断」这条防线（clang 亦有，本次实测默认 1024）；**改三处**：① 按**实例化栈深度**计数（而非收敛轮数，消除「函数 + 类混合计数」）；② 诊断带真实 `SourceLoc`（现状 `lib.rs:397` 是 `SourceLoc::default()`）；③ 文案点名发散模板 |
| **U3#5 T2 类实例化 drain（双层根因）** | `:161`；修复记录 `:810-814` | **已修**：表层 = 类与函数**同收敛循环** drain（`lib.rs:389-421`）；深层 = `replace_template_type_preserve_tiid`（`replace.rs:73-99`）保留 `TemplateId` 形态，让 `visit_func_decl` 触发类合成 | **直接按目标架构实现（不搬）** | 若单态化改成 IR 层纯函数变换（§⑤-1），**这个收敛循环整体不存在**，「drain 时机」这类缺陷从根上消失。这是 U3#5 的最佳归宿：不修，而是让病灶所在的机制消失 |
| **U3#6 C++ 寄生收口** | `:162`（2026-09-06 报告 §0 追踪表销项 + mangled name 单源审计 + §7.1/7.2/7.3/7.5 逐条销项） | **部分完成**：`method_mangled_name`（`cpp/methods.rs:138`）与 `constructor_mangled_name`（`cpp_overload.rs:205`）已成单源（D1，2026-09-12）；**但仍有 26 处裸 `format!` 绕过**（本次实测，跨 8 文件，如 `decl.rs:37/62/103`、`cpp_class_layout.rs:119/248/404/435/463`、`cpp_overload.rs:105/160/207/210/212/245/254/259/318`、`lib.rs:235/428/429`、`methods.rs:145/148`、`call.rs:244`、`cpp.rs:156/193`、`cpp_auto.rs:70`）；且**解析器独立造名**（`vitro_parser/src/stmt.rs:226/228/269/271`、`expr/postfix.rs:28/38`）——**跨 crate 命名契约无共享定义** | **直接按目标架构实现** | 新项目把 mangled 名做成**从 `InstKey` 派生的纯函数**，且把「名字形状」定义在与 parser 共享的最底层包（§⑨ 的 `vitro/names`）。判据：全仓只允许 1 处 `format!` 产出 `__ctor__` / `__dtor__` 前缀 |
| **U3#7 变参实参区溢出** | `:163`（「预留固定 64 字节（16 words），>16 words 静默覆写被调函数局部变量（8 个 double 即到顶）」） | 病灶在 `vitro_codegen/src/func.rs:54-59`（`current_func_arg_bytes = offset - if is_variadic { 64 } else { 0 }`）；typeck 侧只贡献「按类型提升后的字宽」 | **不搬（越界）+ typeck 侧保留提升表** | typeck 侧唯一职责是把变参实参**定型并提升**（`convert.rs:67-74`，消费点 `decl.rs:767-772`、`:805-810`），这部分必须搬；「动态预留」属 codegen |
| **U3#9 完整根治**（嵌套名 mangled 化 `Outer__Inner` + 字段类型引用跟随） | `:165`、`:825-834`（止血已完成，2026-09-14） | 止血在 `cpp_class_layout.rs:250-279`：`structs.get(&decl.name)` 命中且**布局不同** → 报 `E3002`（`:269-275`）；**布局相同则静默保留首个**（`:261-267` 的 `same` 判据成立时不报也不插）；`decl.name` 是**未限定名**（`:277` 直接 `insert`）→ 无作用域。红锚 `crash_regression_tests.rs:814` | **直接按目标架构实现** | 新项目做**类作用域命名空间**：嵌套类型注册为 `Outer.Inner`（名字是结构化路径，不是拼接字符串），字段类型引用随作用域解析。判据：`class A{struct Inner{int x;}; Inner i;}; class B{struct Inner{double y;}; Inner j;};` **零诊断且 `a.i.x` 正确**（现状是 E3002 拒绝） |
| **函数式构造存量缺陷**（`p = Point(3,4)` / `f(Point(1,2))` 裸类失败） | `:817-818`（U3#8 开发中挖出的**独立存量缺陷**，登记「待办」） | 机制已定位：parser 只在**声明上下文**改写为 `__ctor__*`（`vitro_parser/src/stmt.rs:211-280`、`expr/postfix.rs:24-48`）；**表达式上下文**产出裸 `Expr::Call{name:"Point"}` → `expr/mod.rs:191` → `expr/call.rs:4-29` → `visit_call`（`builtin/mod.rs:16-88`，无 `"Point"` arm）→ `check_user_func`（`decl.rs:747`）四级回退全空 → `decl.rs:856` 报 **E3036「未定义的函数 'Point'」**。⚠️ 登记原文写 **E3023**（`UndeclaredVar`，出自 `expr/var.rs:70`）——**具体码待证**（附录 #1） | **直接按目标架构实现** | 新项目在「调用名解析」**单一入口**做：名字若是已注册类型 → 改写为构造调用（含默认实参填充与重载决议）。属 §⑤-2 的名字解析统一改造点 |
| **U6#4 `compute_type_size` 布局缓存** | `:207`（「结构债」，登记未做） | `context.rs:8-42` 每次调用重建 3 张 HashMap（O2） | **直接按目标架构实现** | 新项目：Layout 表在布局 Pass 一次性构建为**不可变值**，`sizeof` / `offsetof` / 字段偏移全部查表 |

### 4.3 未登记但实测存在的在途缺陷（建议补登记）

| # | 现象 | 证据 | 处置 |
|---|---|---|---|
| N1 | **构造函数同 arity 异类型重载被误判歧义（E4031）** | `cpp_overload.rs:205-213` 的 ctor mangling **只编码 arity**（+ copy 特例）→ `Point(int)` / `Point(double)` 落进 `methods` map **同一个 key**（`cpp_class_layout.rs:227-228` 各 push 一条）→ `resolve_constructor_overload` 收集到 2 个候选（`:262-272`）→ `:279-289` 报 E4031 拒绝 | **直接按目标架构实现**：ctor 与 method 共用同一套「参数类型编码」mangled 规则（A7），歧义判定改为「类型序列无法唯一确定」而非「arity 冲突」。**这是 D1 修复未覆盖构造函数的不对称** |
| N2 | **方法重载同分静默取先声明者，无歧义诊断** | `cpp/methods.rs:184-188`（仅 `score > cur` 才替换）；`methods.rs:227-235`（**所有数值类型对一律 2 分**，`int→double` 与 `double→int` 同分）；对比构造函数有 E4031 | **直接按目标架构实现**：补 E4026 同分歧义诊断 |
| N3 | **`printf` / `putchar` 返回类型与 C 标准、头文件存根冲突** | `builtin/io.rs:51` `Type::void()` vs `stdio.h:4 int printf(const char* fmt);`；`io.rs:109` `Type::void()` vs `stdio.h:11 int putchar(int c);` → `int n = printf("hi\n");` 走 `decl.rs:292-299` → E3004 | **待证**（附录 #5）。修复方向：签名单源化（O6） |
| N4 | **`strcpy` / `strcat` / `strncpy` 三套签名并存** | `string.rs:16` `char*` / `bytecode_libc_sig.rs` `void` / `string.h:4` `char*` | 同上，归入 O6 单源化 |
| N5 | **`int a[][3] = {1,2,3,4,5,6};` 首维推断错** | `init.rs:238-247`：`total_elems` 在改 `dims` **前**取（对 `dims=[-1,3]` 得 3），`:245` 把 `dims[0]` 设为 6，`:246` 又把陈旧的 3 写回 `array_size` → `dims=[6,3]` / `array_size=3` 自相矛盾 | **待证**（附录 #2）。新项目：`dims` 与 `array_size` 二选一（§⑤-4） |
| N6 | **`char s[]="..."` 丢运行时边界检查** | `init.rs:372-375` 只改 `array_size` 不改 `dims`；codegen 取 `dims()[0]` 且 `> 0` 才发 trap → `-1` 跳过 | **待证**（附录 #3）。同上 |
| N7 | **`decl.rs:661-663` 常量折叠无溢出检查**（裸 `l + r` / `l * r`） | `decl.rs:657-677`；U1#9 清单未含此处 | **待证**（附录 #8）。新项目：`Int` 算术语义先 spike（§⑦-S4） |

---

## ⑤ 架构优化建议（MoonBit 形态）

### 〔结构优化〕1. 单态化：从「多轮重跑到收敛」改为「显式实例化缓存 + IR 层纯函数变换」

**用本模块代码事实论证可行性（本节核心）**

**现状病灶的 4 条硬证据**：

1. **收敛循环内嵌类型检查** —— `lib.rs:389-421`：
   ```rust
   while !self.pending_instantiations.is_empty() || !self.pending_class_instantiations.is_empty() {
       let pending = take(&mut self.pending_instantiations);
       for (_, mut f) in pending { if f.body.is_some() { self.visit_func_decl(&mut f); }  // ← 类型检查
                                program.funcs.push(f); }                              // ← 改 program
       let pending_classes = take(&mut self.pending_class_instantiations);
       for (_, c) in pending_classes { program.classes.push(c); }                     // ← 改 program
       if got_new_class { self.check_class_methods(program); self.generate_implicit_move_ctors(program); }
   }
   ```
   **新实例的「发现」是类型检查的副作用**（实例化体被 `visit_func_decl` 检查时才会撞见新的模板调用），所以必须反复重跑。这是「需要收敛」的**唯一根因**，也是上限只能按「轮数」计的原因。

2. **实例化缓存确实存在，但是隐式的、且一个概念五种实现**：
   - 函数：`self.funcs.contains_key(&mangled)`（`cpp_monomorph/func.rs:52-54`），并在**创建时**即写入签名（`func.rs:62-70`，**先注册符号、后检查体**）
   - 类：`self.classes.contains_key || self.structs.contains_key`（`cpp_monomorph/class.rs:47-49`）—— 用 `None` 同时表示「不是模板」与「已实例化」
   - 容器专用第三次 probe：`cpp_monomorph/builtin.rs:28-29`
   - 独立去重集：`instantiated_class_names`（`lib.rs:56`，插入点 `lib.rs:273`、`replace.rs:14`）
   - pending 队列线性扫描：`replace.rs:11-12`
   - **不一致**：`builtin_list.rs:79` push 进 pending 却**未**同步 `instantiated_class_names`

3. **mangled 名是缓存键，而它在每个 probe 点被重算**：`mangle_template_name` 调用点 8 处（`func.rs:51/77`、`class.rs:46`、`replace.rs:22/113`、`builtin.rs:28`、`builtin_vec.rs:5`、`builtin_list.rs:6/8`），且带**历史短名特例**（`func.rs:78-98`：`vitro_vec<int>`→`vitro_vec_int`、`vitro_list<int>`→`vitro_list_int`、`vitro_vec<float/char>`、`vitro_string<char>`）——这是通往 JSON 数据驱动布局（A16，5 个 POD 类名）的**兼容桥**，但也意味着名字**不能仅由 `(base, args)` 推导**，缓存键必须复用同一个特例分支。

4. **上游键可能自带源码位置**：`mangle_name_into` 的 `TemplateArg::Expr` 分支用 `format!("{:?}", expr)`（`vitro_ast/src/decl.rs:149-152`），而 `Expr` 携带 `loc: SourceLoc{line,column,file_id}`（`vitro_ast/src/expr.rs:71-72`、`vitro_shared/src/source_loc.rs:1-9`）→ **同形不同行的 NTTP 表达式得到不同键**。

**目标形态（三条设计约束 + 一条必要的让步）**

- **显式缓存**：`Map[InstKey, InstId]`，`InstKey = (TemplateId, Array[TypeArg])`，**`TypeArg` 用规范化编码**（弃 `{:?}`，改用 A7 那套 ASCII 编码的推广版）。名字由 `InstId` 单向派生（`names.mangled(InstId)`），**全仓唯一产出点**。
- **纯函数变换**：`expand(program: Program, queue: Queue[InstRequest]) -> (Program, Queue[InstRequest])` —— 只依赖输入、只产出新值，不触碰检查器状态。收敛循环随之消失（U3#5 的病灶消失）。
- **必要的让步（必须承认）**：纯变换**不能取消固定点**。因为 `try_monomorphize_func` 需要 `arg_types`，而 `arg_types` 来自**表达式类型推断**（`decl.rs:816-823`）；模板体内的调用实参类型又依赖**外层实例化**。所以目标形态是**两阶段 + 一个数据结构上的固定点**：
  1. 类型检查只**记录**实例化请求（模板名 + 已解析类型实参），不改 `program`；
  2. `expand` 在请求队列上做固定点（`Map[InstKey, InstId]` 记忆化），每轮返回「新 program + 新请求」。
  固定点仍然存在，但它**不再是「类型检查副作用的固定点」**：轮数上限从此可按「实例化栈深度」精确计数（修 S1 的三处语义），且每轮可断言「请求集合单调增长 + 无重复键」。
- **可直接删除的配套物**：`instantiated_class_names`（`lib.rs:56`）、`placeholder_class`（`synth.rs:64-75`，S4）、3 处 probe、pending 线性扫描、`class.rs` 的 `None` 双义（S6）。

### 〔结构优化〕2. `SourceLang` enum 替代布尔穿透 —— 改造点清单（实事求是版）

必须先说明：**typeck 里没有布尔可替代**（§0 P-1）。真正的改造分三层，且**第一层不在本模块**：

| 层 | 改造对象 | 位置（逐条） | 备注 |
|---|---|---|---|
| **L1（不在本模块）** | lexer/parser 的 `is_cpp_mode` 布尔穿透 **46 处** | `vitro_lexer/src/lib.rs:31/49/57/66/390`；`vitro_parser/src/lib.rs:76/142/148/158/373/506/510/522/524/555/574`、`decl.rs:46/150/164/259/500/636/652/679`、`type_.rs:27/116/156/187/205/312`、`stmt.rs:140/437/442/465`、`expr/primary.rs:207/215/239`、`expr/unary.rs:25/28`、`expr/postfix.rs:186`、`cpp.rs:61/109`；`native/src/engine/compile_pipeline.rs:631/643/657` | 这 46 处是「语言分派」的真实所在；CS 计划 `CSharp前端引入计划.md:47`（D1）已裁定 `SourceLang enum 替代布尔穿透`，正好在 MoonBit 重写时一并落地 |
| **L2（本模块，6 类隐式判据）** | ① 类上下文 `current_class` / `current_method_is_const`：`expr/var.rs:34/158/186`、`expr/mod.rs:45/80/82`、`expr/call.rs:396` + `var.rs:40/42/190/192`（11 行）<br>② 类型形态 `Type::Class` / `is_class()`：**19 处**（`var.rs:38/105/112/128/188`、`ops.rs:277/282`、`mod.rs:78`、`cpp.rs:28/33/157`、`call.rs:100/138/297/300/311/407/409/416`）<br>③ 引用形态：8 行 / 11 调用（`var.rs:25/123`、`ops.rs:253`、`mod.rs:92`、`cpp.rs:138`、`call.rs:310/415/450`）<br>④ **名字前后缀**：6 处（`cpp.rs:32 "__ctor__"`、`call.rs:50 "std__"`、`call.rs:100 "__lambda_"`、`cpp.rs:156/193/244`）<br>⑤ C++ 专用 API 调用 19 处<br>⑥ AST 节点种类无守卫分派 8 臂（`expr/mod.rs:191/192/209/210/211/212/213/214`） | 改造为：**节点携带语言来源标签**（parser 写入，typeck 读出）+ 前缀判据全部升级为 enum tag（§③-2 S8）。注意 `Type::Class` 同时承担「C++ 类」与「lambda 闭包结构体」两种身份（`expr/cpp.rs:156-160` 把 `__lambda_N` 注册为 `ClassSymbol`） |
| **L3（新设计）** | 语言维度**不是全局开关**，而是三个可参数化 profile：隐式转换表（`convert.rs:6-45` 现为恒定 C 语义）、可见性规则（`AccessSpec` 目前 C++ 专用）、名字规则（mangled 形状） | 便于 CS 前端复用（CS 计划 D4/D5 已规划 E5xxx 与分语言数据表） |

### 〔结构优化〕3. 诊断结构化（协议级，需与出口同步）

现状 `TypeError { message, line, column, code }`（`lib.rs:18-24`）。目标：`Diagnostic { code: ErrorCode, severity, span: Span(file_id, start, end), message_key, args }`。收益：① 解决 O4（无 `file_id`）与 O3（零位置，`context.rs:98`、`lib.rs:397`）；② `message` 由 `message_key + args` 生成，**诊断序列可逐字段 diff**（§⑧ 的前提）；③ 三个出口（capi / serve / wasm）不再各自拼字符串。
**风险**：`message` 是对外契约（serve 错误帧、`error_catalog` 文案）→ 需并行双轨一个版本周期（迁移评估报告 §3.3「裸奔期」风险）。

### 〔结构优化〕4. 数组尺寸：单一表达式 + 不变式

`dims: Vec<i32>` 与 `array_size: i32` **双字段并存**（`vitro_ast/src/types.rs:58-65`），且 `-1` / `0` 是哨兵（`lib.rs:552-555` 注释复述 parser 约定）；`total_elements()` 自带一套负值处理（`types.rs:557-573`）；推断改写分散在 `init.rs:244-247`（多维）、`init.rs:320-329`（一维）、`init.rs:372-375`（字符串，**只改一个字段**）。N5 / N6 与「同族判据第二份」（`lib.rs:556-585` 权威 vs `init.rs:320-329` 仍留 `<= 0`）**同源于此**。
目标：`Type.Array(ArrayType { dims: Dims, ... })`，`Dims` 只有一种表达（未定 = `Unspecified` 构造子，不占数值空间），并强制不变式「推断只发生在一个函数里」。

### 〔沿革保留〕5. 保留清单（不要动的好设计）

- **`type_mangle_suffix`**（`cpp/methods.rs:96-130`）—— ASCII、稳定、可读、覆盖全部 17 个 `Type` 变体；`Type::Typeof` 落到 `"y"`（`:128`）而不是打印表达式，**这正是 §③-1 R6 问题在另一条路径上被规避的正面例子**，两处应统一到这一套。
- **`method_mangled_name` 的「有重载才带后缀」**（`cpp/methods.rs` 第 138-149 行；引用为源码行号，2026-09-18 实测）—— 注释（`:136-137`）明示动机「保持既有单签名场景的短名兼容——不破坏已落库的字节码与回放基线」。⚠️ **沿革保留但必须标风险**：mangled 名**依赖「该类是否有重载」这一上下文**，加一个重载会改掉全类方法的名字 → 破坏已落库产物。新项目建议改为**内容派生（永远带类型后缀）**，并在迁移期一次性重算产物与回放基线。
- **`__ctor__{Class}__copy` / `__move` 语义**（`cpp_overload.rs:205-213`、`decl.rs:37/62/103`）—— 教学子集的移动/拷贝语义，语义有效。
- **数据驱动容器布局**（A16）—— `vitro_cpp_frontend` 的 JSON + `include_str!` 是「资产外置、代码只做解释器」的正确形态（与仓库脚本约定同源）。**建议扩大覆盖**：把 O11 的 895 行 Rust 硬编码合成改造成同一数据驱动路径。

### 〔新设计〕6. 嵌套类型作用域（U3#9 完整根治）

现状 `structs: HashMap[String, StructSymbol]` 是**全局扁平表**（`lib.rs:36`），`StructSymbol` 只存 `fields`（`symbols.rs:23-25`，**无归属类、无 size**）；嵌套名用 `decl.name` 原样插入（`cpp_class_layout.rs:277`）。
目标：符号表拆为 `GlobalNames` + `TypeScope`（类作用域树），类型名是结构化路径（`Outer.Inner`）而非拼接字符串；布局表按路径索引。判据见 §④-4.2 U3#9 行。

### 〔新设计〕7. 诊断顺序显式化

现状：诊断顺序 = AST 遍历顺序（`errors` 是 `Vec`，`report_error` 仅 push：`lib.rs:519-526`）；`program.funcs` / `globals` 是 `Vec` 故确定。**本 crate 的 HashMap 迭代实测 3 处，全部顺序无关**（`context.rs:40` 收集成 map、`context.rs:115` 迭代 `Vec` scopes、`cpp_class_layout.rs:23` 收集类名）。**但另有真风险处**：`cpp_overload.rs:302-309` 有历史实锤（R7）。
目标：不发散依赖「恰好顺序无关」，而是**在诊断出口显式排序**（`(file_id, line, column, code, seq)`），并在 codegen 侧对任何由 map 迭代驱动的生成顺序显式排序（沿用 `cpp_overload.rs:308-309` 的模式）。

### 〔结构优化〕8. 显式上下文替代可变抑制开关

`char_narrow_suppress`（`lib.rs:63`）→ 改为把 `Ctx { narrow_suppress: Bool, current_class, current_method_is_const, ... }` 作为**只读参数**传给 `check_assignable` 一族。MoonBit 无隐式 `&mut self` 状态，这一步是**被迫的**，也正好消除 §③-2 S2 与 §⑥-13 的「覆盖不全」缺陷。

### 〔结构优化〕9. `auto` / `typeof` / 数组尺寸推断统一为「声明定型阶段」

现状同一件事有三个实现：`cpp_auto.rs:5-90`（`deduce_auto_type`）、`decl_types.rs:41-113`（`resolve_typeof_in_type` 与 `replace_auto_in_type` **两份逐行重复的遍历重建**）、`init.rs:244-329`（尺寸推断）。且全局路径还需一份「先解析初始化器再登记」的特判（`lib.rs:287-306`，注释记录条目 2 的历史缺陷）。
目标：一个 `infer_decl_type(decl, init) -> Type` 纯函数 + 一条不变式「声明登记前类型必须已定型」。

---

## ⑥ 坑清单（本模块事故史）

> 格式：现象 → 根因 → 修复 → **新语言下是否复发、为什么**。
> 「是否复发」判定基准：MoonBit 无 `&mut` 别名、无借用检查器、不可变结构共享、`match` 穷尽、无 `Arc` / 裸指针。

| # | 现象 | 根因 | 修复 | 新语言下是否复发 / 为什么 |
|---|---|---|---|---|
| **1** | 函数返回 `double` 值异常：`return 2.5;` 在返回类型 `double` 的函数里生成 `PushConstF` 而非 `PushConstD`（H07，`工程债务维护方案.md:490/819/942`） | `return` 语句未对返回值表达式插入隐式类型转换；`2.5` 被解析为 `float` 字面量 | `decl.rs:481-489`：`check_assignable` 成功后 `insert_implicit_cast(v, &expected)`；配套 `convert.rs:49-52` 的 FloatLiteral→double **重定型特例**（不插 Cast，直接改节点类型） | **会复发（形态变了）**。MoonBit 无「原地改节点类型」的隐式能力（值语义），必须把「字面量默认类型 + 目标类型驱动的重定型」写成显式规则。**若照抄「改节点 ty」会编译不过**，反而逼出正确设计。回归锚 `baseline/float_func_return.c` 必须搬 |
| **2** | 浮点字面量位宽错位（与 #1 同族） | `expr/literal.rs:15` 注释：浮点字面量**曾无条件返回 float**，与 codegen 的 `PushConstD` 位宽错配 | E1 改为按后缀定型（`literal.rs:17-25`、`cpp_auto.rs:8-15` 同步） | **不会复发**：MoonBit 有 `Double` / `Float` 两个明确类型，`enum + match` 穷尽会强制处理两个分支 |
| **3** | **IDE 直接崩溃**：`struct Node { struct Node next; }`（链表节点漏 `*`）→ `compute_type_size` 无限递归栈溢出 | 值语义成员循环包含无检测（T-P0-8） | `lib.rs:139-170`（struct/union 双路径）+ `lib.rs:495-517` `value_member_cycle`；同类第二处：`cpp_class_layout.rs:61-84` `compute_class_has_resource` 用 `visiting: HashSet` 防环（注释 `:62-63` 记录「循环继承（A:B 且 B:A）与自含类字段曾在此无限递归栈溢出」） | **会复发（更易复发）**。MoonBit 递归类型更自由（无需 `Box`），「值包含值」在类型层面更自然；且 `HashSet` 防环若换成不可变集合，路径记录成本上升。**必须保留显式环检测 + 深度预算**（§⑦-S2） |
| **4** | 类方法重载**静默错派发 → 运行时栈下溢 trap**：`show(int)` / `show(double)` 同参数个数 | mangled 名只带**参数个数**（`{Class}__{method}__{arity}`） | D1（2026-09-12）：`type_mangle_suffix` + `method_mangled_name` 单源（`cpp/methods.rs:86-149`）；同族第二处：`expr/mod.rs:55-76` 重载解析不到时**必须报诊断**不得静默放行（否则退回按名字找函数的兜底路径 → 派发到另一实现 → trap） | **不会自动复发，但极易重犯**：MoonBit 无符号名冲突检测，「后写覆盖先写」在 `Map` 里同样静默。**判据必须显式**：mangled 名必须包含完整参数类型编码（§④-4.3 N1/N2 尚未做完：构造函数仍只带 arity） |
| **5** | **同名嵌套 struct 静默覆盖**：`class A{struct Inner{int x;}}` + `class B{struct Inner{double y;}}` → `a.i.x` 报 E3042 假错误（成员访问指向错误布局） | `cpp_class_layout.rs:250` 曾无条件 `insert` 覆盖全局 `structs` 表（展平命名、无作用域） | U3#9 止血（2026-09-14，`cpp_class_layout.rs:250-279`）：保留首个 + 同名**不同布局**报 `E3002`（`:269-275`）；嵌套 union 同路径同批覆盖（parser 把 union 塞进 `NestedStruct`）；红锚 `crash_regression_tests.rs:814` | **会复发（结构性）**：MoonBit 的 `Map` 同样是「同键覆盖静默」，且**没有编译器帮助**。止血只是把静默变显式拒绝；根治要类作用域（§⑤-6）。**当前残留**：同名**同布局**仍静默共享一条（`:261-267`） |
| **6** | **容器第二次实例化报 E3002 假错误**（第二个 `vitro_vec<Foo>` 合法代码被拒，级联后显形为 E3023） | 快路径先返回后查重：`try_synthesize_builtin_container_class` 无条件合成并 `register_single_class_layout`，后者对已注册名报「类重复定义」 | U3#8（2026-09-14）：查重前置（`cpp_monomorph/builtin.rs:28-34`）+ `instantiated_class_names` 单源（`lib.rs:56`）+ push 点查重（`replace.rs:11-15`）+ 占位 decl（`synth.rs:64-75`）；红锚 `cpp_u3_vec_class_twice` | **会复发（同一形状）**：根因是「缺显式实例化缓存」而非 Rust 特性。§⑤-1 的 `Map[InstKey, InstId]` 才能根除；否则 MoonBit 侧会复刻「五处各查一遍」 |
| **7** | 函数模板体内的容器 `v.push_back(a)` 报 E3042（合法 C++ 被拒） | **双层根因**：表层 = 类实例化只在 Pass 3 后排空一次，循环期间新发现的类被静默丢弃；深层 = 函数模板实例化体的 `VarDecl` 替换把 `TemplateId` **静态 mangle 成 Class 名**，`visit` 不再触发类合成注册 → 布局从未注册 → `classes.get(mangled)=None`（调试实证 `sigs=None`） | U3#5（2026-09-14）：`replace_template_type_preserve_tiid`（`replace.rs:73-99`）在**函数体 VarDecl 路径保留 `TemplateId`**，让 `visit_func_decl` 触发合成；类路径仍用 mangle 版（成员不经 visit） | **会复发（若保留「类型替换阶段就定名」）**：根因是「**定名时机 ≠ 定名责任方**」——同一个决定在两条路径上必须不同。MoonBit 下应把「名字延迟到使用点解析」变成数据结构保证（`TemplateId` 与 `Class` 是不同构造子，不允许隐式互转） |
| **8** | 全局 `auto gf = [](int x){ return x+7; };` 报 E3004「无法将 'class __lambda_0' 赋值给 'auto'」 | `declare_var` 登记的是**替换前的 `auto`**，调用点查表得到 `auto` | 条目 2（2026-09-11，`lib.rs:287-306`）：全局 `auto` / `typeof` **先用初始化器定型再登记**，且解析结果缓存给后续循环复用（避免 lambda 二次登记 `pending_lambdas` → Pass 4 重复生成 `__call`） | **会复发（顺序陷阱）**：这是「两遍循环里第一遍必须做完全部定型」的隐式契约。MoonBit 无借用检查器提示这类顺序错误 → 需用类型系统表达（`UnresolvedDecl` vs `ResolvedDecl` 两个类型，阶段不可混用） |
| **9** | 非 int 返回的 lambda 在调用点被当作 int（`printf("%.2f", d(1.5))` 触发 E3062） | `resolve_lambda` 与 Pass 4 生成 `__call` 的**两处都硬编码 `Type::int()`**（双真相源） | 条目 1（2026-09-11）：`LambdaInfo.return_type`（`symbols.rs:70-74`）成单一来源，两处共用 | **不会复发**：MoonBit 的 `struct` 字段是唯一存放点，「两处各写一遍常量」在编译期就会因字段缺失而暴露（无默认值可省略） |
| **10** | 教学代码 `int r = scanf("%d", &x);` 报 E3004；`while (scanf(...) != EOF)` 完全不可用 | `scanf` 被声明为 `void` | 条目 3（2026-09-11，`builtin/io.rs:86-89`）：改 `Type::int()`，与早已正确的 `sscanf` 对齐 | **会复发（同一类）**：`printf` / `putchar` 目前仍是 `void`（`io.rs:51` / `:109`）而 C 标准是 `int` —— **同一缺陷族的剩余成员尚未修**（N3） |
| **11** | **Bytecode Libc 产物每次重生成得到不同的 code 布局** | `HashSet` 迭代顺序随进程随机种子变化，决定隐式移动构造的生成顺序 | 2026-09-11 定位（`cpp_overload.rs:302-305` 注释），修复 = **按类名排序后遍历**（`:306-309`） | **会复发（高危，且 MoonBit 的默认语义更微妙）**：本机 MoonBit core lib 源码显示 `Hasher` 默认种子 **wasm / wasm-gc = 0，native / llvm / js = 随机**（`~/.moon/lib/core/builtin/hasher.mbt:87-103`，文档串 `:60-62`「When omitted, a randomly chosen process-wide seed is used on native, LLVM, and JavaScript targets. Wasm targets default to 0. Pass `0` explicitly when deterministic output is required.」），而 `HashMap::iter` 按**槽位索引序**遍历且文档明写 **"in unspecified order"**（`~/.moon/lib/core/hashmap/utils.mbt:75-92`、`:102`）。⇒ **wasm-gc 出口恰好确定，但开发期本地通道（native / C 后端）与 Node(js) 宿主不确定** → 迁移期差分对账会在不同宿主上得出不同结果。**必须显式用 `LinkedHashMap`（插入序，`~/.moon/lib/core/builtin/linked_hash_map.mbt:753/755-770`）或显式排序**，不得依赖默认 `HashMap` |
| **12** | C++ 向上转型 `Derived* → Base*` 被报「不兼容/截断」警告 | 复用标量转换码 `W3053`（文案「可能导致数据截断」），把多态基础写法讲成危险操作 | P1-6（`convert.rs:339-342`）：新增 `W3067_PointerTypeMismatch` | **不会复发**（`enum` 化诊断码后按 `match` 分支给出各自文案） |
| **13** | `char s[5]={72,101,...}` 逐元素报「可能丢失精度」误报轰炸合法代码 | 缺「字符常量 / char 值域内整常量」的无损判据 | W0-4（R-2026-09-12，`lib.rs:59-63/587-599`）：`is_char_safe_initializer` + 逐（行,码）去重（`lib.rs:529-538`）；**但抑制开关覆盖不全**（`validate_nested_init_list` 与结构体路径未置位 → `char a[2][2]={{1,2},{3,4}}` 仍预期 1 条误报，**待证**，见附录 #7） | **会复发**：根因是「抑制状态散落成对置位」；MoonBit 下改为显式上下文参数（§⑤-8）**可以一次消除**——这是换语言的真实收益点 |
| **14** | 方法符号注册含 `this` 而调用点不含 → **编译失败且无诊断** | 定义处与调用处 mangled 名的参数口径不一致 | D1 加固（`cpp_class_layout.rs:335-338` 注释）：两处统一用**不含 this 的用户参数** | **会复发**：这是「同一函数两个调用点各算一遍名字」的必然风险；MoonBit 侧必须让名字**只在一个函数里产生**（§⑤-1） |
| **15** | `long long` 被系统性误拒 / `++doubleVar` 作为 `printf %f` 实参被误报 | `is_int()` 不含 `LongLong`（`convert.rs:77-79`）；非指针一元运算一律标 `int` | `expr/ops.rs:209`、`expr/ops.rs:232` 逐个补判 | **会复发（判据分散）**：`is_int` 语义与调用点期望不一致，靠逐处补 `matches!(.., LongLong)`（`ops.rs:54/90/98/187/291`）维持。MoonBit 下应把「整型族」做成一个 `match` 覆盖的 enum 谓词 |
| **16** | `[](int a,int b){return a+b;}(2,3)` 立即调用编译错 | lambda 分支依赖 `lookup_var`，`Lambda` 表达式节点查不到 → 落到「非函数指针」兜底 | Issue B1（`expr/call.rs:130-157`）：变量形式与立即调用形式**共用 `rewrite_lambda_call`**（`:228-245`），注释明写「避免双轨语义漂移」 | **不会复发**（单源改写）；但**同类双轨风险仍在**：`expr/call.rs:96-127` 与 `:134-157` 是两段几乎同构的 lambda 重写块（仅错误文案/回退重复） |
| **17** | （**未登记、本次实测发现**）构造函数同 arity 异类型重载被误判 E4031；方法重载同分静默取先声明者 | ctor mangling 只带 arity（`cpp_overload.rs:205-213`）；方法评分对「所有数值类型对」一律 2 分（`cpp/methods.rs:227-235`）且严格 `>`（`:186`） | **未修**。ctor 侧有显式 E4031（`:279-289`，与「显式拒绝优于静默错布局」哲学一致）；方法侧**静默** | **会复发**：同 #4 的根因（名字 / 评分不含足够信息）。新项目必须一次做对 ctor 与 method |

---

## ⑦ MoonBit spike 清单

> 环境事实（本次实测）：本机已装 `moon 0.1.20260915 (2e1a46d 2026-09-15)`，core lib 源码在 `C:\Users\liangjingwei\.moon\lib\core\`（**可直读源码验证**）；`clang 22.1.4` 默认 `-ftemplate-depth` 实测 = **1024**。
> 每条：依赖的语言特性 → 最小验证程序 → 判定标准。**spike 工程应建在仓库外的临时目录**（本次勘察为只读，未创建任何工程）。

| # | 语言特性 | 最小验证程序 | 判定标准 |
|---|---|---|---|
| **S1** | **`Map` / `HashMap` 迭代确定性**（迁移期差分对账的前提；对应 §⑥-11） | ① 同一进程内：插入 200 个 `(String,Int)`，`map.iter()` 收集键序，重复 3 次建 map → 断言三次序列相同。② 跨进程 / 跨宿主：同一程序编译到 `wasm-gc`（Node 宿主）与 `native`（C 后端）各跑 5 次，输出键序的 SHA-256。③ 对照实验：`HashMap` vs `LinkedHashMap`（`@builtin.Map`） | **通过判据**：(a) 同一宿主内 5 次 digest 全同；(b) **跨宿主 digest 也全同**（若不同 → **必须**在符号表 / 实例化缓存 / 布局表全面改用 `LinkedHashMap` 或显式排序）。**已可从源码预判**：`hasher.mbt:98-99` 的 `#cfg(any(target="wasm", target="wasm-gc")) let seed : Int = 0` vs `:87-95` native/llvm 随机；故 (b) **预期会红** → 直接把「显式有序容器」写进架构约束。**另需实测**：`HashMap::iter` 槽位序（`utils.mbt:75-92`）是否随 `capacity` 增长而变（`capacity` 由插入序列决定 → 若「插入序相同但 capacity 不同」也会变序） |
| **S2** | **递归遍历 / 递归类型检查的栈深度**（对应 §⑥-3） | ① 深左结合表达式：`a+a+...+a`（1k / 5k / 20k 项）→ 走一遍 `resolve_expr_type` 等价递归；② 深嵌套 `if` / 复合字面量（1k 层）；③ 病态递归类型 `struct N { N next; }` 检测路径（环非深度）；④ 深层模板实例化链（`f<int>` → `f<int*>` → …）到 1024 | **通过判据**：20k 项左结合链与 1k 层嵌套**不崩且给确定性诊断 / 正常返回**；递归模板在 ≤1024 层给出「深度超限」而非 OOM。若原生栈不足 → 架构约束：**所有 AST 遍历迭代化 + 显式深度预算**（对应路线图 U6#4「深递归迭代化评估」） |
| **S3** | **`enum` + `match` 穷尽检查的工程收益**（对应 §0 P-2 的 lowering 形态） | 把 `vitro_ast::Type` 的 **17 个变体**与 `Expr` 变体各写成 MoonBit `enum`，实现 `mangle(ty)` 与 `kind(ty)`；然后新增一个变体（如 `Type::Decimal`）看编译器报几处错 | **通过判据**：新增变体后编译器**列出全部未覆盖点**（预期：`mangle` / `kind` / `mangle_name_into` 等 3~5 处 + typeck 内所有 `match ty`）。这条直接量度「MoonBit 能否把 §③-1 R6 与 §⑥-4/5 的『漏一个分支就静默错值』变成编译期不通过」 |
| **S4** | **`Int32` 算术溢出语义**（影响 `compute_type_size` 的乘加、`evaluate_constexpr`、数组尺寸、N7） | `let a : Int = 2147483647; println((a + 1).to_string())`；`let d : Int = -1; let u = d.reinterpret_as_uint()`；`2000000000 * 2`。分别在 **debug 与 release**、**wasm-gc 与 native** 四种组合下跑 | **通过判据**：四种组合**语义一致**且**有明确文档化的规则**（trap / wrapping / 提升到 Int64 三者择一，必须显式）。任一组合与其他不同 → 架构约束：**尺寸与常量折叠链全部改 `checked_*` + 显式溢出诊断**（与路线图 U1#9 / U5#4 同族，本模块新增 `decl.rs:661-663` 一处） |
| **S5** | **`String` 不可变 + UTF-16 内部表示 vs 字节流语义** | ① `let s = "中文abc"; s.length()` 与 `s.to_bytes().length()` 的差；② 按**字节列**截取 `line`/`column`：构造一行含中文的源码，取第 N 个字符的列号，与 Rust 侧 `column`（C 源码是字节流）对照；③ 名字前缀匹配 `starts_with("__ctor__")` / `ends_with("__move")` 在 UTF-16 下的行为 | **通过判据**：能给出**byte ↔ column 的确定性换算函数**且与 Rust 版逐条对齐（诊断的 `line`/`column` 是 §⑧ 对账字段）。若 `String` 的 UTF-16 使列号按 UTF-16 code unit 计 → 必须引入 `Bytes` / `StringView` 层做列号计算，否则**含中文注释的源文件诊断列号全错**（本仓库已有「Windows GBK 炸中文诊断」的历史事故族） |
| **S6** | **可变状态与「AST 就地改写」**（本模块最核心的移植问题：88 处就地赋值） | ① 试写 `fn resolve_assign(ast : Program, ...) -> Program`（纯函数，返回新 program）；② 试写 `Ref[Program]` + `mut` 版本；③ 对 1000 语句的源码各跑 100 次，量时间与分配 | **通过判据**：纯函数版**不出现 O(n²) 退化**（结构共享生效：只重建路径上的节点）。若退化严重 → 采用「可变 `Array` 承载 AST 节点 + 索引引用（arena）」形态。**这是决定 §⑤-1「纯函数变换」可行与否的前置 spike** |
| **S7** | **1MB 可变字节承载（迁移评估报告 A3，虽属 VM）** | 与本模块的接触点是 `compute_type_size` / 布局（`Int` 尺寸链）与容器字段 `n`/`m`/`a` 的偏移计算 → 只需确认 `Int` 位宽（32）与指针模型（4 字节）在 MoonBit 下可表达 | **通过判据**：`Int` = 32 位、`UInt` 可承载地址、指针模型 4 字节常量可编译期断言。否则布局计算与 Clang golden 的 `sizeof` 差异表需重写 |
| **S8** | **大 AST 的生命周期 / GC 压力** | 生成 10 万节点 AST，跑 5 次完整 typeck（含实例化展开），观测 wasm-gc 下 GC 次数与峰值内存 | **通过判据**：峰值内存与 GC 时间在预算内（与 Rust 版同量级）。这决定「不可变结构共享」能否替代 `Arc`（迁移评估报告 A5）在**本模块**的适用性 |

**已可从本机 core lib 源码预判的结论（不必等 spike 就有倾向）**：S1 的 (b) 项**预期会红**（native / js 随机种子），S2 与 S6 是**真正的未知**（本机源码无法预判），S3 / S5 的收益方向明确。

---

## ⑧ 等价性验收锚点

### 8.1 先说缺口（必须新建的能力）

**当前全仓没有任何 AST / 符号表 dump 出口**（本次实测：`dump_ast` / `ast_json` / `dump-ast` 与 `ProgramNode` 的 serde 出口 = **0 命中**；`vitro_cli` 只有 `compile` / `run` / `step` / `unified` / `export` / `serve`，见 `vitro_cli.rs:654/691/879-898`）。
⚠️ **但 AST 已具备 serde 派生**（`vitro_ast` 内 `serde::Serialize` 26 处，`Expr` 在 `expr.rs:71`）→ **加一个 dump 出口成本很低**，这是 §⑧ 全部锚点的前提，**必须在迁移期开始前落地**（趁 Rust 版还在）。

### 8.2 锚点表

| # | 对账对象 | 内容 | 格式 | 工具 | 判据 |
|---|---|---|---|---|---|
| **E1** | **诊断序列**（最高优先级） | typeck 产出的 `(code, line, column, message)` **有序数组**（顺序 = push 顺序，当前确定：`errors` 是 `Vec`，`lib.rs:519-526`） | JSON Lines，每行一条；**显式排序** `(file_id, line, column, code, seq)` | 新建 Go 差分驱动（**尚未创建**）读两侧 JSONL 逐行 diff | **逐行完全一致**（含顺序）。允许差异：无（若必须容忍，须显式白名单 + 逐条理由） |
| **E2** | **符号表** | `funcs` / `classes`（含 `fields` / `methods` / `size` / `has_resource` / `vtable`）/ `structs` / `unions` / `templates` 的 `(name → 签名)` 快照 | JSON；**数组化 + 按 name 排序**（不用 JSON object，避免键序不确定——§8.3 的教训） | 同上 | 逐字段一致；`size` 数值一致；`methods` 键集合一致 |
| **E3** | **mangled 名集合** | 全部产出名（`{Class}__{method}[__suffix]`、`__ctor__*`、`__dtor__*`、`__lambda_N__call`、`vitro_vec__class_Foo` 等） | 排序后的字符串数组 | 同上 | 集合完全相等（这是 U3#6 单源化的验收） |
| **E4** | **类型化 AST（lowering 结果）** | typeck 后的 `ProgramNode`（含插入的 `Expr::Cast`、`Identifier→Member{this}`、mangled `Call` 名、推断出的数组 `dims`） | **规范化 JSON**：① 剥离 `loc`（另存 `locs` 数组，便于单独对账）；② 对象键排序；③ `Type` 用 §② A7 的 ASCII 编码而非 `{:?}` | 同上 | 结构完全一致；`locs` 单独对账（允许差异须解释） |
| **E5** | **字节码产物** | `vitro_cli export` 的 bundle（`func_table` + globals 布局） | 现有 JSON | 现有 `scripts/precompile_bytecode_libc` + 新 diff | **必须先规范化键序**：`func_table` 是 `HashMap` 且直接 `to_string_pretty`（`vitro_cli.rs:308` / `:382`）→ 现状字节不稳定，稳定性靠消费侧排序（`precompile_bytecode_libc/main.go:84/191/279/303/349`）。锚点设计上要求 **MoonBit 侧原生输出有序结构** |
| **E6** | **端到端 stdout** | C 用例 675（`shadow_c_cases`，as_of 2026-09-15）+ C++ 用例 99（`shadow_cpp_cases`，as_of 2026-09-15） | 现有 shadow 报告 JSON | 现有 `scripts/shadow_verify` / `shadow_verify_cpp`（Go） | 复用既有口径：`match` / `known_issue` / `gap_extension` 三类；**非预期差异 = 0** |
| **E7** | **C++ 用例单独对照 Clang++** | **必须单独一路**（不可混入 C 侧） | `native/tests/shadow_verification/reports/cpp_shadow_report.json`（字段：`compiler` / `compile_success` / `compile_error` / `run_success` / `stdout` / `stderr`） | `scripts/shadow_verify_cpp` | 现基线 `shadow_cpp_cases` = 99 / `shadow_cpp_match` = 95（as_of 2026-09-15），**4 例已记录的 `clang_compile_fail`**（`cpp_vitro_vec_class` / `cpp_vitro_list_class` 用内置容器无法被 Clang++ 直接编译；`cpp_u3_class_instantiate_in_template` / `cpp_u3_vec_class_twice` 为 U3 类类型模板实参用例）→ 迁移期这 4 例的处置需单独裁定 |
| **E8** | **红→绿锚（语义级）** | U3 两锚 + typeck 定向 70 单测 + 10 内联单测 | 源码字符串 → `ErrorCode` 断言 | 重写 driver 层后跑 | 逐例一致（这是最强验收：断言的是**具体码 + 消息子串**，不可用「能编译」蒙过） |

### 8.3 迁移期必须遵守的两条方法论（本仓库已有实锤）

1. **HashMap 顺序假绿** —— `cpp_overload.rs:302-309` 记录的真实事故：产物布局不可重现，靠**逐字节 diff 产物**才发现。⇒ 所有对账产物必须**先规范化**（排序 / 数组化），且**对账工具自身要有 J9 埋雷记录**（判定型脚本无「注入必然违反 → 必红」记录者，其全绿不得作为结论依据）。
2. **平台相关排序** —— `pathlib` 在 Windows 是 casefold 序、Linux 是码点序（D5 迁移期实测的口径教训）。⇒ 新 diff 工具统一 casefold / 码点序之一并**在报告里声明**。

### 8.4 裸奔期风险提示

引用迁移评估报告 §3.3：防线 1~5 在 MoonBit 下重建期间检测能力为 0，而「跨层语义失配 / 启发式判据误判 / 时序与生命周期」三类易错点**全部是「不报错只错值」**。本模块的 §⑥-1/4/5/6/7/8/11 全部属此类 → **E1 / E2 / E3 / E4 四项（类型检查后的中间产物对账）应在迁移最早期建立**，不能等类型检查器写完。

---

## ⑨ mooncakes 包切分草案

### 9.1 现状耦合实测（切分依据）

- 单 crate、35 文件、8,816 行，内部模块 18 个（含子目录）。
- **跨 crate 的隐性契约**（必须显式化）：
  - **mangled 名形状跨 3 个 crate**：typeck 产名（26 处裸 `format!` + 3 个 helper）、**parser 也产名**（`vitro_parser/src/stmt.rs:226/228/269/271`、`expr/postfix.rs:28/38`）、codegen 消费名（`std__move` 字面量在 `vitro_codegen/src/expr.rs:428`、`expr/call.rs:267`）
  - **签名 4 套真相源**（O6）：头文件存根 / `visit_call` 硬编码 / `bytecode_libc_sig` / codegen 两张表
  - **容器布局两条路径并存**：JSON（5 个 POD 类，A16）**与** Rust 硬编码合成（类类型实参，895 行，O11）
- **依赖方向现状**：`typeck ← {shared, ast, runtime, cpp_frontend}`（`vitro_lexer` 为幽灵依赖）；`typeck → codegen` 无直接边，但通过 mangled 名 + `program` 变异形成**隐式契约**。

### 9.2 建议包切分（8 个包，单向依赖）

```
vitro/lang      ← SourceLang enum + LanguageProfile（隐式转换表 / 可见性 / 名字规则）
vitro/diag      ← ErrorCode / Severity / Span / Diagnostic / catalog（语言中立）
vitro/ast       ← Type(17 变体) / Expr / Stmt / Decl + 类型谓词 + 尺寸布局纯函数
vitro/names     ← ★新：mangled 名唯一产出口（InstKey → InstId → Name），parser / typeck / codegen 共依赖
vitro/typeck    ← 主包：作用域 / 符号表 / 声明 / 表达式 / 初始化器 / lowering（C 语义 + 语言 profile）
vitro/cpp       ← 类布局 / 重载 / 引用 / lambda / 模板单态化 + 模板实例化缓存
vitro/containers← 内置容器（★目标：JSON 布局 + 源码化实现，单一数据驱动路径）
vitro/libc      ← libc 原型（★目标：单一签名表，删 4 套真相源）
```

依赖方向（严格单向，无环）：

```
lang ──┐
diag ──┼──→ ast ──┬──→ names ──┐
       │          │            ├──→ typeck ──→ cpp
       │          │            │        │
       └──────────┘            │        └──→ containers
                               └──→ libc ──┘
```

**依赖规则**：
- `names` 只依赖 `ast`：**任何产名逻辑只能在这里**（含 parser 的 `__ctor__{name}__{argc}`；parser 改为调 `names.ctor(class, argTypes)`）。这条直接消掉 §③-2 S8 与 U3#6。
- `cpp` 依赖 `typeck` 与 `names`；`typeck` **不得**依赖 `cpp`（现状 `lib.rs` 直接调 `cpp_class_layout` / `cpp_overload` / `cpp_monomorph` —— 反向耦合点已列出）。
- `containers` 与 `libc` 只依赖 `ast` + `names`，**数据驱动**（JSON / 源码资产），不含手写实现（消掉 895 行硬编码）。

### 9.3 对上发布形态

- **mooncakes 模块名**：`vitro`（单模块），子包按目录 `vitro/typeck`、`vitro/cpp` …
- **`.mbti`（公共接口）**：只暴露 3 个表面 ——
  1. `vitro/typeck.check : (Program, CheckOptions) -> (Program, Diagnostics)`（`CheckOptions` 含 `SourceLang`、`is_library_mode`）
  2. `vitro/diag.Diagnostic` + `ErrorCode`（诊断契约，供 capi / serve / wasm 三出口共用）
  3. `vitro/ast.Program` 与 `vitro/names.Name`（供 codegen 与差分对账消费）
  其余全部内部（`typeck` 的符号表 / lowering 细节不进 `.mbti`）。
- **测试形态**：黑盒 `*_test.mbt`（对 `check` 的输入输出，即 §⑧ E1 / E2 / E4 的载体）+ 白盒 `*_wbtest.mbt`（`convert` 判据表、`names` 产名唯一性、实例化缓存命中率）。
- **对上承诺的确定性**：`.mbti` 文档中**明写**「诊断序列与符号表输出有序、跨宿主一致」，并要求 `names` / 符号表使用 `LinkedHashMap` 或显式排序（S1 的结论）。
- **发布节奏**：与 §⑧ 的四项锚点绑定——**锚点未全绿不发版**；每版附 `Diagnostics` / `Name` 表快照（可机器 diff）。

---

## 附录 · 待证清单（含验证法）

| # | 待证结论 | 验证法 |
|---|---|---|
| 1 | 函数式构造缺陷的**确切错误码**（登记原文 E3023 vs 代码路径指向 `decl.rs:856` E3036） | `vitro_cli compile` 一个 `class Point{...}; int main(){Point p; p = Point(3,4);}` 观察码 |
| 2 | `int a[][3]={1,2,3,4,5,6};` 的 `sizeof`（Vitro 预期 72 / clang 24） | 新建 baseline 用例跑 `go run ./scripts/shadow_verify` |
| 3 | `char s[]="hi"; s[9]='x';` 是否漏越界 trap | `vitro_cli run` 对比 `char t[3]="hi"; t[9]='x';` |
| 4 | `int a[2][]={1,2};` 在 debug 下是否因 `dims` 含 -1 导致 `as usize` 乘法溢出 panic | debug 构建单测直连 `TypeChecker::check` |
| 5 | `int n = printf("hi\n");` 是否被拒（N3） | 新增 `baseline/printf_return_value.c` 跑 shadow |
| 6 | `#include <stdlib.h>` 后 qsort 是否走非专用检查器（O7） | 对照含 / 不含头文件的错用 qsort 用例 |
| 7 | `char a[2][2]={{1,2},{3,4}}` 是否产生 1 条 W3053 误报（§⑥-13 残留） | 单测断言 warnings 长度 |
| 8 | `decl.rs:661-663` NTTP 常量折叠溢出是否 panic（N7） | debug 单测 + `template<int N>` 溢出表达式 |
| 9 | `builtin_list.rs:79` 未同步 `instantiated_class_names` 是否有可触发路径（S5） | 构造「先注册 node 类、后 list 类查重」顺序的用例 |
| 10 | `ClassSymbol::base` 的 `#[allow(dead_code)]` 是否误标（R8） | 删 `allow` 跑 `cargo clippy -p vitro_typeck` |
| 11 | 类型检查后 AST 是否绕过 parser 的 `MAX_AST_DEPTH = 512`（`vitro_parser/src/lib.rs:103`） | 构造深实例化链的 debug 测试 |
| 12 | 三个出口诊断帧是否已含 `file_id`（决定 O4 的改造是否破坏协议） | 读 `native/src/capi/`、`src/session_api.rs` 的错误帧构造 |

---

### 本次勘察的方法学留痕

- 所有结论均来自对源码的直接阅读（`read` / `grep` / `glob` / PowerShell 计数），**未编译、未运行测试、未 git 操作、未创建或修改任何文件**。
- 三处外部证据为只读引用：`C:\Users\liangjingwei\.moon\lib\core\{builtin\hasher.mbt, hashmap\utils.mbt, builtin\linked_hash_map.mbt}`（MoonBit core lib 源码）、`clang 22.1.4` 的 stdin 探针（**未落盘**）、`reports/facts.json` 与 `reports/engineering_health.md`（产物真值）。
- 与任务书前提不符的 3 条已在 §0 显式纠偏（`is_cpp_mode` 不在本 crate / 语言分派非第四职责 / D14 代码侧已修），未做粉饰。
