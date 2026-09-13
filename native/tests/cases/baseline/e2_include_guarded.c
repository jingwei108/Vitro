// @category: baseline
// E2：守卫 + 双 include（Clang 由守卫保证单次定义；Vitro 额外内置 include-once）
#include <stdio.h>
#include "e2_guarded_helper.h"
#include "e2_guarded_helper.h"
int main() {
    printf("%d %d\n", g_val, g_add(1, 2));
    return 0;
}
