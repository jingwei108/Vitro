# Fuzz 压力测试失败记录（Phase E）

> **原则**：All in. Record don't hide. Fix real bugs, not test cases.
>
> 任何失败必须在此文件中按规范格式追加，禁止通过修改测试预期值来粉饰数据。

---

## 已修复（FIXED）

### VM public 内存读写方法缺少 UAF 检查

- **来源**: Fuzz A — `test_fuzz_malloc_free_uaf_double_free`
- **失败原因**: 安全检查失效
- **最小复现**:
  ```rust
  let mut vm = VitroVM::new();
  let mut session = Session::default();
  vm.push(64); host_malloc(&mut vm, &mut session);
  let addr = vm.pop() as u32;
  vm.push(addr as u64); host_free(&mut vm, &mut session);
  // free 后 freed_logs 已记录
  vm.store_i8(addr, 0x42, &SourceLoc::default());
  // 预期: 触发 E3060 UAF trap
  // 实际: 无 trap，数据被静默写入已释放内存
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是** — VM 的 public API 与 executor 指令层的安全策略不一致
- **学生影响评级**: P0（误导学生）— UAF 检测是核心安全特性，如果 public API 绕过检查，学生会看到"程序正常写入已释放内存"的错误暗示
- **根因**: `VitroVM::load_i32`/`store_i32`/`load_i64`/`store_i64`/`load_i8`/`store_i8` 以及 `write_memory`/`copy_memory` 只调用 `check_mem_access`，未调用 `check_uaf`。executor.rs 中的 `OpCode::LoadMem`/`StoreMem` 等指令在调用这些方法前单独检查 UAF，但直接调用 public API 的代码（如 Host Functions、测试、CLI）绕过了 UAF 检测。
- **修复**: 在上述所有 public 内存读写方法中，于 `check_mem_access` 通过后、实际内存操作前，插入 `check_uaf` 调用。若检测到 UAF，调用 `self.trap()` 并返回。
- **修复提交**: Phase E 实施中同步修复

---

### `host_realloc` 中 `freed_logs` 清理顺序错误

- **来源**: Fuzz A — `test_fuzz_malloc_free_uaf_double_free`
- **失败原因**: 运行时错误 / 安全检测误报
- **最小复现**:
  ```c
  char *p = malloc(100);
  free(p);
  char *q = malloc(100); // 可能重用 p 的地址，清理 freed_logs
  char *r = realloc(q, 200); // 从 free_list 分配新地址，该地址可能仍在 freed_logs 中
  ```
- **是否 Vitro 限制**: 否
- **是否标准库实现偏差**: **是**
- **学生影响评级**: P0（误导学生）— `realloc` 合法调用被误报为 UAF，会严重干扰正常教学代码
- **根因**: `host_realloc` 在分配 `new_addr` 后，先通过 `vm.store_i8` 拷贝旧数据，然后在释放 `old_addr` 后才清理 `freed_logs` 中与新分配重叠的记录。如果 `new_addr` 之前被释放过（来自 `free_list`），其 `freed_log` 在 `store_i8` 时仍然存在，导致 UAF 误报。
- **修复**: 将 `freed_logs.retain(...)` 从"释放 old 之后"移到"拷贝数据之前"，确保新分配的地址在写入前已从 `freed_logs` 中移除。
- **修复提交**: Phase E 实施中同步修复

---

## 已知限制（KNOWN_LIMITATION）

### `session.memory.regions` 与 `vm.freed_logs` 的一致性问题

- **来源**: Fuzz A — 设计层面分析
- **说明**: `session.memory.regions` 保留所有分配历史（包括已释放、已重用的 region），而 `vm.freed_logs` 仅记录当前已释放且未被重用的内存范围。当 `malloc`/`realloc` 重用或合并 free_list 中的块时，`freed_logs` 会被清理，但旧的 `regions` 记录仍然保留（`is_freed=true`）。这导致 `regions` 中可能出现"地址重叠的已释放 region"，而 `freed_logs` 中已无对应记录。
- **影响**: 
  - 不影响正常教学代码（executor 层和 Host Function 层都使用 `freed_logs` 进行 UAF 检测）
  - 不影响泄漏检测（`append_leak_report` 过滤 `!r.is_freed`，重叠的旧 region 被正确排除）
  - 仅影响直接遍历 `regions` 进行诊断的外部工具，这些工具需要自行处理地址重叠
- **建议**: 长期可考虑为 `regions` 引入 generation 计数或地址空间映射，确保每个地址只对应一个有效的 region 记录。

---

## 测试覆盖矩阵

| Fuzz 场景 | 轮次 | 每轮操作数 | 状态 |
|---|---|---|---|
| Fuzz A: malloc/free/realloc + UAF/Double-Free | 200 | 100 | ✅ 通过 |
| Fuzz B: strcpy/strcat/strncpy/memcpy/memmove | 200 | 80 | ✅ 通过 |
| Fuzz C: printf/scanf/getchar/putchar | 200 | 60 | ✅ 通过 |
| Fuzz D: 混合恶意序列 | 200 | 100 | ✅ 通过 |
| Fuzz E: 泄漏检测验证 | 200 | 3-12 次分配 | ✅ 通过 |

---

*文档状态：Phase E 实施完成*
*最后更新：2026-06-07*
