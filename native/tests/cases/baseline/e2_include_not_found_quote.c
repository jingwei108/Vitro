// @category: baseline
// U1#11 H-1 / T5：quote include 找不到头文件必须报 E1021（定位在 include 行），
// 不得静默跳过后把错误错位到使用点。Clang：fatal error 'file not found'（stdout 空）；
// Vitro 修复后：E1021 编译失败（stdout 空）——双侧编译失败判 match。
// 注：修复前 Vitro 报 E3023（使用点错位），stdout 同样为空——shadow 对本缺陷
// 天然盲（只比 stdout），语义红锚在 lexer_unit_test::test_u11_include_quote_not_found。
#include "e2_no_such_header_xyz.h"
int main() {
    printf("%d\n", NO_SUCH_MACRO);
    return 0;
}
