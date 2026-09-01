/* m67_irq.c — M67: 中断合并/减轻 v0 (窗口合并 + 成本统计)
 *
 * 忙等让 PIT 连续 tick (~40 中断):
 *   window=1: batches == irqs (逐 tick)
 *   window=8: batches ≈ irqs/8 (组批)
 * 成本字段: total_cyc/worst_cyc 非零 (IRQ 间隔有界)。
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

static void spin_ticks(void)
{
    volatile long sp = 0;
    for (volatile long i = 0; i < 20000000; i++) {
        sp += 1;
    }
}

void _start(void)
{
    static const char m1[] = "m67: irq merge + cost stats v0\n";
    wr(m1, sizeof(m1) - 1);

    /* window=1: 逐 tick 无合并 (基点=set 时刻, 批数 = Δirqs) */
    (void)sys3(0x6D02, (long)st, 0, 0);
    u64 i0 = st[0];
    (void)sys3(0x6D01, 1, 0, 0);
    spin_ticks();
    (void)sys3(0x6D02, (long)st, 0, 0);
    u64 i1 = st[0], b1 = st[1], t1 = st[2], w1 = st[3];
    int s1 = (i1 - i0) > 0 && b1 == (i1 - i0) && t1 > 0 && w1 > 0;

    /* window=8: 组批 (基点重置 → 批数 = Δirqs/8) */
    (void)sys3(0x6D01, 8, 0, 0);
    spin_ticks();
    (void)sys3(0x6D02, (long)st, 0, 0);
    u64 i2 = st[0], b2 = st[1];
    long d = (long)(i2 - i1);
    long lo = d / 8, hi = d / 8 + 1;
    int s2 = i2 > i1 && (long)b2 >= lo && (long)b2 <= hi;

    static const char h1[] = "m67: w1 d_irqs=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)(i1 - i0));
    static const char h2[] = " b=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)b1);
    static const char h3[] = " w8 d_irqs=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)(i2 - i1));
    static const char h4[] = " b=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)b2);
    wr("\n", 1);

    if (s1 && s2) {
        static const char m2[] = "m67: M67 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m67: M67 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
