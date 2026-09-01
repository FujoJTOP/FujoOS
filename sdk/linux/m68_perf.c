/* m68_perf.c — M68: 帧时间表/性能计数器工具 v0
 *
 * 1. perf_frame_mark ×5 (忙循环隔开) → stats: frames>=2, avg>0,
 *    max>=avg
 * 2. 计数器: enable(3) syscall计数自定义槽 → spin 期间 syscall 数增长
 *    (读前后差值)
 * 3. 计数器: c0 (PIT IRQ) / c1 (syscall) 在 spin 前后差分 >0
 */
typedef long int64_t;
typedef unsigned long long u64;
typedef unsigned int u32;

static int64_t sys3(long nr, long a, long b, long c)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
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

static u64 st[4];
static u64 ctr[8];

static void spin_ticks(void)
{
    volatile long sp = 0;
    for (volatile long i = 0; i < 20000000; i++) {
        sp += 1;
    }
}

void _start(void)
{
    static const char m1[] = "m68: frame timeline + perf counters v0\n";
    wr(m1, sizeof(m1) - 1);

    /* 1) 帧标记 */
    (void)sys3(0x6E01, 0, 0, 0);
    spin_ticks();
    (void)sys3(0x6E01, 0, 0, 0);
    spin_ticks();
    (void)sys3(0x6E01, 0, 0, 0);
    spin_ticks();
    (void)sys3(0x6E01, 0, 0, 0);
    spin_ticks();
    (void)sys3(0x6E01, 0, 0, 0);
    (void)sys3(0x6E02, (long)st, 0, 0);
    u64 frames = st[0], avg = st[1], mx = st[2];
    static const char h1[] = "m68: frames=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)frames);
    static const char h2[] = " avg=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)avg);
    static const char h3[] = " max=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)mx);
    wr("\n", 1);
    int s1 = frames >= 2 && avg > 0 && mx >= avg;

    /* 2) 计数器差分: spin 前后 */
    (void)sys3(0x6E04, (long)ctr, 0, 0);
    u64 c0a = ctr[0], c1a = ctr[1], c2a = ctr[2];
    spin_ticks();
    (void)sys3(0x6E04, (long)ctr, 0, 0);
    u64 c0b = ctr[0], c1b = ctr[1], c2b = ctr[2];
    static const char h4[] = "m68: d_irq=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)(c0b - c0a));
    static const char h5[] = " d_sys=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)(c1b - c1a));
    static const char h6[] = " d_ctx=";
    wr(h6, sizeof(h6) - 1);
    wrhex((u32)(c2b - c2a));
    wr("\n", 1);
    int s2 = (c0b - c0a) > 0 && (c1b - c1a) > 0;

    if (s1 && s2) {
        static const char m2[] = "m68: M68 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m68: M68 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
