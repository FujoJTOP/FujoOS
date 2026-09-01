/* m65_tss.c — M65: 每核 TSS / 中断注入优化 v0
 *
 * 1. lapic_id() -> 真实 CPU 标识 (TCG 虚拟 APIC)
 * 2. tss_info -> (tss0_rsp0, tss1_rsp0, gdt_limit): 双 TSS 就位
 * 3. 中断注入路由:
 *    - irq_route(1) (全核0) 前先记录 stats, sleep 300ms (~30 ticks);
 *      delta: r0>0 && r1==0 && r0==inj
 *    - irq_route(2) (全核1) sleep 300ms; delta: r0==0 && r1>0
 *    - irq_route(3) (轮转) sleep 400ms; delta: r0>0 && r1>0 (双核分散)
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

static u32 st[4];
static u32 tinfo[6];

void _start(void)
{
    static const char m1[] = "m65: per-core TSS + irq routing v0\n";
    wr(m1, sizeof(m1) - 1);

    /* 1) LAPIC id */
    u32 lapid = (u32)sys3(0x6B01, 0, 0, 0);
    static const char h1[] = "m65: lapic_id=";
    wr(h1, sizeof(h1) - 1);
    wrhex(lapid);
    wr("\n", 1);

    /* 2) 双 TSS */
    (void)sys3(0x6B02, (long)tinfo, 0, 0);
    u64 t0 = ((u64 *)tinfo)[0];
    u64 t1 = ((u64 *)tinfo)[1];
    u64 gl = ((u64 *)tinfo)[2];
    static const char h2[] = "m65: tss0_rsp0=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)(t0 >> 20));
    static const char h3[] = " tss1_rsp0=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)(t1 >> 20));
    static const char h4[] = " gdt_limit=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)gl);
    wr("\n", 1);
    int tss_ok = t0 == 0x300000 && t1 == 0x3A0000 && gl == 0x7F;

    /* 3a) 路由核0 */
    (void)sys3(0x6B04, 1, 0, 0);
    (void)sys3(0x6B05, (long)st, 0, 0);
    u32 b0 = st[1], b1 = st[2], bi = st[3];
    { volatile long sp = 0; for (volatile long i = 0; i < 20000000; i++) { sp += 1; } }
    (void)sys3(0x6B05, (long)st, 0, 0);
    u32 a_r0 = st[1] - b0, a_r1 = st[2] - b1, a_inj = st[3] - bi;
    int step0 = a_r0 > 0 && a_r1 == 0 && a_r0 == a_inj;
    static const char h5[] = "m65: route(core0) d_r0=";
    wr(h5, sizeof(h5) - 1);
    wrhex(a_r0);
    static const char h6[] = " d_r1=";
    wr(h6, sizeof(h6) - 1);
    wrhex(a_r1);
    wr("\n", 1);

    /* 3b) 路由核1 */
    (void)sys3(0x6B04, 2, 0, 0);
    (void)sys3(0x6B05, (long)st, 0, 0);
    b0 = st[1];
    b1 = st[2];
    bi = st[3];
    { volatile long sp = 0; for (volatile long i = 0; i < 20000000; i++) { sp += 1; } }
    (void)sys3(0x6B05, (long)st, 0, 0);
    u32 b_r0 = st[1] - b0, b_r1 = st[2] - b1, b_inj = st[3] - bi;
    int step1 = b_r0 == 0 && b_r1 > 0 && b_r1 == b_inj;
    static const char h7[] = "m65: route(core1) d_r0=";
    wr(h7, sizeof(h7) - 1);
    wrhex(b_r0);
    static const char h8[] = " d_r1=";
    wr(h8, sizeof(h8) - 1);
    wrhex(b_r1);
    wr("\n", 1);

    /* 3c) 路由轮转 */
    (void)sys3(0x6B04, 3, 0, 0);
    (void)sys3(0x6B05, (long)st, 0, 0);
    b0 = st[1];
    b1 = st[2];
    bi = st[3];
    { volatile long sp = 0; for (volatile long i = 0; i < 20000000; i++) { sp += 1; } }
    (void)sys3(0x6B05, (long)st, 0, 0);
    u32 c_r0 = st[1] - b0, c_r1 = st[2] - b1, c_inj = st[3] - bi;
    int step2 = c_r0 > 0 && c_r1 > 0 && c_r0 + c_r1 == c_inj;
    static const char h9[] = "m65: route(rotate) d_r0=";
    wr(h9, sizeof(h9) - 1);
    wrhex(c_r0);
    static const char h10[] = " d_r1=";
    wr(h10, sizeof(h10) - 1);
    wrhex(c_r1);
    wr("\n", 1);

    if (tss_ok && step0 && step1 && step2) {
        static const char m2[] = "m65: M65 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m65: M65 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
