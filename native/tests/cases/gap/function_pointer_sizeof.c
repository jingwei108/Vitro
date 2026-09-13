// @category: arch_diff_bug
// U0#1③：本例为已知架构差异（教学子集指针 4 字节模型，clang x64 为 8）——
// 差异记载于 docs/current/03-语言子集/C语言子集规范.md（"所有标量和指针均为 4 字节"）；
// @category bug 通道豁免即"已记录差异"，非待修缺陷。
int main() { printf("%d", sizeof(int (*)(int))); return 0; }
