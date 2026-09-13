/* Vitro time.h stub */
typedef long long time_t;
typedef long long clock_t;

#define CLOCKS_PER_SEC 1000000

time_t time(time_t* tloc);
clock_t clock(void);
