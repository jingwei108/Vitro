# 开发工具目录

本目录是仓库开发工具（从 Rust oracle 源生成 diag 码表），默认入参指向仓库内 ../native/...——**包消费者无需也不应运行**；生成物（diag/*_gen.mbt）已随包分发并带源 sha256 落款。

## S2 新增工具（2026-09-19）

- `gen_stubs/`（Go）：从 `native/runtime_libc/include/*.h`（14 存根）生成 `lexer/internal/host/stubs_gen.mbt`——sha 落款 + `--check` 幂等门禁；包消费者无需运行（生成物随包分发）。
- `gen_corpus.mbtx`（MoonBit 脚本）**已移至 module 外** `scripts/lexer_diff/gen_corpus.mbtx`（2026-09-19 审阅 P1-2：moon publish 打包整个 module 且不读 .gitignore——脚本在 module 内运行会产生 `scripts/_build/` 构建产物并被打进发布包，0.2.0 曾混入 176KB）；用法 `moon run scripts/lexer_diff/gen_corpus.mbtx -- <out_dir> <count> [seed]`（仓库根运行）。
