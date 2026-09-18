#include <stdio.h>
char g[] = "\x4Z\x41Y\012B\7C\xff";
int main(){
    char c7 = '\7';
    char c12 = '\012';
    char cx = '\x9';
    printf("%d %d %d %d %d %d", (int)sizeof(g), g[0], g[2], g[4], g[6], g[8] & 0xff);
    printf(" %d %d %d", c7, c12, cx);
    char local[] = "\x4\77";
    printf(" %d %d %d", (int)sizeof(local), local[0], local[1]);
    return 0;
}
