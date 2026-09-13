#include <stdio.h>
class Tracker {
public:
    int id;
    Tracker() { id = 9; printf("ctor %d\n", id); }
    Tracker(int i) { id = i; printf("ctor %d\n", id); }
    ~Tracker() { printf("dtor %d\n", id); }
};
int main() {
    int n = 0;
    for (Tracker t; n < 3; n++) {
        Tracker body(n * 100);
        printf("iter %d\n", n);
        if (n == 0) continue;
    }
    printf("after for\n");
    return 0;
}
