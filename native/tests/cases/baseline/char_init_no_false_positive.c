/* W0-4 / R-2026-09-12 regression: char initializers are idiomatic C.
 *
 * `char c = 'A'`, `char t[] = {'x','y',0}` and `char s[5] = {72,...}` used to
 * be bombarded with W3053 "implicit conversion to char may lose precision"
 * warnings (9 warnings for this shape; clang is silent). The fix exempts
 * char constants and in-range integer constants in *initializers* (real
 * truncation -- out-of-range constants, variable assignment -- still warns).
 * This case anchors the RUNTIME semantics through the clang golden: the
 * initializer values must execute exactly as clang computes them.
 */
#include <stdio.h>

int main() {
    char s[5] = {72, 101, 108, 108, 111};
    char c = 'A';
    char t[] = {'x', 'y', 0};
    printf("%s %c %s %d\n", s, c, t, t[1] - t[0]);
    return 0;
}
