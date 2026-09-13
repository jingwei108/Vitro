# Vitro C++14 教学子集规范

> 目标：让学生用接近标准 C++14 的语法学习编程，同时明确知道 Vitro 支持哪些特性、边界在哪里。
>
> 本文档面向学生与教师；实现细节见 `docs/current/03-语言子集/C++拓展实施计划.md`。

---

## 1. 设计原则

### 1.1 Honest Subset（诚实子集）

Vitro C++ 子集**不是**假装成完整的标准 C++，而是诚实标注：

- 支持哪些核心教学特性
- 明确不支持哪些工业级特性
- 标准 C++ 写法在 Vitro 中的等价行为

### 1.2 零改动 VM

所有 C++ 特性最终都被编译成 VitroVM 字节码。VM 本身不做任何修改，保证 C 与 C++ 共享同一运行时。

### 1.3 教学优先

优先支持国内考研/竞赛/工程教学中最高频的语法：

| 教学价值 | 需要的语法 | 是否支持 |
|:---|:---|:---|
| 面向对象入门 | class / this / public / private | ✅ |
| 资源管理 | 构造函数 / 析构函数 / RAII | ✅ |
| 泛型入门 | 模板类 / 模板函数 | ✅ |
| 现代 C++ | auto / 范围 for / Lambda | ✅ |
| 智能指针 | unique_ptr（简化版） | ✅ |
| 标准容器 | vector<int> / list<int> / string（Vitro 内置实现） | ✅ |
| 运算符重载 | `operator+` 等 | ❌ 明确排除 |
| 异常处理 | try / catch / throw | ❌ 明确排除 |

---

## 2. 支持的语法

### 2.1 类与对象

```cpp
#include <stdio.h>

class Point {
public:
    int x;
    int y;
    int sum() { return x + y; }  // 成员函数
};

int main() {
    Point p;       // 自动调用默认构造函数
    p.x = 3;
    p.y = 4;
    printf("%d\n", p.sum());
    return 0;
}
```

**支持细节**：
- `class` / `struct`（`struct` 默认 public，`class` 默认 private）
- `public:` / `private:` 访问说明符
- 成员变量与成员函数
- 成员函数重载（**按参数类型区分**，含同参数个数、仅类型不同的重载，如
  `void show(int)` 与 `void show(double)` 并存；mangled 名带参数类型编码，
  `2026-09-12` 修复——此前同形重载会静默错派发并运行时"栈下溢"trap）
- `this` 指针
- 单继承 + `public` 继承
- 虚函数与多态（v-table 实现）

**当前限制**：
- 无匹配重载时报 `E4026 AmbiguousMethodCall` 编译诊断（不再静默放行）；
- 自由函数（类外）**不支持**重载：同名不同签名报 `E3003`（签名不一致）。

### 2.2 构造函数与析构函数

```cpp
class Box {
public:
    int x;
    Box() { x = 0; }            // 默认构造函数
    Box(int v) { x = v; }       // 带参构造函数
    ~Box() {}                   // 析构函数
};

int main() {
    Box a;          // Box()
    Box b(10);      // Box(int)
    return 0;
}   // 局部对象按构造顺序逆序自动析构
```

**支持细节**：
- 构造函数重载（按参数个数区分）
- 析构函数自动调用（块结束 / return / break / continue）
- 构造函数初始化列表：`Box() : x(0) {}`
- 隐式默认构造函数（类无自定义构造时自动生成）

**当前限制**：
- 不支持同参数个数但参数类型不同的构造函数重载（如 `Box(int)` 与 `Box(float)` 并存会报 `E4031`）
- `explicit` 关键字可识别，但隐式转换拒绝语义待完善

### 2.3 引用

```cpp
int a = 10;
int& r = a;            // r 是 a 的别名
const int& cr = 5;     // const 引用可绑定右值

void inc(int& x) { x++; }
```

**支持细节**：
- `T&` 左值引用
- `const T&` 常量引用
- 引用参数与引用返回值
- 引用自动解引用

### 2.4 auto 类型推导

```cpp
auto a = 10;           // int
auto& r = a;           // int&
auto p = new int(5);   // int*
```

### 2.5 范围 for

```cpp
int arr[] = {1, 2, 3, 4, 5};
for (auto x : arr) { printf("%d\n", x); }
for (auto& x : arr) { x++; }  // 引用形式可修改元素
```

### 2.6 Lambda

```cpp
int base = 10;
auto f = [base](int x) { return base + x; };
printf("%d\n", f(5));  // 15
```

**支持细节**：
- 值捕获、`&` 引用捕获、`=` 隐式值捕获、`&` 隐式引用捕获
- 多捕获：`[a, &b]`、`[this]`
- Lambda 作为参数传递

### 2.7 模板

```cpp
template<class T>
T add(T a, T b) { return a + b; }

template<class T>
class Pair {
public:
    T first, second;
};

int main() {
    printf("%d\n", add<int>(3, 4));
    Pair<int> p;
    p.first = 1;
    p.second = 2;
    return 0;
}
```

**支持细节**：
- 函数模板与类模板
- 仅类型模板参数
- 模板显式实例化：`template class Pair<int>;`
- 递归深度 ≤ 8

**当前限制**：
- 不支持函数模板显式实例化语句（仅类模板支持）
- 不支持 SFINAE、模板特化、偏特化

### 2.8 new / delete

```cpp
int* p = new int(42);      // 等价于 malloc + 初始化
delete p;                  // 等价于 free

int* arr = new int[10];    // 分配数组
delete[] arr;              // 释放数组
```

**说明**：`new` / `delete` 内部映射为 VM 的 Host Malloc/Free，教学层面可与 C 的 malloc/free 对照理解。

### 2.9 简化版 unique_ptr

```cpp
template<class T>
class unique_ptr {
    T* p;
public:
    unique_ptr() : p((T*)0) {}
    unique_ptr(T* ptr) : p(ptr) {}
    unique_ptr(unique_ptr<T>&& o) : p(o.p) { o.p = (T*)0; }  // 移动构造
    T* get() { return p; }
    T* release() { T* t = p; p = (T*)0; return t; }
    ~unique_ptr() { if (p) delete p; }
};

int main() {
    unique_ptr<int> p(new int(42));
    printf("%d\n", *p.get());

    unique_ptr<int> q = std::move(p);  // 调用移动构造，p 置空
    printf("%d\n", *q.get());
    printf("%d\n", p.get() ? 1 : 0);   // 0

    int* r = q.release();              // 释放所有权
    delete r;
    return 0;
}
```

**支持细节**：
- 默认构造、从指针构造、析构时自动 `delete`
- `get()` 获取底层指针，`release()` 释放所有权并将内部指针置空
- 隐式移动构造：类含指针/资源字段时，Vitro 自动生成 `__ctor__{Class}__move`；`std::move` 初始化会调用移动构造，源对象指针字段置空，防止双重释放

**说明**：这是教学简化版，未实现工业级 `reset()` / `swap()` / 自定义删除器，但已覆盖 RAII 与所有权转移核心思想。

### 2.10 内置容器

Vitro 内置了 `vector<int/float/char>`、`list<int>`、`string`，用法接近 STL：

```cpp
#include <stdio.h>

// Vitro 内部将 vector<int> 映射到内置实现
int main() {
    vector<int> v;
    v.push_back(3);
    v.push_back(1);
    v.push_back(4);
    for (int i = 0; i < v.size(); i++) {
        printf("%d\n", v.get(i));
    }
    return 0;
}
```

**注意**：Vitro 子集不支持运算符重载，因此 `v[i]` 写作 `v.get(i)`、`v.size()` 是显式方法调用。

### 2.11 指针算术与复合赋值

C++ 子集继承 C 子集的指针运算能力：

```cpp
#include <stdio.h>

int main() {
    int arr[5] = {10, 20, 30, 40, 50};
    int* p = arr;
    p += 2;        // 等价于 p = p + 2；按 sizeof(int) 缩放
    printf("%d\n", *p);  // 30
    p -= 1;        // 指向 arr[1]
    printf("%d\n", *p);  // 20
    return 0;
}
```

**支持细节**：
- 指针 `++` / `--` / `+ n` / `- n` 已支持，自动按 pointee 大小缩放。
- 指针 `+=` / `-=` 整数已支持（2026-06-28），覆盖普通数据指针、`char*`、`void*`（按 1 字节，GCC/Clang 扩展行为）、多级指针、结构体成员指针。
- 函数指针算术、指针与指针的 `+=` / `-=`、以及其他复合赋值运算符（`*=`、`/=` 等）不支持，会报 `E3045_CompoundAssignType`。

---

## 3. 明确不支持的特性

| 特性 | 原因 | 遇到时报错 |
|:---|:---|:---|
| 运算符重载 | 教学价值低，复杂度极高 | `E4002 OperatorOverloadNotSupported` |
| 异常（try/catch/throw） | VM 不支持栈展开 | `E4001 ExceptionNotSupported` |
| 多重继承 | 超出教学子集范围 | `E4005 MultipleInheritanceNotSupported` |
| RTTI（typeid/dynamic_cast） | VM 不支持 | 解析/类型错误 |
| `namespace` / `using namespace` | 当前不支持命名空间 | `E4006 NamespaceNotSupported` / `E4011 UsingDirectiveNotSupported` |
| SFINAE / 模板特化 / 偏特化 | 超出教学子集范围 | `E4003 TemplateSpecializationNotSupported` |
| 多线程 | VM 单线程 | `E4004 ThreadNotSupported` |
| `constexpr` / `consteval` | 超出教学子集范围 | `E4017 ConstexprNotSupported` |
| `volatile` 成员函数 | 超出教学子集范围 | `E4006` 系列 |

---

## 4. 与标准 C++ 的差异

### 4.1 标准头文件映射

```cpp
#include <vector>    // 映射到 Vitro 内置 vector
#include <list>      // 映射到 Vitro 内置 list
#include <string>    // 映射到 Vitro 内置 string
#include <algorithm> // sort 等算法
```

Vitro 会擦除 `std::` 前缀，因此 `std::vector<int>` 等价于 `vector<int>`。

### 4.2 容器接口差异

由于不支持运算符重载，部分接口与 STL 不同：

| STL 写法 | Vitro 子集写法 |
|:---|:---|
| `v[i]` | `v.get(i)` |
| `v.push_back(x)` | `v.push_back(x)` ✅ |
| `v.size()` | `v.size()` ✅ |
| `*p`（unique_ptr）| `*p.get()` |
| `std::move(p)` | 调用自动生成的移动构造函数，源对象指针字段置空 |

### 4.3 nullptr

`nullptr` 当前被解析为值为 0 的整数常量，而非真正的 `std::nullptr_t`。`sizeof(nullptr)` 返回 `sizeof(int)`。

建议教学代码中暂时使用 `(int*)0` 或 `0` 初始化空指针。

### 4.4 值初始化

`T()` 值初始化当前不支持，会解析为函数调用错误。POD 类型请用 `(T)0`。

### 模板活约束（G13，2026-09-12 补记）

- **同一模板类不可跨文件重复定义**：多文件工程中，模板类（如 `vector<int>` 的
  单态化定义）只能在一个编译单元出现一次；跨文件重复 `#include` 同一份模板类
  定义会触发重定义错误。单文件教学场景不受影响。
- **`T()` 值初始化不支持**（见上）：泛型代码中请以显式赋值或带参构造替代。

### 4.5 指针赋值的方向语义（2026-09-11 修订，含诚实记录）

| 场景 | Clang++（`-std=c++14 -Wall -Wextra`，实测） | Vitro | 判定 |
|---|---|---|---|
| **向上转型**<br>`Base* b = new Derived();` | 零警告（多态的基础写法） | 零警告 —— 修复前误报 `W3053`："不兼容的指针类型赋值：Base* ← Derived*" + 建议"隐式类型转换可能导致数据截断" | ✅ **已对齐（2026-09-11）** |
| **向下转型**<br>`Base* b; Derived* d = b;` | **error**：`cannot initialize a variable of type 'Derived *' with an lvalue of type 'Base *'`（拒绝编译） | **警告 `W3067`**："指针类型不兼容：Derived* ← Base*，需要显式转换（向上转型 Derived* → Base* 除外）"，**仍继续编译** | ⚠️ **剩余差异**：Vitro 不阻断编译，严格度低于 Clang；教学时需口头强调必须写 `static_cast`/`dynamic_cast` |
| **无关类型**<br>`double* p; int* q = p;` | 警告 `incompatible pointer types …` | 警告 `W3067`（同上文案） | ✅ 级别一致（措辞不同） |

**修复记录**：此前两种情况共用**标量**转换码 `W3053_ImplicitScalarConversion`（语义是"隐式标量转换 /
可能导致数据截断"），把多态基础讲成了危险操作；且该差异当时未记录在本规范中（违反项目
"以 Clang 为标准、任何不一致必须记录"的纪律）。2026-09-11 新增专用码 `W3067_PointerTypeMismatch`
（`vitro_shared::ErrorCode`，附错误目录条目与建议文案），并在 `TypeChecker::is_upcast` 中实现
**单继承链可达性判定**（带步数上限防环）以区分上/下转型。回归：`native/tests/pointer_upcast_test.rs`（3 用例）。

> **交叉引用**：本项的发现、根因定位与修复批次记录见
> [`ARCHIVE_代码审查复核20260911.md`](../../archive/ARCHIVE_代码审查复核20260911.md —— §1 逐条复核结论（P1-6 行）、
> §2.2「P1-6：Vitro 输出的是警告 `W3053`，不是 `E3053`」与 §4 修复批次
> 「批次 E（2026-09-11）：P1-6（向上转型误报）」。

**已知显示瑕疵（未修，如实记录）**：诊断 JSON 的 `code` 字段与 CLI 输出对**警告**也加 `E` 前缀
（`W3067` 显示为 `E3067`）；`severity` 字段本身正确（`warning`）。修复需调整 `session_api::compile`
的 code 前缀生成规则（按 severity 输出 `E`/`W`/`H`），属独立小项。

---

## 5. 教学建议

### 5.1 从 C 过渡到 C++

1. **先巩固 C 基础**：变量、数组、指针、函数、struct
2. **引入 class**：把 struct 升级为 class，理解封装
3. **引入构造函数/析构函数**：理解 RAII
4. **引入模板**：理解泛型编程
5. **引入现代 C++**：auto、范围 for、Lambda、unique_ptr

### 5.2 常见教学代码

```cpp
// 示例：用 vector<int> 存储并排序
#include <stdio.h>
#include <vector>

int main() {
    vector<int> v;
    v.push_back(3);
    v.push_back(1);
    v.push_back(4);
    for (int i = 0; i < v.size(); i++) {
        printf("%d\n", v.get(i));
    }
    return 0;
}
```

### 5.3 调试提示

- 编译错误会显示原始 C++ 代码行号
- 遇到 `E4031` 说明构造函数重载存在歧义，请检查是否有同参数个数不同类型的构造函数
- 容器方法不存在时，请使用 `get()`、`size()`、`push_back()` 等显式方法

---

## 6. 相关文档

| 文档 | 说明 |
|:---|:---|
| `docs/current/03-语言子集/C语言子集规范.md` | C 语言子集规范 |
| `docs/current/03-语言子集/C++拓展实施计划.md` | C++ 扩展技术路线图 |
| `docs/archive/ARCHIVE_代码审查复核20260911.md` | 代码审阅报告（C++ 子集项见 §2.2 与「批次 E」） |
| `docs/archive/ARCHIVE_M7_BETA_READINESS.md` | M7 Beta 状态检查清单（原 `docs/archive/ARCHIVE_M7_BETA_READINESS.md`，已归档） |
| `native/tests/CPP_FAILURES.md` | C++ 已知失败与偏差记录 |

---

**最后更新**: 2026-09-11 — 文档翻新：页脚推进 + 补记 §4.5 指针赋值方向语义与 `W3067_PointerTypeMismatch`（2026-09-11 修订）+ 增补 `ARCHIVE_代码审查复核20260911.md` 批次 E 交叉引用与失效文档路径修正
