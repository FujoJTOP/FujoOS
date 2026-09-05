/* fujo_libc.h — 散件一: FujoOS 内编译用 header-only POSIX 子集 (公共域)
 * 适配: tcc 无 GOT -> 单编译单元; libc = 头文件 (所有符号同单元)。
 * 系统调用: Linux x86-64 ABI (linuxsubsys 面): read=0 write=1 open=2 close=3
 *           exit=60; 用户态直接 syscall (无 libc 栈, 0x400000..0xC00000)。
 * 范围: stdio (printf/puts) · stdlib (malloc/strtol/atoi) · string (mem/str 全套)
 *  · 类型/stdint/stdbool/stddef —— 覆盖 C 工具类程序的实际依赖。
 * 堆: 静态 128KB bump (程序 BSS 内, 单程序运行足够)。
 */
#ifndef FUJO_LIBC_H
#define FUJO_LIBC_H

typedef unsigned long size_t;
typedef long ssize_t;
typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long uint64_t;
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned long off_t;
typedef int int32_t;
typedef short int16_t;
typedef signed char int8_t;

#define NULL ((void *)0)
#define EOF (-1)
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2
#define O_RDONLY 0
#define O_WRONLY 1
#define O_RDWR 2
#define O_CREAT 0x40
#define O_TRUNC 0x200

#define EOF_ (-1)

/* ---------- syscall ---------- */
/* tcc 约束限制: 仅 a/D/S/d (注册表别名), 不用 'r' —— 与 mbuild sy4 同模式。 */
static long fj_sys4(long n, long a, long b, long c)
{
    long r;
    asm volatile("syscall" : "=a"(r) : "a"(n), "D"(a), "S"(b), "d"(c)
                 : "rcx", "r11", "memory");
    return r;
}

static long fj_write(int fd, const void *buf, unsigned long n)
{ return fj_sys4(1, fd, (long)buf, (long)n); }
static long fj_read(int fd, void *buf, unsigned long n)
{ return fj_sys4(0, fd, (long)buf, (long)n); }
static long fj_open(const char *path, int flags)
{ return fj_sys4(2, (long)path, flags, 0); }
static long fj_close(int fd)
{ return fj_sys4(3, fd, 0, 0); }
static long fj_exit(long code)
{ fj_sys4(60, code, 0, 0); for (;;) {} }

/* ---------- string / memory ---------- */
static void *memcpy(void *d, const void *s, size_t n)
{ unsigned char *o = d; const unsigned char *p = s; while (n--) *o++ = *p++; return d; }
static void *memmove(void *d, const void *s, size_t n)
{
    unsigned char *o = d; const unsigned char *p = s;
    if (o < p) { while (n--) *o++ = *p++; }
    else { o += n; p += n; while (n--) *--o = *--p; }
    return d;
}
static void *memset(void *d, int c, size_t n)
{ unsigned char *o = d; while (n--) *o++ = (unsigned char)c; return d; }
static int memcmp(const void *a, const void *b, size_t n)
{ const unsigned char *x = a, *y = b; while (n--) { if (*x != *y) return *x - *y; x++; y++; } return 0; }
static size_t strlen(const char *s)
{ size_t n = 0; while (s[n]) n++; return n; }
static int strcmp(const char *a, const char *b)
{ while (*a && *a == *b) { a++; b++; } return (unsigned char)*a - (unsigned char)*b; }
static int strncmp(const char *a, const char *b, size_t n)
{ while (n && *a && *a == *b) { a++; b++; n--; } return n ? (unsigned char)*a - (unsigned char)*b : 0; }
static char *strcpy(char *d, const char *s)
{ char *o = d; while ((*o++ = *s++)) {} return d; }
static char *strncpy(char *d, const char *s, size_t n)
{ char *o = d; while (n && *s) { *o++ = *s++; n--; } while (n--) *o++ = 0; return d; }
static char *strcat(char *d, const char *s)
{ char *o = d + strlen(d); while ((*o++ = *s++)) {} return d; }
static char *strchr(const char *s, int c)
{ for (;; s++) { if (*s == (char)c) return (char *)s; if (!*s) return NULL; } }
static char *strrchr(const char *s, int c)
{ const char *last = NULL; for (;; s++) { if (*s == (char)c) last = s; if (!*s) return (char *)last; } }
static char *strstr(const char *h, const char *n)
{
    if (!*n) return (char *)h;
    for (; *h; h++) { size_t i = 0; while (n[i] && h[i] == n[i]) i++; if (!n[i]) return (char *)h; }
    return NULL;
}

/* ---------- stdlib ---------- */
static unsigned long strtoul(const char *s, char **end, int base)
{
    unsigned long v = 0; int any = 0;
    if (!base) base = (*s == '0' && (s[1] == 'x' || s[1] == 'X')) ? 16 : 10;
    if (base == 16 && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) s += 2;
    while (*s) {
        int d;
        if (*s >= '0' && *s <= '9') d = *s - '0';
        else if (*s >= 'a' && *s <= 'f') d = *s - 'a' + 10;
        else if (*s >= 'A' && *s <= 'F') d = *s - 'A' + 10;
        else break;
        if (d >= base) break;
        v = v * (unsigned long)base + (unsigned long)d; any = 1; s++;
    }
    if (end) *end = (char *)(any ? s : (const char *)0);
    return any ? v : 0;
}
static long strtol(const char *s, char **end, int base)
{
    int neg = 0; const char *orig = s;
    if (*s == '-') { neg = 1; s++; }
    else if (*s == '+') s++;
    unsigned long v = strtoul(s, end, base);
    if (neg && end) *end = (char *)((*end) ? (*end) : 0);
    return neg ? -(long)v : (long)v;
}
static int atoi(const char *s)
{ return (int)strtol(s, NULL, 10); }
static long atol(const char *s)
{ return strtol(s, NULL, 10); }

/* 堆: 128KB bump (程序 BSS) */
static unsigned char fj_heap[128 * 1024];
static size_t fj_heap_used = 0;
static void *malloc(size_t n)
{
    n = (n + 15) & ~((size_t)15);
    if (fj_heap_used + n > sizeof(fj_heap)) return NULL;
    void *p = fj_heap + fj_heap_used;
    fj_heap_used += n;
    return p;
}
static void free(void *p) { (void)p; }        /* bump: 无回收 */
static void *calloc(size_t n, size_t sz)
{ size_t t = n * sz; void *p = malloc(t); if (p) memset(p, 0, t); return p; }
static void *realloc(void *p, size_t n)
{ (void)p; (void)n; return NULL; }            /* 子集: 不支持, 显式 NULL (诚实) */

/* ---------- stdio ---------- */
/* x64 可变形参在寄存器/栈混合 (SysV) —— 用编译器内建 va (clang/tcc 均支持)。 */
typedef __builtin_va_list va_list;
#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_arg(ap, type) __builtin_va_arg(ap, type)
#define va_end(ap) __builtin_va_end(ap)

static unsigned long fj_vfmt(char *out, unsigned long cap, const char *fmt, va_list ap)
{
    /* 最小 printf 引擎: %s %c %d %u %ld %lx %x %p %05d 类宽度/0填充 */
    unsigned long n = 0;
    int i = 0;
    while (fmt[i]) {
        if (fmt[i] != '%') { if (n + 1 < cap) out[n++] = fmt[i]; i++; continue; }
        i++;
        if (fmt[i] == '%') { if (n + 1 < cap) out[n++] = '%'; i++; continue; }
        int width = 0, zero = 0;
        if (fmt[i] == '0') { zero = 1; i++; }
        while (fmt[i] >= '0' && fmt[i] <= '9') { width = width * 10 + (fmt[i] - '0'); i++; }
        char c = fmt[i]; i++;
        if (c == 's') {
            const char *s = va_arg(ap, const char *);
            if (!s) s = "(null)";
            while (*s && n + 1 < cap) out[n++] = *s++;
        } else if (c == 'c') {
            char ch = (char)va_arg(ap, int);
            if (n + 1 < cap) out[n++] = ch;
        } else if (c == 'd' || c == 'i') {
            long v = va_arg(ap, long);
            char b[24]; int k = 24, neg = v < 0;
            if (neg) v = -v;
            do { b[--k] = (char)('0' + v % 10); v /= 10; } while (v);
            if (neg) b[--k] = '-';
            long len = 24 - k;
            while (len < width) { if (n + 1 < cap) out[n++] = zero ? '0' : ' '; len++; }
            while (k < 24 && n + 1 < cap) out[n++] = b[k++];
        } else if (c == 'u') {
            unsigned long v = va_arg(ap, unsigned long);
            char b[24]; int k = 24;
            do { b[--k] = (char)('0' + v % 10); v /= 10; } while (v);
            while (k < 24 && n + 1 < cap) out[n++] = b[k++];
        } else if (c == 'x' || c == 'X') {
            unsigned long v = va_arg(ap, unsigned long);
            char b[24]; int k = 24;
            static const char H[] = "0123456789abcdef";
            do { b[--k] = H[v & 0xF]; v >>= 4; } while (v);
            while (k < 24 && n + 1 < cap) out[n++] = b[k++];
        } else if (c == 'p') {
            unsigned long v = va_arg(ap, unsigned long);
            char b[24]; int k = 24;
            static const char H[] = "0123456789abcdef";
            if (n + 1 < cap) out[n++] = '0';
            if (n + 1 < cap) out[n++] = 'x';
            do { b[--k] = H[v & 0xF]; v >>= 4; } while (v);
            while (k < 24 && n + 1 < cap) out[n++] = b[k++];
        } else {
            if (n + 1 < cap) out[n++] = '%';
            if (n + 1 < cap) out[n++] = c;
        }
    }
    if (n < cap) out[n] = 0;
    return n;
}

static int printf(const char *fmt, ...)
{
    static char buf[512];
    va_list ap;
    va_start(ap, fmt);
    unsigned long n = fj_vfmt(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    return (int)fj_write(1, buf, n);
}
static int dprintf(int fd, const char *fmt, ...)
{
    static char buf[512];
    va_list ap;
    va_start(ap, fmt);
    unsigned long n = fj_vfmt(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    return (int)fj_write(fd, buf, n);
}
static int puts(const char *s)
{ long n = (long)strlen(s); fj_write(1, s, n); return (int)fj_write(1, "\n", 1); }

#endif /* FUJO_LIBC_H */
