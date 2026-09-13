// @category: gap
// J2（U0#1②，2026-09-13）：本用例从 baseline 移入 gap——测的是 Cide 对旧式
// 写法的宽容（Cide 扩展，非 C 标准）：`register int x` 后取地址 `&x`（C 标准
// 禁止对 register 变量取地址，clang 拒绝；Cide 宽容接受）、`auto int y` 存储
// 类（C23 起 auto 变为类型推断）。clang 侧预期编译失败 → gap_extension。
inline int add(int a, int b) { return a + b; }

int main() {
    register int x = 5;
    auto int y = 10;
    _Bool b1 = 1;
    bool b2 = 0;
    int *restrict p = &x;
    printf("%d %d %d %d %d", add(x, y), b1, b2, *p, x + y);
    return 0;
}
