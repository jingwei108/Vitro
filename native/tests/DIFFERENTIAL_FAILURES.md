# 差分压力测试失败记录

> **原则**：不预设哪边是对的。差分测试失败时，两边都要审查，不能默认"Host 版一定对"。
>
> 记录所有偏差，即使偏差极小，也要记录。

---

## 已知偏差（KNOWN_DIVERGENCE）

### 整数溢出：Host 回绕 vs Bytecode VM Trap

- **来源**: Differential Stress — `test_diff_abs` / `test_diff_atoi`
- **涉及函数**: `abs(int)`, `atoi(char*)`
- **失败原因**: 同一输入下 Host 和 Bytecode 的行为不一致
- **最小复现**:
  ```c
  abs(-2147483648);   // INT_MIN
  atoi("2147483648"); // 超出 INT_MAX
  ```
- **Host 行为**: `host_abs` 和 `host_atoi` 使用 Rust 的 `wrapping_mul` / `wrapping_neg`，结果按二进制补码回绕（wrap around）。
- **Bytecode 行为**: VitroVM 的 `OpCode::Mul` / `OpCode::Add` 在执行时检查溢出，若超出 `i32` 范围则触发 `Trap`。
- **是否 Vitro 限制**: 是 — VM 的算术指令强制溢出检查是设计决策，目的是保护学生不被静默回绕误导。
- **是否标准库实现偏差**: 是（与 C 标准相比）。C 标准规定 signed integer overflow 是未定义行为（UB），允许任何行为（包括 trap、回绕、或更奇怪的结果）。
- **学生影响评级**: P1（限制已知）— 在常规教学代码中极少出现 `abs(INT_MIN)` 或解析超大整数；一旦出现，Vitro 会明确 trap 并提示溢出，这是比静默回绕更安全的教学行为。
- **建议**:
  - 短期：差分测试中排除会导致溢出的极端输入，聚焦正常路径的一致性验证。
  - 长期：若需要完全匹配 Clang 的 wrap-around 语义，可为 VM 增加一个 "宽松模式" 配置，关闭算术溢出检查。

---

## 已验证一致（VERIFIED）

| 函数 | Host Contract | Bytecode Consistency | Differential | 状态 |
|---|---|---|---|---|
| `abs` | ✅（新增） | ✅（Phase B 通过） | ✅（Phase C 通过） | 已验证 |
| `strlen` | ✅（已有） | ✅（Phase B 通过） | ✅（Phase C 通过） | 已验证 |
| `atoi` | ✅（已有） | ✅（Phase B 通过） | ✅（Phase C 通过） | 已验证 |
| `strcmp` | ✅（已有） | ✅（Phase B 通过） | ✅（Phase C 通过） | 已验证 |
| `isdigit` | N/A | ✅（Phase B 通过） | N/A（Host 无实现） | 待补齐 Host |
| `tolower` | N/A | ✅（Phase B 通过） | N/A（Host 无实现） | 待补齐 Host |

---

*文档状态：Phase C 实施中*
*最后更新：2026-06-07*
