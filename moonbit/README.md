# Vitro MoonBit 引擎

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

C 教学引擎的 MoonBit 实现——137 个诊断错误码、132 条字节码操作码、Type/Expr/Stmt 全族 AST 与 C 渲染/mangle，码表与 [Vitro Rust oracle](https://github.com/jingwei108/vitro) 逐字节对拍对齐。

## 安装

```bash
moon add vitro/engine        # 或按包引入 vitro/engine/diag 等
```

## 包清单

| 包 | 层 | 职责 |
|---|---|---|
| `vitro/engine/source` | L0 | SourceLoc 三字段 + 列单位契约（字节偏移+1 主坐标 / Pos 双坐标预留） |
| `vitro/engine/opcode` | L0 | 132 条 opcode（编号照搬不重排，空号 44–49）+ 双向映射 + Instruction |
| `vitro/engine/diag` | L1 | ErrorCode 137 臂 + Severity/SourceLang + 教学卡片 77 条 + 目录导出（对拍逐字节一致） |
| `vitro/engine/ast` | L2 | Type 17 / Expr 26 / Stmt 16 / decl 全族 + depth + type_eq + to_c_string + mangle + JSON dump |

各包 API 概览见对应目录的 `pkg.generated.mbti`；`diag` 的三上下文用法示例见 [`diag/README.mbt.md`](diag/README.mbt.md)（可执行文档测试）。

## 快速上手（diag）

```moonbit
let code = @diag.ErrorCode::from_code(3053).unwrap()
code.display_code()          // "W3053"
code.severity().to_str()     // "warning"
code.catalog()               // Some(教学卡片) —— 标题 / 解释 / 常见原因
```

## 契约要点

- **码位只增不改**（versioned 常量语义）——`ErrorCode::code` 是稳定 ABI；
- 穷尽 match 无兜底臂：新增枚举臂在依赖方重新编译时立即暴露；
- 坐标契约：`SourceLoc.column` = 行内 UTF-8 字节偏移 + 1；双坐标消费方用 `Pos{byte_off, col_scalar, col_utf16}`；
- 渲染与 mangle 单源（`Type::to_c_string` / `Type::mangle_name_into`）。

## 验证

```bash
moon check && moon test    # 136 测试（source 14 / opcode 10 / diag 21 / ast 13 / lexer 51 / parser 23）——S3 起 parser 入列；分解数以 moon test -p 逐包为准
moon info                  # .mbti 接口面（API 变更信号）
```

## 目录说明

- `scripts/gen_diag/`：**仓库开发工具**（从同仓库 Rust 源生成 diag 码表，默认入参指向 `../native/...`——包消费者无需也不应运行；生成物已随包分发且带源 sha256 落款，`go run ./scripts/gen_diag -check` 可校验其未漂移）；
- `*/pkg.generated.mbti`：`moon info` 生成的公共接口面。

## 上游与对拍

本 module 是 [Vitro 项目](https://github.com/jingwei108/vitro)（C 教学引擎，MIT）MoonBit 迁移的第一阶段产物：诊断目录（77 卡片）与 Rust oracle 出口经 canonicalize 归一后逐字节一致；AST dump 与 mangle 黄金串同源断言。迁移路线与差分锚点见上游仓库 `docs/current/`。
