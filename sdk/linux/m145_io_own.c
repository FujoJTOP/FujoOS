/* m145_io_own.c — W25: I/O 预测职责所有权重判 (二阶马尔可夫基线 vs 模型, docs/86)
 *
 * 背景: W22 实测 io 双引擎均差 (last-num 0/5, 模型 1/5) —— 该职责悬而未决。
 * W25 内核新增二阶马尔可夫基线 (io_markov, engine=4): 自训练访问流
 * (跨调用积累), (a,b)->c 转换表, 反向扫描最近后继。基线命中即零模型调用。
 *
 *   T1 [rules] force=2: 5 个周期样本 (流学习->预测): 预期 >=3/5 (W22 last 0/5)
 *   T2 [auto ] force=0: 基线优先 -> 预期 >=4/5; 模型调用 <=2 (仅基线 miss)
 *   T3 [model] (在线): 纯模型记录 (W22 实测 1/5; 不断言, 打印对照)
 * 结论: 确定性基线在周期流上 > 模型 => io 职责所有权 = 基线; 模型仅基线 miss 时辅助。
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

static long slen(const char *s)
{
    long n = 0;
    while (s[n])
        n++;
    return n;
}

/* 5 个 mod-6 周期样本: "N N+1 N+2 N+3 N+4" (gt = 最近出现的后继/周期续写) */
#define NIO 5
static const char *SEQ[NIO] = { "0 1 2 3 4", "1 2 3 4 5", "3 4 5 0 1",
                                "2 3 4 5 0", "5 0 1 2 3" };
static u64 GT[NIO] = { 5, 0, 2, 1, 4 };

static u64 run_io(int idx)
{
    u64 o[1] = { 0 };
    sy(0x8306, (long)SEQ[idx], slen(SEQ[idx]), (long)o, 8, 0);
    return o[0];
}

static u64 run_engine(u64 mode)
{
    u64 hits = 0;
    int i;
    sy(0x830F, mode, 0, 0, 0, 0);
    for (i = 0; i < NIO; i++) {
        u64 got = run_io(i);
        if (got == GT[i])
            hits++;
    }
    sy(0x830F, 0, 0, 0, 0, 0);
    return hits;
}

static int run(void)
{
    static const char h[] = "m145: IO ownership rejudgement (markov baseline vs model)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 st[4];
    int i;

    sy(0x8101, 6, 0x3F, 0, 0, 0);
    sy(0x830C, (long)st, 0, 0, 0, 0);
    u64 calls0 = st[0];

    /* T1 [rules] 确定性 (markov + last) */
    u64 rh = run_engine(2);
    wrstr("m145: T1 [rules] io=");
    wrdec(rh);
    wrstr("/5 (W22 last-num baseline 0/5)\n");
    if (rh < 3)
        pass_all = 0;

    /* T2 [auto] 基线优先 + 模型调用记录 */
    sy(0x830C, (long)st, 0, 0, 0, 0);
    u64 c0 = st[0];
    u64 ah = run_engine(0);
    sy(0x830C, (long)st, 0, 0, 0, 0);
    u64 dc = st[0] - c0;
    wrstr("m145: T2 [auto] io=");
    wrdec(ah);
    wrstr("/5 model-calls+=");
    wrdec(dc);
    wrstr(" (expect >=4, calls<=2)\n");
    if (!(ah >= 4 && dc <= 2))
        pass_all = 0;

    /* T2b 明细: markov 引擎标识 (审计 duty=4 条目: 非模型即基线/规则) */
    {
        u64 aud[16 * 11];
        long n = sy(0x830D, (long)aud, sizeof(aud), 0, 0, 0);
        u64 mk = 0, other = 0;
        for (i = 0; i < n && i < 16; i++) {
            if (aud[i * 11 + 1] != 4 || aud[i * 11 + 0] == 0)
                continue;
            if (aud[i * 11 + 0] == 4)
                mk++;
            else
                other++;
        }
        wrstr("m145: T2b io audit markov=");
        wrdec(mk);
        wrstr(" nonMarkov=");
        wrdec(other);
        wrstr("\n");
        if (mk < 3)
            pass_all = 0;
    }

    /* T3 [model] 在线对照 (记录) */
    {
        static const char probe[] = "launch program";
        int pr = (int)sy(0x5101, (long)probe, sizeof(probe) - 1, 0, 0, 0);
        if (pr == 1) {
            u64 mh = run_engine(1);
            wrstr("m145: T3 [model] io=");
            wrdec(mh);
            wrstr("/5 (W22 qwen2.5:7b: 1/5; 记录, 不断言)\n");
        } else {
            wrstr("m145: T3 [model] offline (skip)\n");
        }
    }
    (void)calls0;

    if (pass_all) {
        static const char m2[] = "m145: M145 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m145: M145 RESULT: FAIL\n";
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
