// @category: baseline
// E2：include 依赖环（无守卫互包含）。Clang 因无限嵌套包含编译失败；
// Vitro 静态环检测报 E1015 并跳过——两侧均编译失败，stdout 均为空。
#include "e2_cycle_a.h"
int main() {
    return 0;
}
