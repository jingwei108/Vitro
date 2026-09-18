#include <stdio.h>
char *greet = "hi";
char *names[2] = {"alice", "bob"};
int main(){
    printf("[%s]", greet);
    printf("[%s][%s]", names[0], names[1]);
    return 0;
}
