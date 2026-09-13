# K&R C 经典例题失败记录

> 记录原则：诚实记录，不隐藏失败。
> 格式参见 `docs/current/PHASE_KR_LEETCODE_TEST_PLAN.md` 附录 B。

## 统计摘要

| 阶段 | 总数 | 通过 | 已知失败 | 记录时间 |
|:-----|:-----|:-----|:---------|:---------|
| K&R 第 1-2 章 | 25 | 25 | 0 | 2026-06-06 |
| K&R 第 3-4 章 | 22 | 22 | 0 | 2026-06-07 |
| K&R 第 5-6 章 | 22 | 22 | 0 | 2026-06-14 |
| K&R 第 7 章 | 7 | 7 | 0 | 2026-06-18 |
| **合计** | **76** | **76** | **0** | - |

## 历史修复记录（当前无活跃已知失败）

> 以下条目均为历史已修复记录，保留用于追溯问题根因与修复过程。
> 当前 K&R 防线 69/69 通过，活跃 `KNOWN_FAILURE` 数量为 0。

### kr_1_3

- **来源**: K&R 第 1 章，练习 1-3（华氏-摄氏温度转换表）
- **失败原因**: ~~输出不匹配（少最后一行 `300  148.9`）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: `while (fahr <= upper)` 中 `fahr` 为 `float`，VM `LeF` 指令的 epsilon 比较实现存在 bug：`a < b + EPS_F32` 在 f32 大数值（如 300）下因精度舍入导致 EPS 失效，使得 `300.0 <= 300.0` 错误返回 false
- **修复内容**: `execute_float` / `execute_double` 中的 `LtF`/`LeF`/`GtF`/`GeF` 改为与 `EqF`/`NeF` 一致的 epsilon 逻辑：关系比较同时检查原始比较结果与差值是否小于 epsilon（`LeF`: `a <= b || |a-b| < EPS`，`LtF`: `a < b && |a-b| >= EPS`）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `float` 比较语义、`while` 循环
- **学生影响评级**: P1

### kr_1_4

- **来源**: K&R 第 1 章，练习 1-4（摄氏-华氏温度转换表）
- **失败原因**: ~~输出不匹配（少最后一行 `300  572.0`）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_3，VM `LeF` 指令 epsilon 比较 bug 导致 `celsius <= upper` 在 300 时错误返回 false
- **修复内容**: 同 kr_1_3
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `float` 比较语义、`while` 循环
- **学生影响评级**: P1

### kr_1_5

- **来源**: K&R 第 1 章，练习 1-5（温度转换表倒序）
- **失败原因**: ~~输出不匹配~~ ✅ **已修复（2026-06-06）**
- **最小复现**: `printf("%3d %6.1f\n", ...)` 中 `%3d`/`%6.1f` 宽度未填充前导空格
- **修复内容**: `parse_format_spec` 增加 width/flags 解析；`format_printf_string` 增加 `apply_width` 填充逻辑；支持 `%d`/`%f`/`%s`/`%c`/`%x`/`%o`/`%p` 等说明符的宽度与左对齐/零填充。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `printf` 字段宽度格式化
- **学生影响评级**: P1

### kr_1_8

- **来源**: K&R 第 1 章，练习 1-8（统计空白/制表/换行）
- **失败原因**: ~~运行时错误（Exit code 2，等待输入）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: `while ((c = getchar()) != EOF)` 循环，输入耗尽后 `getchar` 进入 waiting_input 状态而非返回 EOF
- **修复内容**: 
  1. 引入 `InputMode::Batch`：当 `runtime.input_mode` 为 `Batch` 时，`host_getchar` 在输入耗尽后 push `-1`（EOF）而非设置 `waiting_input`。
  2. E2E 测试框架对包含 `getchar()` 的 K&R 用例自动启用 `Batch` 模式。
  3. 修复 E2E 输入处理：保留换行符（`.lines()` 会丢弃 `\n`，改为 `split_inclusive('\n')` 并在分割前将 `\r\n` 规范化为 `\n`），使 `getchar()` 能正确读到换行符。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_9

- **来源**: K&R 第 1 章，练习 1-9（替换连续空格为单个）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_10

- **来源**: K&R 第 1 章，练习 1-10（转义字符可视化）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_11

- **来源**: K&R 第 1 章，练习 1-11（单词计数程序测试）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_12

- **来源**: K&R 第 1 章，练习 1-12（每行一个单词输出）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_13

- **来源**: K&R 第 1 章，练习 1-13（单词长度直方图）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_14

- **来源**: K&R 第 1 章，练习 1-14（字符频率直方图）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_15

- **来源**: K&R 第 1 章，练习 1-15（温度转换函数版）
- **失败原因**: ~~输出不匹配 + 行数缺失（15 行 vs 16 行）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: `for (float fahr = 0; fahr <= 300; fahr += 20)` 中 VM `LeF` 指令 epsilon 比较 bug 导致最后一次迭代未执行
- **修复内容**: 同 kr_1_3
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `float` 比较语义、`for` 循环
- **学生影响评级**: P1

### kr_1_16

- **来源**: K&R 第 1 章，练习 1-16（最长行）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_17

- **来源**: K&R 第 1 章，练习 1-17（打印长度大于 80 的行）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_18

- **来源**: K&R 第 1 章，练习 1-18（删除行尾空格/制表/空行）
- **失败原因**: ~~运行时错误（Exit code 2）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 同 kr_1_8，`getchar` 输入耗尽后不返回 EOF
- **修复内容**: 同 kr_1_8
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `getchar()`、EOF
- **学生影响评级**: P1

### kr_1_19

- **来源**: K&R 第 1 章，练习 1-19（字符串反转）
- **失败原因**: ~~运行时错误（Exit code 2，等待输入）~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 
  1. `for (i = 0; s[i] != '\0'; ++i);` 中 for 循环体为空语句 `;`，此前 Parser 不支持空语句。
  2. `for (j = 0; j < i; j++, i--)` 中 for 增量使用逗号表达式 `j++, i--`，此前不支持。
  3. `getchar()` 在输入耗尽后进入 waiting_input 状态，不返回 EOF，导致依赖 EOF 终止的循环无法结束。
- **修复内容**: 空语句与 for 逗号表达式已于 2026-06-06 修复；getchar EOF 行为已于 2026-06-06 通过 `InputMode::Batch` 修复。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 空语句、for 逗号表达式、getchar()、EOF
- **学生影响评级**: P1

### kr_2_8

- **来源**: K&R 第 2 章，练习 2-8（循环右移）
- **失败原因**: ~~编译错误~~ ✅ **已修复（2026-06-06）**
- **最小复现**: `for (i = 1; (v = v >> 1) > 0; i++);` 中 for 循环体为空语句 `;`，此前 Parser 不支持空语句。
- **修复内容**: 添加空语句支持。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 空语句
- **学生影响评级**: P1

### kr_2_9

- **来源**: K&R 第 2 章，练习 2-9（统计 1 的位数）
- **失败原因**: ~~编译错误~~ ✅ **已修复（2026-06-06）**
- **最小复现**: 
  1. `x &= x - 1` 中 `&=` 复合赋值运算符此前 Lexer/Parser 不支持。
  2. `unsigned x; x - 1` 中 unsigned 减法在 VM 层被当作 signed 溢出检查，导致运行时 trap。
- **修复内容**: 添加 `&=` `|=` `^=` `<<=` `>>=` 复合赋值运算符支持；添加 `USub` 操作码以支持 unsigned 减法语义。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 复合赋值运算符 `&=`、unsigned 算术语义
- **学生影响评级**: P1


### kr_3_4

- **来源**: K&R 第 3 章，练习 3-4（itoa 处理最大负数）
- **失败原因**: ~~运行时错误 — 整数取反溢出~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `unsigned un = (unsigned)n; un = -un;` 当 `n = INT_MIN` 时，Vitro VM 在 unsigned 取负操作时仍触发 signed int 溢出检查；此外 `#define INT_MIN -2147483648` 中 `2147483648` 被 Lexer 解析为 `LongLiteral`，`-long_long` 被 TypeChecker 错误推断为 `int`，且 `long long → int` 隐式 cast 被 `check_scalar_assignable` 拒绝；`unsigned /=` 复合赋值错误走 signed `Div` 路径导致计算错误
- **修复内容**:
  1. 新增 `OpCode::UNeg`（VM opcode + executor + JIT template），BytecodeGen 对 `unsigned` 类型发射 `UNeg`，TypeChecker 保留 `unsigned_int()` 结果类型
  2. TypeChecker 中 `-long_long` 返回 `long_long()` 而非 `int()`；BytecodeGen 对 `long_long` 发射 `NegQ`
  3. `check_scalar_assignable` 添加 `TypeKind::LongLong`，允许 `long long → int` 隐式 cast
  4. `gen_assign` 中 `DivAssign`/`ModAssign` 添加 `left_is_unsigned` 检查，对 unsigned 发射 `UDiv`/`UMod`（此前错误使用 signed `Div`/`Mod`）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `unsigned` 取负、`long long` 取负、隐式类型转换、`unsigned` 复合赋值 `/=`
- **学生影响评级**: P0

### kr_4_3

- **来源**: K&R 第 4 章，练习 4-3（栈计算器基础版）
- **失败原因**: ~~运行时错误 — 数组越界 + 输出不匹配~~ ✅ **已修复（2026-06-07）**
- **最小复现**: 
  1. `myatof` 函数内声明局部变量 `double val`，与全局数组 `double val[MAXVAL]` 同名。BytecodeGen 的 `sym_index`/`local_types` 在 `exit_scope` 时未被恢复，导致全局数组符号信息被局部变量覆盖
  2. `printf("%.8g\n", pop())` 中 `%g` 被 Vitro `printf` 当作普通文本输出，不输出数值
- **修复内容**: 
  - BytecodeGen `exit_scope()` 现在同时恢复 `local_types` 和 `sym_index`（此前仅恢复 `local_indices`）
  - `record_scope_var()` 记录 `local_types` 和 `sym_index` 的旧值
  - 新增 `format_g()` 函数支持 `%g`/`%G` 格式说明符（自动选择定点/科学计数法，去掉 trailing zeros）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 局部变量遮蔽、全局数组、`printf` `%g` 格式
- **学生影响评级**: P0（作用域隔离）/ P1（`%g` 不支持）

### kr_4_4

- **来源**: K&R 第 4 章，练习 4-4（栈计算器增加运算符）
- **失败原因**: ~~运行时错误 — 数组越界~~ ✅ **已修复（2026-06-07）**，同 kr_4_3
- **最小复现**: 同 kr_4_3
- **修复内容**: 同 kr_4_3
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 局部变量遮蔽、全局数组、`printf` `%g`
- **学生影响评级**: P0 → P1

### kr_4_5

- **来源**: K&R 第 4 章，练习 4-5（栈计算器增加数学函数）
- **失败原因**: ~~编译错误 — `sin`/`exp`/`pow` 未声明~~ ✅ **已修复（2026-06-07）**
- **最小复现**: ~~`#include <math.h>` 不被 Vitro 支持，`sin`/`exp`/`pow` 不是 Vitro 内置函数~~
- **修复内容**: 引入 `libm` crate，注册 `sin`/`cos`/`sqrt`/`pow`/`atan`/`log`/`exp` 为 Layer B Rust Host Func，TypeChecker 支持 double 参数与返回类型，Host Contract 测试验证精度与边界行为（NaN / -inf）。
- **是否 Vitro 限制**: 否（已解除）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `math.h`、标准数学库函数
- **学生影响评级**: P1 → 已解决

### kr_4_6

- **来源**: K&R 第 4 章，练习 4-6（栈计算器处理变量）
- **失败原因**: ~~运行时错误 — 数组越界~~ ✅ **已修复（2026-06-07）**，同 kr_4_3
- **最小复现**: 同 kr_4_3
- **修复内容**: 同 kr_4_3
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 局部变量遮蔽、全局数组、`printf` `%g`
- **学生影响评级**: P0 → P1

### kr_4_9

- **来源**: K&R 第 4 章，练习 4-9（递归快速排序）
- **失败原因**: ~~编译错误 — `qsort` 内置函数冲突~~ ✅ **已修复（2026-06-07）**
- **最小复现**: 用户自定义 `void qsort(int v[], int left, int right)` 与 Vitro 内置 `qsort(base, nmemb, size, compar)` 同名，Vitro 强制检查参数个数为 4 个
- **修复内容**: `visit_call` 中 `qsort` 分支先检查 `self.funcs.contains_key(name)`，若用户定义了同名函数则走 `check_user_func` 路径，允许用户函数遮蔽内置 `qsort`
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否（K&R 代码自己实现 qsort 是合法 C）
- **是否环境差异**: 否
- **涉及语法特性**: 函数名冲突、内置函数
- **学生影响评级**: P1



## 编译器/VM Bug 修复记录

### `char*` 解引用错误读取 4 字节（P0）

- **发现时间**: 2026-06-06
- **影响用例**: kr_5_4, kr_5_5, kr_5_6, kr_5_11（部分）, kr_3_4, kr_4_3, kr_4_4, kr_4_6, kr_4_9 等所有依赖 `*ptr`（`char*`）的代码
- **根因**: `BytecodeGen::gen_expr` 中 `UnaryOp::Deref` 分支未检查 `TypeKind::Char`，默认发射 `LoadMem`（32 位读取），导致 `*ptr` 读取 4 字节而非 1 字节。`Expr::Index`（`t[0]`）分支正确发射 `LoadMemByte`。
- **修复文件**: `native/src/compiler/codegen/mod.rs`
  - `UnaryOp::Deref` 读取路径：添加 `base_ty == TypeKind::Char → LoadMemByte`
  - `UnaryOp::Deref` 赋值路径（复合赋值 + 简单赋值）：添加 `left_is_char` 检查，使用 `LoadMemByte`/`StoreMemByte`
- **学生影响评级**: P0（所有 `char*` 字符串遍历代码都会行为异常）

### `unsigned` 取负触发 signed overflow 检查（P0）

- **发现时间**: 2026-06-07
- **影响用例**: kr_3_4（itoa 处理 INT_MIN）
- **根因**: VM 仅有 `Neg` 操作码（signed i32），对 `unsigned` 类型的取负操作仍走 `Neg` 路径，触发 `a == i32::MIN` 溢出检查；TypeChecker 将 `-unsigned` 推断为 `int`；此外 `-long_long` 被错误推断为 `int`，且 `long long → int` 隐式 cast 被 `check_scalar_assignable` 拒绝
- **修复文件**:
  - `native/src/vm/opcode.rs`: 新增 `UNeg = 121`
  - `native/src/vm/vm/executor.rs`: 添加 `UNeg` 执行逻辑（`u32.wrapping_neg()`，无溢出检查）
  - `native/src/vm/jit_templates.rs`: 添加 `tpl_uneg`
  - `native/src/compiler/codegen/mod.rs`: `UnaryOp::Neg` 对 `is_unsigned()` 发射 `UNeg`，对 `LongLong` 发射 `NegQ`
  - `native/src/compiler/typeck/expr.rs`: `-unsigned` 返回 `unsigned_int()`，`-long_long` 返回 `long_long()`
  - `native/src/compiler/typeck/mod.rs`: `check_scalar_assignable` 添加 `LongLong`，允许隐式 cast
- **学生影响评级**: P0

### BytecodeGen 作用域退出未恢复 `sym_index`/`local_types`（P0）

- **发现时间**: 2026-06-07
- **影响用例**: kr_4_3, kr_4_4, kr_4_6（栈计算器局部变量遮蔽全局数组）
- **根因**: `BytecodeGen::exit_scope()` 仅恢复 `local_indices`，`sym_index`（符号索引）和 `local_types`（局部变量类型）在作用域退出后仍保留内层值。当局部变量 `val` 遮蔽全局数组 `val[]` 时，`sym_index` 仍指向局部标量符号，导致 `TrapBounds` 获取 `array_size() == 0`，触发 false bounds trap
- **修复文件**: `native/src/compiler/codegen/mod.rs`
  - `local_scope_stack` 元素类型扩展为 `(String, Option<i32>, Option<Type>, Option<i32>)`
  - `record_scope_var()` 同时记录 `old_type` 和 `old_sym_idx`
  - `exit_scope()` 恢复 `local_types` 和 `sym_index`
- **学生影响评级**: P0（局部变量遮蔽全局数组会导致数组越界误判）

### `char*[]` 数组元素大小与初始化路径错误（P0）

- **发现时间**: 2026-06-07
- **影响用例**: kr_5_11（打印月份名）
- **根因**: 
  1. `base_kind()` 递归解引用 Pointer，`char*[]` 被误判为 `Char`，导致 `elem_type_size` 返回 1（应为 4）
  2. 数组初始化路径用 `base_kind(vty) == TypeKind::Char` 判断，使 `char*[]` 走字节初始化路径（只写 `array_size` 个字节）
  3. `flatten_init_list()` 对 `StringLiteral` 返回 0
  4. 静态局部变量初始化路径同样未处理 `StringLiteral`
- **修复文件**: `native/src/compiler/codegen/mod.rs`
  - `elem_type_size()`: 对数组使用 immediate element type，不递归解引用
  - 数组初始化: `is_char_array` 检查 immediate element type 是否为 `Char` 且非 `Pointer`
  - Flat scalar init: `StringLiteral` 元素调用 `gen_expr` 生成字符串地址
  - 静态局部变量 init: `StringLiteral` 元素分配字符串地址并写入 `globals_init_32`
- **残留问题**: `char*[]` 运行时元素访问仍存在异常（`parr[0]`/`parr[1]` 读取值错误，`parr[2]` 正确，`int[]` 完全正常），根因待进一步分析
- **学生影响评级**: P0

### `unsigned` 复合赋值 `/=` `%=` 错误使用 signed 操作码（P0）

- **发现时间**: 2026-06-07
- **影响用例**: kr_3_4（`un /= 10` 在 `unsigned` 下计算错误）
- **根因**: `gen_assign` 的 `emit_compound` 对 `DivAssign`/`ModAssign` 未检查 `left_is_unsigned`，总是发射 signed `Div`/`Mod`。当 `unsigned` 值大于 `INT_MAX`（如 `2147483648U`）时，signed `Div` 将其解释为负数，产生错误结果
- **修复文件**: `native/src/compiler/codegen/mod.rs`
  - `gen_assign` 中添加 `left_is_unsigned` 标志
  - `DivAssign`: `left_is_unsigned` 时发射 `UDiv`
  - `ModAssign`: `left_is_unsigned` 时发射 `UMod`
  - `ShrAssign`: `left_is_unsigned` 时发射 `LShr`（逻辑右移）
- **学生影响评级**: P0

### `base_kind` 递归解引用导致 `char*[]` / `char**` 多处行为异常（P0）

- **发现时间**: 2026-06-07
- **影响用例**: kr_5_11（指针数组初始化/访问）、所有 `char**` / `char*[]` 代码
- **根因**: `base_kind()` 递归解引用直到非 Pointer/Array 类型。对于 `char*[]`，`base_kind` 返回 `Char`，导致：
  1. `elem_type_size()` 返回 1（应为 4），数组索引 stride 错误
  2. 全局/局部数组初始化路径误判为字符数组，走字节初始化路径
  3. `ptr_step_size()` 对 `char**` 返回 1（应为 4）
  4. `UnaryOp::Deref` 对 `char**` 发射 `LoadMemByte`（应为 `LoadMem`）
- **修复文件**: `native/src/compiler/codegen/mod.rs`
  - 新增 `immediate_base_kind()`：只解引用一层（`Pointer`→`pointee.kind()`，`Array`→`element.kind()`）
  - `elem_type_size()`: 对 `Array` 使用 `immediate_base_kind`
  - 全局/局部 `InitList` 初始化: `is_char_array` 检查 immediate element type
  - `ptr_step_size()`: 直接取 `pointee` 的 `type_size`，不再递归 `base_kind`
  - `UnaryOp::Deref`: 使用 `immediate_base_kind` 判断加载宽度
- **学生影响评级**: P0（所有 `char*[]` / `char**` 代码行为异常）

---

## 阶段 3 失败详情（K&R 第 5-6 章）

### kr_5_1

- **来源**: K&R 第 5 章，练习 5-1（getint）
- **失败原因**: ~~编译错误 — `isdigit` 未声明、`ungetc` 未声明、`stdin` 未声明~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `<ctype.h>` 已支持（`isdigit` 走 Bytecode Libc 路径），`stdin` 已预定义宏；`ungetc` 仍非 Vitro 内置函数
- **修复内容**: 新增 `UNGETC = 78` Host Func（`host_ungetc`），在 `RuntimeState` 中增加 `ungetc_char: Option<i32>` 单字符缓存，`host_getchar` 优先返回缓存字符
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `ungetc`
- **学生影响评级**: P1

### kr_5_2

- **来源**: K&R 第 5 章，练习 5-2（getfloat）
- **失败原因**: ~~编译错误 — 同 kr_5_1，`ungetc` 未声明~~ ✅ **已修复（2026-06-07）**
- **是否 Vitro 限制**: 否（已修复）
- **建议**: 同 kr_5_1

### kr_5_8

- **来源**: K&R 第 5 章，练习 5-8（qsort 指针数组版）
- **失败原因**: ~~编译错误 — 函数指针类型转换语法不支持~~ ✅ **已修复（2026-06-14）**
- **最小复现**: 
  1. 原编译失败：`(int (*)(void *, void *))numcmp` 中函数指针类型语法 `int (*)(void *, void *)` 不被 Parser 支持。
  2. 修复后运行输出差异：`atof` 在 C89 隐式声明下返回 `int`，而 Vitro 要求按 `<stdlib.h>` 显式声明返回 `double`。`cases_golden/knr/kr_5_8.out` 已按 Clang + `<stdlib.h>` 重新生成，Vitro 输出与 Clang 一致为 `-1 0 2.71 3.14 42`。
- **修复内容**: 
  1. `native/src/compiler/parser/expr.rs` 的 `parse_type_only` 改为调用 `parse_abstract_declarator`，支持函数指针类型的抽象声明符。
  2. 重新生成 `tests/cases_golden/knr/kr_5_8.out`（Clang + stdlib.h）。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否（Vitro 不支持隐式函数声明，需按现代 C 语义包含标准头文件）
- **涉及语法特性**: 函数指针类型转换（cast）、抽象声明符
- **学生影响评级**: P1

### kr_5_9

- **来源**: K&R 第 5 章，练习 5-9（qsort 递减序）
- **失败原因**: ~~编译错误 — 函数指针调用类型检查错误~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `(*comp)(v[i], v[left])` 被 TypeChecker 报告为"不能对非函数指针类型进行调用"；VM 运行时 `*comp` 解引用错误执行 `LoadMem`，导致读取 NULL 指针区域
- **修复内容**:
  1. `native/src/compiler/typeck/expr.rs`: `CallPtr` 分支增加对 `Type::Function` 的直接匹配，支持 `(*fp)(args)` 形式
  2. `native/src/compiler/codegen/mod.rs`: `UnaryOp::Deref` 对 `TypeKind::Function` 跳过 `LoadMem`（函数解引用即自身，会再次退化为指针）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 函数指针解引用调用 `(*fp)(args)`
- **学生影响评级**: P1

### kr_5_10

- **来源**: K&R 第 5 章，练习 5-10（echo 命令行参数）
- **失败原因**: ~~编译错误 — 三目运算符分支类型不匹配~~ ✅ **已修复（2026-06-14）**
- **最小复现**: 
  1. `main(int argc, char *argv[])` 不被支持：BytecodeGen 调用 `main` 时不推送参数，导致运行时栈下溢。
  2. 三目运算符字符串字面量类型不匹配此前已由 `typeck/expr.rs` 修复。
- **修复内容**: 
  1. `native/src/vm/opcode.rs`: 新增 `PushArgc = 127`、`PushArgv = 128`。
  2. `native/src/vm/vm/mod.rs`: `VitroVM` 新增 `argc`/`argv_addr` 字段、`set_argc`/`set_argv_addr`/`setup_argv` 方法，在全局数据区后分配 argv 字符串数组。
  3. `native/src/vm/vm/executor.rs`: 执行 `PushArgc`/`PushArgv` 时从 VM 字段入栈。
  4. `native/src/compiler/codegen/mod.rs`: 包装代码在 `Call main` 前根据 `main` 参数个数推送 `argc`/`argv`。
  5. `native/src/engine/compile_pipeline.rs`: `setup_vm` 调用 `vm.setup_argv`。
  6. `native/src/flutter_bridge.rs`: 新增 `set_argv`，供 CLI/前端注入命令行参数。
  7. `native/src/bin/vitro_cli.rs`: `run` 命令支持 `-- arg1 arg2 ...` 传递参数。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `argc`/`argv`、`main` 函数签名
- **学生影响评级**: P1

### kr_5_11

- **来源**: K&R 第 5 章，练习 5-11（打印月份名）
- **失败原因**: ~~输出不匹配 — `char*[]` 指针数组初始化/赋值后元素为空~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `char *name[] = {"January", "February", ...}` 初始化后所有元素为空；逐个赋值时每次赋值会清空其他元素
- **修复内容（2026-06-07）**:
  1. `elem_type_size()` 改为使用 immediate element type（不递归解引用），使 `char*[]` 的元素大小正确返回 4（此前 `base_kind(char*[])` 递归到 `Char`，错误返回 1）
  2. 数组初始化路径中 `is_char_array` 判断改为检查 immediate element type，避免 `char*[]` 被误判为字符数组走字节初始化路径
  3. 非静态局部变量 `InitList` 的 flat scalar init 路径中，`StringLiteral` 元素现在调用 `gen_expr` 生成字符串地址
  4. 静态局部变量 `InitList` 路径中，`StringLiteral` 元素现在分配字符串地址并写入 `globals_init_32`
  5. `ptr_step_size()` 改为直接取 `pointee` 的 `type_size`，不再递归 `base_kind`（此前 `char**` 步长被错误计算为 1）
  6. `UnaryOp::Deref` 改为使用 `immediate_base_kind`，不再递归解引用（此前 `char**` 解引用被错误发射 `LoadMemByte`）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `char*` 指针数组初始化、指针数组元素赋值
- **学生影响评级**: P0

### kr_5_13

- **来源**: K&R 第 5 章，练习 5-13（tail 打印最后 n 行）
- **失败原因**: ~~输出不匹配 — 只输出了最后一行~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `getchar()` 在 Batch 模式下读取多行输入时，只返回了最后一行内容
- **修复内容**: 同 kr_1_8（`InputMode::Batch` + 换行符保留修复）
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否（此前误判为环境差异，实为 Batch 模式输入缓冲逻辑已修复）
- **涉及语法特性**: `getchar()`、EOF、换行符处理
- **学生影响评级**: P1

### kr_5_14

- **来源**: K&R 第 5 章，练习 5-14（排序增加字段选项）
- **失败原因**: ~~编译错误 — 函数指针语法不支持~~ ✅ **已修复（2026-06-14）**
- **最小复现**: `void qsortt(void *lineptr[], int left, int right, int (*comp)(void *, void *));` 前向声明中的函数指针参数语法 Parser 报错；实际根因是 cast 表达式 `(int (*)(void *, void *))strcmp_local` 的抽象声明符无法解析
- **修复内容**: `native/src/compiler/parser/expr.rs` 的 `parse_type_only` 改为调用 `parse_abstract_declarator`，完整支持函数指针类型的抽象声明符（如 `int (*)(void *, void *)`）。此前实现仅能解析基础类型 + 星号，无法处理函数指针。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 函数指针类型转换（cast）、抽象声明符
- **学生影响评级**: P1

### kr_5_15

- **来源**: K&R 第 5 章，练习 5-15（查找增加选项）
- **失败原因**: ~~输出不匹配~~ ✅ **已通过（2026-06-06）**
- **修复内容**: `char*` 解引用修复后，`getlinee` 和 `strindexx` 中的 `*p++` / `*r` 操作恢复正常。

### kr_5_16

- **来源**: K&R 第 5 章，练习 5-16（dcl/undcl 复杂声明解析）
- **失败原因**: ~~编译错误 — Parser 不支持匿名 `enum` 声明~~ ✅ **已修复（2026-06-13）**
- **最小复现**: `enum { NAME, PARENS, BRACKETS };` 匿名枚举声明在 `parse_program` 中被错误地强制要求枚举名，导致预期标识符名称错误
- **修复内容**: `native/src/compiler/parser/mod.rs` 的顶层 Enum 分支改为：先尝试可选消费 `Identifier`，仅当后续 token 为 `{` 时才按 enum 声明解析，从而支持匿名枚举
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 匿名 `enum` 声明
- **学生影响评级**: P1

### kr_6_1

- **来源**: K&R 第 6 章，练习 6-1（getword 提取关键字）
- **失败原因**: ~~编译错误 — `isspace`/`isalpha`/`isalnum`/`ungetc`/`stdin` 未声明~~ ✅ **标准库拓展后只剩 `ungetc`（2026-06-07）** → ~~运行时错误：`struct key keytab[] = { {"auto", 0}, ... }` 全局结构体数组中 `char*` 成员的 `StringLiteral` 初始化异常~~ ✅ **已修复（2026-06-14）**
- **根因**: 全局变量区与字符串字面量区共用同一起始地址 `GLOBAL_START + BYTECODE_LIBC_GLOBALS_RESERVED`。`keytab` 全局数组在 Pass 1 中分配于该区起始处，而字符串字面量地址也在该处分配，导致 `keytab[i].word` 的值被后续写入的字符串内容覆盖。
- **修复内容**: 
  - `native/src/compiler/codegen/mod.rs`：全局变量初始化中的 `StringLiteral` 改为延迟到 Pass 1 结束后统一分配地址，确保字符串区位于所有全局变量之后；新增 `pending_string_inits` 列表记录待回填的 `(base_offset, value)`，Pass 1 结束后遍历分配。
  - `native/src/compiler/codegen/stmt.rs`：静态局部变量及其 `StringLiteral` 初始化改用 `next_global_offset` 分配，避免与全局变量/字符串区重叠。
  - `native/src/compiler/codegen/expr.rs`：代码生成阶段的 `StringLiteral` 改用 `next_global_offset` 分配。
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `ungetc`、全局结构体数组初始化、`char*` 成员
- **学生影响评级**: P1
- **建议**: 已回归测试验证；K&R `kr_6_1` 在 `vitro_e2e` 与单独 Shadow 验证中均为 `match`。

### kr_6_2

- **来源**: K&R 第 6 章，练习 6-2（统计 C 关键字，二叉树）
- **失败原因**: ~~编译错误 — Parser 不支持函数指针/结构体组合语法~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `struct tnode *addtree(struct tnode *, char *);` 前向声明中，指针类型无名参数（`struct tnode *`）导致 `parse_param_list` 错误进入 `parse_declarator`，触发"预期标识符名称"
- **修复内容**: `native/src/compiler/parser/mod.rs` `parse_param_list` 增加对 `*` 的前瞻：跳过所有 `*` 后若是 `Comma`/`RParen`，则构造 `Pointer→...→Base` 类型作为无名参数；`parse_func_decl` 返回类型从单 `*` 改为 `while` 支持多级指针；后续 `ungetc` Host Func 拓展解除剩余限制
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: 指针类型无名参数前向声明、`ungetc`
- **学生影响评级**: P1

### kr_6_3

- **来源**: K&R 第 6 章，练习 6-3（交叉引用）
- **失败原因**: ~~编译错误 — 同 kr_6_2~~ ✅ **已修复（2026-06-07）**
- **是否 Vitro 限制**: 否（已修复）
- **建议**: 同 kr_6_2

### kr_6_4

- **来源**: K&R 第 6 章，练习 6-4（统计单词频率）
- **失败原因**: ~~编译错误 — 同 kr_6_2~~ ✅ **已修复（2026-06-07）**
- **是否 Vitro 限制**: 否（已修复）
- **建议**: 同 kr_6_2

### kr_6_5

- **来源**: K&R 第 6 章，练习 6-5（哈希表查找）
- **失败原因**: ~~编译错误 — `strdup` 未声明~~ ✅ **已修复（2026-06-07）**
- **最小复现**: `strdup` 不是 Vitro 内置函数
- **修复内容**:
  1. `native/src/vm/host_func_id.rs`: 新增 `STRDUP = 77`
  2. `native/src/vm/host_funcs.rs`: 新增 `host_strdup`，复用 `read_cbytes` + `allocate_raw` + `MemoryRegion` 追踪
  3. `native/src/compiler/typeck/mod.rs`: 新增 `check_builtin_strdup`
  4. `native/runtime_libc/include/string.h`: 添加 `char* strdup(const char* s);` 存根声明
- **是否 Vitro 限制**: 否（已修复）
- **是否代码本身问题**: 否
- **是否环境差异**: 否
- **涉及语法特性**: `strdup`
- **学生影响评级**: P1

### kr_6_6

- **来源**: K&R 第 6 章，练习 6-6（表查找 define/undef）
- **失败原因**: ~~编译错误 — `strdup` 未声明~~ ✅ **已修复（2026-06-07）**
- **是否 Vitro 限制**: 否（已修复）
- **建议**: 同 kr_6_5
