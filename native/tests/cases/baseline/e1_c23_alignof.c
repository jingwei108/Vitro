// @category: baseline
// C11/C23（E1）：_Alignof（C23 的 alignof 拼写同样支持，见词法单元测试）。
// 不含指针：Vitro VM 指针模型为 4 字节，与 Win64 宿主的 8 字节存在既有差异
//（见 AGENTS.md 已知差异），非本批引入。
#include <stdio.h>
struct S { char c; int i; };
union U { char c; double d; };
int main() {
    printf("%d %d %d %d %d\n",
        (int)_Alignof(char), (int)_Alignof(int), (int)_Alignof(double),
        (int)_Alignof(struct S), (int)_Alignof(union U));
    return 0;
}
