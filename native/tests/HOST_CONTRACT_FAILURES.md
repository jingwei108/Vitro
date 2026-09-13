# Host Function 契约测试失败记录

> **原则**：All in. Record don't hide. Fix real bugs, not test cases.
>
> 任何失败必须在此文件中按规范格式追加，禁止通过修改测试预期值来粉饰数据。

---

## 已修复（FIXED）

### `qsort` 对栈上大数组排序触发 NULL 指针 trap

- **来源**: O10 `host_qsort` 优化验证过程中记录，根因定位后修复
- **失败原因**: 运行时错误 / JIT trace 执行错误
- **最小复现**:
  ```c
  #include <stdlib.h>
  int cmp(const void* a, const void* b) {
      return *(const int*)a - *(const int*)b;
  }
  int main() {
      int arr[1000];
      for (int i = 0; i < 1000; i++) arr[i] = 1000 - i;
      qsort(arr, 1000, sizeof(int), cmp); // 触发：向 NULL 指针区域写入（地址 0x0383）
      return 0;
  }
  ```
- **当前状态**: **已修复**。
- **真实根因**: 与 `host_qsort`、栈分配、`call_user_function` 回调均无关；问题在 **JIT trace 模板 `jit_templates.rs` 中 `StoreMem` / `StoreMemByte` 的弹栈顺序与解释器 `executor.rs` 相反**。
  - 解释器 `StoreMem` 约定：**先 pop 值，再 pop 地址**（栈顶是值）。
  - JIT 模板错误实现为：**先 pop 地址，再 pop 值**。
  - 当填充数组的 `for` 循环执行次数超过 `JIT_THRESHOLD=100` 并被 trace 加速后，`arr[i] = 1000 - i` 实际把“值”当成地址写入，数值较小时（如 0x0383）落入 NULL trap 区，触发 trap。
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: 否（实现 bug）
- **学生影响评级**: P0 — 教学中 1000 元素排序是常见场景，JIT 加速下所有数组写操作都可能把值写到错误地址，后果严重
- **修复**:
  1. `native/src/vm/jit_templates.rs`：将 `tpl_store_mem` 和 `tpl_store_mem_byte` 的 `pop` 顺序改为与解释器一致，即先 `val` 后 `addr`。
  2. `native/src/vm/host_funcs.rs`：将 `host_qsort` 改为 `pub`，供 Host Contract 测试直接调用。
  3. 新增测试：
     - `native/tests/host_contract_tests.rs`：`test_qsort_large_byte_array_default_compare`（128 元素默认字节比较排序）、`test_qsort_single_element_noop`、`test_qsort_empty_array_noop`。
     - `native/tests/qsort_test.rs`：`test_qsort_thousand_int_array`（1000 元素 int 数组 + 用户比较函数回调，覆盖 JIT fast path）。
- **修复提交**: 2026-06-14 根因定位后修复

---

### host_atoi 前缀解析偏差

- **来源**: Host Contract
- **失败原因**: 运行时错误 / 标准一致性偏差
- **最小复现**:
  ```rust
  write_test_string(&mut vm, addr, "  -123abc");
  host_atoi(&mut vm, &mut session);
  // 预期: -123（C 标准行为）
  // 实际: 0（Rust 的 parse::<i32>() 对非纯数字字符串返回 Err）
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P1（限制已知）— `atoi` 处理带后缀的字符串属于边缘场景，但在 K&R 示例中可能出现
- **修复**: 将 `parse::<i32>()` 替换为手动前缀解析（跳过前导空白 → 处理符号 → 读取连续数字 → 遇到非数字停止）。
- **修复提交**: Phase A 实施中同步修复

---

### host_realloc(NULL, size) 栈参数传递错误

- **来源**: Host Contract
- **失败原因**: 运行时错误
- **最小复现**:
  ```rust
  vm.push(64); // new_size
  vm.push(0);  // ptr
  host_realloc(&mut vm, &mut session);
  // 预期: 返回非 NULL（等价于 malloc(64)）
  // 实际: 返回 NULL 并伴随 malloc(0) 警告
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P0（误导学生）— `realloc(ptr, 0)` 和 `realloc(NULL, size)` 是 C 标准中的常见用法，行为错误会导致学生困惑
- **根因**: `host_realloc` 在 `ptr == 0` 分支直接调用 `host_malloc(vm, session)`，但此时 `new_size` 已被 pop 出栈，`host_malloc` 因栈空而读到 size=0。
- **修复**: 在调用 `host_malloc` 前将 `new_size` push 回栈：`vm.push(new_size as u64); host_malloc(vm, session);`。
- **修复提交**: Phase A 实施中同步修复

---

### host_memset NULL 指针安全检查缺失

- **来源**: Host Contract
- **失败原因**: 安全检查失效
- **最小复现**:
  ```rust
  vm.push(1u64);
  vm.push(0x42u64);
  vm.push(0u64); // NULL ptr
  host_memset(&mut vm, &mut session);
  // 预期: 触发 NULL trap（地址 < 0x1000）
  // 实际: 静默写入 VM 内存地址 0，绕过所有安全检查
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P0（误导学生）— 绕过 NULL trap 使学生无法发现未初始化指针的错误
- **根因**: `host_memset` 直接通过 `memory_ref_mut()` 操作原始内存切片，完全绕过了 `check_mem_access`。
- **修复**: 在填充前增加 NULL 指针检查（与 VM `store_i8` 行为保持一致）：
  ```rust
  if ptr < super::vm::NULL_TRAP_SIZE && write_len > 0 {
      vm.trap("向 NULL 指针区域写入...", ...);
      return;
  }
  ```
- **修复提交**: Phase A 实施中同步修复

---

## 已修复（FIXED）

### host_strcpy 越界触发 E3070 Buffer Overflow

- **来源**: Host Contract — `test_strcpy_overflow_must_trap`
- **失败原因**: 安全检查失效
- **最小复现**:
  ```c
  char *buf = malloc(3);
  strcpy(buf, "hello"); // 需要 6 字节（含 \0），但 buf 只有 3 字节
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P0（误导学生）— strcpy 越界是 C 教学中最典型的缓冲区溢出示例，不触发 trap 意味着学生无法得到即时反馈
- **修复**: 在 `host_strcpy` 中通过 `session.memory.regions` 查找 `dest` 所属的已分配堆区域。若找到，验证 `src_len + 1 <= region.size - offset`；若越界则触发 `Trap` 并输出 `E3070 Buffer Overflow` 诊断信息，包含区域名、可用空间和修复建议。
- **限制**: 当前仅对 **堆分配区域**（`malloc`/`realloc`）生效。栈数组和全局数组的边界检查需依赖编译器符号表扩展，待后续实现。
- **修复提交**: Phase A KNOWN_FAILURE 修复

---

### host_strcat 越界触发 E3070 Buffer Overflow

- **来源**: Host Contract — `test_strcat_overflow_must_trap`
- **失败原因**: 安全检查失效
- **最小复现**:
  ```c
  char *buf = malloc(4);
  strcpy(buf, "ab");
  strcat(buf, " world"); // 已有 3 字节 + 需要 7 字节 = 10 字节，但 buf 只有 4 字节
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P0（误导学生）
- **修复**: 与 `host_strcpy` 同理，通过 `session.memory.regions` 检查 `dest_len + src_len + 1 <= region.size - offset`，越界时触发 `E3070 Buffer Overflow`。
- **限制**: 同上，当前仅覆盖堆分配区域。
- **修复提交**: Phase A KNOWN_FAILURE 修复

---

### JIT Trace 录制 Abort 时被错误编译为残缺 trace

- **来源**: 代码审查报告 O5 推进过程中发现
- **失败原因**: 运行时行为错误 / 性能劣化
- **最小复现**:
  ```c
  int main() {
      int sum = 0;
      for (int i = 0; i < 20; i++) {
          for (int j = 0; j < 20; j++) {
              sum = sum + 1;
          }
      }
      return sum;
  }
  ```
  内层循环 backward jump 目标被命中超过 `JIT_THRESHOLD` 后，`TraceRecorder` 在第 N 次迭代（`j >= 20`）录制到 `JumpIfZero` 跳转到循环外，触发 `RecordResult::Abort`；但 `executor.rs` 对 `Finish` 和 `Abort` 都调用 `trace_recorder.finish()`，导致只包含条件判断的 4 条指令被编译为 `CompiledTrace`。
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: 否（实现 bug）
- **学生影响评级**: P1 — 嵌套循环场景下 JIT 会生成无效 trace，导致循环被反复解释执行或触发 `max_steps` 误报为无限循环
- **修复**:
  1. `TraceRecorder::finish` 增加 `aborted: bool` 参数，Abort 时清空 `instructions` 并返回 `None`。
  2. `executor.rs` 区分 `Finish` 与 `Abort`，仅对 `Finish` 编译并插入 `jit_traces`。
  3. `execute_trace_bulk` 同步修正条件跳转 side-exit 逻辑，使完整 trace 能批量执行多轮循环迭代。
- **修复提交**: 2026-06-14 代码审查报告 O5 推进

---

## 待进一步分析（TODO）

### `printf("%.2f", 2.675)` 舍入偏差

- **来源**: Host Contract（尚未编写测试，待补充）
- **预期行为**: C99 标准不要求特定舍入方向，但常见实现（glibc/Clang）使用银行家舍入或向远离零舍入，输出 `"2.67"` 或 `"2.68"`。
- **当前状态**: 未测试。Vitro 的 `format_printf_string` 使用 Rust 的 `format!("{:.2}")`，其行为是银行家舍入（round half to even）。
- **建议**: 编写契约测试记录实际行为，若与主流 Clang 行为偏差则标记。

### `memcpy` / `memmove` Host Contract 缺失

- **来源**: Host Contract
- **当前状态**: `memcpy` / `memmove` 尚未实现为 Host Function（仅有 `memset`）。
- **建议**: 当添加 `memcpy` / `memmove` Host Func 时，必须同步补充边界检查 + UAF 检查契约测试。

---

*文档状态：Phase A 实施中*
*最后更新：2026-06-14*
