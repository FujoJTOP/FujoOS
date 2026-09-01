/* m115_five.c — M115: Five-AI 回归 (docs/44 扩展 · 五职能对照)
 *
 * 一次跑完 A-E + M95 链路, 每职能: 断言 + 基线 vs 模型 对照。
 * 所有职能复用 M112-114 的接口; 模型缺失时规则降级 (对照表标 rules)。
 *
 *   A 异常哨兵  20 分类 (10 正常 + 10 异常)    命中/误报 vs 基线
 *   B 计划-执行  1 闭环 (isolate+resume)       ok/fail/verify
 *   C I/O 预测  10 轮周期-6                    命中 vs LRU
 *   D NL 配置    禁玩全天 + 执行面拒绝           applied + enforce
 *   E 环境侦察  1 次扫描                        场景/档案
 *   链路       0x5101 classify 意图             intent==1
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

static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}

static const char NL[] = "\n";

__attribute__((noinline, noreturn)) static void worker(void)
{
    static const char m[] = "m115: worker running\n";
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
    static const char h[] = "m115: five-AI regression (A-E + link)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    sy(0x8101, 6, 0x3F, 0, 0, 0);

    /* 链路基线: 0x5101 */
    {
        static const char cmd[] = "run the game";
        int r = (int)sy(0x5101, (long)cmd, sizeof(cmd) - 1, 0, 0, 0);
        wrstr("m115: link intent=");
        wrdec((u64)r);
        wrstr(" (expect 1)\n");
        if (r != 1)
            pass_all = 0;
    }

    /* A 异常哨兵 (20) */
    {
        static const char ok1[] = "ev pid=0 rate=3 wr=ok";
        static const char ok2[] = "ev pid=0 rate=5 wr=1";
        static const char bad[] = "ev pid=0 rate=99 wr=dead";
        int hits = 0, fp = 0, i;
        for (i = 0; i < 20; i++) {
            const char *t = (i < 10) ? ((i % 2) ? ok1 : ok2) : bad;
            int expect = (i >= 10) ? 1 : 0;
            u64 o[3] = { 0, 0, 0 };
            sy(0x8304, (long)t, (long)(i < 10 ? 20 : 24), (long)o, 24, 0);
            if (expect && o[0] == 1)
                hits++;
            if (!expect && o[0] == 1)
                fp++;
        }
        wrstr("m115: A sentinel   hits=");
        wrdec((u64)hits);
        wrstr(" fp=");
        wrdec((u64)fp);
        wrstr(" (baseline 10/0)\n");
        if (!(hits >= 8 && fp <= 2))
            pass_all = 0;
    }

    /* B 计划闭环 */
    {
        long tid = sy(0x8105, 3, (long)&worker, 0, 0, 0);
        static const char g[] = "isolate task 1 then resume task 1";
        u64 o[3] = { 0, 0, 0 };
        int i;
        sy(0x8305, (long)g, sizeof(g) - 1, (long)o, 24, 0);
        for (i = 0; i < 100; i++) {
            sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
            if (strstr_pos(tbuf, "t1:1") >= 0)
                break;
        }
        wrstr("m115: B plan       ok=");
        wrdec(o[0]);
        wrstr(" fail=");
        wrdec(o[1]);
        wrstr(" verify=");
        wrdec(o[2]);
        wrstr("\n");
        if (!(o[0] >= 2 && o[1] == 0 && strstr_pos(tbuf, "t1:1") >= 0))
            pass_all = 0;
        sy(0x8105, 1, tid, 0, 0, 0);
    }

    /* C I/O 预测 (10 轮) */
    {
        char seq[32];
        int hits_m = 0, hits_l = 0, r, i;
        for (r = 0; r < 10; r++) {
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
            u64 o[1] = { 0 };
            sy(0x8306, (long)seq, n, (long)o, 8, 0);
            if ((int)o[0] == r % 6)
                hits_m++;
            if (r > 0 && ((r - 1) % 6) == r % 6)
                hits_l++;
        }
        wrstr("m115: C io         model=");
        wrdec((u64)hits_m);
        wrstr(" lru=");
        wrdec((u64)hits_l);
        wrstr("/10\n");
        if (hits_m < hits_l)
            pass_all = 0;
    }

    /* D NL 配置 + 执行面 */
    {
        static const char g[] = "ban games during 0 to 24";
        u64 o[1] = { 0 };
        long r;
        sy(0x8307, (long)g, sizeof(g) - 1, (long)o, 8, 0);
        r = sy(0x6601, 1, 0, 0, 0, 0);
        wrstr("m115: D nlc        applied=");
        wrdec(o[0]);
        wrstr(" enforce_rc=");
        wrdec((u64)r);
        wrstr(" (expect 0xffff..=-1)\n");
        if (!(o[0] >= 3 && r == -1))
            pass_all = 0;
        sy(0x8105, 4, 3, 0, 0, 0); /* 解禁 */
        sy(0x6601, 0, 0, 0, 0, 0);
    }

    /* E 环境侦察 */
    {
        u64 o[3] = { 0, 0, 0 };
        sy(0x8308, (long)o, 24, 0, 0, 0);
        wrstr("m115: E env        profile=");
        wrdec(o[0]);
        wrstr(" scene=");
        wrdec(o[1]);
        wrstr("\n");
        if (o[0] < 1 || o[1] < 1)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m115: M115 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m115: M115 RESULT: FAIL\n";
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
