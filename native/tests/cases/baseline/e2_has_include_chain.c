// @category: baseline
// U1#11 H-2 / T14 的主文件：外层引子目录头，头内 __has_include 探测同目录
// inner.h（见 e2_has_include_nest/outer.h）。Clang 输出 13。
#include "e2_has_include_nest/outer.h"
int main() {
    printf("%d\n", INNER_VAL);
    return 0;
}
