# MoonBit 迁移 · `vitro_parser` 模块勘察报告（2026-09-18）

> **勘察对象**：`native/crates/vitro_parser/` 全部源码（10 文件 4,720 行）
> **勘察时间**：2026-09-18
> **勘察性质**：**只读勘察 + 只读探针**。未修改 Vitro 仓库任何文件（本报告除外）；未执行 git 写操作。
> **数字口径**：本文件为 **AS-OF 冻结**（文件名含日期）——文内"本次实测"数字为 2026-09-18 现场测量值，不参与 `scripts/facts` 回填；
> 引用全局真值一律标 `reports/facts.json` 的 key 与 `as_of`（该文件 `generated_at` = 2026-09-18T13:01:41+08:00，`git.rev` = `4b57191`）。
> **上级文档**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md)（本报告为其 §7 阶段 1「逐 crate 平移」中 `parser` 站的裁定材料）。
> **姊妹报告**：`MoonBit迁移_lexer模块勘察报告20260918.md`（上游接口）、`MoonBit迁移_shared与ast模块勘察报告20260918.md`（AST 归属）、`MoonBit迁移_typeck模块勘察报告20260918.md`（下游消费者）。

---

## 0. 摘要与关键判定

### 0.1 一句话结论

**本模块的语法设计资产质量高、可整体平移；真正的风险不在"能不能写"，而在"防护形态照抄会原样复刻缺陷"——本次实测抓到一条仍在役的合法代码崩溃缺陷（声明符类型通道无任何深度防护），并证明其成因是"保险丝靠人工枚举挂点"的方法论失效，而非某个 Rust 细节。**

### 0.2 关键判定（含对既有记载的修正）

| # | 判定 | 依据 |
|---|---|---|
| J1 | **声明符"类型通道"无深度防护 → 合法 C 代码栈溢出崩溃**（活缺陷，零诊断）。实测边界：具体声明符 **1200 通过 / 1300 崩**；typedef 未使用形态 1000 通过 / 1500 崩；形参/字段/抽象声明符形态同量级崩 | §6-11、附录 A.1 |
| J2 | 该崩溃**与下游 typeck 无关**（typedef 未被使用、typeck 永不遍历该 `Type`，仍崩）→ 排除"下游遍历"假设；**与 `node_cross_count` 无关**（抽象声明符路径跳过它，阈值不变） | 附录 A.1 反证组 |
| J3 | **`DeclaratorGuard.suffix_count` 是死字段**（声明 `lib.rs:39`、递增 `type_.rs:384/396`、**全 `native/` 树零比较**）——2026-09-12 评估报告已登记（H2），**至今未修**（本次复核） | §3.2-Y3、§6-11 |
| J4 | **后置 AST 深度预算（`MAX_AST_DEPTH`）不遍历 `Type`**：`vitro_ast/src/depth.rs` 的 `Stmt::VarDecl` 分支只看 `init`/`extra_vars`（`:130-139`）→ 类型通道既无前置保险丝也无后置兜底 | §3.2-Y2 |
| J5 | **零进度保护覆盖完整**（本次实测 5 种历史死循环形状全部正常收敛、无挂死）；但活性是**三层约定**而非单一不变量，且 `synchronize` 自身可零推进 | §1.4、§6-2、§6-3 |
| J6 | **K&R 旧式函数定义不支持**（实测 `int f(a,b) int a,b;{...}` → E2005）——`c_e2e_knr_cases` = 81（as_of 2026-09-18）是"现代风格用例数"，不等于"支持 K&R 定义语法" | §2 族清单、附录 A.2 |
| J7 | **21 个正向语法族实测 20 通过**（唯一失败 K&R）；`stdarg` 全族**不在 parser**（lexer 宏层实现，parser 内 `va_` 出现 0 次） | 附录 A.2 |
| J8 | **MoonBit 递归下降栈余量远大于 Rust 现状**：21 帧/层瀑布上限 wasm-gc **854 层** / js **538** / native **1005**，vs Rust 现自限 **62 层**（`MAX_PARSE_DEPTH`=256÷4）→ 现有常量可原样沿用（余量 8.7×~16×） | §7-S1 |
| J9 | **栈溢出在 MoonBit 下同样不可捕获**：`thread 'main' has overflowed its stack`、`exit = -1073741571`（0xC00000FD）——**与 Rust 版逐位相同** | §7-S1 |
| J10 | **`String` 语义是 P0 移植坑**：`"中文".length()` = **2**（UTF-16 码元）、`.to_bytes().length()` = **4**（UTF-16 字节）；Rust 版 `primary.rs:146` 用的是**字节长度 6**。直译将使 `char s[]="中文"` 数组尺寸少 4 字节。解药：`@utf8.encode(...).length()` = **6**（已实测） | §7-S5 |
| J11 | **`Int` 静默回绕 + 除零 trap**：`2147483647+1 = -2147483648`、`1<<1000 = 256`（位移量 mod 32）、`1e6² = -727379968`；`x/0` → `RuntimeError: divide by zero`。→ `eval_enum_const` 的 `checked_*` 纪律必须保留 | §7-S6 |
| J12 | **多错误收集模型成立**：`suberror` + `raise` + `try/catch` + `Array` 累积实测可"报错后继续并继续累积"（6 次调用第 3 次抛错 → 累积 1 条、累加值跳过错项 = 12） | §7-S2 |
| J13 | **`match` 穷尽检查是硬门禁**：116 变体 enum 缺 3 支 → `Error: [0011] partial_match`，**构建失败**（默认即 error）。**但诊断不列出缺失变体名**（`some hints:` 为空） | §7-S3 |
| J14 | **GC 无递归释放**：1e6 深链迭代构建 + **迭代遍历返回 1000000** 均成功；而**递归遍历在 1e5 崩** → 迭代式 DFS 必须保留，且 Rust 的 `mem::forget(prog)` 避险在新语言下**不需要** | §7-S7 |
| J15 | **MoonBit 工具链 ICE（一手实证）**：跨文件顶层 `pub let` 被 `main` 引用时，`moonc v0.10.13` 在 **link-core 阶段**崩：`Moonc.Basic_hash_string.Key_not_found("$_M0FP45…mode")`。常量内联即消失 | §7-S8 |

---

## 1. 模块概览与规模

### 1.1 实测规模

| 文件 | 行数 | 字节 | 角色 |
|---|---|---|---|
| `src/lib.rs` | 735 | 28,978 | Parser 状态机 / token helper / 顶层分发 / 深度防护基础设施 |
| `src/decl.rs` | 865 | 36,915 | 声明族（struct/union/class/template/func/typedef/enum）+ `eval_enum_const` |
| `src/type_.rs` | 739 | 33,274 | 类型说明符、声明符螺旋规则、参数列表、构造初始化列表 |
| `src/stmt.rs` | 718 | 27,439 | 语句族 + `_Static_assert` + `[[属性]]` 跳过 |
| `src/expr/ops.rs` | 501 | 16,677 | 赋值/三目 + 11 级二元优先级链 |
| `src/expr/unary.rs` | 305 | 10,778 | 一元、cast、sizeof/alignof/offsetof、抽象声明符 |
| `src/expr/postfix.rs` | 303 | 11,371 | 后缀链、new/delete、lambda、初始化列表 |
| `src/expr/primary.rs` | 299 | 12,142 | 字面量、`_Generic`、复合字面量、标识符 |
| `src/cpp.rs` | 223 | 8,561 | C++ 状态初始化 + 顶层 C++ 入口下沉 |
| `src/expr/mod.rs` | 32 | 779 | `parse_expression` / `parse_comma` |
| **合计** | **4,720** | **≈182.5 KB** | 10 个文件 |

来源：逐文件 `(Get-Content).Count`。与评估报告附录 B 的「`native/` 下 257 文件 / 76,218 行」（AS-OF 2026-09-18，冻结）对比，本模块占 **6.2%**。
crate 依赖仅 3 个：`vitro_shared` / `vitro_ast` / `vitro_lexer`（`Cargo.toml`）。

### 1.2 关键类型

| 类型 | 位置 | 说明 |
|---|---|---|
| `pub struct Parser` | `lib.rs:69-82` | 10 字段：`tokens: Vec<Token>`、`errors: Vec<ParseError>`、`typedef_names: HashMap<String, Type>`、`template_names: HashSet<String>`、`pos: usize`、`anonymous_structs: Vec<StructDecl>`、`is_cpp_mode: bool`、`next_lambda_id: u64`、`current_class: Vec<String>`、`recursion_depth: i32` |
| `pub struct ParseError` | `lib.rs:11-17` | `{ message: String, line: i32, column: i32, code: i32 }` |
| `pub(crate) enum DeclaratorNode` | `lib.rs:20-27` | `Base / Pointer / Reference(is_const) / RValueRef / Array(Option<Box<Expr>>) / Function(Vec<Param>, bool)` |
| `pub(crate) struct DeclaratorGuard` | `lib.rs:35-41` | 保险丝组：`paren_depth / ptr_count / suffix_count / cross_count` |
| `pub(crate) struct Rollback` | `lib.rs:62-67` | `Copy`；三元快照 `{pos, errors_len, anon_len}` |
| `pub(crate) const MAX_PARSE_DEPTH` | `lib.rs:96` | `256` |
| `pub(crate) const MAX_AST_DEPTH` | `lib.rs:103` | `512` |

**公共 API 面（crate 外可见的全部）**：`Parser`、`ParseError` + 方法 `new` / `with_mode` / `parse`（`lib.rs:138-163, 189`）。其余全部 `pub(crate)`。crate 由 `native/src/compiler/mod.rs:11` 以 `pub use vitro_parser as parser;` 挂回 `vitro_native::compiler::parser`。

### 1.3 递归下降结构（实测计数）

- **`parse_*` / `try_parse_*` 定义点 76 个**，其中 **7 个是深度防护壳的配对体**（`parse_statement_inner` `stmt.rs:91`、`parse_assign_body` `ops.rs:15`、`parse_ternary_body` `ops.rs:148`、`parse_unary_body` `unary.rs:15`、`parse_primary_inner` `primary.rs:17`、`parse_init_list_body` `postfix.rs:249`、`parse_abstract_declarator_body` `unary.rs:168`）→ **69 个实质解析函数**。
- **表达式优先级瀑布 21 帧/层**：`parse_expression`(`expr/mod.rs:9`) → `parse_comma`(:13) → `parse_assign`(`ops.rs:4`) → `parse_assign_body`(:15) → `parse_ternary`(:137) → `parse_ternary_body`(:148) → `parse_or`(:170) → `parse_and`(:190) → `parse_bit_or`(:210) → `parse_bit_xor`(:230) → `parse_bit_and`(:250) → `parse_equality`(:270) → `parse_relational`(:308) → `parse_shift`(:374) → `parse_additive`(:412) → `parse_multiplicative`(:450) → `parse_unary`(`unary.rs:4`) → `parse_unary_body`(:15) → `parse_postfix`(`postfix.rs:146`) → `parse_primary`(`primary.rs:4`) → `parse_primary_inner`(:17)。11 级二元：`Or/And/BitOr/BitXor/BitAnd/Eq-Ne/Lt-Le-Gt-Ge/Shl-Shr/Add-Sub/Mul-Div-Mod`。
- **声明符递归**：`parse_declarator_node`（`type_.rs:274-441`，自递归 `:366`）+ 后置 `interpret_declarator_node`（`type_.rs:467-609`，在独立递归树上二次遍历，递归点 `:471,478,482,485,500,504,561,590,595`）。

### 1.4 检查点 / 回溯 / 零进度保护（全部 20 处）

`Rollback` 回滚面 = **pos + errors 长度 + anonymous_structs 长度**（`lib.rs:58-61`，U1#10 修掉的"匿名 struct 二次 push"缺陷）。

| 类别 | 位置 | 回滚内容 |
|---|---|---|
| 顶层前瞻 | `lib.rs:420/429/433`、`:457/469`、`:476/503/517` | 仅 pos 类前瞻 |
| 函数/变量前瞻 | `lib.rs:563`→`589` | `parse_base_type` 之后回滚到声明起点 |
| 语句级 | `stmt.rs:438`→`457`（C++ range-for 前瞻） | 类型 + 引用符前瞻 |
| 声明符/类成员 | `decl.rs:238`→`248`、`:320`；`:477`→`484` | 方法/字段、模板类名判定 |
| 表达式试探 | `unary.rs:32`→`53`（cast）、`:233`→`255`（alignof）；`primary.rs:253`→`273`（复合字面量） | 三处**额外手工补** `typedef_names` 快照 |
| **例外** | `unary.rs:185`→`206`（`parse_sizeof`） | **只 `restore(checkpoint)`，无 `typedef_names` 快照** |

**零进度保护的实际形态（6 处，与 `AGENTS.md` 编码约定不完全一致）**：

| 位置 | 实际代码 | 备注 |
|---|---|---|
| `stmt.rs:152-177` | `if self.pos == checkpoint.pos { self.synchronize(&[…]) }` | **不是 `advance()`**，而是跳到语句边界（约定文档写的是 `advance()`） |
| `stmt.rs:187-196` | `if self.pos == stmt_checkpoint { self.advance(); }` | 块内语句循环 |
| `stmt.rs:681-685` | 同上 | case 体语句循环 |
| `decl.rs:8-14` | `if self.pos == field_checkpoint { self.advance(); break; }` | 结构体字段循环；**break 会放弃剩余字段体** |
| `primary.rs:290-293` | 报 E2003 后 `if !self.is_at_end() { self.advance(); }` | 表达式兜底 |
| `lib.rs:535-543` | 顶层 `else` 报错后无条件 `self.advance()` | **顶层活性的最终保证** |

> **活性是三层结构，不是单一不变量**：① 各分支自身推进；② 回退点后的 `pos == checkpoint` 检查；③ 顶层循环的 `else → advance()`。
> 且 `synchronize`（`lib.rs:311-351`）**不保证推进**——`lib.rs:317-319` 在 `previous().ty == Semicolon` 时直接 `return`，pos 不变。这是理解历史死循环的关键（§6-2/§6-3）。

### 1.5 错误处理形态

- **不 panic**：生产路径 **0 处 `unwrap`/`expect`/`panic!`**；**28 处 `self.errors.push(ParseError{…})`**；唯一 `unreachable!()` 在 `decl.rs:129`（枚举已穷尽，实际不可达）。
- `parse()` 返回 `(Option<ProgramNode>, Vec<ParseError>)`（`lib.rs:189`）——**收集而非首错即停**，错误后继续解析（`consume`→`synchronize`，`lib.rs:276-308`）。
- **无错误数量上限**（对比 lexer 有 token/字节预算）：errors 只受 token 数约束（O(n)）；`restore` 截断试探期错误（`lib.rs:134`）。
- **上层语义**：`compile_pipeline.rs:427-430` / `:658-661` 一旦 `parse_errors` 非空即 `return Err("语法错误")`，**已构造的 AST 被丢弃**。即"多错误收集"只服务诊断输出，不服务 AST 交付。

### 1.6 与 lexer / typeck 的接口面

**入口（lexer → parser）**
- `Parser::new(Vec<Token>)` / `with_mode(Vec<Token>, is_cpp_mode)`（`lib.rs:138-142`）。
- `Token { ty: TokenType, text: String, line: i32, column: i32 }`（`vitro_lexer/src/token.rs:129-135`）；`TokenType` **116 变体**（`token.rs:5-126`，脚本计数 116；本报告 §7-S3 用同名 116 变体在 MoonBit 中重建并编译通过，交叉验证）。
- 访问面 8 个原语：`peek(offset)`(`lib.rs:223`)、`current`(:236)、`previous`(:239)、`check`(:246)、`is_at_end`(:253)、`advance`(:257)、`match_token`(:267)、`consume`(:276)、`synchronize`(:311)。
- **越界语义**：`peek` 越界返回 `static EOF: Token { ty: Eof, text: "", line: -1, column: -1 }`（`lib.rs:224-232`）——EOF 不是哨兵对象，迁移时必须保持等价（诊断行号可为 -1）。
- `is_type_token()`（`lib.rs:353-390`）= 15 关键字 + C++ 模式下 `class`/`auto` + 6 种 `typeof` 拼写 + `typedef_names` 查表 → **"token 是否类型名"是上下文相关的**。
- **C++ 分派入口不在 parser 内**：`is_cpp_mode` 由 `compile_pipeline.rs:631-634` 按**文件扩展名**（`.cpp/.cxx/.vitrocpp`）判定，lexer 与 parser 各自 `with_mode` 收到同一布尔值（`:643 / :657`）。
- 唯一的 token 流"重写"hack：`parse_template_arg_expr`（`lib.rs:655-684`）用 `std::mem::take(&mut self.tokens)` + 切片替换 + 相对位置回填。

**出口（parser → typeck）**
- `ProgramNode`（属 `vitro_ast::decl::ProgramNode`，`vitro_ast/src/decl.rs:199`），字段 `globals / funcs / structs / unions / classes / templates / template_instantiations`；`parse()` 把 `anonymous_structs` append 进 `structs`（`lib.rs:192`）。
- AST 规模：`Expr` **26 变体**（`vitro_ast/src/expr.rs:72-236`）、`Stmt` **16 变体**（`stmt.rs:8-90`）、`Type` **17 变体**（`types.rs:32-104`：`Void/Int/Char/Float/Double/LongLong/Pointer/Array/Function/Struct/Union/Class/Reference/RValueRef/Auto/TemplateId/Typeof`，逐个数过）、`TypeKind` 16、`UnaryOp` 9、`BinaryOp` 19、`AssignOp` 11。
- **AST 全类型已 `serde::Serialize + Deserialize`**（`vitro_ast` 内 27 处 serde derive）→ §8-E1 的 AST dump 锚点无需新增序列化设施，只需一个 dump 入口（**当前不存在**，实测 grep `dump_ast`/`ast_dump` 零命中）。

**调用方（5 处，其中 2 处为编译管线双轨）**
`engine/compile_pipeline.rs:426`（单文件轨）、`:657`（多文件轨）、`engine/completion/mod.rs:161`、`compiler/cfg.rs:446`、`compiler/data_flow.rs:259`、`compiler/intent.rs:364`。

### 1.7 测试资产实测

| 位置 | `#[test]` 数 | 性质 |
|---|---|---|
| crate 内 | **0**（`#[cfg(test)]` 亦 0） | 模块自带零测试 |
| `native/tests/parser_unit_test.rs` | **17** | 黑盒：`parse(src)` → 断言 AST 形状 |
| `native/tests/parser_cpp_unit_test.rs` | **33** | 黑盒 C++（`CPP_FAILURES.md` 记 33/33 通过） |
| `native/tests/crash_regression_tests.rs` | 48（其中 9 条为解析深度族） | 崩溃止血（深括号/深花括号/多星号/深初始化/深赋值/深一元/后缀链/左结合链 + 反向锚） |

---

## 2. 可复用资产清单

| # | 资产 | 位置 | 移植成本 | 理由 |
|---|---|---|---|---|
| R1 | **表达式优先级瀑布的层级划分与结合性** | `expr/ops.rs:170-500`、`expr/mod.rs:13-31` | **低** | 纯语法知识，21 函数一一对应；MoonBit `match` 穷尽反而更短 |
| R2 | **声明符螺旋规则（节点树 + 从内到外解释）** | `type_.rs:274-441` + `:467-609` | **中** | 语义设计完整可搬；但 `DeclaratorNode` 的嵌套 + **两次独立递归遍历**需合并为单次（§5-M2 同时解决 J1） |
| R3 | **错误码映射表**（缺失闭合符 → E2006/E2007/E2008/E2005） | `lib.rs:280-292` | **低** | 纯数据表 |
| R4 | **错误恢复集合 + `synchronize` 同步集**（语句起始 token 全集 26 项） | `lib.rs:300-306`、`:311-351` | **低** | 纯数据 + 纯循环 |
| R5 | **深度防护的"壳函数"模式**（`enter_depth`/`leave_depth` + 中性占位返回） | `lib.rs:169-187` + 7 个 `*_inner`/`*_body` | **中** | 模式有效，但挂点靠人工枚举、覆盖面**已被实测证伪**（J1）→ 移植时应改为统一入口收口，而非复制 7 个挂点 |
| R6 | **迭代式 AST 深度测量（显式栈 DFS）** | `vitro_ast/src/depth.rs:20-196` | **低** | 已是迭代实现（J14 证实必须保留）；覆盖面需扩到 `Type`（J4） |
| R7 | **token 级只读访问原语集合** | `lib.rs:223-274` | **低** | 8 函数，索引语义直译 |
| R8 | **C99/C11/C23 语法族判定逻辑** | 分散（见下表） | **中** | 判定顺序（如"`(` 后是否类型名"）是价值核心 |
| R9 | **enum/static_assert 编译期常量求值器** | `decl.rs:811-865` | **低** | 纯表达式折叠；`checked_*` 语义需在 MoonBit 重标定（J11） |
| R10 | **模板非类型实参的定界符扫描** | `lib.rs:690-728` | **低** | 纯扫描算法（跳过 `()[]{}` 与嵌套 `<>`），零语言特性依赖 |
| R11 | **测试用例资产（50 条黑盒 + 9 条崩溃锚）** | `native/tests/parser*_unit_test.rs`、`crash_regression_tests.rs` | **低** | 断言的是 AST 形状与"是否报错/是否崩溃"，与实现语言无关；`crash_regression_tests` 走 C ABI（需改成新出口） |
| R12 | **文档与注释口径** | `docs/current/01-定位与路线/架构设计.md:97-98`；`lib.rs:84-103` 注释 | **低**（但需修正） | 注释含实测口径（每层 ~3KB 栈、64 层上限），是重写标定基准；**其中"256÷4=64 层精确等价"的推导与实测不符**（§附录 C-2） |

### 支持语法族清单（逐条 file:line + 实测结论）

| 语法族 | 支持 | 证据 | 本次实测 |
|---|---|---|---|
| C99 for 声明 | ✅ | `stmt.rs:495-533` | ✅ exit 0 |
| VLA（一维/多维） | ✅ | `type_.rs:442-466` `array_dim_info` → `is_vla` | ✅ ✅ |
| 复合字面量 | ✅ | `primary.rs:251-278`；`unary.rs:31-55`（cast 歧义消解） | ✅ struct / 匿名 struct / `int[]` 三形态 |
| `_Generic` | ✅ | `primary.rs:172-206` | ✅ |
| 逗号运算符 | ✅ | `expr/mod.rs:13-31`；`parse_arg_list`/`parse_param_list` 刻意用 `parse_assign` 避开（`postfix.rs:296`、`type_.rs:661`） | ✅ |
| Designated Initializer | ✅ | `postfix.rs:257-266` | ✅ `.field` / `[idx]` |
| `offsetof` | ✅ | `unary.rs:22-24` + `:275-292` | ✅ |
| **`stdarg` 宏族** | ✅ **但不在 parser** | parser 内 `va_` 出现 **0 次**；实现在 `vitro_lexer/src/macros.rs:251/257`（`va_start`→`__vitro_va_start`）、`:361/397`（`va_arg`） | ✅ 全族可用 |
| `sizeof`/`_Alignof`/`alignof` | ✅ | `unary.rs:178-223`、`:226-273`（类型名/表达式双形态） | ✅ |
| `_Static_assert`/`static_assert` | ✅ | `stmt.rs:14-60`（真求值，失败 E1020）；顶层分派 `lib.rs:408-413` | ✅（真）/ ✅（假 → 拒绝） |
| `[[属性]]` | ✅（解析忽略） | `stmt.rs:63-77` + `lib.rs:415-418` + `stmt.rs:93-95` | ✅ |
| C23 `enum E : T` | ✅ | `decl.rs:540-557`、`:720-737`（白名单 int/char/long long 及 unsigned） | ✅ `: long long`；非法底层（float）正确拒绝 |
| `typeof` / `typeof_unqual` | ✅ | `type_.rs:58-74`（6 种拼写） | ✅ ✅ |
| 相邻字符串字面量拼接 | ✅ | `primary.rs:134-159` | ✅ |
| 科学计数法 / 无后缀 = double | ✅ | `primary.rs:92-115`（C89 语义修正） | ✅ |
| C23 `nullptr` | ✅ | lexer 关键字 + `primary.rs:160-171` | ✅ |
| 数组形状合法性（负/零/无尺寸） | ✅ 拒绝 | `type_.rs:448-460`（U1#12 折叠负字面量） | ✅ E2002 / 拒绝 / 拒绝 |
| **K&R 旧式函数定义** | ❌ | 无标识符形参列表分支（`type_.rs:610-680` 只处理类型参数） | ❌ E2005（J6） |
| **C++ 分派入口** | ✅ | 三层：① 顶层 `lib.rs:522-525`；② `struct` 分支内 `lib.rs:506-507`；③ `cpp.rs:151-222` 三下沉入口 + `cpp.rs:53/102` 两试探入口。另有 `lib.rs:555`（`look_ahead_skip_stars` 的 C++ 分支）、`unary.rs:25-30`（new/delete）、`primary.rs:207-217`（this/lambda）、`postfix.rs:85-144`（lambda）、`type_.rs:156-181`（模板 id）、`stmt.rs:436-493`（range-for） | C++ 族不在本次逐条实测范围（姊妹报告 `cpp_frontend` 覆盖） |

---

## 3. 抛弃清单

### 3.1 Rust 特有机制（无法直译，需换形态）

| # | 机制 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| X1 | **`std::mem::take(&mut self.tokens)` + 切片替换做子流解析** | `lib.rs:675-683` | MoonBit 无所有权移动，无法"临时掏空字段再放回"；这是借用检查器的绕道写法 | **中**：等价形态是显式 `(tokens, start, end)` 视图或独立子解析器。语义必须保持：`self.pos = saved_pos + consumed`（`:682`）——**最易搬错的一处** |
| X2 | **`std::mem::forget(prog)` 规避递归 Drop** | `lib.rs:212` | GC 语言无递归 `Drop`，该 hack 整体消失（J14 已实测 GC 侧无递归释放） | **低（收益）** |
| X3 | **`Box<Expr>` / `Box<Stmt>` 显式装箱** | 全模块 | MoonBit enum 递归变体不需要手动装箱 | **低**（纯语法差异） |
| X4 | **`typedef_names.clone()` 做试探期符号表快照** | `unary.rs:33,54`、`primary.rs:254,274`、`unary.rs:234,256` | 克隆整个 `HashMap` 换回滚能力；MoonBit 不可变 `Map` 天然支持结构共享式快照 | **低（收益）** |
| X5 | **`&mut self` 全方法穿透 + 手工 `save()/restore()`** | `lib.rs:123-136` + 20 处回滚点 | 无借用检查器；改为状态显式传入传出或状态对象；"回滚面靠约定维护"的缺陷会保留 | **中**：照抄"约定式回滚"则同缺陷复发 |
| X6 | **`unreachable!()`** | `decl.rs:129` | 无等价宏 | **低** |
| X7 | **`i32`/`i64`/`usize` 混用 + `i32::try_from` 值域分派** | `primary.rs:36-68`、`decl.rs:585-611` | MoonBit 数值体系不同；溢出语义已实测（J11）为静默回绕 | **中** |
| X8 | **解析期可变符号表（`HashMap<String, Type>`）** | `lib.rs:72` 及 8 处 `insert` | 非 Rust 特有，但"解析期副作用表"是结构债；应改为显式不可变 `ParseEnv` | **中** |

### 3.2 症状治疗代码（补丁而非设计）

| # | 症状治疗 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| Y1 | **7 处人工挂点式递归深度防护** | `stmt.rs:81`、`ops.rs:7,140`、`postfix.rs:241`、`primary.rs:6`、`unary.rs:7,160` | 覆盖面靠人工枚举——**已被 J1 实测证伪**。属"加保险丝"非"补设计" | **高**：照抄 7 挂点则同漏洞按同比例复刻 |
| Y2 | **`MAX_AST_DEPTH` 后置预算 + `mem::forget`** | `lib.rs:98-119`、`:197-215` | 后置兜底；**只遍历 `Expr`/`Stmt`，完全不遍历 `Type`**（`depth.rs:130-139`）→ 类型通道无兜底（J4） | **高**：J1 的直接成因 |
| Y3 | **`DeclaratorGuard.suffix_count` 死字段** | 声明 `lib.rs:39`；递增 `type_.rs:384`、`:396`；**全树零比较** | 保险丝从未接上（J3）；且 `guard` 每次 `parse_declarator` 调用新建，跨调用不累计 | **高**：2026-09-12 已登记（H2），至今未修 |
| Y4 | **三个真保险丝语义不齐** | `type_.rs:289`（`MAX_DECLARATOR_PTR=32`）、`:351`（`paren_depth > 2`）、`:429`（`cross_count > 4`） | 三个魔数各有来历、无统一判据 | **中**：阈值是实测调出来的，迁移需重标定 |
| Y5 | **`synchronize` 的"零推进早退"分支** | `lib.rs:317-319` | 恢复动作本身可不推进 → 必须靠外层循环再兜一层 | **中**：新语言应改为"恢复必须保证推进"的单一定理 |
| Y6 | **`decl.rs:11-14` 的 `advance(); break;`** | `decl.rs:8-14` | 零进度时 `break` 直接放弃剩余 struct 体（静默截断），不报错 | **中**：会吞掉后续字段（诊断缺失） |

### 3.3 组织债

| # | 债 | 位置 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| Z1 | **C++ 语法入口散落在 C 状态机中**（自认债） | `lib.rs:5` `TODO(#D08)`；`cpp.rs:5-6` `TODO(#D10)` | 顶层分派仍在 `lib.rs:522-525`；`look_ahead_skip_stars` 的 C++ 分支在 `lib.rs:555`；`is_cpp_mode` 布尔贯穿 20+ 处 | **中**：应按"接口 + 独立包"切分，而非复制布尔开关 |
| Z2 | **编译管线双轨**（~350 行×2） | `compile_pipeline.rs:426` vs `:657` | `重构评估报告20260912.md:110` 与 `核心资产重构裁定.md:198` 已登记"补设计（合并双轨）"；parser 入口被复制两份 | **低**（属上层，新项目应一次做对） |
| Z3 | **workspace 两套组织方式** | 评估报告 §4.1 | "组织方式"是真正的债 | **低**：MoonBit 包系统天然单源 |
| Z4 | clippy 豁免 | 无（parser crate 内零豁免） | — | 无 |

---

## 4. 在途工作接纳方案

| # | 条目 | 来源 | 现状 | 新项目接纳方式 |
|---|---|---|---|---|
| A1 | **声明符类型通道栈溢出（J1）** | **本次实测新发现** | **活的崩溃缺陷**，零诊断 | **必须修复后再搬语义，且新架构下不得复用其防护形态**。崩溃输入是**合法 C 语法** → 新项目把"类型通道深度"纳入统一预算（含 `Type` 遍历），用同组锚点（1200 通过 / 1300 拒绝）验收 |
| A2 | `suffix_count` 死字段（H2） | `重构评估报告20260912.md:103`；`核心资产重构裁定.md:185` 判"缺上限不是缺结构" | **未修**（J3 复核） | **直接按目标架构实现**：不搬"再加一个计数器"，把声明符深度做成**统一深度参数**（与表达式共用预算） |
| A3 | `parse_init_list` 无深度防护（H3） | `重构评估报告20260912.md:104` | **已修**（`postfix.rs:241`，U1#8） | 修复形态是"再加一个挂点"→ **不搬形态，搬锚点**（实测 3000 层 → E1006 ✅） |
| A4 | U1#10 回滚面缺口（pos/errors/anonymous_structs） | `CHANGELOG.md:678-686`（实测 23 处回滚点） | **已修**，但回滚面仍不全（见 A5） | **修复后搬**；把"凡解析期有副作用的容器都须进快照"升级为**类型级不变量** |
| A5 | 回滚面缺口第 4 类：`typedef_names` / `template_names` / `current_class` / `next_lambda_id` 不在 `Rollback` 内 | **本次代码勘察新发现**（`lib.rs:62-67` 只含 3 字段） | **未登记、未修；但实测不可观测**（见下） | **风险已降级为"潜在结构性风险"**：唯一非幂等污染站点 `parse_sizeof`（`unary.rs:185-207`）在合法 C 中不可达（要回滚必须"`(` 后是类型名但后面不是 `)`"，已是语法错误），且污染值 `Name→int` 与首次遇到时本就会写入的值相同（`type_.rs:149`），加上解析错误即中止编译（`compile_pipeline.rs:427-430`）→ **不改变程序行为**。新架构用不可变环境值结构性消除 |
| A6 | U3#9 完整根治：嵌套类名 mangled 化 + 字段类型引用与访问路径跟随 | `统一整备路线图.md:825-833`（"完整根治登记下批"） | **止血已完成**；根治**未开始** | **直接按目标架构实现**。parser 侧参与点：`decl.rs:94-102`（`current_class.join("__")`）、`decl.rs:169-172`（**嵌套 union 被塞进 `ClassMember::NestedStruct`**） |
| A7 | D08：Parser 仍含 C++ 语法入口 | `lib.rs:5`；`工程债务维护方案.md:52`（台账标 ✅） | TODO 未消（**文档与代码不一致**） | **直接按目标架构实现**（包切分）；不搬 TODO |
| A8 | D10：C++ 扩展模块耦合度 | `cpp.rs:5-6`；`工程债务维护方案.md:54`（✅） | 同 A7：下沉了三入口，顶层分派与布尔未下沉 | 同上 |
| A9 | 括号深度记账：注释称"256÷4 = 原 64 层精确等价" | `lib.rs:90-95` | **实测不符**：62 层通过、**63 层即拒**（附录 A.2） | **直接按目标架构实现**，常量重新标定并记录实测口径；不要照抄推导 |
| A10 | C++ 子集整体归宿（搬 or 砍） | 评估报告 §3.2 | 未决 | parser 侧影响面 ≈ **910 行 / 4,720 行 = 19%**（`cpp.rs`223 + `decl.rs` class/template 段 ~430 + `type_.rs` 引用/模板段 ~120 + `stmt.rs` range-for 60 + `postfix.rs` lambda/new/delete 80）。**该决策决定 §9 是否切出独立 C++ 包** |
| A11 | `e2e_failures_active = 1` / `cpp_failures_active = 1` | `reports/facts.json`（as_of 2026-09-14） | 未清 | 与 parser 无直接关联（`E2E_FAILURES.md` 现列 `KNOWN_DIVERGENCE` 模板缺陷；`CPP_FAILURES.md` 当前区记"parser_cpp_unit_test 33/33 通过"）→ **随防线重建一起复核，不单独搬** |
| A12 | K&R 旧式函数定义不支持 | 代码勘察 + 实测（J6） | — | **放弃并记录理由**：教学子集边界；新项目在 spec 中显式声明 |

---

## 5. 架构优化建议（MoonBit 形态）

| # | 建议 | 标注 | 依据 |
|---|---|---|---|
| M1 | **深度防护从"7 处人工挂点"改为"单一递归入口 + 深度参数"**：所有自递归函数统一携带 `depth: Int`，超限 `raise DepthExceeded`，顶层统一转诊断 | 〔结构优化〕 | Y1/Y2/Y3 三条症状治疗的共同根因（J1 实测证伪挂点枚举法） |
| M2 | **深度预算测量面必须覆盖 `Type`**（含 `Array/Pointer/Function/Reference/Class/TemplateId`），并把 `parse_declarator_node`→`interpret_declarator_node` 的两次独立递归合并为单次 | 〔结构优化〕 | J1/J4；合并后 `node_cross_count` 这类"只在某一分支跑的附加递归"也一并消失 |
| M3 | **解析环境（`typedef_names`/`template_names`/`current_class`）改为不可变值**，随调用链显式传递；试探解析 = 用旧值跑、成功才返回新值 → 回滚面自动完整 | 〔结构优化〕 | A5：三元快照 + 手工补快照的组合已被证明漏字段 |
| M4 | **"进度单调性"做成类型级不变量**：把每个"可能零推进"的循环收敛到一个组合子 `parse_repeat(step, recovery)`，内部保证"每轮 pos 严格递增或抛诊断" | 〔新设计〕 | §1.4 三层约定 + Y5（`synchronize` 可零推进） |
| M5 | **`is_type_token()` 用穷尽 `match` 表达，6 种 `typeof` 拼写做成数据表** | 〔沿革保留〕+〔结构优化〕 | §7-S3 实测：116 变体缺 3 支 → **E0011 硬错误**（默认即 error）→ 新增 token 未归类会在编译期失败。**但诊断不列出缺失变体名**，需补"118 token 归类审查"约定（`unused_constructor` 警告可辅助） |
| M6 | **错误收集用 `Array[Diagnostic]` 显式累积 + `raise ParseAbort` 分离"致命/可恢复"** | 〔新设计〕 | §7-S2 实测两种控制流可同存；现状"`errors.push` ×28 + 跳 EOF"在 MoonBit 下应用 `raise` 表达控制流、`Array` 表达诊断，**不可用 `Result` 首错短路** |
| M7 | **`parse()` 的 `Option<ProgramNode>` 改为 `Result[ProgramNode, DiagnosticList]`** | 〔结构优化〕 | 现状"两个独立值"的组合语义在 `lib.rs:189-217` 与两处管线各自实现一遍 |
| M8 | **token 子流 hack（X1）改为显式视图参数** | 〔结构优化〕 | `lib.rs:675-683` 的所有权移动在 MoonBit 无对应物；相对/绝对偏移换算是隐蔽缺陷源 |
| M9 | **顶层分发改为"前瞻判定函数 + 穷尽分发"**：现状 11 个 `else if` + 4 处内联前瞻扫描（`lib.rs:484-497` 手写花括号匹配、`:690-728` 模板定界符扫描、`decl.rs:360-396` 显式实例化判定） | 〔结构优化〕 | 前瞻逻辑分散导致 `struct` 分支曾误判（§6-2 的 18GB 事故病灶） |
| M10 | **C++ 拆为独立包 + 显式接口**（若 A10 决定保留 C++） | 〔结构优化〕 | Z1：`is_cpp_mode` 布尔贯穿 20+ 处 |
| M11 | **错误位置保留 `line/column: Int`，含 `peek` 越界的 `-1/-1` 语义** | 〔沿革保留〕 | `lib.rs:224-232` 语义已被 50 条测试与全部诊断位置锚定 |
| M12 | **所有"字节长度/字节偏移"必须走 `@utf8.encode`（或等价 UTF-8 编码器），禁用 `String::length()` 参与 C 语义计算** | 〔新设计〕 | J10 实测：`"中文"` → `length()`=2 / `to_bytes()`=4 / UTF-8=6。`primary.rs:146` 的 `value.len() as i32 + 1` 直译必错 |

---

## 6. 坑清单（事故史 + 本次实测新发现）

> 格式：现象 → 根因 → 修复 → 新语言下是否复发、为什么。

### 6-1 `(int*)100` 被解析成 `Deref(100)` → 宿主 32GB
- **现象**：C++ 时代不支持 cast 时，`(int*)100` 被解析为一元解引用，程序在 wasm3 宿主上无限循环，内存涨到 **32GB**。
- **根因**：`(` 后的类型判定缺失——无法区分"cast 类型名"与"括号表达式"。
- **修复**：加保险丝（步数上限/超时/零进度保护）+ `parse_unary` 的 cast 试探（今 `unary.rs:31-55`）。
- **出处**：`docs/current/07-质量与裁定/核心资产重构裁定.md:84`。
- **MoonBit 下复发？** 不会以同一形态复发（cast 判定语义照搬）。**但复发面转移**：`is_type_token` 依赖可变 `typedef_names`（M3），环境传递写错会造成同类误判——即 A5 那一类。

### 6-2 `struct Node* f(...)` 顶层分支误判 → `consume` 零进度 → 18GB
- **现象**：顶层 `struct` 声明前瞻误判后走入错误分支，`consume` 报错但不推进，`parse_program` 主循环原地打转，每轮累积 AST/错误 → **18GB**。
- **根因**：`consume` 的恢复路径可零推进；主循环当时没有"必须推进"的保证。
- **修复**：零进度保护落地（`CHANGELOG.md:2500`：`struct*`、`ParseBlock`、`parse_case_stmt` 三处死锁）。
- **出处**：`核心资产重构裁定.md:84`；`CHANGELOG.md:2500`。
- **MoonBit 下复发？** **取决于是否照抄"三层约定"**：现版活性依赖 ① 分支推进 ② `pos == checkpoint` 检查 ③ 顶层 `else → advance()`，而 `synchronize` 仍能零推进（`lib.rs:317-319`）。→ 建议 M4 把该不变量收进一个组合子。

### 6-3 `~100MB/秒` 内存缓慢增长（AST 累积特征）
- **现象**：解析器死循环时内存以 ~100MB/s 稳定增长、无输出、无诊断。
- **根因**：与 6-2 同源——零推进循环里每轮 `program.globals.push` / `errors.push`。
- **零进度保护真正防住的场景（逐点核对）**：`parse_statement_inner` 的 `_ =>` 兜底（`stmt.rs:147-179`）、`parse_block`（`:186-197`）与 case 体（`:676-686`）的语句循环、`parse_program` 顶层（`lib.rs:535-543`）、`parse_struct_body`（`decl.rs:11-14`）。
- **出处**：`AGENTS.md` 调试技巧节；`重构评估报告20260912.md:107`（"零进度保护覆盖完整（未发现活着的死循环）"）。
- **本次实测**：5 种历史死循环形状（`struct*`、裸 token 流、缺分号+垃圾、`case 1: @ ;`、孤立 `}}}`）全部正常收敛、无挂死 → **佐证该结论**。
- **MoonBit 下复发？** **形态会变**：GC 语言下表现为**堆增长 + GC 压力**而非 RSS 单调上升，**故障信号更弱**（原本"内存涨"是最直观的现场特征）→ 迁移期必须在防线侧重建"解析器活性"观测（如"解析 N 个 token 后 pos 必须严格大于起点"的内部断言），不能指望外部内存观测。

### 6-4 递归深度：五通道栈溢出（F-P0-1 → U1#8）
- **现象**：release `vitro_cli` 对一批输入直接 `has overflowed its stack`。实测形状：**6 万层 `{{{{`**、**5 万层 `((((`**、**10 万个 `*`**。
- **根因**：纯递归下降 + 无深度上限；每层括号约 3KB 栈（完整优先级瀑布 + 大体积 `Expr` 帧）。
- **修复（两代）**：① `CHANGELOG.md:1831-1835`：`MAX_PARSE_DEPTH=64` 共享计数器 + `ptr_count ≤ 32`；② U1#8（`CHANGELOG.md:549-573`）：补 5 通道 + 上限 64→256 + 后置 AST 预算 512（超限 `mem::forget`）。**首版口径错误教训**：64 上限误伤 30~40 层合法嵌套（`test_legal_deep_expression_still_compiles` 当场红）。
- **本次实测复核**：

| 通道 | 输入 | 结果 |
|---|---|---|
| 括号嵌套 | 62 层 | ✅ 编译成功 |
| 括号嵌套 | **63 层** | ❌ E1006（注释称"等价 64 层"，实测上限 62） |
| 花括号块 | 200 / 300 层 | ✅ / ❌ E1006 |
| 初始化列表 | 3000 层 | ❌ E1006 |
| 赋值链 | 3000 级 | ❌ E1006 |
| 一元链 | 5000 个 `!` | ❌ E1006 |
| 指针链 | 100 个 `*` | ❌ E1007 |
| 表达式式数组后缀链 | 5000 个 `[0]` | ❌ E1006（AST 预算 512 拦截） |
| **声明式数组后缀链** | **1300 个 `[1]`** | **💥 栈溢出，零诊断**（见 6-11） |

- **MoonBit 下复发？** **必然复发，且阈值须重标定**。实测（§7-S1）：21 帧/层瀑布 wasm-gc **854** / js **538** / native **1005**，Rust 现自限 **62** → 新平台余量 8.7×~16×，现有常量可沿用；但**崩溃同样不可捕获**（同 exit code），显式深度计数器仍不可省。

### 6-5 深层表达式在 typeck/Drop 侧崩溃（不是 parser 侧）
- **现象**：5000 项左结合加法链、5000 个 `[0]` 后缀链——"parser 层是循环不递归，构造期无拦截点"，崩在 typeck 或**递归 Drop**。
- **根因**：构造期无拦截点；Rust 的 `Box<Expr>` 递归 `Drop` 自身就是深度递归。
- **修复**：后置 AST 深度预算（迭代 DFS）+ 超限 `mem::forget`（`CHANGELOG.md:551-564`）。
- **MoonBit 下复发？** **Drop 部分消失，遍历部分保留**（J14 实测：1e6 深链迭代遍历成功、递归遍历 1e5 崩）。→ 后置预算仍需保留，且要扩到 `Type`（M2）。

### 6-6 `enum` 初始化器静默错值（F-P0-3）
- **现象**：`NEG = -1`、`BIG = 1+2` 等非裸字面量初始化器**静默保持旧值**（输出 `0 1 2` 而非 `-1 0 3`）。
- **根因**：求值器只匹配裸 `Literal`。
- **修复**：`eval_enum_const` 支持一元/四则/位运算/比较/`sizeof`（`decl.rs:811-865`）。
- **MoonBit 下复发？** 不会（求值器语义照搬）。**但 `checked_*` 必须重标定**（`decl.rs:832`、`:849-850`）——J11 实测 MoonBit `Int` 静默回绕。

### 6-7 `_Static_assert(1<<1000)` 在 debug 下 panic（U1#9）
- **现象**：debug 构建 panic（decl.rs 移位溢出），release 下 UB 绕回静默错值——"构建配置决定语义"。
- **根因**：取负与双移位是裸运算（除/模已 checked）。
- **修复**：全部 `checked_*`，溢出返回 `None` → 走"非常量表达式"诊断。
- **MoonBit 下复发？** **形态改变但风险同等**：MoonBit 无 debug/release 语义分叉（**优于 Rust**），但 **`1<<1000 = 256`、`Int::MIN` 取负 = `Int::MIN`（实测）** = 一律静默回绕 → 必须显式实现 checked 语义，否则原样复刻。

### 6-8 `unsigned` 字面量截断 + `unsigned long long` 退化（F-P0-2）
- **现象**：`4000000000` 经 `parse::<u32>() as i32` 变负数；`unsigned long long` 在限定符循环中提前 return 丢失 `is_unsigned`。
- **修复**：按值域分派 `Literal`/`LongLiteral`（`primary.rs:36-68`）；两处 `long long` 分支统一合成修饰（`type_.rs:45-53`、`:96-102`）。
- **MoonBit 下复发？** 不会（分派逻辑照搬）；风险在 X7 数值类型映射。

### 6-9 数组形状零诊断（U1#12）
- **现象**：`int a[];`、`int b[-5];`（**静默变 VLA**）、`int c[0]={1,2};`（**静默改成 2 元素**）——全部"编译成功"零诊断（clang 均拒绝）。
- **修复**：`array_dim_info` 折叠一元负字面量（`type_.rs:448-460`，U1#12 注释行）；typeck 增 `check_array_dims_legality`；初始化推断收紧为仅 `-1`。
- **误伤教训**：首版只认 `InitList`，`char s[]="hello"` 被误拒 → 影子防线 6 例 `compile_gap` 当场抓回。
- **本次实测**：三形态全部正确拒绝 ✅。
- **MoonBit 下复发？** 不会。但要注意"负尺寸折叠在 parser、合法性判定在 typeck"的**跨层耦合**——新项目应在 spec 中定死职责边界。

### 6-10 匿名 struct 二次 push（U1#10）
- **现象**：合法复合字面量 `(struct {int a;}){7}.a` 被拒（报"重复定义 E3002"，clang 输出 7）。
- **根因**：回滚只恢复 `pos + errors`；试探失败重解析时二次 push，命名 `__anon_struct_{pos}` 同位置同名。
- **修复**：`Rollback` 三元快照 + `save()/restore()`，机械替换 **23 处**回滚点（含 8 处"纯位置回退"型）。
- **MoonBit 下复发？** **同族缺陷仍存（A5），但实测不可观测**——修的是"这一个容器"，没修"所有解析期有副作用的容器都要进快照"这条规则。新架构见 M3。

### 6-11 〔本次实测新发现〕声明符类型通道无深度防护 → 合法代码栈溢出崩溃
- **现象**（`vitro_cli.exe compile -`，stdin 管道，不落盘）：

| 形态 | 输入 | 结果 |
|---|---|---|
| 变量声明 | `int a[1]…[1];` ×1000 / ×1200 | ✅ 编译成功 |
| 变量声明 | ×**1300** | 💥 `thread 'main' has overflowed its stack`，`exit=-1073741571`（0xC00000FD），**零诊断** |
| typedef 未使用 | `typedef int T[1]…[1];` ×1000 / ×1500 | ✅ / 💥 |
| 抽象声明符 | `sizeof(int[1]…[1])` ×1000 / ×1400 | ✅ / 💥 |
| 形参 | `int f(int a[1]…[1])` ×1500 | 💥 |
| 结构体字段 | `struct S{int a[1]…[1];};` ×1500 | 💥 |
| **对照组：表达式式后缀链** | `return a[0][0]…` ×5000 | ❌ 正常报 `AST 嵌套深度 5004 超过预算 512`(E1006) |

- **根因（三层，代码证据）**：
  1. 声明符后缀是**循环收集**（`type_.rs:381-404`），但 `interpret_declarator_node`（`type_.rs:467-609`）在 `DeclaratorNode` 树上**递归解释**，构造 N 层嵌套 `Type::Array`；崩溃点在该递归或紧随其后的 `Type` 递归释放（**两者同深度、同修复面**）。
  2. 唯一的后缀计数器 `suffix_count`（`lib.rs:39`）**只递增、零比较**（`type_.rs:384/396`；全 `native/` 树仅 3 处命中）→ **死保险丝**（J3）。
  3. 后置 AST 预算**只遍历 `Expr`/`Stmt`**（`vitro_ast/src/depth.rs:122-195`），`Stmt::VarDecl` 分支只看 `init`/`extra_vars`（`:130-139`）→ **`Type` 通道既无前置保险丝也无后置兜底**（J4）。
- **实测反证（排除法）**：抽象声明符路径**跳过 `node_cross_count`**（该函数仅在 `!is_abstract` 时调用，`type_.rs:427-438`）而阈值不变 → 排除；**typedef 未被使用**（下游 typeck 永不遍历该 `Type`）仍崩 → 排除下游遍历。
- **修复（建议）**：接上 `suffix_count` 比较（并修正"guard 每次调用新建 → 跨调用不累计"）+ 把 `Type` 纳入 `depth.rs` 遍历面 + 合并两次递归遍历（M2）。
- **新语言下复发？** **照抄现有防护形态则 100% 复发**：这是"保险丝靠人工枚举挂点"的方法论失效。MoonBit 下必须 M1+M2。

### 6-12 〔本次代码勘察新发现〕回滚面缺口第 4 类
见 A5。**实测判定：潜在结构性风险，非活缺陷**（污染站点在合法 C 中不可达 + 污染值与首次写入同值 + 解析错误即中止编译）。新语言下由 M3 结构性消除。

### 6-13 其他已登记的"症状治疗"残留

| 项 | 状态 | 出处 |
|---|---|---|
| `suffix_count` 死字段 | ❌ 未修（本次复核 J3） | `重构评估报告20260912.md:103` |
| `#if` 迷你解析器无深度保护 | ⚠️ 不在本模块（lexer） | 同上 `:106` |
| checkpoint 回滚三处实现不一致 | ⚠️ 部分修（`parse_alignof` 补了 typedef 回滚，`parse_sizeof` 未补） | 同上 `:106`（M3）+ 本次代码复核 |

---

## 7. MoonBit spike：实测结果（8 条全部实跑）

> 环境：本机 `moon 0.1.20260915`（`moonc v0.10.13+cbb11c36f`），Windows；spike 在 `%TEMP%` 独立模块内完成，**Vitro 仓库零改动**。
> 全部数值为「逐深度单跑 + 只看退出码」测得（方法论见附录 C-1）。

### 7-S1（生死项）递归下降栈深度｜✅ 通过，余量远超现状

**最小验证程序**：21 条互相调用函数构成的水位瀑布（`frame0..frame20`，`frame20` 递归回 `frame0`），与 `vitro_parser` 的真实优先级瀑布同帧数。

| 后端 | 21 帧/层瀑布最大深度 | 折算帧数 | 可复现性 |
|---|---|---|---|
| **wasm-gc**（moonrun/Windows） | **854**（855 崩） | ≈17,934 | 两轮独立构建一致；同深度重跑 6 次结论一致 |
| **js**（Node） | **538**（539 崩） | ≈11,298 | 同深度重跑 3 次一致 |
| **native**（C 后端） | **1005**（1006 崩） | ≈21,105 | 同深度重跑 3 次一致 |
| 对照：Rust 1MB 线程栈 | 现自限 **62 层**；无防护的声明符递归 **1200 崩** | — | 附录 A.1 |

- **判定**：门槛"D2 ≥ 62 层"**远超通过**（854 vs 62 = **13.8×**；最紧的 js 后端仍有 **8.7×**）→ `MAX_PARSE_DEPTH=256` 语义可原样沿用。
- **崩溃形态**：`thread 'main' has overflowed its stack`，`exit = -1073741571`（0xC00000FD）——**与 Rust 版逐位相同，不可捕获**。
- **对 6-11 的含义**：无防护通道阈值（wasm-gc 854）与 Rust（1200）同量级 → **同类缺陷在新语言下仍可达**，深度上限必须显式做。

### 7-S2 多错误收集惯用法｜✅ 成立

**最小验证程序**：`suberror ParseErr { ParseErr(String) }` + `risky(n) raise ParseErr` + `try {…} catch { ParseErr(m) => errs.push(m) }` 循环累积 + `Result[Int, String]` 对照。

**实测结果**：6 次调用中第 3 次 raise → `nerrs=1`、`acc=12`（0+1+2+4+5，**跳过错项后继续**）；`raise` 早退路径正常；`Result` 正常。
**判定**：parser 需要的**"收集而非首错即停"+"跳 EOF 收敛"两种控制流可同存**——`Array` 承载诊断、`raise` 承载控制流（M6）。

### 7-S3 `match` 穷尽检查｜✅ 硬门禁成立；⚠️ 但诊断不列漏项

**最小验证程序**：116 变体 `enum Tok`（与 `TokenType` 同名同数）+ `match` **故意缺 3 支**。
**实测结果**：`moon check` → `Error: [0011] partial_match`，`Failed with 115 warnings, 1 errors`，**构建失败**（默认即 error，无需额外开关；`--deny-warn` 可把全部警告升级）。
**⚠️ 不达预期处**：诊断里的 `some hints:` **列表为空**——**不列出缺失的具体变体名**。替代线索是 `unused_constructor` 警告逐条指出"从未被构造的变体"。
**判定**：M5 的"编译期强制归类"成立；"指出漏了哪一支"不成立 → 需在工程约定中补一条（token 归类审查靠 `unused_constructor` 辅助）。

### 7-S4 试探解析 + 可变符号表｜设计级结论（未跑最小程序，见附录 B-U6）

结论来自 §7-S2/S3 与 `Map` 不可变语义的代码勘察：不可变环境值可表达"回滚 = 丢弃新值"。**尚未跑独立最小程序验证零深拷贝**，列为未闭合项。

### 7-S5 `String` 语义｜⚠️ **P0 移植坑，已坐实**

**最小验证程序**：对同一字符串分别取 `length()` / `to_bytes()` / `@utf8.encode()`。

| 表达式 | 实测 | 含义 |
|---|---|---|
| `"中文".length()` | **2** | UTF-16 码元数 |
| `"中文".to_bytes().length()` | **4** | *UTF-16 字节*（LE：`2D 4E 87 65`） |
| `"ab".to_bytes().length()` | **4** | ASCII 也是 2 字节/字符 |
| `@utf8.encode("中文").length()` | **6** | ✅ 真 UTF-8（`E4 B8 AD E6 96 87`） |
| `"中".length()` / `@utf8.encode("中").length()` | **1 / 3** | 单 CJK 字符：1 码元 vs 3 字节 |
| `@utf8.decode(@utf8.encode(s))` | `中文` | 往返正常 |

**对 `primary.rs:146` 的直接影响**：Rust 版 `value.len() as i32 + 1` 是**字节长度**（`"中文"` → 7）；若直译成 `s.length()` → **3**，数组尺寸少 4 字节 → 越界写。
**判定**：**必须改用 `@utf8.encode()` 取字节长度**（M12）；位置 `column` 口径同理必须定死（附录 B-U5）。

### 7-S6 整数语义｜⚠️ 静默回绕 + 除零 trap

**最小验证程序**：11 个算术探针（溢出/位移/取负/除模/64 位/除零）。

| 探针 | 实测 |
|---|---|
| `Int::MAX + 1` | `-2147483648`（回绕） |
| `1 << 31` / `1 << 32` / `1 << 1000` | `-2147483648` / `1` / **`256`**（位移量 mod 32） |
| `-Int::MIN` | `-2147483648`（回绕） |
| `1000000 * 1000000` | `-727379968`（回绕） |
| `7 % -2` / `7 / -2` | `1` / `-3`（C/Rust 一致） |
| `Int64::MAX + 1` | `-9223372036854775808`（回绕） |
| `UInt` 最大值 | `4294967295`（32 位） |
| `x / 0` | **`RuntimeError: divide by zero`**（trap，exit 1） |

**判定**：`checked_*` 纪律必须原样保留（6-7）；`Int` 为 32 位；除零为运行时 trap，常量求值器仍须显式零检查。

### 7-S7 深结构｜✅ 迭代安全 / ❌ 递归崩

**最小验证程序**：`enum Deep { Nil; Cons(Deep) }` + 迭代构建 + 迭代遍历 + 递归遍历。
**实测结果**：迭代构建 1e6 ✅；**迭代遍历 1e6 → 返回 1000000** ✅；迭代构建 1e5 ✅；**递归遍历 1e5 → 栈溢出** 💥。
**判定**：`vitro_ast/src/depth.rs` 的迭代式 DFS **必须保留**（M2）；`mem::forget(prog)`（`lib.rs:212`）在 MoonBit 下**不需要**（GC 无递归释放）= 净收益。
*诚实标注*：「丢弃 1e6 深结构本身是否崩」被同批次递归探针的崩溃掩盖，**未单独验证**（附录 B-U2）。

### 7-S8 其他实测（新）

- **MoonBit 编译器 ICE（一手实证）**：把 `mode`/`depth` 放在**跨文件顶层 `pub let`**、由 `main` 引用时，`moonc v0.10.13` 在 **link-core 阶段**崩：
  `Moonc.Basic_hash_string.Key_not_found("$_M0FP45vitro5spike3cmd4main4mode")`。常量内联进 `main.mbt` 后消失。
  → 对评估报告 §6.1（A6 breaking-change 风险）是一手实证，**建议纳入阶段 0 的门 0 观测项**。
- **API 权威源纪律**：`suberror Name { Ctor(T) }`、`@utf8.encode(StringView)->Bytes` / `decode(BytesView) raise Malformed`、`moon check --deny-warn` 均由本机 core 源码（`~/.moon/lib/core`）与 `moon explain --diagnostic` 确认，**非猜测**。

---

## 8. 等价性验收锚点

| # | 锚点 | 产物形态 | 工具 | 备注 |
|---|---|---|---|---|
| E1 | **AST dump JSON diff**（首要锚） | `ProgramNode` 全字段 JSON | Go 驱动（`scripts/` 默认语言，零第三方依赖）→ 归一化（对象键排序、数组保序）→ 逐字节 diff | **前提已具备**：AST 全类型已 `serde::Serialize`（`vitro_ast` 27 处 derive）→ Rust 侧只需加一个 dump 入口（**当前不存在**，属需新建的验收设施）。MoonBit 侧需自建等价序列化器，**键序与浮点格式必须对齐**（`FloatLiteral` 为 `f64`，`primary.rs:99`） |
| E2 | **错误诊断序列 diff** | `{code, line, column, message}` 序列（**保序**，多错误收集下顺序即语义） | 同上；两侧各取 `serve` 的 `compile` 响应 JSON | **入口已存在**：`vitro_cli serve` 的 `compile` 动作（`native/src/bin/vitro_cli.rs:654-689`，经 `session_api::compile`）。约定：比对 `code+line+column` 三元组序列；`message` 单列做"文案差异告警"（教学资产允许措辞变化但须人工确认） |
| E3 | **病态输入"同等拒绝"锚** | `{是否崩溃, 错误码集合, 首错位置}` | 驱动对每个样本断言"两侧都不崩溃"，**崩溃 = 硬失败** | 样本 = 6-4/6-11 全部 12 条 + 新语言重标定后的阈值样本。**这是唯一能抓住 6-11 类缺陷的锚点**——两侧输出都是"失败"，只有"是否崩溃 + 错误码"能区分 |
| E4 | **合法程序"必须成功"反向锚** | 编译成功布尔 + AST dump（走 E1） | 同上 | 样本 = `crash_regression_tests.rs:776` 的反向锚形状（30 层括号 + 100 项加法）+ baseline 中解析敏感样本 |
| E5 | **编译产物 diff（端到端收敛锚）** | `vitro_cli export` 的字节码 JSON（`vitro_cli.rs:898`） | 同上 | 作为 E1 的下游佐证。**E1 通过前不应作为主判据**（会掩盖解析层错位） |
| E6 | **Clang golden 复用（最终判据）** | 既有 `.out` | `scripts/shadow_verify`（`reports/facts.json`：`shadow_c_cases`=675 / `shadow_c_match`=668，as_of 2026-09-15）+ `scripts/shadow_verify_cpp`（`shadow_cpp_cases`=99 / `shadow_cpp_match`=95，as_of 2026-09-15） | 评估报告 §3.3 判定"golden 真值可直接复用"。**驱动链需重建**（原走 ctypes 调 `vitro_native.dll`，wasm-gc 下该通道不存在） |
| E7 | **位置口径锚（隐性但致命）** | `{line, column}`：`peek` 越界 `-1/-1`（`lib.rs:224-232`）+ 缺分号报在 `previous()` 位置（`lib.rs:288-292`） | 随 E2 | 两条"隐性口径"，E2 的位置列可覆盖 |

**比对纪律（沿用本仓库既有方法论）**：归一化在**读入后**做，比对用**逐字节 diff**；驱动必须 **fail loud**（自检不过直接拒绝给判定，禁止静默 default）；每个锚点上线前必须有**埋雷记录**（J9 可触发性义务）。

---

## 9. mooncakes 包切分草案

### 9.1 建议包划分（4 包 + 1 契约包）

| 包 | 内容 | 对应 Rust 源 | 行数 |
|---|---|---|---|
| **`vitro/ast`** | `Expr`(26) / `Stmt`(16) / `Type`(17) / `TypeKind` / `UnaryOp` / `BinaryOp` / `AssignOp` + `ProgramNode` / `FuncDecl` / `StructDecl` / `ClassDecl` / `ClassMember` / `TemplateDecl` / `TemplateArg` / `Param` / `CaptureMode` / `SourceLoc` + `depth` 模块 | `vitro_ast/src/{lib,expr,stmt,types,decl,depth}.rs`（1,519 行）+ `vitro_shared/src/source_loc.rs` | ≈1,550 |
| **`vitro/parser`** | 核心 C 语法：token 访问原语、表达式瀑布、语句族、声明符螺旋、类型说明符、声明族、深度基础设施、错误码映射与恢复、`eval_enum_const` | `lib.rs` 735 + `expr/` 1,440 + `stmt.rs` 718 + `type_.rs` 739 + `decl.rs` 的非 C++ 段落 | ≈3,400 |
| **`vitro/parser/cpp`** | C++ 扩展：class/template/引用/构造析构/range-for/lambda/new-delete/类外定义 | `cpp.rs` 223 + `decl.rs` class/template 段 + `type_.rs` 引用/模板段 + `stmt.rs:436-493` + `postfix.rs:4-144` + `unary.rs:25-30` + `primary.rs:207-217` | ≈910 |
| **`vitro/parser/contract`**（薄契约包） | `Token` / `TokenType`(116) 的**只读视图类型** + `Diagnostic{code,line,column,message}` + `ProgramNode` 的 JSON 序列化接口（E1 用） | 新建（Rust 现状散在 `vitro_lexer::token` 与 `ParseError`） | ≈200 |

> `Token`/`TokenType` 的**定义归属 lexer 包**（由 lexer 勘察裁定）；`contract` 包只做"parser 对外承诺"的 re-export 与 JSON 面，避免 parser ↔ lexer 双向依赖。

### 9.2 依赖方向（严格单向）

```
vitro/ast ──────────────┐
     ▲                   │
     │                   ▼
vitro/lexer ──► vitro/parser ──► vitro/parser/cpp
                      │
                      ▼
                vitro/typeck ──► vitro/codegen ──► vitro/vm
```

- `vitro/parser` 只依赖 `vitro/ast` + lexer 的 token 契约面（**不依赖** lexer 的预处理实现）。
- `vitro/parser/cpp` 依赖 `vitro/parser`（**反向零依赖**）。
- 与评估报告 §7 阶段 1 的平移顺序 `shared → ast → lexer → parser → typeck → codegen → vm` 一致。

### 9.3 关键边界裁定（开工前必须定，否则重演 Z1）

| 边界问题 | 建议裁定 | 依据 |
|---|---|---|
| `typedef_names` 环境归谁？ | 归 `vitro/parser`，**通过显式参数传递**（M3）；typeck 有独立符号表，两者不得共享 | `lib.rs:72` 现状为解析期可变表；共享会造成跨层语义失配 |
| `is_cpp_mode` 布尔归谁？ | 改为**语法模式枚举**（`.C` / `.Cpp`），由 `contract` 包定义，lexer 与 parser 各持只读副本 | `compile_pipeline.rs:631-634` 现状按扩展名判定；布尔贯穿 20+ 处（Z1） |
| 深度预算常量归谁？ | 归 `contract` 包（单一真相源），按 target 可配置（§7-S1：wasm-gc/js/native 三值不同） | `MAX_PARSE_DEPTH`/`MAX_AST_DEPTH` 属"引擎级参数域" |
| `Type` 布局/尺寸计算归谁？ | 现状 `eval_enum_const` 调 `crate::compute_type_size`（`decl.rs:820`）= **parser 依赖布局计算，跨层耦合**；改为"产出待求值节点 + 由布局包求值" | `decl.rs:817-826` |
| 字节长度口径归谁？ | 由 `contract` 包提供 `byte_len(String) -> Int`（内部走 UTF-8 编码），**parser 内禁止直接用 `String::length()` 参与 C 语义** | J10（§7-S5） |

### 9.4 对上发布形态

- **`vitro/parser`**：对外只暴露 `parse(tokens, mode) -> Result[ProgramNode, Array[Diagnostic]]` + `contract` 类型；**不暴露任何内部状态**（现状 `Parser` 只暴露 3 个方法是好的，直接沿用）。
- **`.mbti` 公共接口文件**：`vitro/ast`（最大对外契约）、`vitro/parser`、`vitro/parser/cpp` 各一份；`contract` 包的 `.mbti` 即 wire format 承诺。
- **测试发布形态**：
  - 黑盒 `*_test.mbt`：搬运 `parser_unit_test.rs`(17) + `parser_cpp_unit_test.rs`(33) = **50 条**；断言从"AST 形状"改为"AST dump JSON 片段"（复用 E1 设施）。
  - 白盒 `*_wbtest.mbt`：**新增**（Rust 版零白盒测试）——至少覆盖 ① 零进度四点的内部不变量（每轮 pos 严格递增）② `Rollback` 四类副作用容器 ③ 深度预算边界。
  - E2E / 崩溃锚：6-4/6-11 的 12 条样本进 Go 驱动（E3），不放在包内。
- **文档**：`README.md` + `doc/` 固化三件事——(a) 支持语法族清单（§2 表，逐条给与 Clang 的差异）；(b) 深度/预算参数的**重标定后实测口径**（替换 `lib.rs:84-103` 的注释口径）；(c) 与 spec 的对应关系（K&R 不支持等边界）。

---

## 附录 A · 本次实测方法与原始数据

### A.1 Rust 侧：崩溃阈值与反证组

**方法**：`native/target/release/vitro_cli.exe compile -`（stdin 管道，不落盘），逐深度单跑，只看退出码（`0` = 通过，`-1073741571` = 0xC00000FD 栈溢出）。

| 组 | 输入形态 | 1000 | 1200 | 1300 | 1400 | 1500 | 结论 |
|---|---|---|---|---|---|---|---|
| 变量声明 | `int a[1]×N;` | ✅ | ✅ | 💥 | 💥 | 💥 | 边界 1200/1300 |
| typedef（未使用） | `typedef int T[1]×N;` | ✅ | — | — | — | 💥 | **排除下游 typeck** |
| 抽象声明符 | `sizeof(int[1]×N)` | ✅ | — | — | 💥 | 💥 | **排除 `node_cross_count`** |
| 形参 | `int f(int a[1]×N)` | — | — | — | — | 💥 | 同量级 |
| 结构体字段 | `struct S{int a[1]×N;}` | — | — | — | — | 💥 | 同量级 |
| 对照（表达式） | `return a[0]×N` | — | — | — | — | ✅ 被 AST 预算拒绝 | 类型通道无对应防护 |

**深度防护各通道实测**：括号 62 ✅ / 63 ❌ E1006；花括号 200 ✅ / 300 ❌；初始化列表 3000 ❌；赋值链 3000 ❌；一元链 5000 ❌；指针链 100 ❌ E1007。

**活性实测**：`struct*`、`@#$%^` 裸 token 流、缺分号+垃圾 token、`case 1: @ ;`、孤立 `}}}` → 全部 exit 1 且出诊断，无挂死。

### A.2 语法族实测（21 项）

正向 20 项全部 `exit=0`：C99 for 声明 / VLA 一维 / VLA 多维 / 复合字面量×3 / `_Generic` / 逗号运算符 / Designated `.field` / Designated `[idx]` / `offsetof` / `stdarg` 全族 / `typeof` / `typeof_unqual` / `enum : long long` / 相邻字符串拼接 / 科学计数法 / `_Static_assert`(真) / `nullptr` / `[[属性]]`。
负向 1 项：**K&R 旧式定义 → E2005**（唯一失败）。
边界拒绝实测：`int a[];`(E2002) / `int b[-5];` / `int c[0]={1,2};` / `_Static_assert(1==2)` / `enum E : float` 全部正确拒绝。

### A.3 MoonBit 侧：spike 原始数据

| spike | 关键实测值 |
|---|---|
| S1 wasm-gc | 21 帧/层 = **854**（855 崩）；两轮独立构建一致；同深度重跑 6 次一致 |
| S1 js | **538**（539 崩）；重跑 3 次一致 |
| S1 native | **1005**（1006 崩）；重跑 3 次一致 |
| S2 | 累积模式 `nerrs=1`、`acc=12`；`raise` 早退正常；`Result` 正常 |
| S3 | `Error: [0011] partial_match`，`115 warnings, 1 errors`；`some hints:` 为空 |
| S5 | `length()`=2 / `to_bytes()`=4 / `@utf8.encode()`=6（`"中文"`） |
| S6 | 11 探针（见 §7-S6 表）；除零 → `RuntimeError: divide by zero` |
| S7 | 迭代构建 1e6 ✅ / 迭代遍历 1e6 ✅（=1000000）/ 递归遍历 1e5 💥 |
| S8 | `moonc v0.10.13` link-core ICE：`Key_not_found("$_M0FP45…mode")` |

---

## 附录 B · 仍未闭合项（逐条给验证方法）

| # | 事项 | 验证方法 |
|---|---|---|
| U1 | 6-11 崩溃在"`interpret_declarator_node` 递归"与"`Type` 递归释放"之间的贡献占比 | 在 `type_.rs:467` 入口加计数器（需改文件）；或 `/STACK:8388608` 链接后看阈值是否线性平移 |
| U2 | 1e6 深结构的**最终回收**是否安全 | S7 去掉递归探针重跑（一轮） |
| U3 | `plain`（1 帧/层）在 wasm-gc 的精确阈值 | 本报告未采信早期粗测值（受 stdout 缓冲影响）；用同口径二分（约 18 轮） |
| U4 | 浏览器内 V8 的 wasm-gc 栈预算 | 本机只测到 Node/moonrun；需在真实浏览器里跑同一段 S1 程序 |
| U5 | 位置 `column` 的字节/字符口径 | 读 `vitro_lexer`（不在本模块范围）+ 用含中文字面量的用例做 E2 位置锚 |
| U6 | 7-S4（不可变环境值的零深拷贝） | 独立最小程序：20 处回滚点形状 + 两种环境形态对比 |

---

## 附录 C · 本次勘察的更正记录（口径错误教训）

| # | 项 | 说明 |
|---|---|---|
| C-1 | **"阶梯扫描 + 读最后一行 OK"测阈值 = 错误方法论** | 首轮用该法测 MoonBit 栈阈值，得出结论"wasm-gc 上限 ~18000"，与后续单跑结果矛盾。根因：**stdout 有缓冲，崩溃前最后一行滞后最多数百步**。改为"逐深度单跑 + 只看退出码"后完全可复现。**这正是本项目"防线口径"纪律的又一次现身：口径错了，数字就是假的** |
| C-2 | 原报告称"括号深度 64 层为上限（按注释 256÷4 推导）" | **实测 62 通过 / 63 拒绝**。`lib.rs:90-95` 注释的"精确等价"推导与实测差 1~2 层（无害，但迁移期照抄常量需重标定） |
| C-3 | 原报告把"回滚面缺口（`typedef_names`）"登记为"未修缺陷" | **实测降级为"潜在结构性风险"**：污染站点在合法 C 中不可达 + 污染值与首次写入同值 + 解析错误即中止编译 → 不改变程序行为（A5） |
| C-4 | 原报告把 6-11 的崩溃归属列为"待证" | **已用两组反证闭合**（typedef 未使用形态排除下游 typeck；抽象声明符形态排除 `node_cross_count`），剩余候选同深度同修复面 |
| C-5 | 原报告将 §7 的 8 条 spike 写成"清单" | **已全部实跑**（S4 除外，见 B-U6），清单条目与判定标准按实测结果重写 |
| C-6 | 本报告首版把 `Type` 记为 **16** 变体 | **实为 17**（`Auto` 无载荷、无花括号，被我的正则漏掉）。已逐个数过 `types.rs:32-104` 并更正；与姊妹报告 shared/ast 的 17 一致。**教训：用正则数枚举变体必须覆盖"无载荷变体 + 带尾逗号"两种书写** |

---

## 附录 D · 与姊妹报告的口径交叉核对

| 项 | 本报告 | 姊妹报告 | 核对 |
|---|---|---|---|
| `TokenType` 变体数 | **116** | lexer 报告亦为 116 | ✅ 一致 |
| `Expr` 变体数 | **26** | shared/ast 报告 26 | ✅ 一致 |
| `Stmt` 变体数 | **16** | shared/ast 报告 16 | ✅ 一致 |
| `Type` 变体数 | **17** | shared/ast 报告 **17** | ✅ 一致（**本报告首版误记 16**，见 C-6） |
| `is_cpp_mode` 出现面 | parser 侧 20+ 处 | cpp_frontend 报告"43 处 / 9 文件，全在 lexer+parser" | ✅ 相容（本报告未做全量计数） |
| C++ 归属行数 | A10 估 **≈910 行**（按"归属 C++ 的代码"口径） | cpp_frontend 报告"**真正独立文件仅 `parser/cpp.rs` 223 行**，其余寄生共享文件" | ✅ 相容（两者口径不同：本报告算"属性归属"，该报告算"独立文件"） |
| **`Int` 溢出语义** | S6：**静默回绕**（`2147483647+1=-2147483648`、`1<<1000=256`、`7/-2=-3`） | 编译管线报告 S1："`Int` 静默回绕 / `-7/2=-3` 与 C 一致" | ✅ **两路独立实测一致**（本报告另测出 `1<<1000=256` 的位移掩码语义） |
| **`String` 码元口径** | S5：`"中文".length()`=2、`get(0)=Some(20013)`（=U+4E2D） | 编译管线报告 S2："`String` 索引为 UInt16 code unit" | ✅ 一致（本报告另测出 `to_bytes()`=UTF-16 字节 与 `@utf8.encode()`=真 UTF-8 的三分对照） |
| 退出码口径 | 栈溢出 `-1073741571`（0xC00000FD）；除零 → MoonBit runtime `RuntimeError` exit 1 | 编译管线报告 S6："正常 0 / `abort` 与未捕获 `raise` 均 1" | ✅ 相容（本报告未测 `abort` / 未捕获 `raise` 的退出码，列为 B-U6 之外的空白） |
