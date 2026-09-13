# Vitro C++ Dogfooding 失败记录

> **原则**：All in. Record don't hide. Fix real bugs, not test cases.
>
> Dogfooding 验证是 Vitro C++ 编译器正确性的终极测试：用 Vitro C++ 子集编写纯 C++ 容器/算法实现，验证其运行时行为与 C 基线一致，并对简单方法进行字节码等价验证。

---

## 当前状态

截至 Stage 1 Dogfooding 验证推进完成：

| 容器/算法 | C++ 编译 | C 基线 | stdout 一致性 | get/size 字节码等价 |
|-----------|---------|--------|--------------|-------------------|
| `vector<int>` | ✅ | ✅ | ✅ | ✅ |
| `vector<float>` | ✅ | ✅ | ✅ | ✅ |
| `vector<char>` | ✅ | ✅ | ✅ | ✅ |
| `list<int>` | ✅ | ✅ | ✅ | size ✅ / get ⚠️ |
| `string` | ✅ | ✅ | ✅ | ✅ |
| `sort_int` | ✅ | ✅ | ✅ | — |

- `cpp_dogfooding_test.rs`: **25/25 通过**
- **当前无运行失败。**

---

## 已修复（FIXED）

### ~~mangled 函数名错误导致字节码比较测试永远 SKIP~~ → 已修复（2026-06-10）

- **位置**：`native/tests/cpp_dogfooding_test.rs`
- **问题**：旧实验性测试使用错误的 mangled 名称 `"get__vector__int"`
- **正确值**：`"vector__int__get"`（mangling 规则为 `format!("{}__{}", class_name, method_name)`）
- **后果**：`func_table.contains_key` 恒为 `false`，测试永远走 `SKIP` 分支
- **修复**：删除旧实验性测试，新增严格的 `assert_bytecode_equivalent_named` 验证

### ~~函数模板实例化 4 个 P0 bug~~ → 已修复（2026-06-10）

Dogfooding 推进 `sort_int` 模板实现过程中暴露并修复：

1. **`infer_template_arg` 未处理 Array→Pointer 退化**
   - 影响：`sort<T>(int a[5], 5)` 无法推断 `T = int`
   - 修复：`Type::Pointer` 分支增加 `Type::Array` 匹配

2. **`CallPtr` 模板实例化后 AST 未重写**
   - 影响：BytecodeGen 仍看到原始标识符名，报错 "未声明的标识符"
   - 修复：`resolve_expr_type` 将 `CallPtr` 重写为 `Call { name: mangled }`

3. **递归模板调用已实例化回退失败**
   - 影响：`sort_rec` 递归调用自身时，`try_monomorphize_func` 对已实例化函数返回 `None`
   - 修复：返回类型改为 `Option<(String, Option<FuncDecl>)>`，已实例化时返回 `Some((mangled, None))`

4. **`pending_instantiations` 函数体未检查**
   - 影响：实例化生成的函数体完全未经 TypeChecker 遍历，内部模板调用无法触发二次实例化
   - 修复：增加 Pass 3.6 循环 drain + `visit_func_decl` 直至收敛

---

## KNOWN_DIVERGENCE（设计决策导致的偏差，非失败）

### 算法实现不同导致字节码不等价

以下方法因 C++ 实现与 C 实现采用不同算法，字节码**不强制等价**，但运行时 stdout 必须一致：

| 方法 | C++ 实现 | C 实现 | 原因 |
|------|---------|--------|------|
| `vector::push_back` | `new[]/delete[]` + 循环复制 | `realloc` | C++ 需逐个元素构造/析构 |
| `vector::pop_back` | `delete[]` 元素析构 | `free` + 指针释放 | C++ RAII 语义 |
| `list::get` | `if/return` 分支 | 三元运算符 `?:` | Parser 生成不同控制流 |
| `sort` | 模板函数间接调用 | 直接函数调用 | mangling 名称不同 |

**验证策略**：
- 运行时 stdout 对比为**硬断言**（必须一致）
- 字节码等价仅对 `get`/`size` 等简单访问方法做**硬断言**
- 算法不同的方法记录为 `KNOWN_DIVERGENCE`，不做字节码强制等价

---

## 测试防线定位

Dogfooding 验证属于 **防线 3c Differential Stress** 的 C++ 扩展子层：

- **3c-C-Dogfooding**：C++ 模板类/函数实现 vs C 手写容器实现，运行时行为交叉对比
- 验收标准：stdout 逐行一致 + 简单方法字节码逐指令等价
