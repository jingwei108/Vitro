# Vitro C++14 教学子集拓展实施计划

**版本**: 2.9（2026-09-11 现状对齐；正文 Stage 章节与历史记录保持原样）  
**日期**: 2026-09-11  
**状态**: **Phase 34~41 已完成，Phase 42 进行中**。C++ 容器收口（Phase 34）、栈对象 RAII（35）、`new[]/delete[]`（36）、引用语义（37）、隐式移动构造（38）、`unique_ptr<T>` dogfooding（39）、M6 测试防线收尾（40）、内置容器布局解耦（41）均已落地；**Phase 42（P0 语法/标准库拓展 + 代码审查报告推进 + 性能优化 + `vitro_vec<T>` 类类型模板实参支持）🚧 进行中**，其中 lambda 相关批次（批次 H：lambda 返回类型推断、文件作用域 lambda 变量）已修复并有回归测试 `native/tests/cpp_lambda_test.rs`，详见 [`ARCHIVE_代码审查复核20260911.md`](../../archive/ARCHIVE_代码审查复核20260911.md §批次 H。  
**可核实指标（2026-09-11 实测）**: `native/tests/cases/cpp/` **78 个 `.cpp` 文件**；C++ Shadow Verification **100 个用例**（98 个与 Clang++ 一致 + 2 个已记录的 `clang_compile_fail`：`cpp_vitro_vec_class` / `cpp_vitro_list_class` 使用 Vitro 内置容器，无法被 Clang++ 直接编译）——口径以 `AGENTS.md` 防线 1 为准；C 侧模板生成用例 82 个（`native/tests/cases_template_generated/*.c`，按 `AGENTS.md` 防线 2 口径 78 绿 / 4 已知失败；`vitro_e2e.rs` 的 `KNOWN_TEMPLATE_FAILURES` 常量当前列 2 项：`bTree_default` / `spfa_default`——两者口径差异见 §对上述历史条目的更正第 3 点）。  
**前置依赖**: `C语言子集规范.md` P0/P1 阶段完成、Phase 31~33 C++ Parser/TypeChecker/BytecodeGen 完成

> **历史状态（v2.8 / 2026-06-13，原文保留）**: **M7 Beta Readiness 已就绪**：M6 + Stage 2b 已完成，`native/tests/cases/cpp/` 61 个 C++ E2E 用例全部通过，C++ Shadow Verification 83/83 一致、0 gap；全量 `cargo test` 719 passed、clippy 0 警告；ci_three_tier_check（当时为 .py，2026-09-18 迁 Go）误报已修复；同参数个数不同类型的构造函数重载已报告 `E4031` 而不是静默错误。已新增 5 个 C++ 教学模板（`cpp_hello` / `cpp_class_basic` / `cpp_vector_int` / `cpp_unique_ptr` / `cpp_range_for`）与学生版 `C++子集规范.md`。内置容器已全面迁移为 `runtime_libc/vitro/*.cpp` 标准模板实现。详见 `docs/archive/ARCHIVE_M7_BETA_READINESS.md`。**当前目标：启动内部试用并收集反馈。**
>
> **对上述历史条目的三点现状更正（2026-09-11，不改动原文）**:
> 1. **统计口径已过时**：61 个 C++ E2E / 83 个 Shadow 已被 Phase 34~42 的用例扩充取代，当前口径见上方"可核实指标"。
> 2. **引用链接状态**：`docs/archive/ARCHIVE_M7_BETA_READINESS.md` 已于 2026-09-11 归档为 [`../archive/ARCHIVE_M7_BETA_READINESS.md`](../../archive/ARCHIVE_M7_BETA_READINESS.md；指向旧路径的引用已在 `C++子集规范.md` 等处更新（`docs/README.md` 亦已按归档清单登记）。C++ 教学模板现存 **6 个**（历史条目中的 5 个 + 后续新增的 `cpp_vector_struct`）。
> 3. **模板失败计数存在三套不一致口径（诚实记录，2026-09-11 核实）**：
>    - `AGENTS.md` 防线 2 记 "82 个，78 绿，**4 已知失败**"（未列出是哪 4 个）；
>    - `native/tests/cases_template_generated/E2E_FAILURES.md` 当前列 **3** 条 `KNOWN_DIVERGENCE`：`bTree_default` / `infixEvaluation_default` / `spfa_default`；
>    - 代码常量只列 **2** 条：`native/tests/vitro_e2e.rs::KNOWN_TEMPLATE_FAILURES` 与 `scripts/shadow_verify/main.go::KNOWN_FAILURE_CASES`（D5 迁移后位置，原 Python 驱动已退役） 均为 `bTree_default` / `spfa_default`——**`infixEvaluation_default` 未进入任一常量**（CI 双向对账因此存在盲点）。
>    本文件不擅自统一这三套口径，仅如实记录差异；精确对账机制见 `AGENTS.md` 防线 5。

---

## 变更摘要（v2.0 → v2.1）

| 问题 | v2.0 | v2.1 |
|------|------|------|
| 前置条件状态 | 将 static、goto、#ifdef、volatile、qsort 等标记为"待实现" | **已修正为"已实现"**（代码事实） |
| 错误码范围 | 声称保护 E1001~E3061 | **已修正为 E1001~E3071**，C++ 预留 E4001~E4999 |
| 容器库策略 | 直接复制 klib 头文件 | **Stage 0：手写 C 容器（临时）→ Stage 1：Dogfooding 用 Vitro C++ 编译器编译 C++ 容器源码 → Stage 2：替换为 C++ 实现** |
| 容器库时机 | 全部前置 | **类型布局前置（硬编码），算法实现后置** |
| 依赖问题 | 示例使用 `lazy_static!`（不在 Cargo.toml） | **改为 `std::sync::LazyLock`（Rust 1.95 已支持）或普通函数** |
| 内部矛盾 | Dogfooding 示例出现被排除的 `operator[]` | **已删除运算符重载示例** |
| 预编译兼容 | 假设 klib `.h` 可直接预编译 | **明确预编译脚本仅支持 `.c` 文件** |

---

## 一、愿景与目标

### 1.1 为什么需要 C++

Vitro 的 C 子集已覆盖 95% 教学场景，但国内考研/竞赛/工程教学的绝对主流语言是 **C++14**。不支持 Lambda、移动语义、`unique_ptr`、`auto`、`范围 for` 的子集没有竞争力。

### 1.2 核心约束（铁律）

| 约束 | 说明 |
|------|------|
| **零改动 VM** | VitroVM（1MB 线性内存、栈式字节码）不做任何修改 |
| **不复用 Clang** | 不引入 libclang 或任何外部 C++ 编译器，保持自研可控 |
| **BytecodeGen 线性扩展** | TypeChecker 直接处理 C++ 语义，BytecodeGen 新增 C++ 节点分支 |
| **诊断体系保护** | E1001~E3071 错误码、知识图谱、UAF/DF 检测、堆内存可视化不得破坏 |
| **测试哲学延续** | All in. Record don't hide. Fix real bugs, not test cases. |
| **狗吃自己狗粮** | 无论是开始时的c写cpp容器，还是后期的cpp写cpp容器等类似场景，都以尽可能以工业的标准去写，如文档提到的以klib为参照标准。不因本项目的缺陷而扭曲代码，如发现标准代码不支持，小问题就修复，大问题记录下来。
### 1.3 C++14 边界

**支持（P1~P5）**:
- Lambda（含泛型，带捕获）
- 右值引用 / 移动语义 / `std::move`
- `unique_ptr` / `shared_ptr`（简化版，无自定义 deleter）
- `auto` 类型推导
- 范围 `for`
- 模板单态化（仅类型参数，无 SFINAE/特化/偏特化）
- 单继承 + 虚函数多态
- `class` / `public` / `private` / `this`
- `nullptr` / `bool` / `true` / `false`
- `new` / `delete`（映射为 malloc/free Host Func）

**排除**:
- 异常（try/catch/throw）
- 多重继承
- RTTI（typeid/dynamic_cast）
- **运算符重载**（教学价值低，复杂度极高，明确排除）
- SFINAE / 模板特化 / 偏特化
- 标准 STL（依赖异常/allocator/复杂元编程，VM 不支持）

### 1.4 编写层面的原生 C++ 兼容性（Honest Subset）

**目标**：让学生用"看起来像原生 C++"的代码编写，但明确知道边界在哪里。

#### 可以做到 80% 语法兼容（编译期映射）

```cpp
// ===== 学生写的（标准 C++ 语法）=====
#include <vector>
#include <algorithm>
#include <memory>
using namespace std;

int main() {
    vector<int> v = {3, 1, 4};
    sort(v.begin(), v.end());
    auto f = [](int x) { return x * 2; };
    unique_ptr<int> p(new int(42));
    for (auto x : v) { printf("%d\n", x); }
    return 0;
}
```

```cpp
// ===== Vitro 编译器内部处理（直接生成）=====
// 1. 头文件映射: <vector> → 编译器内置类型
// 2. 命名空间擦除: using namespace std; → 删除, std::vector → vector
// 3. 语法处理:
//    vector<int> v = {...}  → 类布局已知，生成初始化字节码
//    auto f = [](int x)       → struct __lambda_0 { ... };
//    unique_ptr<int> p        → struct unique_ptr_int p; ...
//    for (auto x : v)         → 范围 for AST 节点，BytecodeGen 展开为索引循环
```

#### 做不到的是语义兼容（架构限制）

```cpp
// 以下代码无法兼容，VM 不支持，会报明确错误
try { throw runtime_error("err"); } catch (...) {}  // [E4001] 不支持异常
class A { A operator+(const A& o); };                // [E4002] 不支持运算符重载
template<> struct max<int> { ... };                   // [E4003] 不支持模板特化
std::thread t([]{ ... });                             // [E4004] 不支持多线程
```

| 维度 | 能做到 | 做不到 |
|------|--------|--------|
| `std::` 前缀 | ✅ 自动擦除 | |
| 标准头文件名 | ✅ 映射到 vitro 头文件 | |
| Lambda / auto / 范围 for | ✅ 完全一致 | |
| 模板基本语法 | ✅ 一致（受限） | ❌ SFINAE / 特化 / 元编程 |
| class 语法 | ✅ 基本一致 | ❌ 运算符重载 |
| 异常 / 多线程 | | ❌ 不支持 |
| 标准库完整语义 | | ❌ allocator / 异常安全 |

#### Honest Subset 原则

> **不是"假装是 C++"，而是"诚实标注这是教学子集"。**

- 文件扩展名：`.vitrocpp`（可选）或 `.cpp`，IDE 标题栏显示 "Vitro C++ 子集"
- 每个特性文档标注 `[标准 C++ 差异]`
- `--show-lowered` 让学生看到 `vector<int>` 底层是什么
- 教材最后一章："从 Vitro C++ 到标准 C++"

---

## 二、前置条件修正（基于代码事实）

C++ 拓展**不可**在以下工作完成前启动：

| 前置任务 | v2.0 状态 | **v2.1 修正（代码事实）** | 说明 |
|----------|-----------|--------------------------|------|
| `static` 完整语义 | ⏳ 待实现 | **✅ 已实现** | `BytecodeGen` 已有 `static_local_indices`/`static_local_types`（`codegen/mod.rs` 行 51-52、638-657） |
| `goto` | ⏳ 待实现 | **✅ 已实现** | `TypeChecker` 已有 `func_labels`/`pending_gotos`（`typeck/mod.rs` 行 50-51、693-711），错误码 `E3071_UndefinedLabel` 已存在 |
| `volatile` | ⏳ 待实现 | **✅ 已实现** | `TokenType::Volatile` 已存在，`Type` 系统已支持修饰符 |
| `#ifdef` / `#ifndef` / `#endif` | ⏳ 待实现 | **✅ 已实现** | `Lexer` 已有 `conditional_stack` 和条件编译状态机 |
| `<limits.h>` / `<stdbool.h>` | ⏳ 待实现 | **✅ 已实现** | `runtime_libc/include/` 下已存在 |
| `qsort` | ⏳ 待实现 | **✅ 已实现** | `C语言子集规范.md` 及 `TypeChecker::visit_call()` 已支持 |
| `puts` / `sprintf` / `calloc` / `bsearch` | ⏳ 待实现 | **✅ 已实现** | `stdlib.h` 已声明，`TypeChecker` 已注册内建函数检查器 |

**事实结论**：C 子集 P0/P1 的绝大部分工作已经完成，前置条件不再是 C++ 拓展的阻塞项。

---

### `volatile` C++ 语义补充

`volatile` 在 C 模式下已实现（`TokenType::Volatile`、`Type` 修饰符支持），C++ 模式下的语义定义如下：

| 场景 | C 模式行为 | C++ 模式行为 | 说明 |
|------|-----------|-------------|------|
| `volatile int x` | 修饰符标记，代码生成与普通 `int` 相同 | 同 C 模式 | 教学子集不模拟内存屏障 |
| `volatile` 成员变量 | struct/union 字段 | class 字段，语义同 C | 布局计算中 `volatile` 不影响 `sizeof` |
| `const volatile` 组合 | 支持 | 支持 | 顺序无关，`is_const && is_volatile` 同时标记 |
| `volatile` 成员函数 | N/A（C 无成员函数） | **不支持** | 明确排除，遇 `volatile` 成员函数报 `E4006_VolatileMemberNotSupported` |
| 模板参数中的 `volatile` | N/A | **不支持** | `volatile T` 作为模板参数实例化后保留修饰符，但无特殊代码生成 |

**结论**：`volatile` 在 Vitro C++ 子集中仅作为**类型修饰符标记**存在，不生成特殊内存屏障指令，与 C 模式保持完全一致。

---

## 三、架构概览：混合架构（TypeChecker + BytecodeGen 直接扩展）

```
┌─────────────────────────────────────────────────────────────┐
│                      学生代码输入层                          │
│         C++14 语法（vector<int>、Lambda、auto...）           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Parser（扩展）                            │
│       C++14 语法 → C++ AST（单一 AST，混合 C/C++ 节点）       │
│              class / Lambda / template / 范围 for             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 TypeChecker（扩展）                          │
│     • auto 类型推导（替换为具体类型）                         │
│     • 模板单态化（生成 FuncDecl 插入 Program）               │
│     • 类/继承/vtable 布局分析                                │
│     • 引用 & / && 语义标记                                   │
│     • 重载决议（移动构造/拷贝构造优先级）                     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                轻量降解层（仅容器 + 语法糖）                  │
│     • vector.push_back → __vitro_vec_push_int               │
│     • 范围 for → 索引循环 AST                                │
│     • Lambda → 闭包 struct + 函数（AST 变换）                │
│     • unique_ptr/shared_ptr → struct + 手动 destroy          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 BytecodeGen（扩展）                          │
│     • This / MemberCall（非虚函数）                          │
│     • MemberCall（虚函数 = vtable 间接调用）                  │
│     • New/Delete（HostCall malloc/free）                     │
│     • 其他节点复用现有 C 分支                                │
│     • SourceLoc 天然保留（原始 C++ 代码位置）                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                现有 VM（零改动）                             │
│              VitroVM（1MB 线性内存，栈式字节码）               │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 容器库基座：参照 klib 手写 C 实现

**不是直接引入 klib 头文件，而是参照 klib 的算法设计，手写 Vitro-C 子集兼容的 `.c` 文件**。

| 误区 | 正解 |
|------|------|
| "直接复制 klib `.h` 文件" | klib 是头文件宏泛型（`kvec_t(int)`），Vitro 预编译脚本只编译 `.c` 文件 |
| "klib 零修改即可使用" | klib 宏泛型与 Vitro `Type` 枚举没有"宏展开"概念，TypeChecker 无法直接理解 |
| "用 C++ 写容器给 Vitro 用" | Vitro 编译器目前不支持 C++ 语法，`vitro_cli` 只能编译 `.c` 文件 |

**核心洞察**：参照 klib 的极简算法（kvec 核心仅 3 个字段 + realloc 扩容），手写每种容器×每种类型的 `.c` 文件，直接放入 `runtime_libc/vitro/` 预编译。

### 3.2 编译器双模式

| 模式 | 名称 | 行为 | 适用场景 |
|------|------|------|----------|
| **Mode A** | `CppExplicit` | 学生显式写 `__init` / `__destroy` | 进阶教学，理解 RAII 本质 |
| **Mode B** | `CppImplicit` | 编译器自动插入作用域守卫析构 | 入门教学，降低心智负担 |

---

## 四、H1 详细方案：Vitro 容器库（参照 klib 自研）

### 4.1 Dogfooding 路线：C 基底 → C++ 替换

> **手写 C 容器是临时方案（Stage 0），最终目标是用 Vitro C++ 编译器编译 C++ 容器源码（Stage 1 Dogfooding），验证通过后替换 C 实现（Stage 2）。**

**注意**：Vitro 编译器本身用 Rust 编写，不存在"自举"（编译器不能用自身编译）。此处是用 Vitro C++ 编译器去编译 C++ 容器库，属于 **Dogfooding**（开发团队用自己的产品解决自己的问题），是验证编译器正确性的终极手段。

**时序约束（关键）**：Stage 1 必须在 C++ 编译器核心（Parser→TypeChecker→BytecodeGen）全部完成后才能启动，不存在循环依赖：
1. **Stage 0** 现在就能做：手写 C 容器，预编译为字节码，学生代码立即调用
2. **Phase 1-3** 实现 C++ 编译器核心（约 4.5 个月）
3. **Stage 1** 编译器完成后：用 C++ 编译器编译 C++ 容器源码，做 Dogfooding 验证
4. **Stage 2** 验证通过后：删除 C 容器，完全替换为 C++ 实现

```
Stage 0（现在 ──→ Phase 1-3 ──→ Stage 1 ──→ Stage 2）
  C 容器（手写）    编译器核心      C++ 容器源码      C++ 容器（运行时）
  预编译为 BC Libc  （Parser/       用 Vitro C++       完全替换 C 实现
  学生代码调用      TypeChecker/    编译器编译
                    BytecodeGen）   字节码对比验证
```

| 阶段 | 容器实现语言 | 编译方式 | 用途 | 时机 |
|------|-------------|----------|------|------|
| **Stage 0** | C（手写极简） | `scripts/precompile_bytecode_libc` | 学生代码运行时调用 | **现在** |
| **Stage 1** | Vitro C++ 子集 | Vitro C++ 编译器 | Dogfooding + 教学源码展示 | **C++ 编译器核心完成后** |
| **Stage 2** | Vitro C++ 子集 | Vitro C++ 编译器 | **完全替换 Stage 0** | **Dogfooding 验证通过后** |

**Stage 1 的验证方法**：
1. 用 Vitro C++ 子集写 `vitro::vector<int>`（class template + 构造函数 + push_back）
2. 用 Vitro C++ 编译器编译 → 生成字节码 A
3. 对比 Stage 0 的 C 版本预编译字节码 B
4. 如果 A ≡ B（逐指令一致）→ 编译器正确，可进入 Stage 2（Dogfooding 通过）

### 4.2 设计原则

- **Stage 0 参照 klib 算法**：数据结构设计和扩容策略与 klib 保持一致（已验证的工业级算法）
- **Vitro-C 子集编写**：使用项目已支持的 C 语法（`runtime_libc/src/string.c` 和 `stdlib.c` 的风格）
- **预编译友好**：每个容器类型是独立的 `.c` 文件，直接由 `scripts/precompile_bytecode_libc`（Go，2026-09-18 前为 .py）编译
- **标准库后置**：容器算法实现（`.c` 文件）在编译器核心完成后补充，但类型布局信息前置硬编码
- **为未来替换留接口**：C 容器的函数签名和内存布局与目标 C++ 容器一致，确保 Stage 1/2 平滑替换

### 4.2 容器组件

| 组件 | 参照来源 | 对应 C++ STL | Vitro 优先级 | 说明 |
|------|----------|-------------|------------|------|
| `vitro_vec` | kvec.h | `vector` | ✅ P0 | 动态数组，核心结构 `{ int n, m; T *a; }` |
| `vitro_list` | klist.h | `list` / `forward_list` | ✅ P0 | 单向链表，简化版（不做内存池） |
| `vitro_string` | kstring.h | `string` | ✅ P0 | 动态字符串，简化版（不做 printf/vsprintf） |
| `vitro_deque` | kdq.h | `deque` | ⚠️ P2 | 双端队列 |
| `vitro_hash` | khash.h | `unordered_map` / `unordered_set` | ⚠️ P3 | 开放寻址哈希表 |
| `vitro_sort` | ksort.h | `algorithm::sort` | ✅ P1 | 内省/归并/堆排序 |

### 4.3 参照实现示例（vector<int>）

```c
// native/runtime_libc/vitro/vec_int.c
// 参照 kvec.h 算法，用 Vitro-C 子集重写

typedef struct {
    int n;      /* 当前元素数 */
    int m;      /* 容量 */
    int *a;     /* 数据指针 */
} vitro_vec_int;

void vitro_vec_init_int(vitro_vec_int *v) {
    v->n = 0;
    v->m = 0;
    v->a = 0;
}

void vitro_vec_push_int(vitro_vec_int *v, int x) {
    if (v->n == v->m) {
        v->m = v->m ? v->m << 1 : 2;
        v->a = (int *)realloc(v->a, sizeof(int) * v->m);
    }
    v->a[v->n++] = x;
}

int vitro_vec_pop_int(vitro_vec_int *v) {
    return v->a[--v->n];
}

int vitro_vec_size_int(vitro_vec_int *v) {
    return v->n;
}

int vitro_vec_get_int(vitro_vec_int *v, int i) {
    return v->a[i];
}

void vitro_vec_clear_int(vitro_vec_int *v) {
    v->n = 0;
}

void vitro_vec_destroy_int(vitro_vec_int *v) {
    free(v->a);
}
```

**与 klib 的对应关系**：

| klib 宏 | 参照实现函数 |
|---|---|
| `kvec_t(int)` | `vitro_vec_int` |
| `kv_init(v)` | `vitro_vec_init_int(&v)` |
| `kv_push(int, v, x)` | `vitro_vec_push_int(&v, x)` |
| `kv_pop(v)` | `vitro_vec_pop_int(&v)` |
| `kv_size(v)` | `vitro_vec_size_int(&v)` |
| `kv_A(v, i)` | `vitro_vec_get_int(&v, i)` |
| `kv_destroy(v)` | `vitro_vec_destroy_int(&v)` |

### 4.4 集成目录结构（Stage 2b 已更新）

```
native/runtime_libc/
├── include/                    # 已有：stdio.h / stdlib.h / ctype.h / math.h / string.h
├── src/                        # 已有：ctype.c / stdlib.c / string.c
└── vitro/                       # Vitro 内置容器库（.cpp 文件，标准模板 C++ 实现）
    ├── vector.cpp              # template <class T> class vitro_vec<T>；显式实例化 int/float/char
    ├── list.cpp                # template <class T> class vitro_list<T>；显式实例化 int
    ├── string.cpp              # template <class T> class vitro_string<T>；显式实例化 char
    └── sort_int.cpp            # 函数模板 vitro_sort_int<T>
```

> 历史：Stage 0 使用手写 `.c` 文件；Stage 2b 已全部替换为 `.cpp` 模板实现，并删除旧 `.c` 文件。

### 4.5 预编译流程

复用并扩展 `scripts/precompile_bytecode_libc`（当时为 .py，2026-09-18 迁 Go）：脚本同时编译 `runtime_libc/src/*.c` 与 `runtime_libc/vitro/*.cpp`，统一生成 `bytecode_libc_data.json`。Stage 2b 中已增加对 `.cpp` 文件及模板显式实例化的支持。

```python
# scripts/precompile_bytecode_libc（当时为 .py，现役 Go 版）——无需修改逻辑，只需扩展路径
# 现有：编译 runtime_libc/src/*.c
# 新增：编译 runtime_libc/vitro/*.c
# 统一生成 bytecode_libc_data.json
# 索引 key 格式: "vitro_vec_init_int", "vitro_vec_push_int", ...
```

**事实依据**：现有脚本第 62-63 行遍历 `.c` 文件，第 71 行调用 `vitro_cli export`。`.vitro/*.c` 文件与现有 `.src/*.c` 文件在编译流程上无差异。

### 4.6 类型映射表（编译器内置）

```rust
// native/src/compiler/cpp_frontend/type_map.rs
// Rust 1.95 支持 std::sync::LazyLock，无需引入 lazy_static crate

use std::collections::HashMap;
use std::sync::LazyLock;

static KLIB_TYPE_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("vector<int>", "vitro_vec_int");
    m.insert("vector<float>", "vitro_vec_float");
    m.insert("vector<char>", "vitro_vec_char");
    m.insert("list<int>", "vitro_list_int");
    m.insert("string", "vitro_string");
    m
});
```

### 4.7 方法映射表

| Vitro C++ 语法 | 映射为容器库调用 | 说明 |
|--------------|----------------|------|
| `vector<int> v;` | `vitro_vec_int v; vitro_vec_init_int(&v);` | 构造 = struct 定义 + init 调用 |
| `v.push_back(x);` | `vitro_vec_push_int(&v, x);` | 插入，自动扩容 |
| `v[i]` | `vitro_vec_get_int(&v, i)` | 索引访问（无边界检查） |
| `v.size()` | `vitro_vec_size_int(&v)` | 元素个数 |
| `v.pop_back()` | `vitro_vec_pop_int(&v)` | 尾部弹出 |
| `v.clear()` | `vitro_vec_clear_int(&v)` | 逻辑清空 |
| `list<int> l;` | `vitro_list_int l;` | 链表 |
| `l.push_back(x);` | `vitro_list_push_back_int(&l, x);` | 尾部插入 |
| `l.push_front(x);` | `vitro_list_push_front_int(&l, x);` | 头部插入 |

### 4.8 内存适配

容器库使用 `malloc`/`realloc`/`free`。Vitro 已将这些映射为 VM Host Call（`HostMalloc`/`HostFree`），**无需修改容器库源码**。

**VM 内存模型事实**：
- `size_t` 在 Vitro 中为 `unsigned int`（32 位，`runtime_libc/include/stdlib.h` 行 1）
- `malloc` 参数类型为 `int`（`stdlib.h` 行 3：`void* malloc(int size)`）
- 容器库中统一使用 `int` 表示大小和容量，与 VM 字长一致

---

## 五、容器类型布局前置方案

### 5.1 为什么类型布局必须前置

编译器核心开发阶段（Parser/TypeChecker/BytecodeGen）需要测试 `vector<int>` 的语义分析和代码生成。如果容器库尚未完成，编译器无法知道 `vector<int>` 占多少字节、`push_back` 的签名是什么。

**解决方案**：容器**类型布局信息**作为编译器内置知识硬编码，**算法实现**后置补充。

### 5.2 前置内容（TOML 配置文件）

类型布局从外部 TOML 配置文件加载，避免在 Rust 中硬编码 16+ 条目：

```toml
# runtime_libc/vitro/layouts.toml

[vector_int]
size = 12
[[vector_int.fields]]
name = "n"
type = "int"
[[vector_int.fields]]
name = "m"
type = "int"
[[vector_int.fields]]
name = "a"
type = "int*"

[[vector_int.methods]]
name = "push_back"
params = ["int"]
ret = "void"
is_virtual = false

# ... 其他类型参见 layouts.toml 完整文件
```

编译器启动时读取并缓存：

```rust
// native/src/compiler/cpp_frontend/builtin_layout.rs

use std::collections::HashMap;
use std::sync::LazyLock;

pub struct ClassLayout {
    pub size: i32,
    pub fields: Vec<(String, Type)>,
    pub methods: Vec<MethodSig>,
}

pub struct MethodSig {
    pub name: String,
    pub params: Vec<Type>,
    pub ret: Type,
    pub is_virtual: bool,
}

static BUILTIN_LAYOUTS: LazyLock<HashMap<String, ClassLayout>> = LazyLock::new(|| {
    let toml_str = include_str!("../../../runtime_libc/vitro/layouts.toml");
    parse_layouts_toml(toml_str)
});

pub fn builtin_class_layout(name: &str) -> Option<ClassLayout> {
    BUILTIN_LAYOUTS.get(name).cloned()
}
```

### 5.3 后置内容（容器算法实现）

容器库的 `.c` 文件在编译器核心稳定后补充。补充流程：
1. 手写 `runtime_libc/vitro/vec_int.c`
2. 运行 `go run ./scripts/precompile_bytecode_libc`
3. 将预编译函数名从 `__vitro_vec_push_int_stub` 替换为真实的 `vitro_vec_push_int`

**注意**：内置布局表中的方法签名必须与后置的 `.c` 实现严格一致。

---

## 六、H2 详细方案：混合架构（TypeChecker + BytecodeGen 直接扩展）

### 6.1 设计原则

**核心变更**：不再将 C++ AST 完全降解为 C AST，而是让 TypeChecker 和 BytecodeGen 直接理解 C++ 语义节点。

| 维度 | 原方案（v1.0 完全降解） | 新方案（v2.1 混合） |
|------|----------------------|-------------------|
| **AST** | C++ AST → C AST（两层） | 单一 AST（混合 C/C++ 节点） |
| **TypeChecker** | 只处理 C | 处理 C + auto 推导 + 模板单态化 + 类布局 |
| **BytecodeGen** | 只处理 C | 处理 C + 虚函数/this/new/delete |
| **SourceLoc** | 需要 Source Map 回查 | **天然保留**（直接指向 C++ 源码） |
| **编译速度** | 慢（完整 AST→AST 降解） | 快（线性扩展） |
| **测试验证** | 可对比降解产物（differential） | 直接验证字节码 + 轻量降解产物审计 |

**复杂度转移**：将"降解器复杂度"转移到"BytecodeGen 分支复杂度"，换取 SourceLoc 天然保留和编译速度提升。

### 6.2 AST 扩展

```rust
// native/src/compiler/ast.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TypeKind {
    // === 现有 C 类型 ===
    #[default]
    Void,
    Int,
    Char,
    Float,
    Double,
    LongLong,
    Pointer,
    Array,
    Struct,
    Union,
    Function,

    // === C++ 新增 ===
    Class,       // P1
    Reference,   // P2: & / const&
    RValueRef,   // P5: &&
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Type {
    // === 现有 C 类型 ===
    Void { is_const: bool },
    Int { is_unsigned: bool, is_const: bool },
    Char { is_unsigned: bool, is_const: bool },
    Float { is_const: bool },
    Double { is_const: bool },
    LongLong { is_unsigned: bool, is_const: bool },
    Pointer { pointee: Box<Type>, is_const: bool },
    Array { element: Box<Type>, array_size: i32, dims: Vec<i32>, is_const: bool, is_vla: bool, vla_dims: Vec<Box<Expr>> },
    Function { return_type: Box<Type>, param_types: Vec<Type>, is_const: bool },
    Struct { name: String, is_const: bool },
    Union { name: String, is_const: bool },

    // === C++ 新增 ===
    Class { name: String, is_const: bool },         // P1
    Reference { base: Box<Type>, is_const: bool },  // P2
    Auto,                                            // P2: 占位类型，TypeChecker 推导后替换
    RValueRef { base: Box<Type> },                  // P5
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    // === 现有 C 表达式（Binary, Unary, Literal, Identifier, Call, CallPtr, Index, Member, Assign, Ternary, Sizeof, Cast, InitList, Offsetof）===
    // ... 省略已有变体

    // === C++ 新增 ===
    This { loc: SourceLoc },                             // P1: this 指针
    MemberCall {                                         // P1: obj.method()
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        is_virtual: bool,
        loc: SourceLoc,
        ty: Type,
    },
    Lambda {                                             // P2
        capture: Vec<CaptureMode>,
        params: Vec<Param>,
        body: Box<Stmt>,
        unique_id: u64,
        loc: SourceLoc,
    },
    New { elem_type: Type, size_expr: Option<Box<Expr>>, init: Option<Box<Expr>>, loc: SourceLoc },      // P2
    Delete { expr: Box<Expr>, is_array: bool, loc: SourceLoc },                                          // P2
    Move { expr: Box<Expr>, loc: SourceLoc },            // P5: std::move(x)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Stmt {
    // === 现有 C 语句（Block, VarDecl, Expr, If, While, DoWhile, For, Return, Break, Continue, Switch, Case, Goto, Label）===
    // ... 省略已有变体

    // === C++ 新增 ===
    RangeFor { var: String, var_type: Type, iter: Box<Expr>, body: Box<Stmt>, loc: SourceLoc },  // P2
    Try { body: Box<Stmt>, catches: Vec<CatchClause>, loc: SourceLoc },                         // P5: 语法占位，TypeChecker 报错
}

// === 新增节点 ===
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassDecl {
    pub name: String,
    pub base: Option<String>,    // 单继承，最多一个基类
    pub members: Vec<ClassMember>,
    pub vtable: Option<VTable>,  // 虚函数表（TypeChecker 生成）
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ClassMember {
    Field { name: String, type_: Type, access: AccessSpec },
    Method { name: String, ret: Type, params: Vec<Param>, body: Stmt, is_virtual: bool, access: AccessSpec },
    Constructor { params: Vec<Param>, body: Stmt, is_default: bool },
    Destructor { body: Stmt },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateDecl {
    pub params: Vec<TemplateParam>,   // 仅类型参数
    pub decl: Box<Templateable>,      // 函数模板或类模板
}
```

**序列化兼容性风险**：新增 `Type`/`Expr`/`Stmt` 变体会改变 serde 序列化格式。如果 VM 快照依赖旧格式，需要验证兼容性。（Flutter Bridge 已随 2026-09-11 前端切割迁出，不再是风险对象。）

### 6.3 Lexer 扩展

```rust
// native/src/compiler/lexer.rs

pub enum TokenType {
    // === 现有 C 关键字 ===
    Int, Void, Char, If, Else, While, Do, For, Return, Break, Continue,
    Struct, Union, Sizeof, Offsetof, Switch, Case, Default, Typedef, Enum,
    Unsigned, Long, Short, Signed, Const, Extern, Float, Double,
    Volatile, Inline, Restrict, Register, Auto, Bool, Goto, Null,

    // === C++ 新增关键字 ===
    Class, Public, Private, Protected, This,          // P1
    Using, Namespace,                                 // P2（Namespace 仅用于报错"不支持"）
    Virtual, Override, Friend,                        // P4
    Template, Typename,                               // P5
    Static_cast, Const_cast, Reinterpret_cast,        // P5
    New, Delete,                                      // P6

    // === C++ 新增运算符/标点 ===
    ColonColon,       // ::
    ArrowStar,        // ->*
    DotStar,          // .*
    LineComment,      // // 行注释

    // ... 其他现有 Token 不变
}
```

**注意**：现有 Lexer 已有 `Auto`（C 存储类）和 `Bool`/`Null` Token。C++ 的 `auto` 语义（类型推导）与 C 的 `auto`（存储类）完全不同，Parser 需要根据上下文区分。

### 6.4 Parser 扩展

基于现有递归下降架构扩展：

```rust
// parser/mod.rs

impl Parser {
    fn parse_class_decl(&mut self) -> Result<ClassDecl, ParseError> {
        // class Name [: public Base] { public: ... private: ... }
        // 降级为 ClassDecl AST 节点
    }

    fn parse_range_for(&mut self) -> Result<Stmt, ParseError> {
        // for (auto x : expr) { body }
        // 生成 Stmt::RangeFor
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        // [capture](params) { body }
        // 生成 Expr::Lambda
    }

    fn parse_member_call(&mut self, object: Expr) -> Result<Expr, ParseError> {
        // obj.method(args)
        // 生成 Expr::MemberCall
    }
}
```

### 6.5 TypeChecker 扩展

#### 6.5.1 auto 类型推导

```rust
// typeck/cpp_auto.rs

impl TypeChecker {
    fn deduce_auto_type(&mut self, init: &Expr) -> Type {
        match init {
            Expr::Literal(_) => Type::Int,
            Expr::FloatLiteral(_) => Type::Float,
            Expr::Identifier(name) => self.lookup_var_type(name).clone(),
            Expr::Call { ty, .. } | Expr::MemberCall { ty, .. } => ty.clone(),
            // ... 其他表达式类型推断
            _ => Type::Int, // fallback
        }
    }
}
```

#### 6.5.2 模板受限单态化

```rust
// typeck/cpp_monomorph.rs

impl TypeChecker {
    fn monomorphize_template(&mut self, name: &str, type_args: &[Type]) -> String {
        let mangle_name = format!("{}__{}", name, type_args.iter().map(|t| t.mangle()).collect::<String>());
        if !self.has_instance(&mangle_name) {
            let template = self.find_template(name);
            let func_decl = self.instantiate(&template, type_args);
            self.program.funcs.push(func_decl);
        }
        mangle_name
    }
}
```

**约束**：仅类型参数，无 SFINAE/特化/偏特化，递归深度 ≤ 8。

#### 6.5.3 类/vtable 布局分析

```rust
// typeck/cpp_class_layout.rs

impl TypeChecker {
    fn analyze_class(&mut self, class: &mut ClassDecl) {
        let mut offset = 0;
        if let Some(base) = class.base.as_ref() {
            offset = self.get_class_layout(base).size;
        }
        let mut vtable = VTable::new();
        for method in &class.members {
            if let ClassMember::Method { is_virtual: true, .. } = method {
                vtable.add(method.name.clone(), method.ty());
            }
        }
        class.vtable = Some(vtable);
    }
}
```

### 6.6 BytecodeGen 扩展

#### 6.6.1 虚函数调用

```rust
// codegen/cpp_member_call.rs

Expr::MemberCall { object, method, args, is_virtual: true, loc, .. } => {
    self.gen_expr(object);
    self.emit(OpCode::Dup, 0, &loc);        // this 指针
    self.emit(OpCode::LoadMem, 0, &loc);    // vptr
    let vtable_offset = self.get_vtable_offset(&object.ty(), method);
    self.emit(OpCode::PushConst, vtable_offset, &loc);
    self.emit(OpCode::Add, 0, &loc);        // &vptr[method]
    self.emit(OpCode::LoadMem, 0, &loc);    // 函数指针
    for arg in args { self.gen_expr(arg); }
    self.emit(OpCode::CallPtr, args.len() as i32 + 1, &loc);
}
```

**VM 事实依据**：`OpCode::CallPtr = 111` 已存在（`opcode.rs`），支持函数指针间接调用。

#### 6.6.2 非虚成员调用

```rust
Expr::MemberCall { object, method, args, is_virtual: false, loc, .. } => {
    let func_name = self.mangle_method(&object.ty(), method);
    self.gen_expr(object);  // this
    for arg in args { self.gen_expr(arg); }
    self.emit(OpCode::Call, self.func_index(&func_name), &loc);
}
```

#### 6.6.3 this 指针

```rust
Expr::This { loc } => {
    self.emit(OpCode::LoadLocal, 0, &loc);  // this = 第 0 个参数
}
```

#### 6.6.4 new / delete

```rust
Expr::New { elem_type, init, loc, .. } => {
    let size = self.type_size(&elem_type);
    self.emit(OpCode::PushConst, size as i32, &loc);
    self.emit(OpCode::CallHost, HOST_MALLOC_ID, &loc);
    if let Some(init_expr) = init {
        self.emit(OpCode::Dup, 0, &loc);
        self.gen_expr(init_expr);
        self.emit(OpCode::Call, self.ctor_index(&elem_type), &loc);
    }
}

Expr::Delete { expr, loc, .. } => {
    self.gen_expr(expr);
    self.emit(OpCode::Call, self.dtor_index(&expr.ty()), &loc);
    self.emit(OpCode::CallHost, HOST_FREE_ID, &loc);
}
```

### 6.7 轻量降解层（仅容器 + 语法糖）

#### 6.7.1 容器方法映射（TypeChecker 阶段）

```rust
// typeck/cpp_container.rs

impl TypeChecker {
    fn check_member_call(&mut self, expr: &mut Expr) {
        if let Expr::MemberCall { object, method, args, .. } = expr {
            if is_builtin_vector(&object.ty()) && method == "push_back" {
                let elem_ty = object.ty().type_param(0);
                let host_func = format!("vitro_vec_push_{}", elem_ty.mangle());
                *expr = Expr::Call {
                    func: Box::new(Expr::Identifier(host_func)),
                    args: vec![
                        Expr::Unary { op: UnaryOp::Addr, expr: object.clone(), loc, ty: Type::pointer_to(object.ty()) },
                        args[0].clone(),
                    ],
                    loc,
                    ty: Type::void(),
                };
            }
        }
    }
}
```

#### 6.7.2 范围 for（AST 轻量变换）

```cpp
// 学生代码
for (auto x : v) { print(x); }
```

```rust
// 轻量降解：RangeFor → For 循环 AST
Stmt::RangeFor { var, var_type, iter, body, loc } => {
    let index_var = gen_unique_name("__i");
    Stmt::For {
        init: Some(Box::new(Stmt::VarDecl {
            name: index_var.clone(),
            var_type: Type::Int,
            init: Some(Expr::Literal(0)),
            extra_vars: vec![],
            is_static: false,
            loc,
        })),
        cond: Some(Expr::Binary {
            op: BinaryOp::Lt,
            left: Box::new(Expr::Identifier(index_var.clone())),
            right: Box::new(Expr::Call {
                func: Box::new(Expr::Identifier("vitro_vec_size_int".to_string())),
                args: vec![iter.clone()],
                loc,
                ty: Type::Int,
            }),
        }),
        step: Some(Expr::Unary {
            op: UnaryOp::PostInc,
            expr: Box::new(Expr::Identifier(index_var.clone())),
        }),
        body: Box::new(Stmt::Block(vec![
            Stmt::VarDecl {
                name: var,
                var_type: var_type,
                init: Some(Expr::Call {
                    func: Box::new(Expr::Identifier("vitro_vec_get_int".to_string())),
                    args: vec![iter.clone(), Expr::Identifier(index_var)],
                    loc,
                    ty: var_type,
                }),
                extra_vars: vec![],
                is_static: false,
                loc,
            },
            *body,
        ])),
        loc,
    }
}
```

#### 6.7.3 Lambda（AST 轻量变换）

```cpp
// 学生代码
auto f = [x](int y) { return x + y; };
int z = f(5);
```

```rust
// 轻量降解：Lambda → 闭包 struct + 函数
// 1. 生成闭包 struct
struct __lambda_0 {
    int x;  // 捕获变量
};

// 2. 生成调用函数
fn __lambda_0__call(this: &__lambda_0, y: int) -> int {
    this.x + y
}

// 3. 替换原 AST
// auto f = [x](int y) { ... };
// →
// struct __lambda_0 f;
// f.x = x;  // 捕获初始化
// int z = __lambda_0__call(&f, 5);
```

**注意**：Lambda 的轻量降解保留 SourceLoc 映射（`__lambda_0__call` 函数体的每个语句的 loc 指向原始 Lambda 体对应行）。

### 6.8 模块结构

```
native/src/compiler/
├── typeck/
│   ├── mod.rs                       # 现有入口
│   ├── cpp_auto.rs                  # auto 类型推导
│   ├── cpp_monomorph.rs             # 模板单态化
│   ├── cpp_class_layout.rs          # 类/vtable 布局分析
│   ├── cpp_container.rs             # 容器方法映射（轻量降解）
│   └── cpp_overload.rs              # 重载决议（移动/拷贝构造）
├── codegen/
│   ├── mod.rs                       # 现有入口
│   ├── cpp_member_call.rs           # 虚函数/非虚成员调用生成
│   ├── cpp_this_new_delete.rs       # this/new/delete 生成
│   └── cpp_lambda.rs                # Lambda 闭包生成
├── cpp_frontend/
│   ├── mod.rs
│   ├── type_map.rs                  # C++ 类型名映射
│   └── builtin_layout.rs            # 内置容器类型布局
├── ast.rs                           # 扩展：新增 C++ 节点
└── parser.rs                        # 扩展：新增 C++ 语法
```

---

## 七、编译器双模式详解

### 7.1 CppExplicit 模式

学生显式管理资源生命周期，与 C 模式一致。此模式用于进阶教学，让学生理解 RAII 的本质：

```cpp
vector<int> v;
v.push_back(1);
v.push_back(2);
// ...
v.__destroy();   // 显式调用
```

### 7.2 CppImplicit 模式

编译器在每个作用域退出点（块结束、`return`、`break`、`continue`）按 **LIFO（先构造的后析构）** 自动插入析构调用，学生无需手写 `__destroy`。此模式用于入门教学，降低心智负担。

```cpp
void foo(bool cond) {
    vector<int> v;          // __ctor__vector_int 自动插入
    v.push_back(1);
    if (cond) {
        vector<int> w;      // __ctor__vector_int 自动插入
        w.push_back(2);
        return;             // __dtor__vector_int(w) → __dtor__vector_int(v) 自动插入
    }
    v.push_back(3);
}                           // __dtor__vector_int(v) 自动插入
```

覆盖场景：

| 场景 | 行为 |
|------|------|
| 块作用域正常结束 | 逆序析构当前块内所有类对象 |
| `return` | 逆序析构函数内所有活跃作用域的类对象 |
| `break` / `continue` | 逆序析构当前循环内部作用域的类对象，再跳转 |
| 嵌套作用域 | 内层对象先于外层对象析构 |
| 循环体 | 每次进入循环体 = 新作用域；循环正常结束或跳转时析构 |
| `goto` 跳出作用域 | 由 BytecodeGen 根据目标 label 所在作用域深度，析构中间跳过的作用域（教学子集建议避免此类写法，行为与 GCC -O0 保持一致） |

### 7.3 全面作用域守卫生成算法

CppImplicit 模式的核心不是 AST 层面的"降解"，而是 BytecodeGen 在代码生成阶段维护**作用域帧（ScopeFrame）**，并在每一条可能退出作用域的控制流边前注入析构调用。这样 SourceLoc 天然保留，且无需对 AST 做前置变换。

#### 7.3.1 数据结构

```rust
// native/src/compiler/codegen/mod.rs

#[derive(Debug, Clone)]
struct ClassVarEntry {
    name: String,       // 变量名，用于调试
    offset: i32,        // 相对于栈帧基址的局部变量偏移
    class_name: String, // 类名，用于查找 __dtor__{ClassName}
}

#[derive(Debug, Clone, Default)]
struct ScopeFrame {
    shadows: Vec<ShadowEntry>,
    /// 在当前 scope 中声明的类类型局部变量，按声明顺序排列。
    /// 作用域退出时按 LIFO（逆序）调用析构函数。
    class_vars: Vec<ClassVarEntry>,
}

pub struct BytecodeGen {
    local_scope_stack: Vec<ScopeFrame>,
    /// 当前 loop 对应的 scope 深度栈，与 loop_start_ips 同步 push/pop。
    /// 用于 break/continue 时计算需要析构的 scope 层数。
    loop_scope_depths: Vec<usize>,
    // ...
}
```

#### 7.3.2 构造时登记

当 `Stmt::VarDecl` 声明一个 class 类型局部变量时，BytecodeGen 完成两件事：

1. 若变量无初始化表达式，生成对默认构造函数 `__ctor__{ClassName}` 的调用；
2. 将该变量登记到当前 `ScopeFrame.class_vars` 末尾。

```rust
// native/src/compiler/codegen/stmt.rs

if vty.is_class() {
    if let Type::Class { name: class_name, .. } = vty {
        self.record_class_var(name, local_offset, class_name);
        self.emit_class_default_ctor(class_name, local_offset, loc);
    }
}
```

`record_class_var` 把条目追加到当前 scope 的 `class_vars`，保证顺序与构造顺序一致：

```rust
fn record_class_var(&mut self, name: &str, offset: i32, class_name: &str) {
    if let Some(frame) = self.local_scope_stack.last_mut() {
        frame.class_vars.push(ClassVarEntry {
            name: name.to_string(),
            offset,
            class_name: class_name.to_string(),
        });
    }
}
```

#### 7.3.3 析构函数调用生成

```rust
/// 生成对栈上指定偏移处类对象的析构函数调用。
fn emit_class_dtor(&mut self, class_name: &str, offset: i32, loc: &SourceLoc) {
    let dtor_name = format!("__dtor__{}", class_name);
    if let Some(&idx) = self.func_index.get(&dtor_name) {
        self.emit(OpCode::GetFrameBase, 0, loc);
        self.emit(OpCode::PushConst, offset, loc);
        self.emit(OpCode::Add, 0, loc);
        self.emit(OpCode::Call, idx, loc);
    }
}

/// 生成对栈上指定偏移处类对象的默认构造函数调用。
fn emit_class_default_ctor(&mut self, class_name: &str, offset: i32, loc: &SourceLoc) {
    let ctor_name = format!("__ctor__{}", class_name);
    if let Some(&idx) = self.func_index.get(&ctor_name) {
        self.emit(OpCode::GetFrameBase, 0, loc);
        self.emit(OpCode::PushConst, offset, loc);
        self.emit(OpCode::Add, 0, loc);
        self.emit(OpCode::Call, idx, loc);
    }
}
```

#### 7.3.4 正常块退出析构

当 Block 结束、调用 `exit_scope` 时，先逆序调用当前 scope 内所有类对象的析构函数，再恢复被 shadow 的外部变量：

```rust
fn exit_scope(&mut self) {
    if let Some(frame) = self.local_scope_stack.pop() {
        // 逆序调用析构函数（C++ 销毁顺序与构造顺序相反）
        for cv in frame.class_vars.iter().rev() {
            self.emit_class_dtor(&cv.class_name, cv.offset, &SourceLoc { line: 0, column: 0 });
        }
        for entry in frame.shadows { /* 恢复 shadow 变量 */ }
    }
}
```

#### 7.3.5 提前退出控制流的统一析构

`return`、`break`、`continue` 都会跳转出若干作用域。统一入口是 `emit_dtors_for_scope_exit(target_depth, loc)`：

```rust
/// target_depth 是目标 scope 在 `local_scope_stack` 中的索引：
/// - 0 表示函数最外层 block 之前的 scope（函数参数）
/// - 1 表示函数最外层 block
fn emit_dtors_for_scope_exit(&mut self, target_depth: usize, loc: &SourceLoc) {
    let current_depth = self.local_scope_stack.len();
    if current_depth == 0 || current_depth < target_depth {
        return;
    }
    // 收集待析构对象，先按 scope 从外到内收集，每层内部逆序
    let start_frame_idx = target_depth;
    let mut dtors: Vec<(String, i32)> = Vec::new();
    for frame_idx in (start_frame_idx..current_depth).rev() {
        let frame = &self.local_scope_stack[frame_idx];
        for cv in frame.class_vars.iter().rev() {
            dtors.push((cv.class_name.clone(), cv.offset));
        }
    }
    // 统一生成析构调用
    for (class_name, offset) in dtors {
        self.emit_class_dtor(&class_name, offset, loc);
    }
}
```

##### `return`

`return` 需要析构函数内所有活跃作用域中的对象，因此 `target_depth = 0`：

```rust
Stmt::Return { value, loc } => {
    if let Some(ref mut v) = value {
        // 1. 计算返回值（必须在析构之前，可能依赖局部对象）
        self.gen_expr(v);
        // 2. 析构所有活跃 scope
        self.emit_dtors_for_scope_exit(0, loc);
        self.emit(OpCode::Ret, 0, loc);
    } else {
        self.emit_dtors_for_scope_exit(0, loc);
        self.emit(OpCode::RetVoid, 0, loc);
    }
}
```

##### `break` / `continue`

`break` 和 `continue` 只退出当前循环内部的作用域，因此 `target_depth` 等于进入循环体时记录的 scope 深度：

```rust
Stmt::Break { loc } => {
    let target_depth = self.loop_scope_depths.last().copied().unwrap_or(1);
    self.emit_dtors_for_scope_exit(target_depth, loc);
    let ip = self.current_ip();
    self.emit(OpCode::Jump, 0, loc);
    self.break_patches.push(ip);
}

Stmt::Continue { loc } => {
    let target_depth = self.loop_scope_depths.last().copied().unwrap_or(1);
    self.emit_dtors_for_scope_exit(target_depth, loc);
    let ip = self.current_ip();
    self.emit(OpCode::Jump, 0, loc);
    self.continue_patches.push(ip);
}
```

循环进入时同步记录深度：

```rust
Stmt::While { cond, body, loc } => {
    // ...
    self.loop_scope_depths.push(self.local_scope_stack.len());
    self.gen_stmt(body);
    // ...
    self.loop_scope_depths.pop();
}
```

#### 7.3.6 嵌套作用域 LIFO 示例

```cpp
void bar() {
    A a;          // 深度 1
    {
        B b;      // 深度 2
        {
            C c;  // 深度 3
        }         // __dtor__C(&c)
    }             // __dtor__B(&b)
}                 // __dtor__A(&a)
```

若 `return` 发生在 `C c;` 之后：

1. `emit_dtors_for_scope_exit(0)` 收集深度 3、2、1 的所有类对象；
2. 收集顺序（由外到内，每层逆序）：`c`、`b`、`a`；
3. 生成调用：`__dtor__C(&c)` → `__dtor__B(&b)` → `__dtor__A(&a)`。

#### 7.3.7 与 TypeChecker 的协作

- TypeChecker 在 `cpp_class_layout.rs` 中为每个 class 注册显式析构函数，若未声明默认构造函数则**自动注册隐式默认构造函数** `__ctor__{ClassName}`。
- `cpp_overload.rs` 将 `Constructor` / `Destructor` / `Method` 体转换为普通 `FuncDecl`（mangled 名称）并追加到 `program.funcs`，供 BytecodeGen 统一生成代码。
- 隐式移动构造函数 `__ctor__{ClassName}__move` 在 Stage 5 自动生成，用于 `std::move` 初始化时的源对象资源转移。

#### 7.3.8 测试覆盖矩阵

| 测试 | 场景 | 预期 |
|------|------|------|
| `test_cpp_stack_ctor_dtor_basic` | 栈对象默认构造/析构 | 输出构造+析构顺序 |
| `test_cpp_nested_scope_dtors_lifo` | 嵌套作用域 LIFO | 输出 `21` |
| `test_cpp_early_return_dtors` | `return` 前自动析构 | 输出 `21` |
| `test_cpp_break_dtors` | `break` 前自动析构 | 输出 `12` |
| `test_cpp_continue_dtors` | `continue` 前自动析构 | 输出 `123` |
| `test_cpp_deep_nested_scope_raii` | 深层嵌套 `return` | 输出 `321` |
| `test_cpp_goto_with_dtor_scope` | `goto` 跳过 scope | 行为与 GCC -O0 一致 |

#### 7.3.9 与 AST 降解方案对比

| 维度 | AST 降解方案 | BytecodeGen 内嵌 ScopeFrame 方案（当前实现） |
|------|-------------|------------------------------------------|
| SourceLoc | 需要 Source Map 回查 | **天然保留**（析构调用 loc 指向原始跳转语句） |
| 实现位置 | 独立 `implicit_dtor.rs` | 内嵌 `codegen/mod.rs` + `codegen/stmt.rs` |
| 控制流 | 需处理全部跳转边 | 统一 `emit_dtors_for_scope_exit` 入口 |
| 嵌套/循环 | 复杂 | `loop_scope_depths` 栈直接索引 |
| 编译速度 | 慢（额外 AST 遍历） | 快（与代码生成同阶段完成） |



---

## 八、实施阶段

### 8.1 前置条件（已满足）

| 前置任务 | 状态 | 代码事实 |
|----------|------|----------|
| `static` 完整语义 | ✅ 已实现 | `codegen/mod.rs` 行 638-657 |
| `goto` | ✅ 已实现 | `typeck/mod.rs` 行 837-847 |
| `volatile` | ✅ 已实现 | `lexer.rs` 行 8 |
| `#ifdef` / `#ifndef` / `#endif` | ✅ 已实现 | `lexer.rs` 条件编译状态机 |
| `<limits.h>` / `<stdbool.h>` | ✅ 已实现 | `runtime_libc/include/` |
| `qsort` | ✅ 已实现 | `stdlib.h` 行 11 |
| `puts` / `sprintf` / `calloc` / `bsearch` | ✅ 已实现 | `stdlib.h` 行 4-5、12 |

**预估时间**：0 周（已满足）。

### 8.2 Phase 1：编译器核心 + 内置类型布局（4 周）

| 周 | 任务 | 产出 |
|----|------|------|
| W1 | AST 扩展（Type/Expr/Stmt 新增节点） | `ast.rs` 含 C++ 节点；新增 `Class`/`Reference`/`Auto`/`RValueRef` TypeKind |
| W2 | Lexer 扩展（C++ 关键字 + // 注释） | `lexer.rs` 识别 C++ Token；区分 C-auto 与 C++-auto |
| W3 | Parser 扩展（class / template / auto / 范围 for / new / delete） | 解析通过，生成 C++ AST |
| W4 | 内置容器类型布局表 | `builtin_layout.rs`；`vector<int>`/`string` 硬编码布局 |

### 8.3 Phase 2：TypeChecker + BytecodeGen 扩展（5 周）

| 周 | 任务 | 产出 |
|----|------|------|
| W5 | TypeChecker 扩展（auto 推导 + 引用语义） | `cpp_auto.rs` |
| W6 | BytecodeGen 扩展（this / new / delete） | `cpp_this_new_delete.rs` |
| W7 | 模板受限单态化（TypeChecker） | `cpp_monomorph.rs`；递归深度 ≤ 8 |
| W8 | 类布局分析 + BytecodeGen 虚函数调用 | `cpp_class_layout.rs`, `cpp_member_call.rs` |
| W9 | 范围 for + Lambda 轻量降解 | `cpp_container.rs`, `cpp_lambda.rs` |

### 8.4 Phase 3：容器库实现 + 集成（3 周）

| 周 | 任务 | 产出 |
|----|------|------|
| W10 | 参照 klib 手写 vec_int / vec_float / string | `runtime_libc/vitro/*.c` |
| W11 | 预编译容器库 + 方法映射替换 | `bytecode_libc_data.json` 含容器函数 |
| W12 | 容器方法调用一致性测试 | `tests/cpp_container/` |

### 8.5 Phase 4：高级特性 + 双模式（3 周）

| 周 | 任务 | 产出 |
|----|------|------|
| W13 | 右值引用 / 移动语义 | `cpp_overload.rs`（隐式移动构造） |
| W14 | unique_ptr / shared_ptr（简化版） | `cpp_dogfooding_test.rs` + `native/tests/dogfooding/` |
| W15 | CppImplicit 模式（全面作用域守卫） | `codegen/mod.rs`（`ScopeFrame` / `emit_dtors_for_scope_exit`）|

### 8.6 Phase 5：测试防线 + 教材回归（3 周）

| 周 | 任务 | 产出 |
|----|------|------|
| W16 | Host Contract 测试（C++ ↔ C 降解一致性） | `tests/cpp_lower/` |
| W17 | Bytecode Consistency 测试 | `tests/bytecode/` |
| W18 | Differential 测试 + 教材回归 | `tests/diff/`, 教材验证 |

**总计**：C++ 拓展 18 周 ≈ **4.5 个月**。

---

## 九、测试防线

延续 Vitro 的五层测试防线：

### 9.1 第一层：轻量降解审计测试

```cpp
// tests/cpp_light_lower/test_vector_basic.cpp
#include <vitro_vector>

int main() {
    vector<int> v;
    v.push_back(1);
    v.push_back(2);
    assert(v.size() == 2);
    assert(v[0] == 1);
    v.__destroy();
    return 0;
}
```

验证：
1. `--show-lowered` 输出轻量降解产物（范围 for 展开、Lambda 闭包 struct）
2. 轻量降解产物与手写等价 C 代码**逐字节一致**
3. TypeChecker 容器方法映射正确（`v.push_back` → `vitro_vec_push_int`）

### 9.2 第二层：Bytecode Consistency 测试

同一 C++ 源码在 Vitro 编译两次，生成的字节码逐指令一致。

### 9.3 第三层：Differential 测试

```cpp
vector<int> v;
v.push_back(3);
v.push_back(1);
v.push_back(4);
sort(v.begin(), v.end());
```

对比：Vitro 编译执行结果 vs GCC -O0 编译执行结果。数组顺序必须一致。

### 9.4 第四层：错误诊断测试

```cpp
vector<int> v;
auto x = v[100];   // 越界访问 → E3001 TrapBounds
```

验证：C++ 代码产生的错误码与 C 代码完全一致。

### 9.5 第五层：教材回归测试

选取国内主流教材/ OJ 题目（洛谷、PTA、LeetCode 简单题），用 Vitro C++ 子集重写，验证通过。

---

## 十、Dogfooding 与 C++ 容器验证

> **当前状态（2026-06-13）**：Stage 0/1/2 全部完成。`runtime_libc/vitro/*.c` 已在提交 `a16b489` 中全部替换为标准模板 C++ 实现（`.cpp`），旧 `.c` 文件已删除，force-instantiate 桩已删除。Stage 2 栈 RAII 已完成；Stage 3 `new[]/delete[]` 元素构造析构已完成；Stage 4 引用语义已完成；Stage 5 隐式移动构造已完成；Stage 6 `unique_ptr<T>` dogfooding 已完成。Stage 2b 内置容器全面迁移：`vector<int/float/char>` 合并为 `vector.cpp`，`list<int>` 合并为 `list.cpp`，`string` 改写为模板，通过 `template class vitro_vec<int>;` 显式实例化导出方法，`method_map` 指向 mangled 方法名。Dogfooding 测试 28 个全部通过；C++ E2E 60 个全部通过；C++ Shadow Verification 82 用例全部一致、0 gap；Parser/TypeChecker/BytecodeGen CPP 单元测试 104 个全部通过；clippy 0 警告。原 3 个 `compile_gap`（`cpp_rvalue_ref`、`cpp_const_ref_rvalue`、`cpp_range_for_ref_modify`）已消除。

### 10.1 Stage 0：验证 BytecodeGen（已完成 ✅）

开发团队用 Vitro C++ 子集写一个 class template 封装，调用 Stage 0 的 C 容器函数：

```cpp
// tests/dogfooding/stage0_vec_cpp_wrapper.h
// 验证 Vitro C++ BytecodeGen 是否能正确生成对 C 容器的调用

template<class T>
class vector {
public:
    vector() { vitro_vec_init(this); }
    void push_back(T x) { vitro_vec_push(this, x); }
    T get(size_t i) { return vitro_vec_get(this, i); }
    size_t size() { return vitro_vec_size(this); }
    ~vector() { vitro_vec_destroy(this); }
};
```

**注意**：不使用运算符重载（已排除），使用显式 `get()` 方法。

**验证流程**：
1. 用 Vitro C++ 编译器编译 `stage0_vec_cpp_wrapper.h` → 生成字节码 A
2. 手写等价的 C 代码（直接调用 `vitro_vec_*`）→ 生成字节码 B
3. 对比 A ≡ B（逐指令一致）→ 证明 BytecodeGen 正确

### 10.2 Stage 1：Dogfooding 验证（已完成 ✅）

当 Vitro C++ 编译器成熟后，用 Vitro C++ 子集写**不依赖 C 容器函数**的纯 C++ 容器：

```cpp
// tests/dogfooding/stage1_vec_cpp.h
// 目标：用 Vitro C++ 子集实现一个自包含的 vector<int>
// 不调用任何 vitro_vec_* C 函数，完全用 C++ 语法实现

template<class T>
class vector {
    int size_;
    int capacity_;
    T* data;
public:
    vector() : size_(0), capacity_(0), data(nullptr) {}
    
    void push_back(T x) {
        if (size_ >= capacity_) {
            int new_cap = capacity_ == 0 ? 4 : capacity_ * 2;
            T* new_data = new T[new_cap];  // Vitro new → HostMalloc
            for (int i = 0; i < size_; i++) new_data[i] = data[i];
            delete[] data;                 // Vitro delete → HostFree
            data = new_data;
            capacity_ = new_cap;
        }
        data[size_++] = x;
    }
    
    T get(int i) { return data[i]; }
    int size() { return size_; }
    
    ~vector() { delete[] data; }
};
```

**Dogfooding 验证流程**：
1. 用 Vitro C++ 编译器编译 `stage1_vec_cpp.h` → 生成字节码 C
2. 对比 C ≡ B（Stage 0 的 C 版本字节码）
3. 如果 C ≡ B → **Dogfooding 通过**，Vitro C++ 编译器可以正确编译 C++ 项目（容器库）
4. 进入 Stage 2：用 `stage1_vec_cpp.h` 替换 `vitro_vec_int.c`，删除 C 实现（已于提交 `a16b489` 完成）

**Dogfooding 完成结论**：
- `vector<int/float/char>`、`string`、`list<int>`、`sort_int` 的 C++ 模板实现运行时 stdout 与 C 基线完全一致。
- `get`/`size` 等简单访问方法在 C++ class 与 C struct 字段布局对齐后逐指令等价。
- `push_back`/`pop_back` 等因算法实现不同（`new[]/delete[]` + 循环复制 vs `realloc`）字节码不等价，但运行结果一致。
- Dogfooding 是编译器正确性的**终极测试**，已通过后进入 Stage 2。

---

## 十一、风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| 模板单态化爆炸 | 中 | 编译时间/内存爆炸 | 仅支持类型参数，限制递归模板深度 ≤ 8 |
| BytecodeGen 复杂度膨胀 | 中 | 新增 C++ 节点分支难以维护 | 模块化拆分（`cpp_member_call.rs` / `cpp_lambda.rs`），单测覆盖 |
| CppImplicit 作用域守卫 bug | 中 | 内存泄漏/双重释放 | 严格 CFG 分析 + 单元测试覆盖所有跳转组合 |
| AST serde 兼容性破坏 | 中 | 快照序列化失败（Flutter Bridge 已随前端切割迁出） | 新增变体向后兼容测试；若失败则追加 serde 版本号 |
| 学生混淆 C/C++ 语法 | 高 | 教学体验下降 | IDE 层面区分模式提示，错误消息标注 "C++ 模式" |
| klib 参照实现偏差 | 低 | 容器行为与预期不一致 | 逐函数对照 klib 源码，Differential 测试验证 |
| `Auto` Token 语义冲突 | 中 | C-auto 与 C++-auto 解析歧义 | Parser 根据上下文区分（C 模式 vs C++ 模式） |
| Dogfooding 失败 | 低 | C++ 编译器无法正确编译 C++ 容器 | 已通过 Stage 1/2 验证；若未来失败可保留 Stage 0 C 容器作为 fallback |

---

## 十二、里程碑

| 里程碑 | 时间 | 验收标准 |
|--------|------|----------|
| M1：编译器核心就绪 | T+2 周 | class / auto / 范围 for / new / delete 解析通过，AST 结构正确 |
| M2：TypeChecker 扩展完成 | T+5 周 | auto 推导、模板单态化、类布局分析通过全部单元测试 |
| M3：BytecodeGen 扩展完成 | T+7 周 | this / MemberCall / 虚函数调用字节码与手写 C 一致 |
| M3.5：Stage 0.5 容器收口完成 | T+0 周 | `list<int>` / `vector<char>` / `sort_int` 测试绿；`CPP_FAILURES.md` 创建；C++ Parser/TypeChecker/BytecodeGen 三 tier 纳入 CI；55/55 C++ 单元测试通过 |
| M4：容器库集成完成 | T+10 周 | vector<int> / string 预编译通过，`v.push_back` 生成正确字节码 |
| M4.5：Stage 2 栈 RAII 完成 | T+1 周 | 局部类对象自动调用默认构造函数；scope exit / return / break / continue 自动按 LIFO 调用析构函数；嵌套 scope + early return + loop 跳转测试全绿 |
| M4.6：Stage 3 `new[]/delete[]` 元素构造析构完成 | T+0 周 | `new A[n]` 逐元素调用构造函数；`delete[]` 从 `base[-4]` 读取 count 并逆序调用析构函数；临时变量槽位扩展至 4 个；新增 2 个数组构造析构测试全绿 |
| M5：高级特性完成 | T+13 周 | **移动语义 ✅（隐式移动构造函数自动生成）** / **`unique_ptr<T>` ✅（简化版 dogfooding）** / **CppImplicit 模式 ✅（全面作用域守卫，块结束 / return / break / continue / 嵌套作用域 LIFO 析构）** |
| **M6：测试防线完成** | **T+16 周** | **✅ 五层测试防线全部通过，59 道 C++ 教材/OJ 题目回归通过（超过计划的 50 道）** |
| M7：Beta 发布 | T+18 周 | 内部试用，收集反馈 |
| M8：正式发布 | T+22 周 | 文档完整，教学场景验证通过 |
| **M9：容器 Dogfooding（Stage 1）** | **T+26 周** | **✅ 用 Vitro C++ 编译器编译 C++ 容器源码，运行时 stdout 与 C 版本一致，`get`/`size` 等方法逐指令等价** |
| **M10：全面迁移（Stage 2）** | **T+30 周** | **✅ 运行时库全部替换为 C++ 实现，删除所有手写 C 容器；`method_map` 指向 mangled 方法名** |

---

## 十三、Vitro 平台内用户体验保障

### 13.1 SourceLoc 天然保留

**优势**：BytecodeGen 直接处理 C++ AST 节点，`&loc` 始终是原始 C++ 代码位置。

```rust
Expr::MemberCall { object, method, is_virtual: true, loc, .. } => {
    // ... 生成 vtable 间接调用 ...
    self.emit(OpCode::CallPtr, args.len() as i32 + 1, &loc);
    // loc = 原始 C++ 代码位置，VM trap 直接指向 obj.area() 行号
}
```

### 13.2 编译速度目标

| 指标 | 目标 | 测试方法 |
|------|------|----------|
| C++ 编译时间 / C 编译时间 | `< 1.5x` | 同规模代码对比 |
| TypeChecker 扩展耗时 | `< 20%` 总编译时间 | `--profile` 输出 |
| 单文件编译延迟 | `< 30ms`（教学代码量级） | 100 行 C++ 代码测试 |

### 13.3 CLI 集成

```bash
# C 用户：零变化
vitro_cli compile hello.c
vitro_cli run hello.c
vitro_cli step hello.c

# C++ 用户：扩展名自动检测，命令完全一致
vitro_cli compile hello.cpp        # 自动启用 C++ 模式
vitro_cli run hello.cpp            # 运行
vitro_cli step hello.cpp           # 源码级调试

# 教学专用
vitro_cli compile hello.cpp --show-lowered   # 显示降解产物
vitro_cli compile hello.cpp --show-ast       # 显示 C++ AST
```

---

## 十四、附录

### A. 错误码预留

| 范围 | 用途 | 当前最大 |
|------|------|----------|
| E1001~E1013 | Lexer 错误 | E1013 |
| E2001~E2008 | Parser 错误 | E2008 |
| E3001~E3071 | TypeChecker / 运行时错误 | E3071 |
| **E4001~E4999** | **C++ 扩展预留** | — |
| W3050~W3057 | Warning / Hint | W3057 |

### B. 类型映射表完整版

详见 `native/src/compiler/cpp_frontend/type_map.rs`（已实现，覆盖 vector<int/float/char>、list<int>、string）。

### C. 相关文档

| 文档 | 路径 | 说明 |
|------|------|------|
| C 子集规范 | `docs/current/03-语言子集/C语言子集规范.md` | C++ 拓展的前置语法基座 |
| 标准库支持矩阵 | `docs/current/04-标准库与防线/标准库支持矩阵.md` | 容器库集成的测试防线定义 |
| 错误码体系 | `native/src/diagnostics/error_codes.rs` | 新增 C++ 诊断码预留区间 E4001~E4999 |

---

**计划制定**: Kimi Code CLI  
**状态更新**: 2026-06-13 — M6 完成，10 项边界清零；Stage 2b 彻底迁移完成（本段为历史状态）  
**后续进展**: 见文档头部 v2.9 状态——Phase 34~41 已完成，**Phase 42 进行中**。详见 `docs/archive/ARCHIVE_C++容器模板迁移笔记.md`。

---

## 十五、Stage 2b 实施约束与最终方案

> 记录时间：2026-06-13  
> 记录位置：`docs/current/03-语言子集/C++拓展实施计划.md`

`native/runtime_libc/vitro/*.c` 手写容器迁移为纯 C++ 实现的工作已完成（提交 `a16b489`）。本节记录迁移过程中遇到/曾遇到的编译器/工具链约束，以及最终采用的解决方案。

### 15.1 最终目录结构

```
native/runtime_libc/vitro/
├── vector.cpp   # template <class T> class vitro_vec<T>；显式实例化 int/float/char
├── list.cpp     # template <class T> class vitro_list<T> / vitro_list_node<T>；显式实例化 int
├── string.cpp   # template <class T> class vitro_string<T>；显式实例化 char
└── sort_int.cpp # 自由函数模板 vitro_sort_int<T>（仍用调用桩触发实例化）
```

已删除：所有 `.c` 实现（`vec_int.c` / `vec_float.c` / `vec_char.c` / `list_int.c` / `string.c` / `sort_int.c`）以及早期 `.cpp` 单类型实现（`vector_int.cpp` 等）。

### 15.2 曾被认为不支持但已解决的约束

| 约束 | 原状态 | 解决方式 | 验证测试 |
|------|--------|----------|----------|
| 类外方法定义 `Class::method()` | ❌ 不支持 | Parser 已实现类外方法定义解析，支持 `Bar::set`、`Box<T>::set` 形式 | `test_parser_cpp_member_func_outside_class`<br>`test_cpp_out_of_line_method_definition` |
| 类模板显式实例化 `template class X<Y>;` | ❌ 不支持 | Parser 已支持类模板显式实例化 | `test_cpp_explicit_class_template_instantiation` |
| force-instantiate 桩强制导出方法 | 临时必要 | 通过显式实例化替代，已删除所有容器 force-instantiate 桩 | `cpp_dogfooding_test`（28 个全绿） |

### 15.3 仍然有效的约束与当前写法

| 约束 | 说明 | 当前缓解写法 |
|------|------|--------------|
| `T()` 值初始化不被支持 | Parser 将 `T()` 解析为对非函数指针的调用 | 使用 `(T)0` 对 POD 类型做零初始化 |
| 函数模板显式实例化不支持 | 仅类模板显式实例化已支持 | `sort_int.cpp` 保留 `__vitro_force_instantiate_vitro_sort_int()` 调用桩 |
| 同一模板类不能跨文件重复定义 | Bytecode Libc 多文件一起编译 | `vector<int/float/char>` 合并到单个 `vector.cpp` |
| 嵌套模板类需显式实例化 | TypeChecker 不会自动注册隐式实例化的节点类 | `list.cpp` 末尾同时写 `template class vitro_list_node<int>;` |
| `method_map` 必须指向 mangled 方法名 | MemberCall 路径生成 `Class__method` | `extract_cpp_builtin_layout.py` 已输出 mangled 名；`bytecode_libc_sig.rs` 已同步 |

### 15.4 布局提取与预编译流程

1. `extract_cpp_builtin_layout.py` 从 `.cpp` 提取类布局，输出 `native/src/compiler/cpp_frontend/builtin_layout_data.json`。
2. 脚本已升级为 brace-aware，能正确区分 class 顶层字段与方法体内的局部变量。
3. 脚本内置 `FILE_RULES`，将 `vitro_vec`/`vitro_list`/`vitro_string` 等模板基名映射到用户可见名（`vector<int>` / `list<int>` / `string`）。
4. `scripts/precompile_bytecode_libc` 编译 `runtime_libc/vitro/*.cpp`，生成 `bytecode_libc_data.json` / `bytecode_libc_index.rs`。
5. 由于 `vitro_cli export` 会链接已嵌入的 Bytecode Libc，迁移过程中需删除旧 `.c` 文件后重新预编译，确保产物干净。

### 15.5 验证结果

- **Dogfooding 测试**：28 个全部通过（`cargo test --test cpp_dogfooding_test`）
- **C++ E2E 测试**（2026-06-13 as-of，M6 交付时点）：81 个全部通过（`cargo test --test vitro_e2e cpp`）
- **C++ Shadow Verification**（2026-06-13 as-of，M6 交付时点）：97 个用例（95 一致 + 2 个已记录 `clang_compile_fail`），0 gap（原 3 个 gap 已消除）
- **Parser/TypeChecker/BytecodeGen CPP 单元测试**：33 + 28 + 40 = 101 个全部通过
- **clippy**：0 警告

更多实现细节见 `docs/archive/ARCHIVE_C++容器模板迁移笔记.md`。

---

**计划制定**: Kimi Code CLI  
**状态更新**: 2026-06-13 — M6 完成，10 项边界清零；Stage 2b 彻底迁移完成（本段为历史状态，当前进展见文档头部 v2.9：Phase 42 进行中）
