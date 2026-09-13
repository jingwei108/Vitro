/* JIT trace regression case (2026-09-13) -- FIXED, kept green.
 *
 * Red->green audit trail: this case was filed RED first (shadow run before the
 * fix: 665 cases / 644 match / 2 output_gap, both gaps = this file and its
 * long long sibling). The one-line fix (fast path disabled while recording,
 * executor/mod.rs) turned it green: 665 / 646 match / gate passed.
 *
 * Shape: a pure nested counting loop -- the textbook JIT target. It is the
 * ONLY shape that tripped the JIT trace correctness bug: teaching cases loop
 * fewer than JIT_THRESHOLD(100) times (recording never starts), sorting-style
 * bodies contain a conditional branch (recording Aborts and falls back to the
 * interpreter), and single-layer loops cannot be "pierced" by an outer trace.
 *
 * Expected  (clang + `vitro_cli unified`): inner=40000 i=200 j=200
 * Bug (before fix, `vitro_cli run` executor + JIT): inner=20200 i=200 j=0
 *   -> inner stops at 101*200: from outer iteration 102 on, the inner loop is
 *      skipped entirely (`j=0` is still executed each round, hence j ends at 0).
 *
 * Root cause (fixed): the JIT fast path in VitroVM::run stayed active while a
 * trace was being recorded. When recording reached the inner loop head it hit
 * the already-compiled inner trace and ran the whole inner loop in one bulk
 * call, so the recorder never saw the inner instructions; the outer trace was
 * then registered WITHOUT the inner loop. Fix: fast path is disabled while
 * trace_recorder.is_recording() -- any trace that would contain an inner loop
 * now Aborts at the inner back-edge, so only innermost loops get JIT-compiled.
 *
 * Discipline: this case must STAY GREEN. If it ever regresses, the JIT fast
 * path / recording interaction broke again. Full analysis:
 * docs/current/07-质量与裁定/核心资产重构裁定.md section 14.
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
