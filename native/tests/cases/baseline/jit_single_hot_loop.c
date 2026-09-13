/* JIT single hot-loop case (2026-09-13) -- J10 coverage, kept green.
 *
 * Shape: a single-layer hot loop (> JIT_THRESHOLD=100 iterations, no
 * conditional branch in the body). This is the shape where the JIT bulk
 * executor ACTUALLY runs the program body -- the main battlefield of the
 * trace templates. The nested-loop cases anchor "recording must not pierce
 * an inner loop"; THIS case anchors "the bulk-executed innermost-loop
 * semantics are correct" (arithmetic, bitwise, and loop-carried state).
 *
 * Expected (hand-computed, cross-checked with clang):
 *   sum of (2i+1) for i=0..499 = 500^2        = 250000
 *   x   = XOR of (i & 3) over 500 iterations  = 0 (125 full 0..3 groups,
 *        each group XORs to 0)
 *   i   = 500 (loop exit value)
 * => "sum=250000 x=0 i=500"
 *
 * Discipline: must STAY GREEN. If it regresses, the JIT template semantics
 * (jit_templates.rs / execute_trace_bulk) broke -- the interpreter path would
 * still be correct, so only parity-style tests or this clang-golden case can
 * catch it. Analysis: docs/current/07-质量与裁定/核心资产重构裁定.md §14 (J10).
 */
#include <stdio.h>

int main() {
    int sum = 0, x = 0, i;
    for (i = 0; i < 500; i++) {
        sum = sum + (i * 2 + 1);
        x = x ^ (i & 3);
    }
    printf("sum=%d x=%d i=%d\n", sum, x, i);
    return 0;
}
