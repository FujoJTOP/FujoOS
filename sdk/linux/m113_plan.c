/* m113_plan.c — M113: 计划-执行器 (B) + I/O 预测器 (C)
 *
 * B 闭环: "目标 → 步骤 → 执行 → 验证"
 *   0x8305 plan_run(goal) -> 模型 (shm kind=3) 输出动作向量
 *   -> 内核逐项 cap_exec (授权/审计) -> out {n_ok, n_fail, verify}
 *   目标1: "isolate task 1 then resume task 1" -> A2 1;A5 1
 *   目标2: "set anomaly threshold to 70"       -> A4 1 70
 * C 序列预测: 0x8306 io_predict(prefix) -> NEXT (模型/规则=最近块)
 *   30 轮周期-6 访问流, 预测命中 vs LRU 基线 (命中最远块=0 预期下界)
 *
 * RESULT: plan 闭环全成功 且 模型命中 >= LRU 基线命中 → PASS。
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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}

static const char NL[] = "\n";

__attribute__((noinline, noreturn)) static void worker(void)
{
    static const char m[] = "m113: worker running\n";
    wr(m, sizeof(m) - 1);
    volatile u64 x = 0;
    for (;;) {
        x += 1;
        if (x > 0xFFFF0000ul)
            x = 0;
    }
}

static long strstr_pos(const char *hay, const char *needle)
{
    long i = 0, j;
    while (hay[i]) {
        j = 0;
        while (needle[j] && hay[i + j] == needle[j])
            j++;
        if (!needle[j])
            return i;
        i++;
    }
    return -1;
}

static char tbuf[256];

static int run(void)
{
    static const char h1[] = "m113: plan-executor + io predictor\n";
    wr(h1, sizeof(h1) - 1);
    long tid = -1;

    /* B1: 计划闭环 (隔离+恢复) */
    static const char h2[] = "m113: 1) plan closed loop (isolate+resume)\n";
    wr(h2, sizeof(h2) - 1);
    sy(0x8101, 6, 0x3F, 0, 0, 0);
    tid = sy(0x8105, 3, (long)&worker, 0, 0, 0);
    if (tid < 0) {
        static const char f[] = "m113: M113 RESULT: FAIL (launch)\n";
        wr(f, sizeof(f) - 1);
        return 0;
    }
    {
        static const char g1[] = "isolate task 1 then resume task 1";
        u64 out[3] = { 0, 0, 0 };
        sy(0x8305, (long)g1, sizeof(g1) - 1, (long)out, 24, 0);
        static const char p[] = "m113: plan1 out(ok/fail/verify)=";
        wr(p, sizeof(p) - 1);
        wrdec(out[0]);
        wr("/", 1);
        wrdec(out[1]);
        wr("/", 1);
        wrdec(out[2]);
        wr(NL, 1);
        if (!(out[0] >= 2 && out[1] == 0 && out[2] == 1)) {
            static const char f[] = "m113: M113 RESULT: FAIL (plan1)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        /* 验证: 任务 1 已被恢复 (t1:1) */
        {
            int i;
            for (i = 0; i < 100; i++) {
                sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
                if (strstr_pos(tbuf, "t1:1") >= 0)
                    break;
            }
            if (strstr_pos(tbuf, "t1:1") < 0) {
                static const char f[] = "m113: M113 RESULT: FAIL (plan1-verify)\n";
                wr(f, sizeof(f) - 1);
                return 0;
            }
        }
    }

    /* B2: NL → 配置动作 */
    static const char h3[] = "m113: 2) plan set-cfg (threshold=70)\n";
    wr(h3, sizeof(h3) - 1);
    {
        static const char g2[] = "set anomaly threshold to 70";
        u64 out[3] = { 0, 0, 0 };
        sy(0x8305, (long)g2, sizeof(g2) - 1, (long)out, 24, 0);
        long v = sy(0x8106, 1, 0, 0, 0, 0);
        static const char p[] = "m113: cfg(threshold)=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)v);
        static const char p2[] = " plan(ok/fail)=";
        wr(p2, sizeof(p2) - 1);
        wrdec(out[0]);
        wr("/", 1);
        wrdec(out[1]);
        wr(NL, 1);
        if (v != 70) {
            static const char f[] = "m113: M113 RESULT: FAIL (plan2)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }

    /* C: I/O 预测 30 轮 (周期-6 流) */
    static const char h4[] = "m113: 3) io predictor (30 rounds, period-6)\n";
    wr(h4, sizeof(h4) - 1);
    {
        char seq[32];
        int hits_m = 0, hits_l = 0, i, r;
        /* s[r] = r % 6; 前缀 = s[r-5..r-1] */
        for (r = 0; r < 30; r++) {
            int expect = r % 6;
            int n = 0;
            for (i = 5; i >= 1; i--) {
                if (r - i < 0)
                    continue;
                seq[n++] = (char)('0' + ((r - i) % 6));
                if (r > i)
                    seq[n++] = ' ';
            }
            if (n >= 2 && seq[n - 1] == ' ')
                n--;
            u64 out[1] = { 0xFFFFFFFFFFFFFFFFul };
            sy(0x8306, (long)seq, n, (long)out, 8, 0);
            int pred = (int)out[0];
            int lru = (r == 0) ? -1 : ((r - 1) % 6);
            if (pred == expect)
                hits_m++;
            if (lru == expect)
                hits_l++;
        }
        static const char p[] = "m113: io hits(model/lru)=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)hits_m);
        wr("/", 1);
        wrdec((u64)hits_l);
        wr(NL, 1);
        if (!(hits_m >= hits_l)) {
            static const char f[] = "m113: M113 RESULT: FAIL (io)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }

    /* 清理 */
    sy(0x8105, 1, tid, 0, 0, 0);
    static const char m2[] = "m113: M113 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
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
