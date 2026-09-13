/* JIT trace regression case #2 (2026-09-13) -- FIXED, kept green.
 *
 * Purpose: pin the ATTRIBUTION. The original field report blamed "writing a
 * `long long` local inside the loop body". That attribution was WRONG -- the
 * pure-int case (jit_nested_counting_loop.c) failed identically. This case
 * keeps the "long long is not the trigger" fact anchored in the corpus.
 *
 * Expected  (clang + `cide_cli unified`): outer=200 inner=40000 sum=40000 i=200 j=200
 * Bug (before fix, `cide_cli run` executor + JIT): outer=200 inner=20200 sum=20200 i=200 j=0
 *
 * Trigger conditions (all three were required):
 *   1. outer-loop back-edge count reaches JIT_THRESHOLD(100);
 *   2. the inner loop has already been JIT-compiled;
 *   3. the outer loop body contains no conditional branch (otherwise recording
 *      Aborts and the interpreter produces correct results).
 *
 * Discipline: this case must STAY GREEN (red->green trail: shadow before fix
 * = output_gap, after fix = match). Analysis:
 * docs/current/07-质量与裁定/核心资产重构裁定.md section 14.
 */
#include <stdio.h>

int main() {
    long long sum = 0;
    int i, j, outer = 0, inner = 0;
    for (i = 0; i < 200; i++) {
        outer = outer + 1;
        for (j = 0; j < 200; j++) {
            inner = inner + 1;
            sum = sum + 1;
        }
    }
    printf("outer=%d inner=%d sum=%lld i=%d j=%d\n", outer, inner, sum, i, j);
    return 0;
}
