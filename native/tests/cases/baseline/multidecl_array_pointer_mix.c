/* Field-report regression case (student session, before 2026-06-07).
 *
 * Shape: a single declaration statement mixing an unsized array initializer
 * with scalars and a pointer --
 *     int a[]={2,4,6,8,10}, y=0, x, *p;
 * This exact program was reported as a *compile error* by a student. Root
 * cause (SHADOW_VERIFICATION_FRAMEWORK.md, 2026-06-07 entry): the parser's
 * extra_vars path called parse_expression(), which consumes the comma as the
 * comma operator, so multi-declarator declarations failed to parse. Fixed in
 * the same batch that added the general comma operator.
 *
 * Why the case is kept: this declarator-mix shape had ZERO coverage in the
 * 662-case corpus (the closest matches are three K&R cases of the homogeneous
 * form `char *p, *q, *r;`), i.e. the fix was never anchored at the *shape*
 * level -- only at the *feature* level. See D-2026-09-07.
 *
 * The case carries <stdio.h>, so clang compiles it and produces a real
 * golden (expected stdout: 14); it cannot fall into the vitro_better /
 * no-oracle channel.
 *
 * See: native/tests/CORE_ASSET_VERDICT_FAILURES.md (D-2026-09-07)
 */
#include <stdio.h>

int main()
{
int a[]={2,4,6,8,10},y=0,x,*p;
p=&a[1];
for(x=1;x<3;x++)y+=p[x];
printf("%d\n",y);
return 0;
}
