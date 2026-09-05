/* sha256tool.c — 散件工厂拼装产物 (make_scatter_tool.py 生成) */
/* 组成: fujo_libc.h + sha256.h(适配) + sha256.c(原样) + fujo_main.c */

/* fujo_libc.h — 散件一: FujoOS 内编译用 header-only POSIX 子集 (公共域)
 * 适配: tcc 无 GOT -> 单编译单元; libc = 头文件 (所有符号同单元)。
 * 系统调用: Linux x86-64 ABI (linuxsubsys 面): read=0 write=1 open=2 close=3
 *           exit=60; 用户态直接 syscall (无 libc 栈, 0x400000..0xC00000)。
 * 范围: stdio (printf/puts) · stdlib (malloc/strtol/atoi) · string (mem/str 全套)
 *  · 类型/stdint/stdbool/stddef —— 覆盖 C 工具类程序的实际依赖。
 * 堆: 静态 128KB bump (程序 BSS 内, 单程序运行足够)。
 */

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
#define va_list __builtin_va_list
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




#define SHA256_H



#define SHA256_HEX_SIZE (64 + 1)
#define SHA256_BYTES_SIZE 32

void sha256_hex(const void *src, size_t n_bytes, char *dst_hex65);
void sha256_bytes(const void *src, size_t n_bytes, void *dst_bytes32);

typedef struct sha256 {
    uint32_t state[8];
    uint8_t buffer[64];
    uint64_t n_bits;
    uint8_t buffer_counter;
} sha256;

void sha256_init(struct sha256 *sha);
void sha256_append(struct sha256 *sha, const void *data, size_t n_bytes);
void sha256_finalize_hex(struct sha256 *sha, char *dst_hex65);
void sha256_finalize_bytes(struct sha256 *sha, void *dst_bytes32);



/* sha256.c — 原样副本 (公共域, 983/SHA-256; https://github.com/983/SHA-256)
 * 散件工厂将本文件与 fujo_libc.h/sha256.h 拼装为单编译单元 (tcc 无 GOT)。
 */

static inline uint32_t rotr(uint32_t x, int n){
    return (x >> n) | (x << (32 - n));
}

static inline uint32_t step1(uint32_t e, uint32_t f, uint32_t g){
    return (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ ((~ e) & g));
}

static inline uint32_t step2(uint32_t a, uint32_t b, uint32_t c){
    return (rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
}

static inline void update_w(uint32_t *w, int i, const uint8_t *buffer){
    int j;
    for (j = 0; j < 16; j++){
        if (i < 16){
            w[j] =
                ((uint32_t)buffer[0] << 24) |
                ((uint32_t)buffer[1] << 16) |
                ((uint32_t)buffer[2] <<  8) |
                ((uint32_t)buffer[3]);
            buffer += 4;
        }else{
            uint32_t a = w[(j + 1) & 15];
            uint32_t b = w[(j + 14) & 15];
            uint32_t s0 = (rotr(a,  7) ^ rotr(a, 18) ^ (a >>  3));
            uint32_t s1 = (rotr(b, 17) ^ rotr(b, 19) ^ (b >> 10));
            w[j] += w[(j + 9) & 15] + s0 + s1;
        }
    }
}

static void sha256_block(struct sha256 *sha){
    uint32_t *state = sha->state;

    static const uint32_t k[8 * 8] = {
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    };

    uint32_t a = state[0];
    uint32_t b = state[1];
    uint32_t c = state[2];
    uint32_t d = state[3];
    uint32_t e = state[4];
    uint32_t f = state[5];
    uint32_t g = state[6];
    uint32_t h = state[7];

    uint32_t w[16];

    int i, j;
    for (i = 0; i < 64; i += 16){
        update_w(w, i, sha->buffer);

        for (j = 0; j < 16; j += 4){
            uint32_t temp;
            temp = h + step1(e, f, g) + k[i + j + 0] + w[j + 0];
            h = temp + d;
            d = temp + step2(a, b, c);
            temp = g + step1(h, e, f) + k[i + j + 1] + w[j + 1];
            g = temp + c;
            c = temp + step2(d, a, b);
            temp = f + step1(g, h, e) + k[i + j + 2] + w[j + 2];
            f = temp + b;
            b = temp + step2(c, d, a);
            temp = e + step1(f, g, h) + k[i + j + 3] + w[j + 3];
            e = temp + a;
            a = temp + step2(b, c, d);
        }
    }

    state[0] += a;
    state[1] += b;
    state[2] += c;
    state[3] += d;
    state[4] += e;
    state[5] += f;
    state[6] += g;
    state[7] += h;
}

void sha256_init(struct sha256 *sha){
    sha->state[0] = 0x6a09e667;
    sha->state[1] = 0xbb67ae85;
    sha->state[2] = 0x3c6ef372;
    sha->state[3] = 0xa54ff53a;
    sha->state[4] = 0x510e527f;
    sha->state[5] = 0x9b05688c;
    sha->state[6] = 0x1f83d9ab;
    sha->state[7] = 0x5be0cd19;
    sha->n_bits = 0;
    sha->buffer_counter = 0;
}

void sha256_append_byte(struct sha256 *sha, uint8_t byte){
    sha->buffer[sha->buffer_counter++] = byte;
    sha->n_bits += 8;

    if (sha->buffer_counter == 64){
        sha->buffer_counter = 0;
        sha256_block(sha);
    }
}

void sha256_append(struct sha256 *sha, const void *src, size_t n_bytes){
    const uint8_t *bytes = (const uint8_t*)src;
    size_t i;

    for (i = 0; i < n_bytes; i++){
        sha256_append_byte(sha, bytes[i]);
    }
}

void sha256_finalize(struct sha256 *sha){
    int i;
    uint64_t n_bits = sha->n_bits;

    sha256_append_byte(sha, 0x80);

    while (sha->buffer_counter != 56){
        sha256_append_byte(sha, 0);
    }

    for (i = 7; i >= 0; i--){
        uint8_t byte = (n_bits >> 8 * i) & 0xff;
        sha256_append_byte(sha, byte);
    }
}

void sha256_finalize_hex(struct sha256 *sha, char *dst_hex65){
    int i, j;
    sha256_finalize(sha);

    for (i = 0; i < 8; i++){
        for (j = 7; j >= 0; j--){
            uint8_t nibble = (sha->state[i] >> j * 4) & 0xf;
            *dst_hex65++ = "0123456789abcdef"[nibble];
        }
    }

    *dst_hex65 = '\0';
}

void sha256_finalize_bytes(struct sha256 *sha, void *dst_bytes32){
    uint8_t *ptr = (uint8_t*)dst_bytes32;
    int i, j;
    sha256_finalize(sha);

    for (i = 0; i < 8; i++){
        for (j = 3; j >= 0; j--){
            *ptr++ = (sha->state[i] >> j * 8) & 0xff;
        }
    }
}

void sha256_hex(const void *src, size_t n_bytes, char *dst_hex65){
    struct sha256 sha;

    sha256_init(&sha);

    sha256_append(&sha, src, n_bytes);

    sha256_finalize_hex(&sha, dst_hex65);
}

void sha256_bytes(const void *src, size_t n_bytes, void *dst_bytes32){
    struct sha256 sha;

    sha256_init(&sha);

    sha256_append(&sha, src, n_bytes);

    sha256_finalize_bytes(&sha, dst_bytes32);
}

/* fujo_main.c — 散件工厂测试驱动 (工具功能验证: 标准向量 + 多块 + 文件输入)
 * 拼装进单编译单元 (见 tools/make_scatter_tool.py)。
 */


static int check(const char *name, const char *got, const char *want)
{
    int ok = strcmp(got, want) == 0;
    printf("sfx: %s = %s (want %s) %s\n", name, got, want, ok ? "OK" : "BAD");
    return ok;
}

/* __attribute__((noreturn)) 保持与 mbuild 一致 (无 CRT, 入口 = _start) */
void _start(void)
{
    char hex[65];
    int pass = 1;

    /* FIPS 180-4 标准向量: "abc" */
    sha256_hex("abc", 3, hex);
    pass &= check("abc", hex,
                  "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    /* 空串 */
    sha256_hex("", 0, hex);
    pass &= check("empty", hex,
                  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

    /* 多块 (>64B: 110B 跨两块) */
    {
        static const char data[] = "abcdefghijklmnopqrstuvwxyz0123456789"
                                   "_abcdefghijklmnopqrstuvwxyz0123456789_"
                                   "abcdefghijklmnopqrstuvwxyz0123456789";
        sha256_hex(data, sizeof(data) - 1, hex);
        pass &= check("data110", hex,
                      "b7121997d66bf89f5078cb7229faf5c7f56ea1a1efd222686500a69de199f1dd");
    }

    /* 文件输入: 读 /tmp/sfx-data, hash, 打印 (散件工厂案例的"文件树"路径) */
    {
        int fd = fj_open("/tmp/sfx-data", O_RDONLY);
        if (fd >= 0) {
            char buf[256];
            long n = fj_read(fd, buf, sizeof(buf));
            fj_close(fd);
            if (n > 0) {
                sha256_hex(buf, (size_t)n, hex);
                printf("sfx: file /tmp/sfx-data (%ldB) = %s\n", n, hex);
            }
        } else {
            printf("sfx: no /tmp/sfx-data (skip file case)\n");
        }
    }

    if (pass) {
        printf("SFACTORY RESULT: PASS\n");
    } else {
        printf("SFACTORY RESULT: FAIL\n");
    }
    fj_exit(0);
    for (;;) {}
}
