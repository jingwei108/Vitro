/* Bytecode Libc: stdlib 子集 —— 用 Vitro-C 子集重写的纯算法实现 */

int abs(int n) {
    return n < 0 ? -n : n;
}

int atoi(char *s) {
    int sign = 1;
    int val = 0;
    while (*s == ' ') {
        s++;
    }
    if (*s == '-') {
        sign = -1;
        s++;
    } else if (*s == '+') {
        s++;
    }
    while (*s >= '0' && *s <= '9') {
        val = val * 10 + (*s - '0');
        s++;
    }
    return sign * val;
}

static unsigned int __rand_seed = 1;

void srand(unsigned int s) {
    __rand_seed = s;
}

int rand(void) {
    __rand_seed = __rand_seed * 1103515245U + 12345U;
    return (int)((__rand_seed >> 16) & 0x7fff);
}


