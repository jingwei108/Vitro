// category: gap
#include <stdio.h>

class Point {
public:
    int x, y;
    Point() : x(0), y(0) {}
    Point(int a, int b) : x(a), y(b) {}
    void set(int a, int b) { x = a; y = b; }
};

int main() {
    vitro_vec<Point> v;
    Point p;
    p.set(1, 2);
    v.push_back(p);
    p.set(3, 4);
    v.push_back(p);
    for (int i = 0; i < v.size(); i++) {
        Point q = v.get(i);
        printf("%d %d\n", q.x, q.y);
    }
    Point t(5, 6);
    v.push_back(t);
    Point r = v.get(2);
    printf("%d %d\n", r.x, r.y);
    return 0;
}
