#include <stdio.h>
class Tracker {
public:
    int id;
    Tracker(int i) { id = i; printf("ctor %d\n", id); }
    ~Tracker() { printf("dtor %d\n", id); }
};
int main() {
    Tracker outer(1);
    int i = 0;
    while (i < 3) {
        Tracker inner(i + 10);
        if (i == 1) break;
        i++;
    }
    printf("after loop\n");
    return 0;
}
