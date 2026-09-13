// @category: baseline
// E2：预定义宏族。__STDC_VERSION__ 为名义锚点 202311L（spec 2.11），
// golden 只比较下界（clang gnu17 为 201710L，同样 >= 199901L）。
// 引擎专属宏 __VITRO_SUBSET__ 不在此例（clang 无定义，差异已入 spec，
// 由词法单元测试覆盖）。
#include <stdio.h>
int main() {
#if __STDC_VERSION__ >= 199901L
    printf("modern");
#else
    printf("ancient");
#endif
#if __has_include(<stdio.h>)
    printf("+stdio");
#endif
#if !__has_include(<no_such_header_xyz.h>)
    printf("+nope");
#endif
    printf("\n");
    return 0;
}
