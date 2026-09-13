/* Bytecode Libc: string 子集 —— 用 Vitro-C 子集重写的纯算法实现 */

int strlen(char *s) {
    int len = 0;
    while (*s != '\0') {
        len++;
        s++;
    }
    return len;
}

int strcmp(char *s1, char *s2) {
    while (*s1 != '\0' && *s1 == *s2) {
        s1++;
        s2++;
    }
    return *s1 - *s2;
}

char *strcpy(char *dest, char *src) {
    char *d = dest;
    while (*src != '\0') {
        *d = *src;
        d++;
        src++;
    }
    *d = '\0';
    return dest;
}

char *strcat(char *dest, char *src) {
    char *d = dest;
    while (*d != '\0') {
        d++;
    }
    while (*src != '\0') {
        *d = *src;
        d++;
        src++;
    }
    *d = '\0';
    return dest;
}

char *strncpy(char *dest, char *src, int n) {
    char *d = dest;
    int i = 0;
    while (i < n && *src != '\0') {
        *d = *src;
        d++;
        src++;
        i++;
    }
    while (i < n) {
        *d = '\0';
        d++;
        i++;
    }
    return dest;
}

void *memcpy(void *dest, void *src, int n) {
    char *d = (char *)dest;
    char *s = (char *)src;
    int i = 0;
    while (i < n) {
        d[i] = s[i];
        i++;
    }
    return dest;
}

void *memmove(void *dest, void *src, int n) {
    char *d = (char *)dest;
    char *s = (char *)src;
    if (d < s) {
        int i = 0;
        while (i < n) {
            d[i] = s[i];
            i++;
        }
    } else {
        int i = n - 1;
        while (i >= 0) {
            d[i] = s[i];
            i--;
        }
    }
    return dest;
}
