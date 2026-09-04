/* m129_smp.c — W17b: SMP AP 唤醒 (0x8B03 smp_state; docs/72)
 *
 * 断言 (必须 -smp 2 启动):
 *   T1 0x8B03 smp_state: ncpu>=2 (CPUID 探测到多核)
 *   T2 ap_online==1 (trampoline 执行标记 0xCAFEBABE + 完成标记 0xDEADBEEF
 *      均命中 + fujo_ap_main 已进入)
 *   T3 lapic_id==0 (调用者=BSP; AP1 在线 id=1)
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

static u64 buf[3]; /* ncpu, ap_online, lapic_id */

static void run(void)
{
    static const char h[] = "m129: smp_state probe (W17b)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;

    long ret = sy(0x8B03, (long)buf, 0, 0, 0, 0);
    u64 ncpu = buf[0], ap_online = buf[1], lapic_id = buf[2];

    wrstr("m129: T1 ncpu=");
    wrdec(ncpu);
    wrstr("\n");
    if (ret != 0 || ncpu < 2)
        pass_all = 0;

    wrstr("m129: T2 ap_online=");
    wrdec(ap_online);
    wrstr("\n");
    if (ap_online != 1)
        pass_all = 0;

    wrstr("m129: T3 lapic_id=");
    wrdec(lapic_id);
    wrstr("\n");
    if (lapic_id != 0)
        pass_all = 0;

    if (pass_all) {
        static const char m2[] = "m129: M129 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m129: M129 RESULT: FAIL\n";
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
