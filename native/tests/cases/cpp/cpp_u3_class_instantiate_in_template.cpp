// category: gap
// U3#5/T2（2026-09-14）：函数模板体内发现的类实例化必须与函数实例化
// 同收敛 drain——此前 pending_class_instantiations 只在 Pass 3 后排空一次，
// Pass 3.6（函数模板实例化循环）期间新发现的类被静默丢弃，合法 C++ 误拒。
#include <stdio.h>

class Point {
public:
    int x, y;
    Point() : x(0), y(0) {}
    Point(int a, int b) : x(a), y(b) {}
};

template <class T>
int collect(T a, T b) {
    vitro_list<Point> lst;
    lst.push_back(a);
    lst.push_back(b);
    return lst.size();
}

int main() {
    Point a(1, 2);
    Point b(3, 4);
    printf("%d\n", collect(a, b));
    return 0;
}
