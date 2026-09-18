# MoonBit 迁移第一阶段计划（S0.5 Rust 止血批 + S1 基础片）

> **定稿**：2026-09-18 ｜ 上位文档：[`MoonBit迁移总计划.md`](MoonBit迁移总计划.md)
> **范围**：S0 已实质完成（四门实测关闭，见总计划 §2）——本阶段从 **S0.5（Rust 侧止血批）** 起步，止于 **S1（基础片 mooncakes 首发）**。
> **纪律**：每条修复先红后绿（锚用例名进提交信息）；判"构建失败"前 tail 全量输出（SIGPIPE 两犯教训）；每条护栏上线前证红（J9）；全程未获允许不 git 提交的部分按仓库现行纪律执行。

---

## 1. 阶段目标与总验收

1. Rust oracle 侧的已知**静默错值/崩溃/协议瑕疵**清零（P1–P7），使差分锚点不"两侧一致地错"；
2. 差分基础设施就位（AST dump 出口、golden fail-loud、列号口径、M-0 基线冻结）；
3. MoonBit 侧 `vitro/{source,diag,opcode,ast}` 四包建成并通过 B 级锚点，`vitro/diag` 首发 mooncakes。

**总验收门**：P1–P7 全部红→绿留痕 ＋ E1（AST dump）/E3（诊断帧）/E4（error_catalog）三条 B 级锚可跑 ＋ `moon check` 干净 ＋ 码表生成脚本幂等。

---

## 2. S0.5 Rust 止血批（P1–P7 + U1/U2，逐项含亲证锚）

### P1 · parser 声明符类型通道栈溢出（活的零诊断崩溃）

- **现象（亲证）**：`int a[1]…[1];` 链式后缀——release 1300 层崩（`thread 'main' has overflowed its stack`，exit 0xC00000FD 系）；**debug 1400 通过 / 1500 崩**（边界二进制相关，锚点钉 release）。
- **根因（亲验代码）**：① `interpret_declarator_node`（`type_.rs:467-609`）递归解释无防护；② `suffix_count` 死保险丝（全仓 3 处：声明 `lib.rs:39` + 递增 `type_.rs:384/396`，**零比较**）；③ `depth.rs:130-139` VarDecl 分支只看 init/extra_vars，不遍历 Type。
- **修法（Rust 侧止血）**：接上 suffix_count 比较（修"guard 每调用新建不累计"）+ Type 纳入 depth 遍历 + 超限转 E1006 诊断。
- **验收**：release 二进制 1200 通过 / 1300 确定性诊断（非崩溃）；红锚 `j1_declarator_depth.c` 新建入 baseline。
- **MoonBit 侧不复刻防护形态**：S3 片统一"单一递归入口 + depth 参数"（勘察档案 parser 报告 M1/M2）。

### P2 · ★A 全局/静态字符串指针静默错值（双侧根因）

- **现象（亲证）**：`char *p = "hi"; printf("[%s]", p)` → Vitro `[]`，clang `[hi]`（static 形态同）；676 用例零覆盖（regex 实测命中 0）。
- **根因（亲验代码）**：codegen `lib.rs:392-397` StringLiteral 分支按外部 `sz` 写字节无目标类型校验 **且** typeck 全局初始化不插隐式转换（`typeck/lib.rs:320-355` 只 `check_assignable`；局部路径 `decl.rs:288/321` 有 `insert_implicit_cast`——不对称亲证）。
- **修法**：codegen 加目标类型判据（非 char 数组走取址路径）+ typeck 全局/局部对称化。
- **验收**：`baseline/global_string_pointer.c` / `static_string_pointer.c` 新建，红→绿留痕；clang golden 实跑（勿再依赖语义直判）。

### P3 · 诊断 E 前缀（4 处伪造点，W/H 码打 E）

- **现象（亲证）**：`[警告] … (E3053)`；serve 同帧 `code=E3053`+`severity=warning` 自相矛盾。
- **4 处（亲验）**：`session_api.rs:50`、`error_catalog.rs:536`（`"code_str":"E{}"`）、`vitro_cli.rs:94`、**`vitro_cli.rs:295`（export 侧，第八轮清点新发现）**。
- **修法**：按 severity 输出 `E/W/H` 前缀（`vitro_shared` 错误码段位定义已含前缀语义）；serve 帧 `code` 与 `severity` 解耦单源。
- **验收**：E3 诊断帧锚点可建（否则两侧一致地错）；`typeck_e3053_regression_test` 同批核对。

### P4 · string 侧转义与 char 侧收口

- **现象（亲证）**：`sizeof("\x4")`=3（clang 2）、`sizeof("A\012B")`=6（clang 4）、`"\xff"` 落 2 字节 UTF-8；char 侧已修（U1#7）形成分叉。
- **修法**：`vitro_lexer/src/string.rs:36-52` 与 `:104-149` 单点收口（hex 1~2 位 + 八进制同 char 口径）。
- **验收**：非 ASCII/\xHH/八进制用例进锚点集（现覆盖 0）；红锚 `string_escape_octal_hex.c`。

### P5 · golden 完整性与 fail-loud

- **修法（四件）**：① `vitro_e2e.rs:186` 缺 golden 改必红；② 补 4 例 C++ golden（`cpp_copy_ctor`/`cpp_default_args`/`cpp_nested_class_instance`/`cpp_nttp_class`，先确认 live-clang 判定）；③ **4 例手写 golden（`cpp_vitro_vec_class` 等，与 Vitro stdout 逐字节相同的 Vitro 自证）改 std::vector 等价物取真对照或显式登记"无独立 oracle 仅回归锚"**；④ 五个 `KNOWN_*` 常量成对（跳过 + 反向"转绿即 panic"）；⑤ 7 条 gap 用例逐例审计（对齐 C 侧 J2 先例）。
- **附**：golden 生成口径记录（sync_templates 前置注入头 + 不喂 stdin；`.out` 只作第二来源）。

### P6 · 列号口径冻结（双坐标契约输入）

- **✅ 已完成（2026-09-19）**：口径档案 [列号口径冻结](../07-质量与裁定/列号口径冻结.md)（词法 +1 / 解析非 ASCII −4 / make_token 字符计数减字节数根因亲证）+ 10 形状防漂移锚 `source_column_convention_test`。MoonBit `vitro/source` 契约输入（byte_off+1 主坐标 / 双坐标预留 / 禁量纲混算 / 不复刻 +1）已入档案 §2。
- **现象（亲证）**：`int main(){ int "中文"; }` 报 `1:13`，字符串 token 实际起始列 17（逐字符核算）；词法路径自洽、解析路径偏差 −4（两路径区分本身是关键发现）。
- **修法**：Rust 侧先冻结现状口径入文档（不急修算法）；`int main(){ int "中文"; }` 固化为位置锚用例；MoonBit `vitro/source` 包的坐标单位契约以此为输入（建议字节偏移+1，双坐标 `Pos{byte_off, col_scalar, col_utf16}`）。

### P7 · AST/符号表 dump 出口新建（B 级锚点硬前提）

- **✅ 已完成（2026-09-19）**：serve `ast.dump` / `symbols.dump`（session 不保留 AST，dump 内重解析；emitter 纪律落注释：Rust 侧 serde 派生唯一 emitter、MoonBit 侧禁 ToJson 直拼）+ Go canonicalizer `scripts/canonicalize`（键排序/数字保形/转义统一/缩进固定/fail loud/`--check` 锚定模式；J9 ×8）+ 管道锚 `ast_dump_test`（dump→canonicalize 幂等——E1 逐字节对拍的可信前提）。

- **现状（亲证）**：全仓 `dump_ast` 零命中；AST 已有 27 处 serde 派生但无出口——typeck 与 parser 双报告独立确认。
- **修法**：测试内加 dump 出口（不动生产代码语义）；Go canonicalizer（键排序/转义统一/缩进固定/fail loud）配套；**两侧显式 emitter 纪律**（禁一侧 serde 一侧 ToJson）。

### U1 · 认知链二/三/四层与补全补最小导出（阶段 1 硬前置）

- **现状（亲证）**：knowledge_graph / misconception / learning_path / completion / data_flow / intent / auto_fix 外部生产调用全为 0——"趁 Rust 版仍在做差分扫描"这条退路**对它们不存在**。
- **修法**：serve 加 4 方法或一个 `diagnostics_probe`（返回结构化 JSON）+ 4 组用例外置 JSON。不补则 S8 片无等价性证据。

### U2 · `vitro_capi.h` 19 声明与开工同批拍板

- **✅ 已拍板（2026-09-19）**：依据 [wasm多实例并发模型与U2拍板](../06-出口与协议/wasm多实例并发模型与U2拍板.md)——第一批 19 声明**冻结现状**（维护至 Rust oracle 退役，不新增能力）；第二批 capi（memory/breakpoints 语言中立化导出）**裁不做**，下游并发改道 wasm 多实例（.NET Wasmtime 多 Store）、交互以 serve 协议为终态载体。**通知义务**：须正式通知 SharpTutor（原降级线升级为改道，含 Wasmtime 集成成本说明）。
- 原任务描述：SharpTutor 当前阻塞项；砍 capi 后对象消失。**规则：不得默认搁置**——若 S1 开工即裁"不做"，须通知下游改期。

### M-0 · 基线冻结（S0.5 收尾动作）

release 重建（当前 HEAD 版本串已含，replay 61/61 + serve_smoke 57/57 复跑留痕）→ shadow 报告 + `cases_golden/` 快照 + facts.json 入版本控制或固定 tag，clang 版本串记录在报告内。

---

## 3. S1 基础片（`vitro/{source,diag,opcode,ast}` 四包 + 首发）

### 3.1 工程约定（门 0 三大差异 + 本会话教训，写进包文档）

1. **derive(Eq) 位置在类型体之后**（放名与 `{` 之间会级联十几个"构造器不存在"）；
2. **mut 半颠倒**：`Array.push` 改内容不需要 mut 绑定，mut 只管重新赋值，且 unused_mut 默认即 error；
3. **带参构造器非一等值**：给变体加字段后旧引用点集体回炸（Constr Type Mismatch）——重构预期全量回爆。
4. 工具陷阱五条：SIGPIPE 杀编译器（禁 `| head`）｜管道 `$?` 是尾命令退出码（裸命令或 PIPESTATUS）｜哨兵先证红再采信｜基准同口径必须校验和一致｜Windows 路径 grep 过滤要 `[/\\]` 双兼容。

### 3.2 任务分解

| 任务 | 内容 | 验收 |
|---|---|---|
| T1 `vitro/source` | SourceLoc 三字段（line/column/file_id）+ **列单位契约**（字节偏移+1 声明，双坐标预留）+ 显式 Eq/Ord/Hash | 白盒单测；一条 import 路径（现 Rust 侧 5 条 re-export 收敛为 1） |
| T2 `vitro/diag` | ErrorCode 137 臂（**Go 脚本从 error_codes.rs 生成 .mbt，禁手抄**）+ `code_of/name_of/severity_of/lang_of/catalog_of` 穷尽 match 无 `_` 兜底 + Severity/SourceLang + catalog 77 条 JSON + 覆盖率断言（基线 77，含"码无卡片"可断言清单） | `moon check` 干净；生成幂等（重复运行字节一致 + 源变则产物变检测）；E4 锚点（error_catalog JSON 逐条 diff，码升序） |
| T3 `vitro/opcode` | 132 条 opcode + 编号↔名字双向映射 + operand 语义校验 + `from_u8` 空号/越界返 None（0..255 全空间断言） | 白盒单测；编号手工显式（照搬不重排） |
| T4 `vitro/ast` | Type 17 / Expr 26 / Stmt 16 / decl 全族 + depth（显式栈迭代）+ 类型判等显式函数（Typeof 自反 + vla_dims 策略裁定）+ 渲染单源 `to_c_string`（删双轨）+ `compute_type_size` 移出（三副本之一是死文件亲证）；`Stmt::Try` 保留标 reserved-for-csharp | E1 AST dump（与 Rust 侧 P7 出口对拍，canonicalizer 归一）；E5 mangle 黄金串（`"prefix_p_a2_3_int"`）白盒同断言 |
| T5 首发 | `vitro/diag` 上 mooncakes；`.mbti` 入版本控制作 API 变更信号；137 码位作 versioned 常量只增不改 | mooncakes 页面可安装；README 附三上下文示例 |
| T6 facts 接线 | 新引擎侧真值键空间独立（`moonbit_*` 前缀）；指标自报行格式沿用 | `facts check` 对新键可采 |

### 3.3 S1 完成判据（可机判）

`moon check` 零错 ｜ E1/E3/E4 三锚在双实现上跑通且 diff 为空 ｜ 码表生成幂等 ｜ P1–P7 红锚全绿 ｜ M-0 基线入库 ｜ `vitro/diag` 发布。

---

## 4. 阶段边界（不做什么）

- 不动 lexer/parser/typeck 的 MoonBit 实现（S2–S4 片）；
- 不做 JIT/C++/libc 机制裁定（S9）；
- 不切任何出口（Rust 版继续承担全部消费者直到 S7）；
- Rust 侧除 P1–P7/U1/U2 外只收安全修复（冻结纪律）。
