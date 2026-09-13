// U1#5 红锚：循环体内 int a[12] 的零初始化逐字节展开（5 指令/字节 = 240 条）
// 使循环体指令数超过 JIT MAX_TRACE_LEN=256；旧实现录满返回 Finish 注册半截
// trace，重放中值栈溢出 TRAP（合法教学代码被判错）。
// golden 来自 clang：total=89700
#include <stdio.h>
int main() {
    int total = 0;
    for (int n = 0; n < 300; n++) {
        int a[12];
        a[0] = n; a[1] = n; a[2] = n; a[3] = n;
        a[4] = n; a[5] = n; a[6] = n; a[7] = n;
        a[8] = n; a[9] = n; a[10] = n; a[11] = n;
        total += a[0] + a[11];
    }
    printf("total=%d\n", total);
    return 0;
}
