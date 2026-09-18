# Vitro 项目 Agent 指南

> [English Version](AGENTS_EN.md)
>
> 本文件只保留**双区路由与全域纪律**。Rust 冻结区的完整操作手册（技术栈 / 构建命令 / 测试防线 / 编码约定 / C 子集概览 / 已知 Clang 差异 / 调试技巧 / CLI）在 [`native/AGENTS.md`](native/AGENTS.md)——**仅在触碰 Rust 区时才读取**，避免上下文挤占与跨区幻觉。

## 仓库双区制（2026-09-18 起，MoonBit 迁移期 · 绞杀者模式）

| 区 | 范围 | 状态 | 规则来源 |
|---|---|---|---|
| **MoonBit 活跃区** | `moonbit/`（S1 起创建，MoonBit workspace） | 全部新开发在此 | [`MoonBit迁移总计划`](docs/current/01-定位与路线/MoonBit迁移总计划.md) + [`MoonBit迁移第一阶段计划`](docs/current/01-定位与路线/MoonBit迁移第一阶段计划.md)（含包切分 / 锚点体系 / 工程约定 / 工具陷阱） |
| **Rust 冻结对照区** | `native/`、`scripts/`、`.github/` | **diff oracle，已冻结**（tag `rust-oracle-freeze`） | [`native/AGENTS.md`](native/AGENTS.md)（**按需读取**）；只允许：第一阶段计划 §2 白名单（P1–P7/U1/U2）+ 安全修复 + 防线维护 |

**路由规则**：只在 MoonBit 区 / 文档 / 讨论中工作 → **不要读** `native/AGENTS.md`；触碰 `native/`、`scripts/`、CI，或需要跑防线（cargo / `go run ./scripts/*` / shadow）→ **先读** `native/AGENTS.md`。

## 全域纪律（两区共守，语言无关）

1. **必须中文输出思考与回答**
2. **未经允许禁止 git 提交**
3. **实测大于脑测、统一真相来源**：结论须来自亲跑命令 / 亲读代码；报告与文档声明只作线索不作依据；数字对真值（`reports/facts.json` 的 key + as_of）
4. **诚实记录**：以 Clang 为标准，任何与标准不符之处必须记录；禁止修改测试预期值粉饰数据
5. **红→绿纪律**：每个缺陷修复先有会失败的用例，修复提交引用用例名；护栏 / 判定型脚本必须先证会红（J9 埋雷义务）
6. `docs/archive/` 下的归档文档**不具备参考价值**，禁止引用
7. **文档体系**：新文档进 `docs/current/` 对应分类子目录（中文文件名），同步 `docs/README.md` 索引；被取代的移入 `docs/archive/`（`ARCHIVE_` 前缀 + 归档横幅）
8. **判定型脚本默认 Go**：零第三方依赖、规则外置 JSON、fail loud、禁止静默 default
9. **工具陷阱五条**：`| head` 会 SIGPIPE 杀编译器（判"构建失败"前必须 tail 全量输出）；管道 `$?` 是尾命令退出码（用 PIPESTATUS 或裸命令取）；哨兵先证红再采信结论；基准对照必须校验和逐位一致才计时；Windows 路径的 grep 过滤要 `[/\]` 双兼容

## 当前阶段与档案

- **当前**：第一阶段（S0.5 Rust 止血批 + S1 基础片）——逐项任务与验收锚见[第一阶段计划](docs/current/01-定位与路线/MoonBit迁移第一阶段计划.md)
- **退役**：MoonBit 全量切换（总计划 §10）完成后，Rust 区**整体删除**（不移入子文件夹——死树留在盘上与 Agent 上下文里才是干扰）；档案 = tag `rust-oracle-freeze` + git 历史（MoonBit 探测阶段 16 份文档在提交 `917251e`，取回方法见总计划 §11）
