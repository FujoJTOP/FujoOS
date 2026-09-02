/* m128_wf.c — W16b: 源码+桥对象写入器
 *
 * 写 /tmp/hello.c (raw syscall 程序 C 源, 无头文件/libc) 与 /tmp/sy.o (syscall 桥),
 * 随后注入: tcc -nostdlib -static -o /tmp/hello /tmp/hello.c /tmp/sy.o -> runfile。
 */
typedef long int64_t;
typedef unsigned long u64;

static int64_t sy(int64_t nr, int64_t a, int64_t b, int64_t c, int64_t d, int64_t e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx),
                 "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sy(1, 1, (long)s, len, 0, 0); }
static void wrdec(u64 v)
{
    char b[22];
    int i = 22;
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}

static const char PATH[] = "/tmp/hello.c";
static const char PATHSY[] = "/tmp/sy.o";
#include "sy_o.h"

/* hello.c: 由 tcc 编译; sy() 在 sy.o 中 (tcc 不支持 GCC asm 约束) */
static const char SRC[] =
    "typedef long i64;\n"
    "extern long sy(long, long, long, long, long, long);\n"
    "static const char MSG[] = \"tcc-compiled hello from fujo!\\n\";\n"
    "void _start(void) {\n"
    "  sy(1, 1, (long)MSG, sizeof(MSG) - 1, 0, 0);\n"
    "  sy(60, 7, 0, 0, 0, 0);\n"
    "  for (;;) {}\n"
    "}\n";

static long write_file(const char *path, const void *data, unsigned long len)
{
    long fd = sy(2, (long)path, 0, 0, 0, 0); /* 先探测? 不行: tmpfs open 即建 */
    return fd;
}

static void run(void)
{
    static const char h[] = "m128: write hello.c + sy.o for tcc (W16b)\n";
    wr(h, sizeof(h) - 1);
    /* /tmp/hello.c (WRONLY) */
    long fd = sy(2, (long)PATH, 11, 1, 0, 0);
    long w = fd >= 3 ? sy(1, fd, (long)SRC, sizeof(SRC) - 1, 0, 0) : -1;
    if (fd >= 3)
        sy(3, fd, 0, 0, 0, 0);
    /* /tmp/sy.o (WRONLY) */
    long fd2 = sy(2, (long)PATHSY, 9, 1, 0, 0);
    long w2 = fd2 >= 3 ? sy(1, fd2, (long)sy_o, sy_o_len, 0, 0) : -1;
    if (fd2 >= 3)
        sy(3, fd2, 0, 0, 0, 0);
    wrstr("m128: c=");
    wrdec((u64)w);
    wrstr(" o=");
    wrdec((u64)w2);
    wrstr("\n");
    if (w == sizeof(SRC) - 1 && w2 == (long)sy_o_len) {
        static const char m2[] = "m128: M128 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m128: M128 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(60, 7, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
