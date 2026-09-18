// @category: file_io
// P5 gap 审计（2026-09-18）：补漏掉的 stdio.h（此前 gap_extension 判定是
// 漏头文件伪装，非 VFS 扩展语义）。两侧 fopen 失败返 NULL 后 fread(NULL)
// 行为：clang 崩溃/UB，Vitro trap——具体分类以 shadow 实跑为准。
#include <stdio.h>
int main() { FILE* f = fopen("test.txt", "r"); char buf[20]; fread(buf, 1, 5, f); buf[5] = 0; printf("%s", buf); fclose(f); return 0; }
