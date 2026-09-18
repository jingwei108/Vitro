// @category: file_io
// P5 gap 审计（2026-09-18）：此前漏 #include <stdio.h>，clang 报 undeclared
// FILE 而 shadow 判 gap_extension——是"漏头文件"伪装的扩展，非 VFS 语义差异。
// 补头转真对照：两侧 fopen 不存在的文件同返 NULL，预期 match。
#include <stdio.h>
int main() { FILE* f = fopen("test.txt", "r"); if (f) printf("ok"); fclose(f); return 0; }
