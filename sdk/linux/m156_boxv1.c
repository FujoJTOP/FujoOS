/* m156_boxv1.c — W37/B-2v1: 大产物带外 + file2pdf + framebuf (B-3 通路版)
 *
 * 验证 (docs/109 §13 v1 升级):
 *   T1 hash 常量 (v0 面不变)
 *   T2 file2pdf (verb5): 产物 >512B (微 PDF 555B), 头 %PDF- + 尾 %%EOF
 *   T3 framebuf (verb6): BMP 32x24 RGB24 = 2358B (>512 大产物带外), 0x8319 元数据
 *   T4 双列台账计数
 * 宿主模式: normal (或 golden — box_golden.json 校验在内核外, demo 无感知)。
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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}
static long sLen(const char *s)
{
    long n = 0;
    while (s[n])
        n++;
    return n;
}
static int eq(const char *a, const char *b)
{
    long i = 0;
    while (a[i] && b[i] && a[i] == b[i])
        i++;
    return a[i] == b[i];
}
static int starts_with(const char *h, const char *pre)
{
    while (*pre) {
        if (*h != *pre)
            return 0;
        h++;
        pre++;
    }
    return 1;
}
static int ends_with(const char *h, const char *suf)
{
    long hn = sLen(h), sn = sLen(suf);
    if (sn > hn)
        return 0;
    long i;
    for (i = 0; i < sn; i++)
        if (h[hn - sn + i] != suf[i])
            return 0;
    return 1;
}

static int run(void)
{
    static const char h[] = "m156: BOX-BRIDGE v1 path (big-artifact + pdf + framebuf)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;
    static const char arg[] = "fujobox-v0 payload";
    static const char argpdf[] = "FujoOS BoxBridge v1";
    static const char expect_hash[] =
        "d4a997afdb0442de74a37dd7cad5eed9822b2f3084f605fd6d172d00d28f1520";

    sy(0x8101, 6, 0x7F, 0, 0, 0);
    long d1 = sy(0x8107, 0x40, 1, 1, 0, 0);
    sy(0x8108, d1, 0, 0, 0, 0);

    /* T1 hash (v0 面) */
    {
        char out[96];
        long rc = sy(0x8316, 1, (long)arg, sLen(arg), 0, 0);
        if (rc == 0) {
            long n = sy(0x8318, (long)out, sizeof(out), 0, 0, 0);
            out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
            wrstr("m156: T1 hash=");
            wr(out, n);
            wrstr("\n");
            if (n != 64 || !eq(out, expect_hash))
                pass = 0;
        } else {
            pass = 0;
        }
    }

    /* T2 file2pdf (verb5, 大产物 >512B) */
    {
        char out[3200];
        long rc = sy(0x8316, 5, (long)argpdf, sLen(argpdf), 0, 0);
        if (rc == 0) {
            long n = sy(0x8318, (long)out, (long)sizeof(out), 0, 0, 0);
            out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
            wrstr("m156: T2 pdf len=");
            wrdec((u64)n);
            wrstr(" head=");
            wr(out, n < 8 ? n : 8);
            wrstr("\n");
            if (n <= 512 || !starts_with(out, "%PDF-") || !ends_with(out, "%%EOF\n"))
                pass = 0;
        } else {
            wrstr("m156: T2 pdf rc=");
            wrdec((u64)rc);
            wrstr("\n");
            pass = 0;
        }
    }

    /* T3 framebuf (verb6, BMP 32x24 = 2358B) */
    {
        char out[2600];
        u64 fb[3] = { 0, 0, 0 };
        long rc = sy(0x8316, 6, (long)arg, sLen(arg), 0, 0);
        sy(0x8319, (long)fb, 0, 0, 0, 0);
        if (rc == 0) {
            long n = sy(0x8318, (long)out, (long)sizeof(out), 0, 0, 0);
            wrstr("m156: T3 fb len=");
            wrdec((u64)n);
            wrstr(" meta=");
            wrdec(fb[0]);
            wrstr("x");
            wrdec(fb[1]);
            wrstr(" expect=2358 (32x24x3+54)\n");
            if (n != 2358 || out[0] != 'B' || out[1] != 'M'
                || fb[0] != 32 || fb[1] != 24 || fb[2] != 2358)
                pass = 0;
        } else {
            wrstr("m156: T3 fb rc=");
            wrdec((u64)rc);
            wrstr("\n");
            pass = 0;
        }
    }

    /* T4 台账 */
    {
        u64 st[4];
        sy(0x8317, 1, (long)st, 0, 0, 0);
        wrstr("m156: T4 ledger up=");
        wrdec(st[0]);
        wrstr(" hit=");
        wrdec(st[1]);
        wrstr(" total=");
        wrdec(st[2]);
        wrstr(" schema=");
        wrdec(st[3]);
        wrstr("\n");
        if (st[0] != 1 || st[3] < 3)
            pass = 0;
    }

    if (pass) {
        static const char m2[] = "m156: BX V1 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m156: BX V1 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(0x8109, d1, 0, 0, 0, 0);
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
