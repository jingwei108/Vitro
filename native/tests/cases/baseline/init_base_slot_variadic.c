// U1#3 红锚：初始化基址槽被嵌套调用覆盖。数组/结构体初始化的写入基址
// 此前存 temp_slot0，初始化列表元素的求值内部（变参调用的 double 实参
// StoreLocalD slot0）会覆盖它——后续元素写内存变野地址（实测 trap：
// "向 NULL 指针区域写入（地址 0x0010）"）。修复：初始化路径改专用基址槽。
// golden 来自 clang：1.0 2.5 3.0 4.0
#include <stdio.h>
double dv(int n, ...) { return 2.5; }
int main() {
    double a[4] = {1.0, dv(1, 3.5), 3.0, 4.0};
    printf("%.1f %.1f %.1f %.1f\n", a[0], a[1], a[2], a[3]);
    return 0;
}
