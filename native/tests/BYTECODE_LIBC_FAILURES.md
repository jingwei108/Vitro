# Bytecode Libc 自举一致性测试失败记录

> **原则**：NO_CODE_DISTORTION — Bytecode Libc 的 C 源码不得为了通过 Vitro 编译器而改写。如果 Vitro 编译失败，那是编译器的缺口，记录为 `compile_gap`。

---

## 编译器缺口（compile_gap）

### ~~`void*` 参数隐式转换不完整~~ ✅ 已修复

- **来源**: Bytecode Consistency — `test_bc_memcpy` / `test_bc_memmove`
- **失败原因**: 编译错误 / 类型不匹配 (E3038)
- **最小复现**:
  ```c
  void *memcpy(void *dest, void *src, int n);
  int main() {
      char dest[16];
      char src[] = "ABCDEF";
      memcpy(dest, src, 6);  // E3038: 函数 'memcpy' 第 1/2 个参数类型不匹配
  }
  ```
- **根因分析**: Vitro TypeChecker 虽然对 `void*` 到具体指针的赋值有隐式转换提示（`H3057_ImplicitConversionHint`），但在**函数调用参数匹配**时，`char[]` / `char*` 到 `void*` 的转换未正确生效，导致类型检查失败。
- **修复**: 在 `typeck/mod.rs::check_pointer_assignable` 中补充规则：当目标类型为 `void*` 且值类型为任意指针或数组时，允许隐式转换并给出 `H3057` 提示。
- **是否 Vitro 限制**: ~~是~~ → 否（已修复）
- **是否标准库实现偏差**: 否
- **学生影响评级**: P1 → P0（修复后 `memcpy`/`memmove` 可作为 Bytecode Libc 正常编译运行）

---

### ~~`rand()` 无符号整数溢出回绕~~ ✅ 已修复

- **来源**: Bytecode Consistency — `test_bc_rand`
- **失败原因**: 运行时错误 / 整数乘法溢出 Trap
- **最小复现**:
  ```c
  unsigned int seed = 1;
  int rand(void) {
      seed = seed * 1103515245 + 12345;  // VM 触发"整数乘法溢出"
      return (int)((seed >> 16) & 0x7fff);
  }
  ```
- **根因分析**: Vitro VM 的 `OpCode::Mul` 对所有 32 位乘法执行有符号溢出检查，若乘积超出 `i32` 范围则 Trap。C 标准中 `unsigned int` 的溢出应定义为模 2^32 回绕，但 Vitro 的 codegen 对 `unsigned` 类型的加法和乘法未生成对应的 `UAdd`/`UMul` 指令，而是错误地使用了带溢出检查的 `Add`/`Mul`。
- **修复**:
  1. 在 `opcode.rs` 中新增 `UAdd = 122`、`UMul = 123`；
  2. 在 `executor.rs` 中实现 `UAdd`/`UMul`（`wrapping_add`/`wrapping_mul`）；
  3. 在 `codegen/mod.rs` 的 `BinaryOp::Add`/`Mul` 以及 compound assignment（`+=`、`-=`、`*=`）中添加 `is_unsigned` 分支，生成对应的 unsigned 指令；
  4. 在 `jit_templates.rs` 中映射 `UAdd`/`UMul` 到对应的 JIT 模板。
- **是否 Vitro 限制**: ~~是~~ → 否（已修复）
- **是否标准库实现偏差**: 否
- **学生影响评级**: P2 → P0（修复后 `rand`/`srand` 可作为 Bytecode Libc 正常编译运行；同时所有 `unsigned int` 的加减乘运算均获得正确的回绕语义）

---

## 已修复的编译器 bug

### `strcpy` 被 TypeChecker 错误推断为 `void` 返回类型

- **来源**: Bytecode Consistency — `test_bc_strcpy`
- **失败原因**: 编译错误 / `printf` 参数类型不匹配 (E3062)
- **根因**: `compiler/typeck/mod.rs` 中的 `check_builtin_strcpy` 硬编码返回 `Type::void()`，而非 `Type::pointer_to(Type::char())`。
- **修复**: 将返回类型修正为 `Type::pointer_to(Type::char())`，与 `check_builtin_strcat` 保持一致。
- **修复提交**: Phase D 实施中同步修复

---

## 当前 Bytecode Libc 覆盖状态

| 文件 | 函数 | Bytecode Consistency | 状态 |
|---|---|---|---|
| `ctype.c` | `isdigit`, `isalpha`, `islower`, `isupper`, `tolower`, `toupper` | ✅ | 已验证 |
| `ctype.c` | `isspace`, `isalnum`, `isprint`, `iscntrl`, `isxdigit` | ✅ | 已验证 |
| `stdlib.c` | `abs` | ✅ | 已验证 |
| `stdlib.c` | `atoi` | ✅ | 已验证 |
| `string.c` | `strlen` | ✅ | 已验证 |
| `string.c` | `strcmp` | ✅ | 已验证 |
| `string.c` | `strcpy` | ✅ | 已验证（修复 TypeChecker bug 后） |
| `string.c` | `strcat` | ✅ | 已验证 |
| `string.c` | `strncpy` | ✅ | 已验证 |
| `string.c` | `memcpy` | ✅ | 已验证（修复 `void*` 隐式转换后） |
| `string.c` | `memmove` | ✅ | 已验证（修复 `void*` 隐式转换后） |
| `stdlib.c` | `rand`/`srand` | ✅ | 已验证（修复 unsigned wrap-around 后） |

---

*文档状态：Phase D 实施中 — `rand`/`srand` unsigned wrap-around 已修复*
*最后更新：2026-06-07*
