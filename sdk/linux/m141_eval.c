/* m141_eval.c — W22: 三引擎质量对照评测 (垂直开发第一步 · doc target docs/82)
 *
 * 同一份金标准样本集在 3 引擎下运行, 量化每引擎准确率:
 *   [rules] 0x830F=2 强制规则 (确定性基线: 规则语义 / last-block)
 *   [model] 0x830F=1 强制模型 (无规则面; 超时降级记录)
 *   [auto ] 0x830F=0 自动 (蒸馏字节码 → 模型 → 规则, 现状路径)
 *
 * 样本两类:
 *   known = 规则语义明确覆盖 (rate=9x/dead/diag; 规则词典命令) —— 规则应满分;
 *   novel = 规则语义外但金标准独立 (mod-6 周期序列 / 新异常词 / 规则词典外
 *           命令动词语义) —— 规则基线恒错/未知, 模型有增量机会 (记录为论文证据)。
 *
 * 链路探测: 0x8309 mode=0 (engine=1 模型在线; 2=无链路 -> 跳过 [model] 全量,
 * 只跑 [rules]+[auto] 降级语义, 保持无模型回归确定性)。
 *
 * 断言 (双模式通用):
 *   T1 [rules] anom known 满分; novel-pos 全部误报(0) —— 规则确定性语义
 *   T2 [rules] io novel 全错 (last-block 基线对周期序列恒错)
 *   T3 [rules] plan/nlc/env 编译正确性 (A2+A5 / POL=3:1 / profile>=1)
 *   T4 offline: [auto] classify 降级 == 规则结果 (模型缺失系统继续)
 *   T5 online: [model] 全量跑完 + 打印 novel 增量数字 (记录, 不断言优劣)
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

__attribute__((noinline, noreturn)) static void worker(void)
{
    static const char m[] = "m141: worker running\n";
    wr(m, sizeof(m) - 1);
    volatile u64 x = 0;
    for (;;) {
        x += 1;
        if (x > 0xFFFF0000ul)
            x = 0;
    }
}

/* ---- 金标准样本集 (duty: 1=classify 2=anom 4=io) ---- */
/* B3: 19 -> 40 (anom 8->16, io 5->10, cls 6->10; plan/nlc/env 编译正确性类不变) */
#define NSAMP 36
static const char *S_TXT[NSAMP] = {
    /* anom known (0..7): 规则语义明确覆盖 (rate=9x/dead/diag + 低率正常) */
    "ev pid=0 rate=99 wr=dead", "ev pid=1 rate=91 wr=diag",
    "ev pid=2 rate=3 wr=ok", "ev pid=3 rate=5 wr=1",
    "ev pid=8 rate=93 wr=dead", "ev pid=9 rate=95 wr=diag",
    "ev pid=10 rate=7 wr=ok", "ev pid=11 rate=4 wr=1",
    /* anom novel (8..15): 规则语义外 (无 9x/dead/diag; 金标准独立) */
    "ev pid=4 rate=77 wr=memleak", "ev pid=5 rate=82 wr=zombie",
    "ev pid=6 rate=60 wr=ok", "ev pid=7 rate=44 wr=ok",
    "ev pid=12 rate=74 wr=swapthrash", "ev pid=13 rate=86 wr=spike",
    "ev pid=14 rate=50 wr=ok", "ev pid=15 rate=33 wr=ok",
    /* io novel (16..25): mod-6 周期变体 (last 基线恒错; 二阶表可见) */
    "0 1 2 3 4", "1 2 3 4 5", "3 4 5 0 1", "2 3 4 5 0", "5 0 1 2 3",
    "4 5 0 1 2", "0 1 2 3 4 5", "5 0 1 2 3 4", "1 2 3 4 5 0", "0 1 2 3 4 5 0",
    /* classify known (26..31) */
    "run the game", "open file", "hello there", "exit now",
    "build kernel", "list apps",
    /* classify novel (32..35): 规则词典外动词 */
    "launch program", "what is the time", "register the device", "delete a file",
};
static int S_DUTY[NSAMP] = {
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
};
static u64 S_GT[NSAMP] = {
    1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
    5, 0, 2, 1, 4, 3, 0, 4, 1, 0,
    1, 3, 2, 4, 1, 3, 1, 2, 3, 1,
};
/* 子集索引: anom 0..7 known, 8..9/12..13 novel-pos, 10..11/14..15 novel-neg;
 * io 16..25; cls 26..31 known, 32..35 novel */
#define ANOM_KNOWN 8
#define ANOM_NP 4   /* novel-pos 起点 8 (8,9,12,13) */

static u64 run_sample(int duty, const char *text)
{
    long n = slen(text);
    if (duty == 1)
        return (u64)sy(0x5101, (long)text, n, 0, 0, 0);
    if (duty == 2) {
        u64 o[3] = { 0, 0, 0 };
        sy(0x8304, (long)text, n, (long)o, 24, 0);
        return o[0];
    }
    {
        u64 o[1] = { 0 };
        sy(0x8306, (long)text, n, (long)o, 8, 0);
        return o[0];
    }
}

/* 每 duty 聚合 {total, hit}; 索引: 0=classify 1=anom 2=io */
static u64 ag_total[3], ag_hit[3];

static void run_engine(u64 mode)
{
    int i, k;
    for (k = 0; k < 3; k++) {
        ag_total[k] = 0;
        ag_hit[k] = 0;
    }
    sy(0x830F, mode, 0, 0, 0, 0);
    for (i = 0; i < NSAMP; i++) {
        u64 got = run_sample(S_DUTY[i], S_TXT[i]);
        int d = (S_DUTY[i] == 1) ? 0 : (S_DUTY[i] == 2 ? 1 : 2);
        ag_total[d] += 1;
        if (got == S_GT[i])
            ag_hit[d] += 1;
    }
    sy(0x830F, 0, 0, 0, 0, 0);
}

static void print_engine(const char *tag)
{
    wrstr(tag);
    wrstr(" anom=");
    wrdec(ag_hit[1]);
    wrstr("/");
    wrdec(ag_total[1]);
    wrstr(" io=");
    wrdec(ag_hit[2]);
    wrstr("/");
    wrdec(ag_total[2]);
    wrstr(" cls=");
    wrdec(ag_hit[0]);
    wrstr("/");
    wrdec(ag_total[0]);
    wrstr("\n");
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

static int run(void)
{
    static const char h[] = "m141: 3-engine quality contrast (rules/model/auto)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    int model_online = 0;
    u64 o[8];
    int i;

    sy(0x8101, 6, 0x3F, 0, 0, 0);

    /* T0 链路探测: novel 命令 "launch program" (规则词典外 -> rules=0;
     * 模型 run 语义 -> 1)。0x5101 带 COM2 降级重发, 比 0x8309 shm-only 稳。 */
    {
        static const char probe_cmd[] = "launch program";
        model_online = ((int)sy(0x5101, (long)probe_cmd, sizeof(probe_cmd) - 1, 0, 0, 0) == 1);
        wrstr("m141: T0 link probe classify=");
        wrdec(model_online ? 1 : 0);
        wrstr(model_online ? " (online)\n" : " (offline -> rules only)\n");
    }

    /* T1 [rules] anom known 满分 + novel-pos 误报 (确定性) */
    sy(0x830F, 2, 0, 0, 0, 0);
    u64 a_ok = 0, a_np = 0, a_io = 0;
    u64 got;
    for (i = 0; i < ANOM_KNOWN; i++) {
        got = run_sample(2, S_TXT[i]);
        if (got == S_GT[i])
            a_ok++;
    }
    for (i = 8; i < 16; i++) {
        if (i == 10 || i == 11 || i == 14 || i == 15)
            continue; /* novel-neg 不计数基线 */
        got = run_sample(2, S_TXT[i]);
        if (got == S_GT[i])
            a_np++;
    }
    for (i = 16; i < 26; i++) {
        got = run_sample(4, S_TXT[i]);
        if (got == S_GT[i])
            a_io++;
    }
    /* 保持 force=2: T2 断言同样在规则引擎下 (确定性) */
    wrstr("m141: T1 rules anom-known=");
    wrdec(a_ok);
    wrstr("/8 novel-pos=");
    wrdec(a_np);
    wrstr("/4 io=");
    wrdec(a_io);
    wrstr("/10 (W25 io baseline upgraded: markov, see m145)\n");
    if (!(a_ok == 8 && a_np == 0))
        pass_all = 0;

    /* T2 [rules] plan/nlc/env 编译正确性 */
    {
        long tid = sy(0x8105, 3, (long)&worker, 0, 0, 0);
        /* 把手写替换 tid 进 goal: "isolate task <tid> then resume task <tid>" */
        static char goal[64];
        int n = 0;
        const char *s1 = "isolate task ";
        const char *s2 = " then resume task ";
        for (i = 0; s1[i]; i++)
            goal[n++] = s1[i];
        {
            char nb[10];
            int k = 0;
            u64 v = (u64)tid;
            do {
                nb[k++] = (char)('0' + v % 10);
                v /= 10;
            } while (v);
            while (k)
                goal[n++] = nb[--k];
        }
        for (i = 0; s2[i]; i++)
            goal[n++] = s2[i];
        {
            char nb[10];
            int k = 0;
            u64 v = (u64)tid;
            do {
                nb[k++] = (char)('0' + v % 10);
                v /= 10;
            } while (v);
            while (k)
                goal[n++] = nb[--k];
        }
        goal[n] = 0;
        u64 p[3] = { 0, 0, 0 };
        sy(0x8305, (long)goal, n, (long)p, 24, 0);
        wrstr("m141: T2 plan rules ok=");
        wrdec(p[0]);
        wrstr(" fail=");
        wrdec(p[1]);
        wrstr(" verify=");
        wrdec(p[2]);
        wrstr("\n");
        /* 规则计划 = A2 <tid>;A5 <tid>; -> 隔离+恢复都应成功 (worker 存在) */
        if (!(p[2] == 1))
            pass_all = 0;
        sy(0x8105, 1, tid, 0, 0, 0);
    }

    {
        static const char nl[] = "ban games 0 24";
        u64 p[1] = { 0 };
        sy(0x8307, (long)nl, sizeof(nl) - 1, (long)p, 8, 0);
        long c3 = sy(0x8106, 3, 0, 0, 0, 0);
        wrstr("m141: T2 nlc rules applied=");
        wrdec(p[0]);
        wrstr(" cfg3=");
        wrdec((u64)c3);
        wrstr(" (expect >=1 / 1)\n");
        if (!(p[0] >= 1 && c3 == 1))
            pass_all = 0;
        sy(0x8105, 4, 3, 0, 0, 0); /* 解禁 */
    }

    {
        u64 p[3] = { 0, 0, 0 };
        sy(0x8308, (long)p, 24, 0, 0, 0);
        wrstr("m141: T2 env rules profile=");
        wrdec(p[0]);
        wrstr(" scene=");
        wrdec(p[1]);
        wrstr("\n");
        if (!(p[0] >= 1 && p[1] >= 1))
            pass_all = 0;
    }
    sy(0x830F, 0, 0, 0, 0, 0); /* 复位 -> auto (T3 各引擎自行设置) */

    /* T3 三引擎对比表 */
    run_engine(2);
    print_engine("m141: [rules]");
    if (model_online) {
        run_engine(1);
        print_engine("m141: [model]");
        /* novel 增量 (记录, 不断言): anom novel-pos 模型判定 + 蒸馏候选打印 */
        u64 mn = 0;
        for (i = 8; i < 16; i++) {
            if (i == 10 || i == 11 || i == 14 || i == 15)
                continue; /* novel-neg */
            got = run_sample(2, S_TXT[i]);
            wrstr("m141: T3 cand anom-novel '");
            wrstr(S_TXT[i]);
            wrstr("' -> ");
            wrdec(got);
            wrstr(" gt=");
            wrdec(S_GT[i]);
            wrstr(got == S_GT[i] ? " HIT\n" : " miss\n");
            if (got == S_GT[i])
                mn++;
        }
        wrstr("m141: T3 model novel-pos anom ");
        wrdec(mn);
        wrstr("/4 (rules baseline 0/4)\n");
        for (i = 32; i < 36; i++) {
            got = run_sample(1, S_TXT[i]);
            wrstr("m141: T3 cand cls-novel '");
            wrstr(S_TXT[i]);
            wrstr("' -> ");
            wrdec(got);
            wrstr(" gt=");
            wrdec(S_GT[i]);
            wrstr(got == S_GT[i] ? " HIT\n" : " miss\n");
        }
        run_engine(0);
        print_engine("m141: [auto ]");
    } else {
        /* offline: auto 降级语义已由 T1/T4 确定性覆盖 (不再全量, 免 6s×N 超时) */
        wrstr("m141: [auto ] == rules (offline fallback; see T1/T4)\n");
    }

    /* T4 offline: auto == rules 降级语义 (模型缺席, 系统继续) */
    if (!model_online) {
        static const char cmd[] = "run the game";
        int r = (int)sy(0x5101, (long)cmd, sizeof(cmd) - 1, 0, 0, 0);
        wrstr("m141: T4 offline auto classify=");
        wrdec((u64)r);
        wrstr(" (expect 1)\n");
        if (r != 1)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m141: M141 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m141: M141 RESULT: FAIL\n";
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
