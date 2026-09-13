/* JIT trace regression case (2026-09-13) -- EXPECTED FAIL (red) until fixed.
 *
 * Shape: a pure nested counting loop -- the textbook JIT target. It is the
 * ONLY shape that trips the JIT trace correctness bug: teaching cases loop
 * fewer than JIT_THRESHOLD(100) times (recording never starts), sorting-style
 * bodies contain a conditional branch (recording Aborts and falls back to the
 * interpreter), and single-layer loops cannot be "pierced" by an outer trace.
 *
 * Expected  (clang + `cide_cli unified`): inner=40000 i=200 j=200
 * Observed  (`cide_cli run`, executor + JIT): inner=20200 i=200 j=0
 *   -> inner stops at 101*200: from outer iteration 102 on, the inner loop is
 *      skipped entirely (`j=0` is still executed each round, hence j ends at 0).
 *
 * Root cause (located, NOT yet fixed): the JIT fast path in CideVM::run stays
 * active while a trace is being recorded. When recording reaches the inner
 * loop head it hits the already-compiled inner trace and runs the whole inner
 * loop in one bulk call, so the recorder never sees the inner instructions;
 * the outer trace is then registered by Finish WITHOUT the inner loop. Every
 * later outer iteration is executed by that incomplete trace.
 *
 * Discipline: keep this RED (do NOT mark it known_issue, do NOT soften the
 * expected output). The one-line fast-path fix must turn it green, leaving a
 * red->green audit trail. Full analysis: docs/current/07-质量与裁定/
 * 核心资产重构裁定.md section 14.
 */
#include <stdio.h>

int main() {
    int i, j, inner = 0;
    for (i = 0; i < 200; i++) {
        for (j = 0; j < 200; j++) {
            inner = inner + 1;
        }
    }
    printf("inner=%d i=%d j=%d\n", inner, i, j);
    return 0;
}
