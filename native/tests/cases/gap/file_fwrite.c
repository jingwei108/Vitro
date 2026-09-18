// @category: file_io
// P5 gap 审计（2026-09-18）：补漏掉的 stdio.h（此前 gap_extension 判定是
// 漏头文件伪装，非 VFS 扩展语义）。补头后两侧写文件语义一致，预期 match。
#include <stdio.h>
int main() { FILE* f = fopen("out.txt", "w"); fwrite("hello", 1, 5, f); fclose(f); printf("ok"); return 0; }
