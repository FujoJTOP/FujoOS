/* m77_win.c — M77: 性能计数器窗口 (rdtsc/中断计数窗口)
 *
 * win_begin(0) → 忙循环 20M → win_end(0) → win_read:
 *   us>0, irq_delta>0 (PIT 期间), sys_delta>=1 (读回自身/开始)
 * PASS 条件: us>0 && irq>0 && sys>=1 && calls>=1
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

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

void _start(void)
{
    static const char m1[] = "m77: perf counter windows v0\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x7801, 0, 0, 0);
    volatile long sp = 0;
    for (volatile long i = 0; i < 20000000; i++) {
        sp += 1;
    }
    (void)sys3(0x7802, 0, 0, 0);
    (void)sys3(0x7803, (long)st, 0, 0);
    u64 us = st[0], irq = st[1], sys = st[2], calls = st[3];

    static const char h1[] = "m77: us=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)us);
    static const char h2[] = " irq=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)irq);
    static const char h3[] = " sys=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)sys);
    static const char h4[] = " calls=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)calls);
    wr("\n", 1);

    int ok = us > 0 && irq > 0 && sys >= 1 && calls >= 1;
    if (ok) {
        static const char m2[] = "m77: M77 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m77: M77 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
