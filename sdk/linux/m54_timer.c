/* m54_timer.c — M54: 高精度定时器 + 帧同步验证
 *
 * 0x6101 timer_us() / 0x6102 timer_ms() / 0x6103 sleep_us(us)
 * 0x6104 frame_wait(us_per_frame) / 0x6105 timer_info(ptr)
 * 流程: us 前进 (两次 ≥90ms) -> sleep(200000) 实测 ~200ms ±50 ->
 * frame_wait(50000) x3 -> 帧间距 ~50ms ±20 -> PASS。
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
static void wrdec(long v)
{
    char b[24];
    int i = 24;
    int neg = 0;
    if (v < 0) {
        neg = 1;
        v = -v;
    }
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    if (neg) b[--i] = '-';
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m54: high-resolution timer & frame sync\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6100, 0, 0, 0); /* 校准起点 */
    {
        long spin;
        for (spin = 0; spin < 2000000; spin++) { __asm__ volatile("" ::: "memory"); }
    }
    u64 info[2];
    (void)sys3(0x6105, (long)info, 0, 0);
    static const char c1[]="m54: cyc/us="; wr(c1, sizeof(c1)-1);
    wrdec((long)info[0]);
    static const char s1[] = " ticks=";
    wr(s1, 7);
    wrdec((long)info[1]);
    wr("\n", 1);

    long t0 = sys3(0x6101, 0, 0, 0);
    long t1 = sys3(0x6101, 0, 0, 0);
    static const char c2[]="m54: us advance="; wr(c2, sizeof(c2)-1);
    wrdec(t1 - t0);
    wr("\n", 1);

    long b0 = sys3(0x6101, 0, 0, 0);
    (void)sys3(0x6103, 200000, 0, 0);
    long b1 = sys3(0x6101, 0, 0, 0);
    long slept = b1 - b0;
    static const char c3[]="m54: sleep(200000) took="; wr(c3, sizeof(c3)-1);
    wrdec(slept);
    wr(" us\n", 4);

    /* 帧同步: 3 帧边界间隔 */
    long f0 = sys3(0x6101, 0, 0, 0);
    int i;
    long frame_gap = 0;
    for (i = 0; i < 3; i++) {
        (void)sys3(0x6104, 50000, 0, 0);
        long c = sys3(0x6101, 0, 0, 0);
        if (i > 0) {
            frame_gap = c - f0;
        }
        f0 = c;
    }
    static const char c4[]="m54: frame gap="; wr(c4, sizeof(c4)-1);
    wrdec(frame_gap);
    wr(" us (want ~50000)\n", 16);

    int ok = slept >= 150000 && slept <= 250000 && frame_gap >= 40000 && frame_gap <= 60000;
    if (ok) {
        static const char m2[] = "m54: M54 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m54: M54 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
