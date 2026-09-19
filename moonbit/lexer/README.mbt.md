# vitro/engine/lexer — Vitro C 词法器

C 教学子集的完整词法与预处理：**116 变体 token 体系 + 独立预处理 pass**（`\` 续行拼接 → 字符串感知的注释剥离 → 指令消费 → include（显式栈 + LineMap）→ 宏展开），与 Rust oracle（冻结对照实现）的 token 流**逐字节对齐**——经 2400 例随机语料 + 444 例真实语料（baseline/K&R）的 L1/L2 双层差分验证。

## 安装

```bash
moon add vitro/engine/lexer
```

## 三个上下文

### 1. 词法分析：tokenize 拿到完整结果（token 流 + 诊断单值同返）

```mbt check
///|
test {
  let r = @lexer.tokenize(
    "#define MAX(a, b) ((a)>(b)?(a):(b))\nint x = MAX(3, 4);\n",
  )
  inspect(@lexer.TokenType::to_name(r.tokens[3].ty), content="LParen")
  inspect(r.tokens[5].text, content="3")
  // 宏展开产物的位置钉在调用点（与 Rust oracle 对拍口径一致）
  inspect(r.tokens[8].loc.line, content="2")
  inspect(r.errors.length(), content="0")
}
```

### 2. 教学前端：诊断与展开链

```mbt check
///|
test {
  let r = @lexer.tokenize("#define SQ(x) ((x)*(x))\nint y = SQ(i++);\n")
  // W1019：宏参数副作用经典陷阱（非致命警告通道）
  inspect(r.warnings.length(), content="1")
  // 结构化教学 trace，render 输出 wire format 中文串
  inspect(r.trace[0].render(), content="SQ(i ++) ⇒ ( ( i ++ ) * ( i ++ ) )")
}
```

### 3. 宿主注入：wasm 形态下的自定义 include

```mbt check
///|
test {
  // 内存文件表 provider——wasm-gc 无文件系统，include 能力经此注入；
  // 不注入时自定义 include 显式报错（绝不静默失效）
  let files : Map[String, String] = { "src/util.h": "int helper(void);\n" }
  let r = @lexer.tokenize_with_vfs(
    "#include \"util.h\"\nint main;\n",
    files~,
    base_dir="src",
  )
  inspect(r.errors.length(), content="0")
}
```

## 对拍口径（差异分级的依据）

- **L1**（`tokenize_raw`）：字符级原始词法——`#` 产出 Hash/HashHash、无指令消费、无展开；六字段 TSV（含 byte_off）；
- **L2**（`tokenize`）：完整管线，五字段 TSV + errors/warnings 计数；
- `text` 为**规范化值**（整数十进制 / 字符串解码值 / 字符数值）；
- 真实文件归属：`token.loc.file_id` + `token.real_line`（文件内真实行号，0.3.0 起）——`loc.line` 是仿古主坐标（对拍口径）；
- 列号复刻 oracle 口径 `column − text 的 UTF-8 字节数`（已知量纲混算缺陷**故意保留**换可比性；无缺陷坐标用 `byte_off`，真实文件归属用 `loc.file_id` + `line_map`）。

## 差分基础设施（仓库内）

- Rust 侧出口：`vitro_cli dump-tokens <dir> --out <dir> --raw --pp`；
- 比对驱动：`go run ./scripts/lexer_diff <corpus>`（fail loud + `--selftest` 埋雷证红）；
- 语料生成器：`moon run scripts/lexer_diff/gen_corpus.mbtx -- <dir> <count> [seed]`（xorshift64* 确定性；仓库根运行）。
