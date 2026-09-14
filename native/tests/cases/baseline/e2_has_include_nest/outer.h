// U1#11 H-2 / T14：头文件内 __has_include("...") 与 #include "..." 候选链统一
//（包含者目录优先）。修复前 __has_include 只查源文件目录 → 判 0 走 #else
//（output_gap 红：输出 0 vs Clang 13）；修复后两者一致输出 13。
#if __has_include("inner.h")
#include "inner.h"
#else
#define INNER_VAL 0
#endif
