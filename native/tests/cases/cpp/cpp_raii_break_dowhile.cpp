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
    do {
        Tracker inner(i + 20);
        if (i == 1) {
            Tracker deep(i + 30);
            break;
        }
        i++;
    } while (i < 3);
    printf("after do-while\n");
    return 0;
}
