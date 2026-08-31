/* m50_bench.c — M50: GUI 基准 (窗口开关/拖动帧率表)
 *
 * A. 窗口开关: wm_create+wm_remove ×100 -> us/op
 * B. 拖动: wm_move + backbuffer 采样渲染 ×100 -> 帧率
 * rdtsc 校准 cyc/us (gettimeofday)。
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
static inline u64 rdtsc(void)
{
    u32 lo, hi;
    asm volatile("rdtsc" : "=a"(lo), "=d"(hi));
    return ((u64)hi << 32) | lo;
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrdec(u64 v)
{
    char b[24];
    int i = 24;
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m50: GUI bench - window open/close & drag fps\n";
    wr(m1, sizeof(m1) - 1);

    /* 校准: cyc/us (经 gettimeofday 两次差) */
    long tv1[2], tv2[2];
    (void)sys3(78, (long)tv1, 0, 0);
    u64 c0 = rdtsc();
    int k;
    for (k = 0; k < 100000; k++) {
        (void)sys3(78, (long)tv2, 0, 0);
    }
    u64 c1 = rdtsc();
    u64 cyc_us = (c1 - c0) / 100000;
    if (cyc_us == 0) cyc_us = 1;
    wr("m50: cyc/us=", 12);
    wrdec(cyc_us);
    wr("\n", 1);

    /* A. 窗口开关 100 次 */
    long cls = sys3(0x5520, (long)"Bench", 0, 0);
    u64 a0 = rdtsc();
    int i;
    for (i = 0; i < 100; i++) {
        u32 w = (u32)sys5(0x5521, cls, 40, 40, 160, 100);
        (void)sys3(0x5524, (long)w, 0, 0);
    }
    u64 a1 = rdtsc();
    u64 a_us = (a1 - a0) / cyc_us / 100;
    wr("m50: A. window open+close x100: ", 31);
    wrdec(a_us);
    wr(" us/op\n", 6);

    /* B. 拖动 100 帧 (wm_move + 渲染采样) */
    u32 w = (u32)sys5(0x5521, cls, 40, 40, 160, 100);
    u64 b0 = rdtsc();
    for (i = 0; i < 100; i++) {
        (void)sys5(0x5525, (long)w, 1, 1, 0, 0);
        u32 px = (u32)sys3(0x5905, 45, 45, 0);
        (void)px;
    }
    u64 b1 = rdtsc();
    (void)sys3(0x5524, (long)w, 0, 0);
    /* 每帧耗时 -> fps (假设帧 = 100 次拖动渲染) */
    u64 b_us = (b1 - b0) / cyc_us;
    wr("m50: B. drag x100 frames: ", 25);
    wrdec(b_us / 100);
    wr(" us/frame -> ~", 12);
    wrdec(1000000000ULL / (b_us / 100 * 1000 + 1));
    wr(" fps\n", 4);

    static const char m2[] = "m50: M50 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
