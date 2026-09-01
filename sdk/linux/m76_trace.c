/* m76_trace.c — M76: syscall trace 工具化 (后台记录/过滤/统计)
 *
 * 1. trace_bg(1) → 若干 syscall (含写日志) → trace_stats:
 *    total>=2 (含统计读自身), nonzero>=1
 * 2. trace_filter(1) (仅 write) → 3 次 write → stats 前后差分:
 *    d_total == 3 (过滤生效: 其它 syscall 不记)
 * 3. trace_bg(0) → PASS
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

void _start(void)
{
    static const char m1[] = "m76: syscall trace toolkit v0\n";
    wr(m1, sizeof(m1) - 1);

    /* 1) 后台记录 */
    (void)sys3(0x7701, 1, 0, 0);
    (void)sys3(0x7703, 0, 0, 0); /* 无过滤 */
    (void)sys3(0x7702, (long)st, 0, 0);
    u64 t0 = st[0], nz0 = st[1];

    static const char s1[] = "m76: sample line\n";
    wr(s1, sizeof(s1) - 1);
    (void)sys3(0x7702, (long)st, 0, 0);
    u64 t1 = st[0], nz1 = st[1];
    int ok1 = t1 > t0 && nz1 >= 1;

    /* 2) 过滤 only write */
    (void)sys3(0x7703, 1, 0, 0);
    (void)sys3(0x7702, (long)st, 0, 0);
    u64 t2 = st[0];
    wr(s1, sizeof(s1) - 1);
    wr(s1, sizeof(s1) - 1);
    wr(s1, sizeof(s1) - 1);
    (void)sys3(0x7702, (long)st, 0, 0);
    u64 t3 = st[0];
    long d = (long)(t3 - t2);
    int ok2 = d == 3; /* 只有 3 次 write 被记录 */

    /* 3) 关闭 */
    (void)sys3(0x7701, 0, 0, 0);
    (void)sys3(0x7703, 0, 0, 0);

    static const char h1[] = "m76: t0=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)t0);
    static const char h2[] = " t1=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)t1);
    static const char h3[] = " d_filter=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)d);
    wr("\n", 1);

    if (ok1 && ok2) {
        static const char m2[] = "m76: M76 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m76: M76 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
