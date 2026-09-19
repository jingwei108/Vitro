# vitro/engine/parser

Vitro C 教学引擎的语法分析器（S3 片）——token 流（`vitro/engine/lexer/token`）→ AST（`vitro/engine/ast`）。
解析语义照搬 Rust oracle `native/crates/vitro_parser`（差异对拍：597 真实语料样本的 AST dump + 诊断序列归一后逐字节一致）。

## 快速上手

```mbt check
///|
test {
  // 空 token 流：越界哨兵提供 Eof 语义，产出空 ProgramNode
  let r = @parser.parse([])
  inspect(r.errors.length(), content="0")
  inspect(r.stall_count, content="0")
  let p = r.program.unwrap()
  inspect(p.funcs.length(), content="0")
  inspect(p.globals.length(), content="0")
}
```

```mbt check
///|
test {
  // 顶层裸 token：E2005 + 零进度保护推进（多错误收集而非首错即停）
  let toks : Array[@token.Token] = []
  toks.push({
    ty: Number,
    text: "42",
    loc: { line: 1, column: 1, file_id: 0 },
    byte_off: 0,
    real_line: 1,
  })
  let r = @parser.parse(toks)
  inspect(r.errors[0].code, content="2005")
  inspect(r.errors[0].line, content="1")
  inspect(r.errors[0].column, content="1")
}
```

## 防护形态（与 Rust oracle 的差异——刻意不复刻）

Rust 用"共享计数器 + 8 处壳挂点"，本实现改为**显式 depth 参数**沿调用链传递
（新增递归点必须声明 depth，编译器强制）；计数口径与 Rust 完全同构
（62 层括号通过 / 63 层拒绝的边界逐位一致）。声明符类型通道的纯 Array 链
**迭代解释**（wasm 栈预算内 1250 层存活——Rust 递归形态实测 1200 层即溢出，
J1 缺陷的 MoonBit 复现被结构性根治）。

## 诊断面

`ParseError { message, line, column, code }`——多错误收集而非首错即停；
`column` 语义 = 行内 UTF-8 字节偏移 + 1（`vitro/engine/source` 契约）。
`stall_count` 为主循环零推进计数（活性内部断言的观测面，恒 0）。

差分管线（E1–E4）见仓库 `scripts/parser_diff`（Go 驱动，fail loud +
`--selftest` 埋雷）；MoonBit 侧批量出口 `cmd/dump_ast`。
