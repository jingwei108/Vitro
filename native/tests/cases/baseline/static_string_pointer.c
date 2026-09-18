#include <stdio.h>
static char *prefix = ">> ";
int main(){
    static char *suffix = " <<";
    printf("[%s|%s]", prefix, suffix);
    return 0;
}
