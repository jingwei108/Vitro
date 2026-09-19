# Vitro MoonBit 引擎（活跃区）

> Rust 冻结对照区（`../native/`）的渐进迁移目标实现。上位文档：
> [MoonBit迁移总计划](../docs/current/01-定位与路线/MoonBit迁移总计划.md) ｜
> [第一阶段计划](../docs/current/01-定位与路线/MoonBit迁移第一阶段计划.md)

## 包清单（S1 基础片）

| 包 | 层 | 职责 | 锚 |
|---|---|---|---|
| `vitro/engine/source` | L0 | SourceLoc 三字段 + 列单位契约（字节偏移+1 主坐标 / Pos 双坐标预留） | 白盒单测（三量纲分歧锚） |
| `vitro/engine/opcode` | L0 | 132 条 opcode（编号照搬不重排，空号 44–49）+ 双向映射 + Instruction | 0..255 全空间断言 |
| `vitro/engine/diag` | L1 | ErrorCode 137 臂（**gen_diag 生成，禁手抄**）+ Severity/SourceLang + catalog 77 条 + E4 出口 | 覆盖率断言 + E4 全量对拍（canonicalize 后逐字节一致） |
| `vitro/engine/ast` | L2 | Type 17 / Expr 26 / Stmt 16 / decl 全族 + depth（显式栈）+ type_eq + to_c_string 单源 + mangle + E1 dump emitter | E1 对拍两样本 diff 空 + E5 黄金串 `prefix_p_a2_3_int` |

## 生成物纪律

```bash
cd moonbit
go run ./scripts/gen_diag          # 从 Rust 源再生成 diag 包机器单源部分
go run ./scripts/gen_diag -check   # 幂等校验（源变产物变 / 产物被篡改即红）
```

- `diag/error_code_gen.mbt`、`diag/catalog_gen.mbt` 为生成物（**禁手改**，文件头有源 sha256 落款）；
- 生成流程内置 `moon fmt`——产物最终形态以 fmt 输出为准，gen 与 fmt 不互踩；
- 基线漂移（137 臂 / 77 卡片）fail loud：源变更须人工核对后更新 `expectedArms` / `expectedCatalog` 并登记差异。

## 验证

```bash
moon check && moon test    # 112 测试（source 14 / opcode 10 / diag 24+ / ast 13+ / lexer 51）——S2 起 lexer 入列；分解数以 moon test -v 为准
moon info                  # .mbti 接口面（API 变更信号）
```

## 坐标契约（vitro/engine/source）

输入档案：[列号口径冻结](../docs/current/07-质量与裁定/列号口径冻结.md) §2。

```mbt nocheck
SourceLoc { line, column, file_id }  // column = 行内 UTF-8 字节偏移 + 1
Pos { byte_off, col_scalar, col_utf16 }  // 双坐标预留，三值同源派生
```
