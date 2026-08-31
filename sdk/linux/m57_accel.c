/* m57_accel.c — M57: 加速探测 (TCG/KVM 识别) + 对照基准 */
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
static void wrdec(long v)
{
    char b[24];
    int i = 24;
    int neg = 0;
    if (v < 0) { neg = 1; v = -v; }
    if (v == 0) b[--i] = '0';
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    if (neg) b[--i] = '-';
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m57: accel detection + TCG/KVM bench protocol\n";
    wr(m1, sizeof(m1) - 1);

    char vendor[16];
    long accel = sys3(0x6401, (long)vendor, 0, 0);
    static const char h1[] = "m57: hypervisor='";
    wr(h1, sizeof(h1) - 1);
    {
        int n = 0;
        while (vendor[n] && n < 12) n++;
        wr(vendor, n);
    }
    static const char h2[] = "' accel=";
    wr(h2, sizeof(h2) - 1);
    wrdec(accel);
    wr("\n", 1);

    /* m35 类基准再跑一遍 (对照入口): 纯 syscall 延迟 */
    long t0 = sys3(0x6101, 0, 0, 0);
    {
        long i;
        for (i = 0; i < 50000; i++) {
            (void)sys3(0x6102, 0, 0, 0); /* timer_ms 往返 */
        }
    }
    long t1 = sys3(0x6101, 0, 0, 0);
    static const char h3[] = "m57: 50k timer_ms roundtrips=";
    wr(h3, sizeof(h3) - 1);
    wrdec(t1 - t0);
    static const char h4[] = " us -> ";
    wr(h4, sizeof(h4) - 1);
    wrdec((t1 - t0) / 50000);
    static const char h5[] = " us/call\n";
    wr(h5, sizeof(h5) - 1);

    int ok = accel == 0; /* TCG (windows host) — KVM 对照留 tools 脚本 */
    if (ok) {
        static const char m2[] = "m57: M57 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m57: M57 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
