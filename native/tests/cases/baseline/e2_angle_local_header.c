// @category: baseline
// U1#11 H-3 / T4：<> 不再搜索文件系统目录（只查标准库存根表），与 Clang 对齐。
// Clang：file not found with <angled> include（stdout 空）；Vitro 修复前：加载
// 同目录自定义头成功输出 444（compile_gap 红）；修复后：E1021 编译失败
//（stdout 空）——双侧编译失败判 match。
// 存量语料扫描（2026-09-14）：655 处 <> include 全部为标准头名，零存量依赖。
#include <e2_angle_local_header.h>
int main() {
    printf("%d\n", ANGLE_LOCAL_VAL);
    return 0;
}
