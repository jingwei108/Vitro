// U1#3 红锚（外部审查原始形状）：初始化列表元素是按值传 struct 的调用链
// （take(mk(4))）——Call 实参为 struct 时 call.rs 的地址临时写 temp_slot0，
// 覆盖初始化基址槽。与 init_base_slot_variadic.c 覆盖同一病灶的另一分支。
// golden 来自 clang：1.0 9.0 3.0
#include <stdio.h>
struct P { int x; int y; };
struct P mk(int v) { struct P p; p.x = v; p.y = v + 1; return p; }
int take(struct P p) { return p.x + p.y; }
int main() {
    double a[3] = {1.0, take(mk(4)), 3.0};
    printf("%.1f %.1f %.1f\n", a[0], a[1], a[2]);
    return 0;
}
