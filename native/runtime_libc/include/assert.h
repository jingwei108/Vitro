/* Vitro assert.h stub */
void __vitro_assert_fail(void);
#define assert(expr) if (!(expr)) __vitro_assert_fail()
