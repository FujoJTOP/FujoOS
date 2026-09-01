/* m64_smp.c — M64: 多核 v0 (CPUID 探测 / 调度亲和 / 负载均衡统计)
 *
 * fork 出子任务:
 *   父 (tid0) aff_set(0, 1) — 亲和核0; 子 (tid1) aff_set(1, 2) — 亲和核1
 * 双方忙等给 PIT 轮转: 统计 smp_stats (ncpu, c0, c1, switches),
 * 断言: c0>0; ncpu<2 时 c1==0, ncpu>=2 时 c1>0; c0+c1 == switches。
 */
typedef long int64_t;
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

static u32 st[4];

void _start(void)
{
    static const char m1[] = "m64: smp affinity + balance v0\n";
    wr(m1, sizeof(m1) - 1);

    long rc = sys3(57, 0, 0, 0); /* fork() */
    if (rc == 0) {
        /* 子: 亲和核1 (bit1), 忙等给调度轮转 */
        (void)sys3(0x6A01, 1, 2, 0);
        volatile long spin = 0;
        for (volatile long i = 0; i < 20000000; i++) {
            spin += 1;
        }
        static const char c1[] = "m64: child done (aff=core1)\n";
        wr(c1, sizeof(c1) - 1);
        for (;;) {
        }
    } else if (rc > 0) {
        /* 父: 亲和核0 (bit0), 忙等 */
        (void)sys3(0x6A01, 0, 1, 0);
        volatile long spin = 0;
        for (volatile long i = 0; i < 20000000; i++) {
            spin += 1;
        }
        (void)sys3(0x6A04, (long)st, 0, 0);
        u32 ncpu = st[0], c0 = st[1], c1 = st[2], sw = st[3];
        static const char h1[] = "m64: ncpu=";
        wr(h1, sizeof(h1) - 1);
        wrhex(ncpu);
        static const char h2[] = " c0=";
        wr(h2, sizeof(h2) - 1);
        wrhex(c0);
        static const char h3[] = " c1=";
        wr(h3, sizeof(h3) - 1);
        wrhex(c1);
        static const char h4[] = " sw=";
        wr(h4, sizeof(h4) - 1);
        wrhex(sw);
        wr("\n", 1);

        int ok = c0 > 0 && c0 + c1 == sw
                 && (ncpu < 2 ? c1 == 0 : c1 > 0);
        if (ok) {
            static const char m2[] = "m64: M64 RESULT: PASS\n";
            wr(m2, sizeof(m2) - 1);
        } else {
            static const char m3[] = "m64: M64 RESULT: FAIL\n";
            wr(m3, sizeof(m3) - 1);
        }
        for (;;) {
        }
    } else {
        static const char m4[] = "m64: M64 RESULT: FAIL (fork)\n";
        wr(m4, sizeof(m4) - 1);
        for (;;) {
        }
    }
}
