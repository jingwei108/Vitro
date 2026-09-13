# Cide CLI 使用手册

> 最后核对日期：2026-09-11
> 修订说明（2026-09-11）：去前端化——移除已删除的 `CIDE_CLI_EN.md` 链接，入口表述改为"无前端依赖"并补三出口交叉引用。
> 出口定位：本文档描述的是"三出口一核心"中的**出口 3**（`cide_cli serve` JSON-lines 会话模式）；另两个出口为 C ABI（`native/src/capi/`）与 wasm32，完整清单与职责边界见 [`后端定位与白箱计划.md`](../../01-定位与路线 §2.2。

`cide_cli` 是 Cide 项目 Rust 后端的命令行调试工具，**无前端依赖（headless 交互第一入口）**，可直接编译、运行和单步调试 C 代码。

## 构建

```bash
cd native
cargo build --release --bin cide_cli
```

构建产物位于 `native/target/release/cide_cli`（Linux/macOS）或 `native/target/release/cide_cli.exe`（Windows）。

## 基本用法

```bash
cide_cli <command> <file> [options]
```

## 命令

| 命令 | 说明 |
|------|------|
| `compile <file>` | 编译 C 文件并显示诊断信息（错误/警告/建议） |
| `run <file>` | 编译并全速运行程序 |
| `step <file>` | 交互式单步调试 |
| `unified <file>` | 统一模式（时间旅行引擎）批量执行并输出摘要（支持 `--max-steps <n>`） |
| `serve` | **JSON-lines 会话模式**（headless 交互；无文件参数，内容经 stdin 提供），见下文 |

## 选项

| 选项 | 说明 |
|------|------|
| `-i <file>` | 从指定文件读取标准输入（多行输入，供 `scanf`/`fgets` 等使用） |
| `--max-steps <n>` | 统一模式下允许的最大执行步数（默认 100_000），用于长程序时间旅行或性能基线测试 |

## 特殊文件名

使用 `-` 作为文件名时，CLI 从**标准输入**读取源代码，便于快速测试代码片段：

```bash
# 管道方式
echo '#include <stdio.h>
int main() { printf("hello\n"); return 0; }' | cide_cli run -

# here-document 方式
cide_cli compile - <<'EOF'
#include <stdio.h>
int main() {
    int a = 10, b = 20;
    printf("%d\n", a + b);
    return 0;
}
EOF
```

## 使用示例

### 1. 编译并检查诊断

```bash
cide_cli compile hello.c
```

输出示例：
```
编译成功。
检测到算法:
  • 数组遍历 (置信度: 95%)
```

若存在错误：
```
=== 诊断信息 ===
[错误] 4:5  类型不匹配：无法将 'char[6]' 赋值给 'int' (E3004)
    建议: 赋值或传参时，左右两边的类型不一致...

编译失败。
```

### 2. 全速运行

```bash
cide_cli run hello.c
```

输出示例：
```
编译成功。

=== 运行输出 ===
Hello, Cide CLI!

程序运行完成，返回值：0
```

### 3. 带输入运行

```bash
# input.txt 内容：
# 5 7

cide_cli run sum.c -i input.txt
```

### 4. 交互式单步调试

```bash
cide_cli step hello.c
```

进入调试交互后，支持的命令：

| 调试命令 | 说明 |
|----------|------|
| `Enter`（空输入） | 执行下一步 |
| `p` / `print` | 打印当前局部变量 |
| `o` / `output` | 打印当前程序输出 |
| `r` / `run` | 全速运行到结束 |
| `q` / `quit` | 退出调试 |

输出示例：
```
=== 交互式单步调试 ===
命令: [Enter]=下一步, p=打印变量, o=打印输出, q=退出, r=运行到结束

步    0 | 行   0:   >
步    1 | 行   3: int main() {  > p
  a: Int = 10
  b: Int = 20
步    2 | 行   4: int a = 10;  >
```

### 5. 统一模式（时间旅行引擎）

```bash
cide_cli unified hello.c
```

输出示例：
```
=== 统一模式执行（时间旅行引擎）===
  共执行 117 步

=== 执行摘要 ===
总步数: 117
状态: 正常结束

=== 最终输出 ===
sum=15
```

统一模式会完整记录每一步的 VM 状态，支持检查点保存和回溯；`cide_cli serve` 的 `step.begin` / `step.next` / `seek` 与之共用同一 `UnifiedEngine`（三出口一套语义，见 §6）。

对于可能超过默认 10 万步限制的长程序，可使用 `--max-steps` 放宽限制：

```bash
cide_cli unified long_sort.c --max-steps 500000
```

### 6. serve：JSON-lines 会话模式（Phase 1 出口 3）

长寿命 headless 会话进程：**stdin 每行一个 JSON 请求，stdout 每行一个 JSON 响应**（NDJSON）。
供 IDE 后端/判分服务/自动化脚本以任意语言消费，无需 ctypes 或 FFI。

```bash
cide_cli serve
```

协议契约：

| 契约 | 说明 |
|---|---|
| **id 关联** | 请求可带 `id`（任意 JSON 值），响应原样回填 —— 便于异步/乱序对账 |
| **帧同构** | 成功帧 `{"id":N,"ok":true,"result":{…}}`，错误帧 `{"id":N,"ok":false,"error":{"kind":…,"message":…}}` —— 解析路径统一 |
| 错误 kind | `protocol`（请求格式/未知方法）/ `state`（会话状态不满足）/ `internal` |
| 入口语义 | 与 capi **共用 `session_api`**（运行结果/诊断/步 payload 形状完全一致），三出口不产生语义分叉 |
| 会话配置 | `quarantine_budget` / `deterministic` / `max_steps` / `call_depth_limit` 与 capi 同名 setter 一致 |
| 重置语义 | `session.reset` 清空编译/运行状态，**保留会话级配置**（隔离预算、判分确定性、argv） |
| **会话拓扑** | **单 serve 进程 = 单活跃会话**：方法表无并发句柄参数，`session.create`/`destroy` 都是"清空重建同一实例"；需要并发逻辑会话（如"长寿命诊断进程 + 瞬态运行进程"）时**起多个 serve 进程**——这是当前唯一受支持的并发形态（下游需求清单 D2）|

方法一览：

| 方法 | 参数 | 说明 |
|---|---|---|
| `ping` | — | 存活探测，返回 ABI 版本 |
| `compile` | `source`（或 `files:[{filename,source}]`） | 覆盖式编译当前单元集合，返回诊断 JSON |
| `run` | `input` / `argv` / `batch_input` / `max_steps` / `deterministic` | 全速运行，返回 `status`/`return_value`/`steps_executed` |
| `input.feed` | `text` | 增量喂入交互输入并续跑（`run` 返回 `waiting_input` 后调用；`text` 可多行，省略则仅续推进一步） |
| `output.delta` | `cursor` | 增量取输出（字节游标，UTF-8 边界安全） |
| `step.begin` | — | 初始化统一模式（时间旅行），需先编译成功 |
| `step.next` | — | 单步推进，返回 `payloads`（StepPayload，见 [`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md`](../../spec/STEP_PAYLOAD_SCHEMA_V0_1.md） |
| `payload.get` | `start` / `end` | 取窗口内步 payload（窗口 2000 帧，越窗静默裁剪） |
| `seek` | `step` | 时间旅行定位（窗口外走检查点恢复 + 正向重放） |
| `breakpoints.set` | `lines:[int]` | 设置断点行集合（应在 `step.begin` 之后） |
| `memory.regions` | — | **三段式内存地图**（`kind` = `global`/`stack`/`heap`，见下方说明）+ 隔离区统计 |
| `config.get` / `config.set` | 同上配置项 | 读写会话级配置 |
| `error_catalog` | — | 错误码表机器可读导出（`{catalog:[{code,code_str,lang,category,emoji,title,explanation,common_causes[]}]}`，按 code 升序） |
| `semantic_labels` | — | `semantic_label` 受控词汇表导出（`{schema,discipline,labels:[{id,domain,template,example,status,since}]}`；词汇只增不改） |
| `contracts` | — | schema 版本轨道与行为契约（预留位字段名、v0.2 激活清单与字段台账、行为契约表） |
| `capabilities` | — | 机器可读能力清单（`engine_version`（含构建期 git 短哈希，可用于产物自检）、版本宏名义锚点、语言子集、内存模型常量、schema 轨道、行为契约） |
| `session.create` / `session.reset` / `session.destroy` | — | 会话生命周期（响应带 `session` 拓扑语义字段，见上表"会话拓扑"）|
| `shutdown` | — | 结束 serve 进程（EOF 亦可） |

**`memory.regions` 的三段式（2026-09-12，下游需求清单 C2）**：`regions` 是统一数组，
每项带 `kind`，数组按地址升序；响应另带 `region_counts{global,stack,heap}`。

| `kind` | `name` | `alloc_line` | `alloc_by` | 备注 |
|---|---|---|---|---|
| `heap` | `heap_N` / `FILE:<path>` | 分配点行号 | `malloc` / `calloc` / `realloc` / `strdup` / `fopen` / `vfs` | 保留 `is_freed`（三色堆图） |
| `global` | 变量名 | 声明行 | `static` | 由 VM 全局符号合成 |
| `stack` | 函数名 | **进入该帧的调用行**（`main` 为 0） | `call` | `size` = 帧跨度 |

栈/全局区域**只在导出层合成**，不写回内部堆清单（堆统计口径不受影响）。
`capi` 第二批将把该形状语言中立化。

示例（一次会话跑完编译 → 运行 → 取输出 → 单步 → 收尾）：

```bash
$ cide_cli serve <<'EOF'
{"id":1,"method":"compile","params":{"source":"#include <stdio.h>\nint main(){ printf(\"%d\", 1+2); return 0; }\n"}}
{"id":2,"method":"run"}
{"id":3,"method":"output.delta","params":{"cursor":0}}
{"id":4,"method":"step.begin"}
{"id":5,"method":"step.next"}
{"id":6,"method":"session.reset"}
{"id":7,"method":"shutdown"}
EOF
{"id":1,"ok":true,"result":{"diagnostics":[],"ok":true}}
{"id":2,"ok":true,"result":{"ok":true,"return_value":0,"status":"finished","steps_executed":13,"trap":"","waiting_input":false}}
{"id":3,"ok":true,"result":{"cursor":36,"delta":"3程序运行完成，返回值：0\n","stream":"display","total":36}}
{"id":4,"ok":true,"result":{"max_collected_step":-1,"ready":true}}
{"id":5,"ok":true,"result":{"cache_start_step":0,"current_line":0,"finished":false,"paused":false,"payloads":[{"accessed_vars":[],"algorithm_step":null,"array_snapshots":[],"call_stack":[],"code_line":0,"func_name":"","heatmap_count":0,"heatmap_line":0,"local_vars":[],"pointer_snapshots":[],"root_cause_hint":null,"semantic_label":"","step_index":0,"vis_events":[]}],"trapped":false,"waiting_input":false}}
{"id":6,"ok":true,"result":{"config":{"compiled":false,"deterministic":false,"input_mode_batch":false,"quarantine_budget":262144},"reset":true}}
{"id":7,"ok":true,"result":{"shutdown":true}}
```

> 上述输出为 2026-09-11 实测（字段顺序由 JSON 对象语义决定，消费方不应依赖顺序）。
>
> **E-P1-5（输出通道）**：`output.delta` 默认返回 `stream:"display"` —— 即展示视图，除程序输出外
> 还含 Cide 追加的"程序运行完成"提示与（有泄漏时的）泄漏报告，供 UI 原样显示。
> 需要**纯净程序 stdout**（判分 / 与 Clang golden 比对）时传 `"stream":"stdout"`：
>
> ```json
> {"id":3,"method":"output.delta","params":{"cursor":0,"stream":"stdout"}}
> ```
>
> 可用取值：`display`（默认）/ `stdout` / `stderr` / `note`（引擎附注）。引擎内部按
> `OutputKind` 给每段输出打标，消费方**不得**再对文本做正则清洗——此前散落十余处的
> "程序运行完成"清洗规则已在 E-P1-5 中全部废除（程序自己打印同类文本时会被误删）。
>
> 与 `jq` 配合：`cide_cli serve < session.ndjson | jq -c 'select(.ok|not)'` 可只筛错误帧。
>
> **输入语义（`InputMode`）**：`run` 的 `batch_input`（默认 `false`）决定"输入耗尽"的含义：
>
> - `batch_input:false`（默认，交互）：`scanf`/`getchar` 在流耗尽时置 `waiting_input` 挂起，
>   等待 `input.feed` 供给——适合"学生逐行键入"的教学交互；
> - `batch_input:true`（批量/判分）：流耗尽即 **EOF**（`scanf` 返回 `-1`、`getchar` 返回 `-1`），
>   程序正常 `finished`——`while (scanf("%d", &n) != EOF)` 这类 C 第一课习语依赖此语义。
>   **判分 / 批量路径应以 `batch_input:true` 为准**（一次性给全 stdin 时语义等价于 EOF）。
>
> CLI 的 `cide_cli run <file> -i <input>`（headless 批处理）固定走 Batch，无需额外参数。
>
> **EOF 粘滞**（2026-09-12 补，对齐 C11 7.21.5.1 `feof`）：Batch 下**任一路径**首次判定
> "流耗尽"即置位 `stdin_eof` 并把读取游标推到底——此后 `scanf`/`getchar` 一律返回 `-1`，
> 未消费的尾部空白不会被"复活"重读。此前 `scanf` 触发的 EOF 对 `getchar` 不可见：
> 输入 `7\n` 时 Clang 给 `r1=1 r2=-1 c=-1`，Cide 曾给 `c=10`。交互模式下调 `input.feed`
> 属"新内容到达"，会清除该粘滞位（管道语义下本不可复活，教学交互例外）。
>
> **交互喂入状态机**：`run` → `waiting_input` → `input.feed {text}` → `running`
> → `waiting_input | finished | trap`（`feed` 在非等待态亦可调用，文本追加到缓冲末尾后续跑）。
>
> ```json
> {"id":2,"method":"run","params":{"input":"7\n"}}          // → waiting_input
> {"id":3,"method":"input.feed","params":{"text":"35\n"}}   // → finished，读入 a=7, b=35
> ```
>
> 防线：`python scripts/serve_smoke.py` 覆盖 id 关联 / 帧同构 / 生命周期 / 配置一致性 / 三段式内存地图 / schema 轨道与词汇表的 51 项断言（CI 已纳入，脚本自报口径）。

## 快速测试片段

无需创建临时文件，直接通过标准输入快速验证代码：

```bash
# 测试 printf
echo '#include <stdio.h>
int main() { printf("ok\n"); return 0; }' | cide_cli run -

# 测试循环
cide_cli unified - <<'EOF'
#include <stdio.h>
int main() {
    int s = 0;
    for (int i = 1; i <= 100; i++) s += i;
    printf("%d\n", s);
    return 0;
}
EOF

# 测试 scanf + 输入
cat <<'EOF' | cide_cli run - -i /dev/stdin
#include <stdio.h>
int main() { int a,b; scanf("%d%d",&a,&b); printf("%d\n",a+b); return 0; }
EOF
# 然后输入两个数字并按 Ctrl+D（Unix）或 Ctrl+Z（Windows）
```

> **注意**：当使用 `-` 从 stdin 读取源代码时，不能再通过 `-i -` 从同一 stdin 读取输入数据，建议将输入数据写入文件后使用 `-i data.txt`。
