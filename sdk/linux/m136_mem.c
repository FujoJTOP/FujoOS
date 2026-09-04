/* m136_mem.c — W20 p6: 内存拓扑 (0x8F02; -m 4096 QEMU 大内存)
 *
 * 断言:
 *   T1 mem_topology: (usable_total, high_usable, mapped_pages)
 *   T2 usable >= 2GiB (大盘存在)
 *   T3 high_usable > 0 (>1GiB 可用区)
 *   T4 mapped_pages > 0 (高位 RAM 已映射)
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
static void wrmb(u64 bytes)
{
    char b[22];
    int i = 22;
    u64 v = bytes / (1024 * 1024);
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
    wr("MiB", 3);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}

static u64 info[3];

static void run(void)
{
    static const char h[] = "m136: memory topology (W20 p6)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;

    long ret = sy(0x8F02, (long)info, 0, 0, 0, 0);
    u64 usable = info[0], high = info[1], mapped = info[2];

    wrstr("m136: T1 usable=");
    wrmb(usable);
    wrstr(" high=");
    wrmb(high);
    wrstr(" mapped_pages=");
    wrdec(mapped);
    wrstr("\n");
    if (ret != 0) pass = 0;

    wrstr("m136: T2 usable>=2GiB\n");
    if (usable < (2UL << 30)) pass = 0;

    wrstr("m136: T3 high>0\n");
    if (high == 0) pass = 0;

    wrstr("m136: T4 mapped>0\n");
    if (mapped == 0) pass = 0;

    if (pass) {
        static const char m2[] = "m136: M136 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m136: M136 RESULT: FAIL\n";
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
