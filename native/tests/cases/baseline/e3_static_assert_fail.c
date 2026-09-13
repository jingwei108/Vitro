// @category: baseline
// E3：static_assert 断言失败 → 两侧均编译失败（Clang: assertion failed；
// Vitro: E1020），stdout 均为空 → Shadow match
#include <stdio.h>
static_assert(1 == 2, "math broken");
int main() {
    printf("never\n");
    return 0;
}
