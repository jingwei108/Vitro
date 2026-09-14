// category: gap
// U3#8（2026-09-14）：内置容器 + 类实参的第二次实例化必须查重——
// 此前 try_synthesize_builtin_container_class 无条件合成 + register，
// 第二个 vitro_vec<Point> 在 register_single_class_layout 报 E3002
// "类重复定义"（合法代码被拒，外部审查实锤）。修前本用例编译失败。
// 注：赋值右侧的函数式构造 `p = Point(3,4)` 是另一独立存量缺陷
//（裸类同形失败，与本批无关，已登记路线图待办），本用例用规避写法。
#include <stdio.h>

class Point {
public:
    int x, y;
    Point() : x(0), y(0) {}
    Point(int a, int b) : x(a), y(b) {}
};

int fill_and_sum() {
    vitro_vec<Point> v;
    Point p(1, 2);
    v.push_back(p);
    Point r(3, 4);
    v.push_back(r);
    return v.get(0).x + v.get(1).y;
}

int fill_and_count() {
    vitro_vec<Point> w;
    Point q(5, 6);
    w.push_back(q);
    return w.size();
}

int main() {
    printf("%d\n", fill_and_sum());
    printf("%d\n", fill_and_count());
    return 0;
}
