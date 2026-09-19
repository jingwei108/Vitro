# MoonBit 活跃区操作手册（moonbit/）

> **本文件是按需读取的分区手册**——根 [`AGENTS.md`](../AGENTS.md) 双区制路由指向此处，**仅在触碰 MoonBit 活跃区（`moonbit/`）时才读取**，与 [`native/AGENTS.md`](../native/AGENTS.md)（Rust 冻结对照区）对称。
> **全域纪律**（中文输出 / 禁擅自 git 提交 / 实测大于脑测 / 诚实记录 / 红→绿 / J9 / archive 规则 / 提交署名规则）以根 AGENTS.md 为准，同样约束本区。
> **上位文档**：[MoonBit迁移总计划](../docs/current/01-定位与路线/MoonBit迁移总计划.md)（包切分 L0–L9 / 锚点体系 / F1–F9 语言事实）+ [第一阶段计划](../docs/current/01-定位与路线/MoonBit迁移第一阶段计划.md)。本手册只沉淀**工程操作层**（命令 / 陷阱 / 纪律 / 发布），不重复上位文档内容。

## 包清单与状态（S1 后）

| 包 | 层 | 职责 | 状态 |
|---|---|---|---|
| `vitro/engine/source` | L0 | SourceLoc + 列单位契约（column = 行内 UTF-8 字节偏移+1）+ Pos 双坐标 | ✅ 已发布 0.1.1 |
| `vitro/engine/opcode` | L0 | 132 条 opcode（空号 44–49）+ Instruction | ✅ 已发布 |
| `vitro/engine/diag` | L1 | ErrorCode 137 臂（gen_diag 生成）+ catalog 77 + E4 出口 | ✅ 已发布 |
| `vitro/engine/ast` | L2 | Type 17 / Expr 26 / Stmt 16 全族 + depth + E1 emitter | ✅ 已发布 |

命名规则：module = `vitro/engine`（mooncakes owner `vitro`），包全名 `vitro/engine/<pkg>` 一律全名，代码与配置禁用简称。

## 构建与验证命令

```bash
cd moonbit
moon check                # 快速类型检查（日常常跑）
moon check --target all   # 全后端检查（发布前）
moon test                 # 58 测试（白盒 _wbtest.mbt + 黑盒 _test.mbt + doc 测试）
moon test --update        # 快照更新（inspect content= 变更时；核对 diff 再提交）
moon fmt                  # 格式化（生成物也参与——见 gen_diag 内置 fmt）
moon info                 # 生成 .mbti 接口面（API 变更信号；pkg.generated.mbti 入版本控制）
go run ./scripts/gen_diag          # diag 码表再生成（内置 moon fmt）
go run ./scripts/gen_diag -check   # 幂等校验（源变产物变 / 篡改即红）
```

测试计数已入 facts 机判（`moonbit_test_passed` 键）：moonbit/README*.md 的测试数漂移会被 `go run ./scripts/facts check` 抓红——改测试数必须同步 README。

## MoonBit 语言与工具链陷阱（全部一手实证，2026-09-19）

> 总计划 §3 的 F1–F9 仍有效；以下是**工程语法/工具层**补充，多数来自 S1 实作的编译器报错。

1. **`ref` 是保留字**——可变局部状态用 `let mut x = 0`（`Array.push` 改内容**不需要** mut，mut 只管重新绑定；unused_mut 默认 error）。
2. **比较 trait 是 `Compare` 不是 `Ord`**——`derive(Debug, Eq, Compare, Hash, Default)`；derive 子句在类型体 `{}` 之后。
3. **`derive` 不能作方法名**（关键字）——S1 用 `Pos::from_byte_off` 替代。
4. **枚举构造器可与其余类型名同名**（`Type::Int` 与内置 `Int` 按期望类型消歧）；但**两个枚举的同名构造器**（`AssignOp::Assign` vs `Expr::Assign`）在构造与模式两处都要显式 `Expr::Assign(...)` 限定。
5. **跨包类型必须 `@pkg.` 前缀**——包括 struct 字段类型与函数签名参数（`loc : @source.SourceLoc`），裸类型名会撞"未定义"或静默歧义。
6. **无参构造器模式**要逐参数通配：`Literal(_, _, _)`——`Literal(..)` 不是合法通配。
7. **`for k, v in` 只解构 Map**——元组数组迭代用 `for t in arr` 后 `t.0 / t.1`。
8. **fn 参数不能 `mut`**（`mut stack : ...` 是解析错误）；局部重绑定不需要时勿加 mut。
9. **大写开头的局部绑定非法**（`let L = ...` 编译错）。
10. **`assert_true(x)` 单参数**——带消息断言用 `guard cond else { fail("...") }`。
11. **`String::substring` 已弃用**——用切片 `s[a:b]`（ASCII 安全场景免 try）+ `.to_owned()`（视图转拥有串；`.to_string()` 在视图上同样弃用）。
12. **`<+` / `<?` 宏右侧只能接模板字符串/对象字面量**——任意表达式（函数调用、Int）不合法，用 `buf.write_string("...\{interp}")`。
13. **`1e16` 等 e 记法浮点字面量不可用**——写定点 `10000000000000000.0`；指数值用除法/构造。
14. **`Double::to_string` 与 serde_json/ryu 两处分歧**（实测）：整值 `1.0 → "1"`、负零 `-0.0 → "0"`；中段指数区间（≥1e16 / ≤1e-6）两侧记法不同。**JSON 浮点文本化必须走 `@ast.double_to_json_text`**（ryu-pretty 全区间对齐 + 非有限 → null）——那是单源，禁再写一份。
15. **moon fmt 宽度按 UTF-8 字节计**（中文 3 字节；超 ~84 字节爆开/折行）——**生成物与 fmt 会互踩**：gen_diag 的解法是生成流程内置 `moon fmt`，产物形态以 fmt 为准，gen 原始输出只是中间态。任何新生成器沿用此模式。
16. **doc 测试（docstring `mbt check` 块与 `*.mbt.md`）是黑盒**——被测包自动 import 为 `@self`，构造器要 `@pkg.Type::Variant` 形态；README.mbt.md 同理（它同时是可执行测试文件）。
17. **`moon.mod` / `moon.pkg` 是新格式**（非 .json）；`moon.pkg` 的 import 块声明依赖，代码内一律 `@alias.fn` 调用。
18. **mooncakes 发布规则**：module 名首段**必须等于发布者用户名**（`vitro/engine` ↔ 账号 `vitro`；403 User mismatch 即此因）；发布后 checksum 入 registry **不可覆盖**——修复只能递增版本；readme 字段须指向**根 `README.md`**（`README.mbt.md` 不会被模块页渲染）；索引同步有数分钟延迟（`moon add` 暂时 404 是正常节奏，轮询即可）。
19. **本仓库 bash 工具层的 heredoc 会吃一层反斜杠转义**（`\\n` 变真换行、`\r\n` 字面量损毁）——跨 heredoc 写 Go/代码文件时用 `chr()` 拼接或 python 中转写盘；此坑曾致 gen_diag 归一逻辑静默变空操作（P2-1 复盘）。

## 编码与架构纪律（S1 已定型）

1. **穷尽 match 无兜底臂**——枚举增删变体必须让全仓编译红（gen_diag 生成的 137 臂 / opcode 全表 / emitter 均如此）；`from_code` / `from_u8` 这类 Int→枚举允许 `_ => None`（值域无限，数学必然）。
2. **显式 emitter 纪律**（B 级锚前提）：结构化导出（JSON）两侧手写序列化，禁一侧 serde 一侧 ToJson 派生——字段序、转义、浮点文本化在 emitter 单点对齐 Rust oracle。对拍管道：Rust serve 出口 → Go canonicalize 归一 → diff（`go run ./scripts/canonicalize`，stdin 进 stdout 出）。
3. **生成物纪律**：`diag/error_code_gen.mbt`、`diag/catalog_gen.mbt` 禁手改（文件头源 sha256 落款）；源（native 侧）变更后必须再生成并提交，`-check` 在 CI/审阅中防漂移；行尾归一双保险（.gitattributes 锁 moonbit/** LF + `-check` 比较前归一）。
4. **照搬不私改**：从 Rust oracle 迁移的定义（opcode 编号、判等语义、Display 渲染口径、mangle）逐字照搬；发现 oracle 现状可疑（如 Display 丢 unsigned）**登记不修正**——差分对拍期两侧一致优先，裁定走差异台账。
5. **坐标契约**（vitro/engine/source）：`column` = 行内 UTF-8 字节偏移 + 1；双坐标消费方用 `Pos`；禁字节/字符量纲混算；词法 +1 现状不复刻。输入档案：`docs/current/07-质量与裁定/列号口径冻结.md`。
6. **诚实延后登记**：暂缓项在包注释/计划文档写明原因与回归时机（如 operand 语义校验待 S5 发射侧实证、E1 全量对拍管道待 S3 parser）。

## 发布流程（T5 定型）

```bash
# 前置：moon check --target all 干净 + moon test 全绿 + moon info 无意外 diff + gen_diag -check 绿
cd moonbit && moon publish        # Server 200 OK 后：
# 验收：moon search vitro 可查 → 全新项目 moon add vitro/engine@<ver> →
#       安装后跑一段示例；包 zip（~/.moon/registry/cache）核对根级四件：
#       LICENSE / README.md / README.mbt.md / moon.mod
```

版本语义：修复发布内容 → patch；新增包/公共 API → minor。137 码位等 versioned 常量只增不改。

## facts 接线

`moonbit_test_passed`（`--run` 门采集，cached 沿用）——新增防线产物时按 `moonbit_*` 前缀扩键（总计划 §10 每片回填义务），采集器在 `scripts/facts/facts.go::collectMoonbit`（冻结区，改动属防线维护白名单）。
