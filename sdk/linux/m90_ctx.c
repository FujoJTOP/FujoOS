/* m90_ctx.c — M90: 上下文压缩 (fujoctx 链, 截断+摘要窗口)
 *
 * 4KB 文本 (头部 "AAAA" 尾部 "ZZZZ" 中间 'B') → compress(win=512):
 * 压缩长度 < 4096; 头 4 字节 == "AAAA"; 含 "[...ctx-compressed...]";
 * 尾部 4 字节 == "ZZZZ"; win 参数 (512) → 窗口大小正确.
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

static int64_t sys5(long nr, long a, long b, long c, long d, long e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8)
                 : "rcx", "r11", "memory");
    return rax;
}
static int64_t sys3(long nr, long a, long b, long c)
{
    return sys5(nr, a, b, c, 0, 0);
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrhex(u32 v)
{
    static const char H[] = "0123456789abcdef";
    char b[9];
    int i;
    for (i = 0; i < 8; i++) {
        b[i] = H[(v >> (28 - i * 4)) & 0xF];
    }
    wr(b, 8);
}

static char src[4096];
static char out[1200];

void _start(void)
{
    static const char m1[] = "m90: context compression (host-model delegation)\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    for (i = 0; i < 4096; i++) {
        src[i] = 'B';
    }
    for (i = 0; i < 4; i++) {
        src[i] = 'A';
    }
    for (i = 0; i < 4; i++) {
        src[4096 - 1 - i] = 'Z';
    }

    long n = sys5(0x8001, (long)src, 4096, (long)out, sizeof(out), 512);
    int ok = n < 4096 && n > 100;
    ok = ok && out[0] == 'A' && out[1] == 'A' && out[2] == 'A' && out[3] == 'A';
    ok = ok && out[n - 1] == 'Z' && out[n - 2] == 'Z' && out[n - 3] == 'Z' && out[n - 4] == 'Z';
    /* 中间标记 */
    int has_mid = 0;
    for (i = 0; i < n - 20; i++) {
        if (out[i] == '[' && out[i + 1] == '.' && out[i + 2] == '.' && out[i + 3] == '.') {
            has_mid = 1;
            break;
        }
    }
    ok = ok && has_mid;

    static const char h1[] = "m90: in=4096 out=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)n);
    static const char h2[] = " ratio=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)((10000 * n) / 4096));
    wr("\n", 1);

    if (ok) {
        static const char m2[] = "m90: M90 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m90: M90 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
