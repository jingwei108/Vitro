# C 语言子集规范（教学场景专用）

> 核心问题：对于一个教学场景，C 子集应该支持到什么程度？
> 文档状态：**现行有效**（教学 C 子集的行为契约；与 Clang 的差异按"诚实记录"原则逐条保留）
> 最后核对日期：2026-09-11
> 修订说明（2026-09-11）：去前端化——入口口径改为"三出口一核心"（capi / wasm32 / serve），补文档状态与核对日期，§2.9 补堆分配决议的反向链接。带日期的历史补记与修订条目保持原样。

---

## 1. 设计原则

### 1.1 最小必要集（Minimum Viable Subset）

**目标**：用最少的语法，覆盖 C 语言最核心的教学价值。

| 教学价值 | 需要的语法 | 是否必须 |
|:---|:---|:---|
| 程序的基本结构 | 变量、表达式、语句 | ✅ 必须 |
| 算法思维 | if/else、循环、函数、递归 | ✅ 必须 |
| C 语言的灵魂 | 指针（&、*） | ✅ 必须 |
| 复合数据 | 数组、struct | ✅ 必须 |
| 内存管理 | malloc/free | ✅ 必须 |
| 底层原理 | 内存布局、栈/堆/指针关系 | ✅ 必须 |

### 1.2 排除原则

**排除标准**：
1. 会分散初学者注意力的细节（如 printf 的格式化字符串）
2. 增加编译器复杂度但教学价值低（如 double 精度问题）
3. 可以用现有语法等价表达的（如 break/continue 可用 return 替代）
4. 增加实现复杂度但教学价值已在其他方式覆盖的（如 bitfield——可用普通成员 + 位运算等价表达；全局 VLA——局部 VLA 已覆盖运行时定界语义）

> **历史勘误（2026-09-14，U1#11 随批修正 D-1）**：本条旧版曾把"完整预处理器、
> 自定义头文件"列为排除例——两者已由 E2 批次（模块化预处理器内核，见 §2.11）
> 全量实现，该例子作废。

---

## 2. 支持的语法（Phase 1 MVP）

### 2.1 数据类型

```c
// 标量类型：int（32位有符号整数）、char（8位字符，按 i32 存储）
int a;
int a = 5;
char c = 'A';
char c = 65;   // char 与 int 可隐式转换（带警告）

// 无符号整数（语义上与 int 相同，教学子集不区分有/无符号）
unsigned u = 5;
unsigned int v = 10;

// 一维数组（大小必须是编译期常量或省略）
int arr[10];
int arr[] = {1, 2, 3, 4, 5};  // 自动推断大小为 5
char s[] = "hello";           // 字符串初始化 char 数组，自动推断大小为 6（含 '\0'）

// 多维数组（支持嵌套初始化列表和函数参数传递）
int mat[3][3] = { {1,2,3}, {4,5,6}, {7,8,9} };
void foo(int m[][3]) { m[0][0] = 1; }

// 变长数组 VLA（C99，局部作用域，运行时栈分配）
int n = 5;
int arr[n];                  // 一维 VLA
int mat[n][3];               // 多维 VLA（混合常量维度）
int mat2[n][m];              // 全 VLA 多维
printf("%d", sizeof(arr));   // VLA 的 sizeof 运行时计算
void bar(int n, int a[n]);   // 函数参数 VLA 自动退化为指针

// 函数按值返回结构体
struct S make_s(int x) {
    struct S s;
    s.x = x;
    return s;
}
struct S s = make_s(5);      // 赋值
int v = make_s(5).x;         // 直接成员访问

// 多级指针
int** pp;
int x = **pp;
pp[0] = &x;
pp = (int**)malloc(4);

// 指针
int* p;
int* p = &a;      // 取地址
int* p = malloc(4);  // 动态分配（4 = sizeof(int)）
char* str = "hello"; // 字符串字面量退化为 char*（用于 printf/scanf）

// 结构体
struct Node {
    int val;
    struct Node* next;
};
struct Node node;     // 值语义（简化：不需要理解 struct 拷贝）
struct Node* np;      // 指针语义

// 结构体初始化（新增）
struct Node n = {10, 0};      // 完整初始化
struct Node m = {5};          // 部分初始化（剩余字段自动为 0）
struct Node a = {1, 0};
struct Node b = {2, &a};      // 初始化列表中可使用取地址表达式
struct Node c = {.val = 10, .next = 0};  // Designated Initializer

// 数组 Designated Initializer
int arr[5] = {[0] = 1, [3] = 4};  // 稀疏初始化，未指定元素自动为 0

// 枚举（编译期常量，底层为 int）
enum Color { Red, Green, Blue };
enum Color { Red, Green = 2, Blue };  // 可显式指定值

// 类型别名
typedef int MyInt;
typedef int* IntPtr;
```

**设计决策**：
- **int 为主，char 为辅**：char 用于字符串教学；char 本质是小整数，按 i32 存储
- **一维数组**：足够演示排序、搜索等算法
- **多维数组**：支持二维数组声明、嵌套初始化列表、索引访问和函数参数传递（如 `int[][3]`）
- **数组/字符串初始化**：支持 `{1,2,3}` 和 `"hello"` 两种初始化方式，自动推断大小
- **基本指针**：&（取地址）、*（解引用）是 C 的灵魂，必须支持
- **多级指针**：`int**`、`struct Node**` 等，支持解引用、取地址、数组索引、指针算术、显式 cast
- **struct**：链表、树等数据结构的基础；支持按值返回（Hidden Return Pointer ABI）
- **VLA（变长数组）**：C99 局部变长数组，运行时栈分配；支持一维/多维、sizeof 运行时求值、函数参数退化
- **enum**：编译期计算常量值，生成 VitroVM 全局常量，便于教学演示状态机
- **typedef**：简化复杂类型声明，提升代码可读性

### 2.2 语句

```c
// 变量声明（支持每行多个变量）
int a;
int a = 5;
int arr[10];
int a = 1, b = 2, c = 3;  // 多变量声明

// 赋值语句
a = 10;
a += 5;   // 复合赋值

// 指针复合赋值（仅支持 += / -=，右侧为整数）
int arr[5] = {10, 20, 30, 40, 50};
int* p = arr;
p += 2;       // 等价于 p = p + 2，指向 arr[2]
p -= 1;       // 等价于 p = p - 1，指向 arr[1]
void* vp = arr;
vp += 3;      // GCC/Clang 扩展：void* 按 1 字节步进

a++;      // 后缀自增
++a;      // 前缀自增

// 表达式语句
foo(a, b);

// 块作用域
{
    int b = 20;  // b 只在这个块内可见
}

// if/else
if (a > 5) {
    // ...
} else {
    // ...
}

// while 循环
while (i < n) {
    // ...
}

// do...while 循环
do {
    // ...
} while (i < n);

// for 循环（C99 风格：可在初始化中声明变量）
for (int i = 0; i < n; i++) {
    // ...
}

// switch / case / default
switch (x) {
    case 1:
        // ...
        break;
    case 2:
        // ...
        break;
    default:
        // ...
        break;
}

// break / continue
for (int i = 0; i < n; i++) {
    if (arr[i] == target) {
        found = i;
        break;      // 跳出循环
    }
    if (arr[i] == 0) {
        continue;   // 跳过本次循环剩余代码
    }
}

// return
return a;
return;       // 等价于 return 0;
```

**设计决策**：
- **多变量声明**：`int a = 1, b = 2;` 支持同一类型多个变量同时声明
- **支持 for 循环**：这是算法教学的核心语法（排序、遍历等）
- **支持块作用域**：让学生理解变量的生命周期
- **break/continue**：循环控制的核心语法，搜索/过滤算法必备
- **switch/case**：多分支选择的经典语法，支持 fallthrough（不写 break 自然落入下一 case）
- **do...while**：至少执行一次的循环，与 while 形成互补教学

### 2.3 表达式

```c
// 算术运算（整数）
a + b
a - b
a * b
a / b      // 整数除法
a % b      // 取模

// 比较运算
a == b
a != b
a < b
a <= b
a > b
a >= b

// 逻辑运算
a && b     // 短路求值
a || b     // 短路求值
!a

// 赋值
a = b
a += b
a -= b
a *= b
a /= b
a %= b

// 数组索引
arr[i]
arr[0] = 10;

// 函数调用
foo(a, b)

// 取地址
&a

// 解引用（带空指针检查）
*p
*p = 10;

// 结构体访问（-> 和 . 行为一致，简化教学）
node.val
node->val
np->val

// 自增自减
++a
a++
--a
a--

// sizeof（编译期常量，教学子集中所有标量和指针均为 4 字节）
sizeof(int)      // 4
sizeof(char)     // 4（按 i32 存储）
sizeof(a)        // 4
sizeof(p)        // 4
```

**设计决策**：
- **整数除法**：`5 / 2 = 2`，让学生理解整数运算的特点
- **短路求值**：`&&` 和 `||` 必须支持短路，这是重要的概念
- **-> 和 . 行为一致**：struct 统一为引用语义，学生不需要理解 `(*p).val` 的转换
- **sizeof**：编译期计算，帮助学生理解类型大小和内存布局
- **逗号运算符**：优先级最低的表达式运算符，用于 `while (a--, a > 0)`、`for` 步进多操作等场景

### 2.4 函数

```c
// 函数定义
int add(int a, int b) {
    return a + b;
}

// 无参数函数
void hello() {
    // ...
}

// 递归函数
int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

// main 函数作为入口
int main() {
    // ...
    return 0;
}
```

**设计决策**：
- **支持递归**：这是算法教学的核心（阶乘、斐波那契、树遍历）
- **void 返回类型**：简化无返回值函数的定义
- **main 作为入口**：符合 C 语言惯例

### 2.5 内存管理（简化版）

```c
// 动态分配（参数为字节数）
int* arr = malloc(10 * 4);   // 分配 10 个 int（每个 4 字节）

// 释放
free(arr);

// 使用分配的内存
arr[0] = 1;
arr[1] = 2;
```

**设计决策**：
- **参数是字节数**：`malloc(10 * 4)` 或 `malloc(10 * sizeof(int))`
  - `sizeof(int)` 和 `sizeof(struct S)` 已支持，帮助学生理解类型大小
- **宿主管理堆分配**：`malloc` / `realloc` / `free` 是宿主导入函数，宿主记录分配元数据（用于内存泄漏检测）
- **`realloc` 已支持**：完整支持扩容/缩容、NULL ptr（等价 malloc）、size 0（等价 free）

### 2.6 VFS 沙盒文件 I/O

```c
#include <stdio.h>

FILE* fp = fopen("data.txt", "w");
fputs("hello\n", fp);
fclose(fp);

fp = fopen("data.txt", "r");
char buf[32];
fgets(buf, sizeof(buf), fp);
printf("%s", buf);
fclose(fp);
```

**支持细节**：
- `fopen` / `fclose` / `fread` / `fwrite` / `fgets` / `fputs` / `fgetc` / `fputc` / `fseek` / `ftell` / `rewind` / `feof`
- 所有文件操作在 VitroVM 虚拟文件系统（VFS）沙盒内进行，路径相对于 VFS 根目录
- `"r"` / `"w"` / `"a"` / `"rb"` / `"wb"` 等模式均可识别；**文本模式已完整模拟 Windows CRT 的 `\n` ↔ `\r\n` 自动换行转换**
  - 写入 `"w"` 时 `\n` 自动展开为 `\r\n`
  - 读取 `"r"` 时 `\r\n` 自动压缩为 `\n`
  - `fseek` 使用逻辑位置，`ftell` 返回物理位置，匹配 Windows CRT 行为

**已知限制**：
- 已修复：文本模式换行转换差异已消除，`vfs_io_extensions.c` 与 `file_fread.c` 已恢复匹配

---

### 2.7 GCC 扩展（有限支持）

为兼容部分教学代码和 K&R / 模板用例，Vitro 对以下 GCC 扩展提供**有限支持**（仅保证 Shadow Verification 覆盖的用法可用，不保证完整语义）：

```c
// __asm__("...")：GCC 风格内联汇编占位
// 教学子集不执行汇编指令，仅消费语法并忽略，不影响程序控制流
int main() {
    int x = 1;
    __asm__ ("nop");   // 允许出现，但不会生成任何机器码
    printf("%d", x);   // 输出 1
    return 0;
}

// _Static_assert(expr, "msg")：编译期静态断言
// 教学子集仅消费语法；expr 目前不会被编译期求值，因此不会触发断言失败
// 支持出现在顶层和函数体内
_Static_assert(1 == 1, "ok");
int main() {
    _Static_assert(sizeof(int) == 4, "int size");
    printf("ok");
    return 0;
}

// typeof(expr)：根据表达式推断类型
// 支持 typeof / __typeof__ / __typeof 三种写法
// 目前主要用于局部变量声明，推断依据为初始化表达式
int main() {
    int x = 5;
    typeof(x) y = 10;   // 等价于 int y = 10;
    typeof(x) z;        // 无初始化时从 typeof 内的表达式推断，等价于 int z;
    printf("%d", y);    // 输出 10
    return 0;
}
```

**设计决策**：
- `__asm__`：教学场景不需要真实执行汇编，只需不报错即可
- `_Static_assert`：编译期求值复杂度高；当前仅做语法兼容，未来可在 TypeChecker 中扩展常量表达式求值
- `typeof`：主要用于兼容依赖 GCC 扩展的代码；推断路径与 C++ `auto` 共享机制，当前要求变量有初始化表达式（否则回退到 `int`）

---

### 2.8 scanf 族格式串支持范围（2026-09-11 补记）

| 格式串成分 | 状态 | 说明 |
|---|---|---|
| `%` 转换符（`d i u x o c s f lf` + `l` / `ll` / `h` 长度修饰符） | ✅ | 指针参数按转换符个数从栈中依次取，空白指令不占参数位 |
| **空白指令**（格式串中的空白字符） | ✅ 已修复（2026-09-11） | 按 C11 7.21.6.2 匹配输入中任意数量（含零）的空白字符。此前被整段丢弃，导致 `scanf("%d %c %d", &a, &op, &b)` 读 `3 + 4` 时 `%c` 捕获空格而非 `+`（SharpTutor Issue A，教学阻断）。scanf / sscanf / fscanf 共享同一份解析，全族同修 |
| `%c` 不自动跳前导空白 | ✅ | 既有正确语义，不受空白指令修复影响（`%d` 等数值转换符仍自动跳白） |
| **普通字符指令**（非空白非 `%`，如 `"a=%d"` 中的 `a=`） | ✅ **已实现（2026-09-11）** | 按 C11 7.21.6.2 与输入流的下一个字符**精确比较**，不匹配即停止解析并返回已成功赋值的项数；`%%` 展开为字面 `%` 同样参与匹配。修复前被整段忽略：`scanf("a=%d", &x)` 读 `a=42` 得到 `x=0`（Clang 得 42）。回归用例 `baseline/scanf_literal_match.c` / `scanf_literal_mismatch.c`（含负向） |
| **scanf 返回值**（成功匹配并赋值的项数） | ✅ **已实现（2026-09-11）** | `scanf` / `sscanf` 返回成功项数（`sscanf` 早已如此，本次对齐 `scanf`）；修复前 `scanf` 被视作 `void`，`int r = scanf(...)` 报 `E3004`、`while (scanf(...) != EOF)` 不可用。回归用例 `baseline/scanf_return_value.c` |

> **2026-09-11 更新**：上表最后两项（普通字符指令、scanf 返回值）原为"已知差异、未实现"，
> 现已实现并以 Clang 实测对照（新增 Shadow/E2E 用例 3 个，含负向的字面不匹配场景）。
> 同批修复：**标准输入换行口径统一** —— capi `vitro_set_input` / serve `run.input` / CLI `-i` 三个入口此前用 `str::lines()`
> 拆分输入、丢掉行尾 `'\n'`，导致 `getchar()` 永远读不到换行（K&R 用例 `kr_1_8` 的换行计数恒为 0）；
> 现统一走 `RuntimeState::split_stdin`（保留换行）。

### 2.9 堆分配模型：bump + 有界隔离（2026-09-11 决议落地）

依据 [`堆有界隔离决议.md`](../../06-出口与协议（ASAN quarantine 原版机制）：

| 项 | 行为 |
|---|---|
| `malloc` / `calloc` | bump 顶指针推进；隔离区超预算时先按 **FIFO 驱逐**最老块归还复用（first-fit） |
| `free` | 块进入 FIFO **隔离区** —— 地址在隔离期内不复用，不立即归还 |
| `realloc` | **恒为新块拷贝**（旧块进隔离区，不再有"堆顶原地收缩"特例） |
| 隔离预算 | 堆上限的 1/4 = **256KB**（会话级可调） |
| 堆耗尽 | 分配返回 **NULL** + 教学提示（不 trap，见下） |

**设计动机**：把 churn（分配-释放循环，合法）与 leak（只分配不释放，教学信号）正确分离 —— 隔离窗口保证 UAF / Double-Free 必被检出，超预算 FIFO 驱逐保证合法 churn 无限可跑。实测：100 字节 × 10 万次 `malloc`/`free` 循环不撞墙（若无驱逐复用，10485 次即耗尽 1MB）；只分配不释放则在 1MB 处得到 NULL。

**与 Clang 的差异（诚实记录）**：

1. **隔离窗口外的 UAF 可能漏检**：`free(p)` 与错误访问之间若隔离区已整体轮换（其间 churn 超过 256KB），`*p` 落在已复用块上，表现为"读到别人的值"而非 UAF 诊断。与 ASAN quarantine 行为一致；
2. **`realloc` 恒搬移**：`realloc(p, 更小)` 在 glibc 下常原地返回同一地址，Vitro 下必返回新地址（旧地址进隔离区）。标准不保证 realloc 不移动，依赖该行为的代码本就不合规，但**实测输出会与 Clang 不同**（如 `p = realloc(p, 8); p == old_p` 在 Vitro 下为假）；
3. **`free` 后地址的复用时机不同**：glibc 立即可复用，Vitro 需等待隔离区驱逐（教学上更利于暴露"free 后仍持有旧指针"的错误）；
4. **堆耗尽不 trap**：决议文本措辞为"教学 trap"，实现取 NULL + 输出教学提示 —— C 标准要求分配失败返回 NULL，Clang 同样返回 NULL，trap 会偏离"必须检查 malloc 返回值"这一编程习惯。

> **反向链接**：本节隔离预算（堆上限 1/4 = 256KB）、FIFO 驱逐与"第三道墙"（region 表封顶）的完整推导、决议原文与验收用例见 [`堆有界隔离决议.md`](../../06-出口与协议（语言中立的引擎层决议，三出口共用）。

### 2.10 C23 锚定特性（E1 批次，2026-09-11）

语言锚定 ISO C23（ISO/IEC 9899:2024）。E1 批次落地的 lexer/typeck 级特性：

```c
// 0b 二进制字面量（C23）
int a = 0b1010;              // 10
// ' 数字分隔符（C23；0x/0b/十进制内均可）
int m = 1'000'000;
int h = 0x1'0000;
// u8 前缀字符串（C23）
// 教学子集差异：无独立 char8_t 类型，按 char[] 处理（见下方差异清单）
printf("%s", u8"hi");
// alignof / _Alignof（C11/C23）
int k = (int)_Alignof(double);   // 8
int k2 = (int)alignof(int);      // 4（alignof 拼写同支持）
// typeof_unqual（C23）：推导并剥离顶层限定符
const int ci = 9;
typeof_unqual(ci) x = ci + 1;    // x 为 int（非 const），可再赋值
// enum 底层类型（C23）
enum Small : unsigned char { S1 = 200, S2 = 255 };  // sizeof(enum Small) == 1
enum Big : long long { B2 = 5000000000LL };          // 成员常量支持 64 位
```

E1 B 档（基础能力补齐）：

```c
// 相邻字符串字面量拼接（C89）
char s[] = "ab" "\t" "cd";            // "ab<TAB>cd"
// long long 位运算（原 E3048 误拒；BitAndQ/BitOrQ/BitXorQ/BitNotQ/ShlQ/ShrQ/LShrQ）
long long x = 12;  x << 40;  x & 10;
unsigned long long u;  u >> 60;       // 逻辑右移
// limits.h 全宏（INT_MAX/UINT_MAX/LLONG_MIN/ULLONG_MAX/CHAR_BIT 等）
printf("%llu", ULLONG_MAX);           // 18446744073709551615
// float.h（依赖科学计数法字面量 2.2e-16，同批补齐）
DBL_EPSILON;  DBL_MIN;  DBL_MAX;  FLT_DIG;
// va_copy（stdarg.h）
va_list ap, ap2;  va_start(ap, n);  va_copy(ap2, ap);
// __func__ 预定义标识符（C99）
printf("in %s", __func__);
```

**浮点字面量语义（本批修正）**：无后缀浮点字面量类型为 **double**（C 标准
C89~C23 一致），带 `f`/`F` 后缀为 float。此前一律建模为 float，导致 `2.2e-308`
经 f32 位模式存储下溢为 0、与 Clang 存在系统性 epsilon 偏差。

**浮点比较语义（本批修正）**：double/float 比较改为 **IEEE 754 精确语义**。
原实现带 1e-6 容差，使 `0.1 + 0.2 == 0.3` 判真——与 C 标准和 Clang golden 直接
矛盾。教学上"浮点比较不能直接用 =="恰恰是核心一课，精确语义才是正确示范。

**与 Clang 的差异（诚实记录，本批新增/暴露）**：

- **struct/union 布局为 packed**（预存）：Vitro 不做成员对齐填充
  （`struct S { char c; int i; }` 的 sizeof：Vitro=5，Clang Win64=8）；
  `alignof` 按"成员最大自然对齐"取值（与 Clang 口径一致），与自身 packed
  布局的 sizeof/offsetof 存在内部不一致。教学映射/堆可视化依赖 packed
  布局，改动需整体评估。
- **指针为 4 字节**（预存）：VM 指针模型 4 字节，Win64 宿主实际 8 字节
  （`_Alignof(int*)`：Vitro=4，Clang=8）。1MB 线性内存模型使 4 字节指针
  自洽，非缺陷。
- **数字分隔符**（C23-only 语法）在 Clang gnu17 默认模式下无法编译，
  对应用例由词法单元测试覆盖（`lexer_unit_test.rs`），不进 Shadow baseline。
- **printf 动态宽度/精度（`%*d` / `.*f`）不可用**（2026-09-14，U2#9 审查暴露）：
  `parse_format_spec` 遇 `*` 不取宽度参数（`width` 留 None），typeck 随即以
  E3032（格式说明符数量与参数数量不匹配）拒收——合法 C
  （`printf("[%*d]\n", 5, 42)`，Clang 输出 `[   42]`）被误拒。静态宽度/精度
  受 1MB 字段预算约束（超限教学 trap）；**未来支持 `%*d` 时必须为运行时
  width 值补同口径 clamp**（保险丝可触发性义务）。

---

### 2.11 模块化预处理器（E2 批次，2026-09-11）

架构：皮肤与内核分离——学生写标准 C 预处理语法，引擎内部为模块化内核
（`vitro_lexer/src/preprocessor/`：`resolver` / `macro_table` / `expander` /
`cond` / `splice` / `directives`），不做文本变换黑魔法。

**支持**：

| 能力 | 口径 |
|------|------|
| 对象宏 / 参数化宏 | token 模板受控展开；参数先完整展开再替换（C99 §6.10.3.1） |
| `#` 字符串化 / `##` 拼接 | 操作数取**未展开**实参（C 规则特例）；拼接结果必须为单个合法 token，否则 E1016 |
| `#if` / `#elif` / `#else` / `#endif` / `#ifdef` / `#ifndef` | 整数常量表达式（`+ - * / %`、比较、`! && \|\|`，短路；短路分支内除零不触发）；`defined(X)` 宏展开前提取 |
| `#undef` | 支持 |
| `__STDC_VERSION__` | **名义锚点 202311L**——不随宿主 std 模式变化；真实能力见 capabilities JSON |
| `__VITRO_SUBSET__` | 引擎专属探测宏（值为 1） |
| `__has_include(<h>)` / `__has_include("h")` | `#if` 内可用；与 `#include` 解析口径**单源**（U1#11，2026-09-14）：`<>` 只查标准库存根、`"` 走 quote 候选链——头文件内部两者判定不再互相矛盾 |
| `#include` 候选链 | quote-include 优先"包含者目录"，其次源文件目录；`<>` 形式只查标准库存根、**不搜索文件系统目录**（U1#11 H-3 收紧，与 Clang `<>` 语义对齐；存量语料 655 处 `<>` 全为标准头名，零迁移） |
| include 目标不存在 | **E1021**（定位在 include 行，文案列出已搜索目录；U1#11 H-1——修复前静默跳过、错误错位到使用点，学生找不到根因） |
| include-once | 同一文件只拼接一次（守卫语义内置，写不写守卫都正确）|
| include 环检测 | 依赖图静态 DFS（深度封顶 64 / 节点封顶 512，与动态嵌套深度保险丝共用上限常量——U1#11 修复 20 文件环在旧封顶 16 处静默漏报）；A↔B 互包含报 E1015 并跳过该 include |
| include 嵌套深度保险丝 | 动态拼接嵌套上限 64 层（`dir_stack` 深度即嵌套层数），超限报 E1015 并跳过该 include（U1#11 新增，量级对齐 clang `-fmax-include-depth=200` 的教学压缩） |
| 跨文件条件栈边界 | 头文件的条件编译组必须在头文件内闭合（U1#11）：未闭合的 `#if` 在头文件边界自动闭合并报 E1013（不再吞掉包含者后续代码）；头内多余的 `#endif` 被拦截报 E1011（不再弹掉包含者的条件组） |
| `#define` 遮蔽诊断 | 不同体重定义 → W1018 警告；相同体静默（C 标准允许） |
| 宏参数副作用检测 | 参数在体中出现 ≥2 次且实参含 `++`/`--`/赋值 → W1019 警告 |
| 展开链 / `#if` 分支原因 | 白箱教学追踪（容量封顶 64 条），随 `compile.preprocessor_trace` 导出（serve compile 响应含该字段） |
| 展开保险丝 | 深度 64（`expand_inner` 每层入口检查，U1#6 复活死代码）+ 累计产出**字节**预算 16MB（U1#6 改口径——旧 token 计数下 4KB 源码可产出 67MB 零诊断），超限 E1017 |

**诚实放弃清单**（教学替代建议；每条在黑箱 clang 下也是被劝退的写法）：

1. **自引用宏 trick**（如 `#define A A` 的展开期取值技巧）——展开栈查重直接
   停止，宏名按普通标识符保留。替代：直接写目标表达式。
2. **`##` 动态拼标识符的元编程**（拼出任意新名字）——拼接结果必须为单个合法
   token，失败报 E1016。替代：显式命名或数组索引。
3. **X-macro 高级用法**——依赖任意深度的重扫描时可能不工作。替代：代码生成脚本。
4. **宏拼接 include 路径**（`#include MACRO(name)`）——不支持。替代：直接写路径。
5. **无守卫双 include**——Clang 会重定义报错，Vitro include-once 静默跳过
   （守卫语义内置的差异面，教学上鼓励写守卫或依赖内置语义均可）。
6. **空实参 placemarker 语义**、拼接出预处理数字的边界形态——按"结果必须合法"
   从简处理。
7. **`#if` 中的字符常量/枚举**——字符常量按其词法文本处理，不支持 `'A'` 求值。

**诊断对照**：预处理错误 E1011~E1017（致命），教学警告 W1018/W1019（非致命，
走 diagnostics severity=1）。

---

### 2.12 C23 语义级（E3 批次，2026-09-12）

| 特性 | 口径 |
|------|------|
| `nullptr` | 与 `NULL` 同路径（`void*` 空值 0）。教学子集无独立 `nullptr_t` 类型（差异，§2.10 u8 同源口径） |
| `static_assert` / `_Static_assert`（双拼写） | **编译期真求值**（与 enum 初始化器同一常量求值器，支持 sizeof(内建类型)）；为假 → 编译错误 E1020（携带消息）；单参形态（C23）支持；顶层与块作用域均可用 |
| `constexpr` 对象 | 按 `const` 语义处理（`constexpr int N = 42;` 可用）。**边界（诚实记录）**：不强制初始化器为常量表达式、不做常量传播——数组尺寸/case 标签用 `constexpr` 变量不支持，请用字面量或 `#define` |
| `[[属性]]` | 解析并忽略（前缀位置：顶层/语句）。无任何属性语义（`[[maybe_unused]]` 不抑制警告等）；与 Clang 默认模式"未知属性警告后忽略"的可见行为一致。属性参数内嵌套方括号不支持 |
| `unreachable()` | `<stddef.h>` 声明；**执行到即教学 trap**（"执行了标注为不可达的代码……检查分支条件"）；死代码中的调用不执行、不影响输出。与 .NET/C23 的 UB 语义差异：Vitro 给出确定性教学诊断 |

**与 Clang 的差异（诚实记录）**：`nullptr`/`constexpr` 在 Clang gnu17 默认模式下
编译失败（C23-only），因此不出 Shadow golden（由单元测试覆盖，同数字分隔符
口径）；`static_assert`/`[[属性]]`/`unreachable` 死代码在默认模式可用，已入
Shadow（e3_* 用例）。

---

## 3. 明确不支持的语法

### 3.1 排除清单

| 特性 | 排除理由 | 遇到时的错误提示 |
|:---|:---|:---|
| `double` | ⚠️ **部分支持**：`double` 字面量、变量、数组、函数参数、算术运算、printf `%lf` / scanf `%lf` 正常；**函数返回 `double` 值存在 ABI 异常**（调用方可能得到 `0.0`，见 `AGENTS.md` 已知差异与 `LEETCODE_FAILURES.md` 中 `lc_4` 记录），建议通过整数缩放或指针参数输出浮点结果 | 运行时输出 `0.0` |
| `char` / `char*` / 字符串 | ✅ **已支持**：char 按 i32 存储，字符串通过 Data Segment 注入；支持 `strlen`/`strcpy`/`strcmp`/`strcat` | — |
| `break` / `continue` | ✅ **已支持**：循环控制的核心语法 | — |
| `goto` | ✅ **已支持**：无条件跳转到函数内标签 | — |
| `do...while` | ✅ **已支持**：至少执行一次的循环 | — |
| `switch` / `case` / `default` | ✅ **已支持**：多分支选择，支持 fallthrough | — |
| 预处理 (`#include`) | ✅ **已支持（E2 模块化预处理器，2026-09）**：标准库存根（14 个名字）+ 自定义头文件 `"header.h"`（候选链：包含者目录优先 → 源文件目录）+ include-once + 依赖环检测；`<>` 形式**只匹配标准库存根**、不搜索文件系统目录（与 Clang 对齐，2026-09-14 U1#11 收紧）；include 目标不存在报 `E1021`（定位在 include 行）。详见 §2.11 | `<stdio.h>` 等标准头自动加载存根；`"xxx.h"` 按候选链解析；找不到报 E1021 |
| `union` | ✅ **已支持**：全管线支持（声明、`sizeof(union U)`、成员访问、`p->i`），内存布局为所有字段 offset=0、size=max(fields) | — |
| `bitfield` | 进阶特性，初学者不需要 | "暂不支持该特性" |
| 多维数组 | ✅ **已支持**：二维数组声明、嵌套初始化、索引访问、函数参数传递 | — |
| `sizeof` | ✅ **已支持**：编译期常量，所有标量/指针返回 4 | — |
| 逗号分隔的多变量声明 (`int a, b;`) | ✅ **已支持**：`int a = 1, b = 2;` | — |
| 标准库函数 (`printf` / `scanf` / `malloc` 除外) | ✅ **已支持**：printf / scanf / malloc / free 为宿主导入函数 | — |
| `typedef` | ✅ **已支持**：类型别名，提升代码可读性 | — |
| `enum` | ✅ **已支持**：编译期常量，底层为 int | — |
| `extern` | ✅ **已支持**：声明外部符号，不分配存储空间，允许与后续定义共存 | — |
| `static`（全局/函数） | ✅ **已支持**：全局 static 内部链接性（跨文件隔离）、函数 static 文件级可见性 | — |
| `volatile` | ✅ **已支持**：类型修饰符已解析，教学 VM 中无特殊语义（与现代编译器一致） | — |
| `restrict` | 存储类和类型修饰符，增加复杂度 | "暂不支持存储类修饰符" |
| `const` | ✅ **已支持**：直接变量 `const` 语义，阻止赋值和自增/自减 | — |

### 3.2 隐式转换与编译器警告

教学子集允许部分隐式转换（不阻断编译），但会发出警告，帮助学生理解类型系统：

| 转换方向 | 是否允许 | 警告信息 |
|:---|:---|:---|
| `int` → `char` | ✅ | "int 被隐式转换为 char。不同类型的标量之间赋值可能会丢失精度。" |
| `char` → `int` | ✅ | "char 被隐式转换为 int。不同类型的标量之间赋值可能会丢失精度。" |
| `int` → `pointer` | ✅ | "整数被隐式转换为指针。建议确保这是有意义的地址值（如 NULL = 0）。" |
| `array` → `pointer` | ✅ | "数组隐式转换为指针。数组名在表达式中会自动退化为指向首元素的指针。" |
| `void*` → 具体指针 | ✅ | "void* 指针被隐式转换为具体类型的指针。请确保内存布局正确。" |

**设计决策**：
- 教学场景下，隐式转换不应该卡死学生（如 `char c = 65;` 是常见写法）
- 通过警告而非错误的方式，既保证代码能运行，又提醒学生注意类型安全

---

## 4. 与教学功能的映射

### 4.1 语法支持 → 教学能力

| 教学场景 | 需要的语法 | 本项目支持？ |
|:---|:---|:---|
| Hello World（变量与输出） | 变量声明、赋值、内置输出函数 | ✅ |
| 冒泡排序 | 数组、for、if、函数 | ✅ |
| 二分查找 | 数组、while、if/else、函数 | ✅ |
| 矩阵运算 | 多维数组、嵌套循环、函数 | ✅ |
| 链表操作 | struct、指针、malloc/free | ✅ |
| 二叉树遍历 | struct、指针、递归 | ✅ |
| 阶乘/斐波那契 | 递归、if | ✅ |
| 指针基础教学 | &、*、指针作为参数 | ✅ |
| 内存布局教学 | 变量、数组、指针、malloc | ✅ |
| 字符串操作 | char、char*、字符串字面量、printf/scanf | ✅ |
| 文件读写 | VFS 沙盒文件 I/O：`fopen`/`fclose`/`fread`/`fwrite`/`fgets`/`fputs`/`fgetc`/`fputc`/`fseek`/`ftell`/`rewind` | ✅（文本模式与二进制模式行为一致，不模拟 Windows CRT 的 `\n` ↔ `\r\n` 换行转换） |
| 浮点运算 | float/double | ✅ |
| 枚举与状态机 | enum | ✅ |
| 类型抽象 | typedef | ✅ |

### 4.2 内存视图能展示什么

基于支持的语法，内存视图可以展示：

```c
int main() {
    int a = 10;                  // 栈变量
    int arr[5] = {1,2,3,4,5};   // 栈数组
    char s[] = "hello";          // 栈字符数组
    int* p = &a;                 // 栈指针 → 栈变量
    int* heap = malloc(3 * 4);   // 堆数组
    heap[0] = 100;
    
    struct Node node;            // 栈结构体
    node.val = 1;
    
    struct Node* np = malloc(4); // 堆结构体
    np->val = 2;
    np->next = NULL;
    
    enum Color c = Green;        // 枚举变量（底层为 int）
}
```

内存视图可以展示：
- ✅ 栈变量（绿色）
- ✅ 栈数组（绿色块）
- ✅ 指针变量及其指向关系（黄色 → 箭头）
- ✅ 堆分配（蓝色）
- ✅ 结构体内存布局（多个字段并排）
- ✅ 悬垂指针检测（红色）
- ✅ 内存泄漏检测（程序结束时未 free 的堆内存）

---

## 5. 与 VisualBinaryTree 的对比

| 特性 | VisualBinaryTree Algo-C Subset | 本项目 Vitro-C Subset |
|:---|:---|:---|
| int | ✅ | ✅ |
| 数组 | ✅（一维） | ✅（一维 + 多维） |
| struct | ✅ | ✅ |
| 指针 | ⚠️ 有限（不支持 & 和 *） | ✅ 完整支持（&、*、作为参数） |
| malloc/free | ❌ | ✅（简化版） |
| if/else | ✅ | ✅ |
| for | ❌ | ✅ |
| while | ✅ | ✅ |
| return | ✅ | ✅ |
| 函数/递归 | ✅ | ✅ |
| break/continue | ❌ | ✅ |
| do...while | ❌ | ✅ |
| switch/case/default | ❌ | ✅ |
| char / 字符串字面量 | ❌ | ✅ |
| sizeof | ❌ | ✅ |
| typedef | ❌ | ✅ |
| enum | ❌ | ✅ |
| printf / scanf | ❌ | ✅（printf 支持可变参数） |
| float/double | ❌ | ✅ |
| 预处理 | ❌ | ❌ |
| 标准库（除 printf/scanf/malloc/free） | ❌ | ❌ |
| 指针运算 | ❌ | ❌ |

**本项目的扩展**：
- **新增 for 循环**：算法教学的核心语法
- **新增完整指针**（&、*）：C 语言教学的灵魂，内存视图和指针视图的基础
- **新增 malloc/free**：动态内存教学的基础，内存泄漏检测的前提
- **新增 break/continue**：循环控制的核心语法
- **新增 do...while / switch/case**：控制流教学完整性
- **新增 char / 字符串字面量**：字符串操作教学的基础
- **新增 sizeof / typedef / enum**：类型系统教学的基础
- **新增 printf / scanf**：格式化输入输出教学的基础

---

## 6. 编译器实现工作量评估

基于 Rust + VitroVM 自定义字节码架构：

### 6.1 各模块代码量估算

| 模块 | 代码量 | 复杂度 | 说明 |
|:---|:---|:---|:---|
| Lexer | ~300 行 | 🟢 低 | 关键字、标识符、数字、运算符、字符串 |
| Parser（递归下降） | ~600 行 | 🟡 中 | 表达式优先级、语句解析、函数定义 |
| AST 节点定义 | ~200 行 | 🟢 低 | ~20 种 AST 节点类型 |
| TypeChecker | ~400 行 | 🟡 中 | 类型推导、类型兼容性检查 |
| **BytecodeGen** | **~1200 行** | **🔴 高** | **栈机代码生成、内存布局、控制流、指针步长、float 指令** |
| Source Map | ~100 行 | 🟢 低 | 指令偏移 → 源码位置映射 |
| 内置函数（print_int 等） | ~50 行 | 🟢 低 | 宿主导入的辅助函数 |
| **合计** | **~4000 行** | | |

### 6.2 降低风险的策略

**风险**：BytecodeGen 是编译器中最复杂的部分（~1200 行）。

**缓解方案**（已全部验证有效）：

| 策略 | 说明 | 效果 |
|:---|:---|:---|
| **Phase 1 缩小子集** | 先实现变量+数组+函数+指针+if/while/for | 减少 ~30% CodeGen 工作量 ✅ |
| **Rust 枚举 AST** | 用 enum 替代 C++ 多态类层次 | 减少内存管理错误，Borrow Checker 保障安全 ✅ |
| **端到端测试驱动** | 每增加一个语法特性，立即添加 E2E 测试 | 早发现错误，防止回归 ✅ |

---

## 7. 推荐实施方案

### 7.1 Phase 1：核心子集（已完成）

支持：
```c
int a = 5;
int arr[10];
int arr[] = {1, 2, 3};
int* p = &a;

if (a > 5) { }
while (a < 10) { }
for (int i = 0; i < n; i++) { }

int foo(int x) { return x + 1; }
int main() { return 0; }
```

**教学能力**：变量、数组、基本指针、控制流、函数、递归。

### 7.2 Phase 2：扩展子集（已完成）

新增：
- struct、malloc/free（简化版）
- 内置输出函数（`print_int`、`__vitro_output`）
- 内存视图与内存泄漏检测

**教学能力**：链表、树、动态内存、内存泄漏检测。

### 7.3 Phase 3：核心语法扩展（已完成）

新增：
- `do...while`、`break` / `continue`
- `switch` / `case` / `default`（支持 fallthrough）
- `char` 类型与字符串字面量（Data Segment 注入）
- `sizeof`（编译期常量，返回 4）
- `typedef`（类型别名）
- `enum`（编译期常量）
- `unsigned` / `signed`（语义上与 int 相同）
- 数组/字符串初始化列表（`int a[] = {1,2,3};` / `char s[] = "hello";`）
- `printf` / `scanf`（宿主导入函数）
- 隐式转换警告机制（不阻断编译，提示类型安全问题）

**教学能力**：完整的控制流、字符串操作、类型系统、格式化 I/O。

### 7.4 Phase 4：可选增强（根据反馈）

- [x] **多维数组** — 已支持声明、嵌套初始化列表、索引访问、函数参数传递（如 `int[][3]`）
- [x] **结构体初始化**（`struct Node n = {10, &a};`）— 已支持完整/部分初始化，含指针字段
- [x] **函数前向声明** — 已支持 `int foo(int);` 原型声明，实现可放在调用者之后
- [x] **字符串库函数** — 已支持 `strlen` / `strcpy` / `strcmp` / `strcat`（宿主导入函数）
- [x] **显式类型转换（Cast）** — 已支持 `(int*)p`、`(char*)arr`、`(float)a` 等标量/指针间转换
- [x] **预处理器（宏定义）** — 已支持 `#define` 简单常量替换
- [x] **位运算** — 已支持 `& | ^ ~ << >>`
- [x] **三目运算符** — 已支持 `? :`
- [x] **指针算术** — 已支持 `p++` / `p+i` / `p-q`，自动按 pointee 大小缩放
- [x] **`const` 语义** — 已支持直接变量 `const`，阻止赋值和自增/自减
- [x] **`NULL` 关键字** — 已支持，`NULL` 被解析为 `(void*)0`
- [x] **新增宿主函数** — `getchar`/`putchar`/`rand`/`srand`/`memset`/`exit`/`strcat`/`atoi`
- [x] **`fprintf`/`realloc`/`qsort`** — 已支持
- [x] **函数指针完整支持** — 已支持声明变量、赋值、间接调用、结构体成员、typedef、多级
- [x] **`double` 类型** — 已支持完整 64 位精度
- [x] **函数按值返回结构体** — 已支持（Hidden Return Pointer ABI），支持赋值、直接成员访问、作为函数参数
- [x] **多级指针** — 已支持 `int**` / `struct Node**`，含解引用、取地址、数组索引、指针算术、显式 cast
- [x] **VLA（变长数组）** — 已支持局部一维/多维 VLA、`sizeof` 运行时求值、函数参数退化；全局/静态 VLA 编译期拒绝
- [x] **通用逗号运算符** — 已支持 `(a, b)` 表达式，左值求值后丢弃，返回右操作数类型
- [x] **Designated Initializer** — 已支持 `.field = val`（结构体）和 `[i] = val`（一维数组），局部变量上下文；全局/静态变量 designated init 暂不支持
- [x] **`offsetof(struct S, field)`** — 已支持编译期常量计算（结构体/联合体）
- [x] **`__asm__("...")`（GCC 内联汇编占位）** — 已支持语法消费，不生成真实机器码
- [x] **`_Static_assert(expr, "msg")`** — 已支持语法消费；当前不执行编译期求值，仅保证兼容
- [x] **`typeof(expr)` / `__typeof__(expr)`** — 已支持变量声明类型推断，无初始化时回退到 `int`
- [x] **`_Generic` 泛型选择（C11）** — 已支持编译期类型匹配与 `default` 分支；字符串字面量等数组类型会退化为指针后匹配（如 `char*`）
- [x] **复合字面量（C99/C11）** — 已支持 `(struct S){1,2}`、`(int[]){1,2,3}`、`(int){5}` 等；作为 lvalue 可用于取地址；变量初始化时可被直接展开为对应初始化列表

---

## 8. 下一阶段语法拓展蓝图

> 目标：与标准库拓展同步推进，一次性补齐会导致学生代码编译失败的语法缺口。

### 8.1 🔴 P0 — 立即填补（编译失败最高频）

| 特性 | 典型触发场景 | 实现路径 | 复杂度 |
|------|-------------|----------|--------|
| ~~**通用逗号运算符** `a, b`~~ | ~~`while (a--, a > 0)`、表达式语句多操作~~ | ✅ 已完成 | — |
| ~~**Designated Initializer** `.field = val` / `[i] = val`~~ | ~~`struct S s = {.x = 1};`、稀疏数组初始化~~ | ✅ 已完成（局部变量） | — |
| ~~**`offsetof(struct S, field)`~~ | ~~数据结构内存布局教学~~ | ✅ 已完成（编译期常量） | — |

### 8.2 🟠 P1 — 短期实现（教学/算法必备）

| 特性 | 典型触发场景 | 实现路径 | 复杂度 |
|------|-------------|----------|--------|
| ~~**`static`（全局/函数）完整语义**~~ | ~~链接性控制、内部函数~~ | ✅ 已完成 | — |
| ~~**`goto`**~~ | ~~状态机、错误处理清理（虽不鼓励但存在）~~ | ✅ 已完成 | — |
| ~~**条件编译** `#ifdef` / `#ifndef` / `#else` / `#endif`~~ | ~~头文件保护、跨平台代码~~ | ✅ 已完成（Lexer 层状态栈，支持嵌套） | — |

### 8.3 🟡 P2 — 中期实现（进阶需求）

| 特性 | 典型触发场景 | 实现路径 | 复杂度 |
|------|-------------|----------|--------|
| ~~**`restrict`**~~ | ~~高性能数组操作优化提示~~ | ✅ 已完成（关键字识别，教学 VM 中无特殊语义） | — |
| ~~**`inline`**~~ | ~~小型函数内联~~ | ✅ 已完成（关键字识别并忽略） | — |
| ~~**`_Bool` / `bool`**~~ | ~~C99 原生布尔类型~~ | ✅ 已完成（底层映射为 `int`） | — |
| ~~**`register`** / **`auto`**~~ | ~~存储类说明符~~ | ✅ 已完成（关键字识别并忽略） | — |
| ~~**`sizeof(VLA类型)`** `sizeof(int[n])`~~ | ~~VLA 元编程~~ | ✅ 已完成（BytecodeGen 运行时求值） | — |

### 8.4 ⚫ 明确排除项（实现复杂 / 教学价值极低）

| 特性 | 排除理由 |
|------|---------|
| `bitfield`（位域） | 文档已排除；嵌入式专用，初学者不需要 |
| `_Complex` / `_Imaginary` / `<complex.h>` | 数学/工程专用，教学不用 |
| ~~`_Generic`（C11 泛型选择）~~ | ~~学生几乎不用，实现复杂~~ → **已支持**：编译期类型匹配 + `default` 分支 |
| ~~复合字面量 `(Type){...}`~~ | ~~C99/C11 特性，实现复杂~~ → **已支持**：结构体/数组/标量复合字面量，lvalue 语义简化 |
| `_Alignas` / `_Alignof` | C11 进阶，教学很少涉及；`_Static_assert` 已提供语法兼容 |
| `_Noreturn` / `_Thread_local` / `_Atomic` | 同上 |
| `union` 的复杂初始化规则 | 当前已支持基本 union，复杂初始化极少见 |
| **`va_list` / `va_start` / `va_arg` / `va_end`** | 自定义变参需全编译管线 + ABI 改造；`printf`/`scanf` 已内置支持，教学价值有限 |
| **全局 VLA** | 标准允许但教学/实际代码中极少见；实现需全局运行时栈分配机制 |
| 完整预处理器（`#` / `##` 操作符、多行宏、条件宏表达式计算） | 教学场景 `#define` 常量宏已足够 |

---

## 9. 结论

### 对于一个教学场景，多少合适？

**答案**：

> **足够演示 C 语言的核心概念（变量、控制流、函数、指针、内存），覆盖 C89/C99 教学高频语法与标准库，排除会分散注意力的进阶特性。**

### 黄金法则

1. **如果去掉这个特性，学生还能理解 C 的灵魂吗？**
   - 指针（&、*）→ **不能去掉**
   - break/continue → **教学价值高，已支持**（循环控制必备）

2. **这个特性会增加多少编译器复杂度？**
   - for 循环 → 复杂度中等，但教学价值极高 → **保留**
   - float/double → 复杂度中等，教学价值中等（数值计算入门）→ **保留** ✅ 已实现

3. **学生第一次接触这个特性时会困惑吗？**
   - `int a, b;` → 可能困惑（为什么可以一行两个？）→ **已支持** ✅（`int a = 1, b = 2;`）
   - `p++` vs `arr[i++]` → 需要理解步长缩放，但已支持并带教学提示 → **保留**

### 最终推荐的 Vitro-C 子集（Phase 1 ~ 5 完整版）

```
数据类型：int、char、float、double、unsigned、long long、int*、char*、float*、double*、
          int[]、char[]、double[]、struct、union、enum
类型系统：typedef、sizeof、const、**static（局部+全局+函数）**、extern
语句：变量声明、赋值、if/else、while、do...while、for、switch/case/default、
       break、continue、return、**goto**、块作用域
表达式：算术、比较、逻辑、位运算、赋值、三目运算符、逗号运算符、数组索引、
        函数调用、&、*、struct访问、++/--、字符串字面量、sizeof、offsetof、显式类型转换、
        designated initializer（.field / [index]）
函数：定义/调用/递归/前向声明/函数指针/变参（printf/scanf + 未来自定义）
内存：malloc/free/realloc/calloc
I/O：printf、scanf、sprintf、snprintf、sscanf、fprintf、puts、getchar、putchar、
     fopen、fclose、fread、fwrite、fgets、fputs、fgetc、fputc、fseek、ftell、rewind
字符串：strlen、strcpy、strncpy、strcmp、strncmp、strcat、strncat、memcpy、memmove、
        memset、memcmp、strchr、strrchr、strstr、memchr、strdup
其他：rand/srand/exit/abort/qsort/bsearch/atoi/atof/atol/time/clock/assert
数学：sin、cos、tan、sqrt、pow、atan、log、log10、exp、fabs、ceil、floor、round、fmod
字符：isdigit、isalpha、islower、isupper、isalnum、isspace、isprint、iscntrl、isxdigit、
      tolower、toupper
宏/类型：NULL、EOF、INT_MAX、INT_MIN、bool、true、false、size_t、ptrdiff_t、
         EXIT_SUCCESS、EXIT_FAILURE

不支持：bitfield、_Complex、_Static_assert、_Alignas/_Alignof、
       _Noreturn/_Thread_local/_Atomic、`__attribute__((cleanup(...)))` 等 GCC 扩展属性、
       完整预处理器（仅 #define 常量宏 + 条件编译）
```

这个范围覆盖了 C 语言的核心教学价值（变量、控制流、函数、指针、内存、字符串、类型系统、标准库），
能让学生刷 LeetCode（95%+ C 解法编译通过）、学习数据结构教材（95%+ 示例代码直接运行）、
学习 K&R / 谭浩强 C 语言教材（95%+ 示例可直接运行），
同时保持了编译器实现的可控性。
