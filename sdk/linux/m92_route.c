/* m92_route.c — M92: 意图路由增强 (qwen/qwen3-0.6b 切换, 对照表)
 *
 * 1. route_set(1) (qwen3-0.6b) → classify("open the file") == OPEN
 * 2. route_set(0) (qwen) → 同输入 → 同判定 (跨引擎一致)
 * 3. route_table → 3×3: samples (run/open/query) × 3 engines 全 ==
 *    规则判定 → PASS
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

static u64 tbl[9];

void _start(void)
{
    static const char m1[] = "m92: intent routing (qwen3-0.6b switch, tabulation)\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x8201, 1, 0, 0);
    long v1 = sys3(0x8202, (long)"open the file", 14, 0);
    (void)sys3(0x8201, 0, 0, 0);
    long v0 = sys3(0x8202, (long)"open the file", 14, 0);

    (void)sys3(0x8203, (long)tbl, 0, 0);
    /* 期望: sample0(run) → RUN(1), sample1(open) → OPEN(3),
       sample2(hello) → QUERY(2); 3 引擎同列 */
    int ok = v1 == v0 && v1 == 3;
    int expect[3] = { 1, 3, 2 };
    for (int s = 0; s < 3; s++) {
        for (int e = 0; e < 3; e++) {
            if (tbl[s * 3 + e] != (u64)expect[s]) {
                ok = 0;
            }
        }
    }

    static const char h1[] = "m92: v1=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)v1);
    static const char h2[] = " v0=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)v0);
    static const char h3[] = " t00=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)tbl[0]);
    static const char h4[] = " t04=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)tbl[4]);
    wr("\n", 1);

    if (ok) {
        static const char m2[] = "m92: M92 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m92: M92 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
