// @category: baseline
// U1#11 / T11：<> 引不存在的非标准库头。Clang：fatal error（stdout 空）；
// Vitro 修复前：静默通过、程序照跑（compile_gap 红）；修复后：E1021 编译
// 失败（stdout 空）——双侧编译失败判 match。
#include <no_such_stdlib_xyz.h>
int main() {
    printf("ran\n");
    return 0;
}
