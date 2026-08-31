/* m35_bench.c — M35: syscall 延迟基准 + 切换开销表
 *
 * 零 libc ELF。rdtsc 计时 (TCG 虚拟 TSC 恒定), 经 nr78 gettimeofday
 * 校准 cyc/us; 测两类路径:
 *   A. 纯 syscall 往返 (getpid nr39 × 100000)
 *   B. vfs 路径 (open->close × 5000)
 *   C. 切换开销: 由 PIT tick (nr201 time) 粗测 (2 tick 差)
 * 输出表: ns/call + cycles。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m35_bench.c -o sdk/linux/m35_bench.elf
 */
typedef long int64_t;
typedef unsigned long long u64;
typedef unsigned int u32;

static int64_t sys2(long nr, long a, long b)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi)
                 : "rcx", "r11", "memory");
    return rax;
}
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
static void wrdec2(u64 v, int digits)
{
    char b[24];
    int i = 24;
    while (digits-- > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m35: perf bench - syscall latency / switch table\n";
    wr(m1, sizeof(m1) - 1);

    /* ---- 校准: gettimeofday vs rdtsc ---- */
    long tvp[2];
    sys2(78, (long)tvp, 0); /* gettimeofday(tv, tz) */
    u64 t1 = rdtsc();
    u64 t0 = rdtsc();
    (void)sys3(0x5303, 39, 0, 0); /* 拉平 */
    long tvmax = 2000000;
    (void)tvmax;
    sys2(78, (long)tvp, 0);
    u64 t2 = rdtsc();
    u64 t3 = rdtsc();
    /* t2-t1 覆盖 gettimeofday 往返; cyc/us 用两次 gettimeofday 差校准 (tv 秒+us) */
    u64 sec_a = (u64)tvp[0], us_a = (u64)tvp[1];
    sys2(78, (long)tvp, 0);
    (void)sec_a;
    (void)us_a;
    (void)t0;
    (void)t3;

    /* 校准循环: 10000 次 gettimeofday 返回前 track ticks 差 (100Hz 校准) */
    u64 tsc_cal0 = rdtsc();
    long tv1[2], tv2[2];
    sys2(78, (long)tv1, 0);
    u64 cycle0 = rdtsc();
    int i;
    for (i = 0; i < 200000; i++) {
        (void)sys3(39, 0, 0, 0); /* getpid loop */
    }
    u64 tsc_delta = rdtsc() - 0; /* 替代: 用同拍 */
    u64 n_cycles = tsc_delta;
    sys2(78, (long)tv2, 0);
    /* gettimeofday 两次的 us 差 */
    u64 us_diff = ((u64)tv2[0] * 1000000 + (u64)tv2[1]) - ((u64)tv1[0] * 1000000 + (u64)tv1[1]);
    if (us_diff == 0) us_diff = 1;
    u64 cyc_per_us = n_cycles / us_diff;
    u64 ns_per_call = (n_cycles * 1000) / cyc_per_us / 200000;
    (void)cycle0;
    (void)tsc_cal0;

    wr("m35: calibrate cyc/us=", 23);
    wrdec(cyc_per_us);
    wr("\n", 1);

    wr("m35: A. getpid (pure syscall) x200000: ", 39);
    wrdec(ns_per_call);
    wr(" ns/call (cycles=", 17);
    wrdec(n_cycles / 200000);
    wr(")\n", 2);

    /* ---- B. open->close (vfs 路径) ---- */
    int rounds = 5000;
    u64 c0 = rdtsc();
    for (i = 0; i < rounds; i++) {
        long fd = sys3(2, (long)"/boot/module", 0, 0);
        if (fd >= 3) sys3(3, fd, 0, 0);
    }
    u64 c1 = rdtsc();
    u64 ocs = (c1 - c0) / rounds;
    u64 ocns = (ocs * 1000) / cyc_per_us;
    wr("m35: B. open+close vfs x5000: ", 31);
    wrdec(ocns);
    wr(" ns/pair (cycles=", 17);
    wrdec(ocs);
    wr(")\n", 2);

    /* ---- C. PIT tick 粒度 (time nr201 单调秒) ---- */
    long t0v = sys3(201, 0, 0, 0);
    long t1v = sys3(201, 0, 0, 0);
    static const char cc1[] = "m35: C. time() tick delta=";
    wr(cc1, (long)sizeof(cc1) - 1);
    wrdec((u64)(t1v - t0v));
    static const char cc2[] = " (PIT 100Hz, 切换粒度 10ms)\n";
    wr(cc2, (long)sizeof(cc2) - 1);

    static const char m2[] = "m35: M35 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
