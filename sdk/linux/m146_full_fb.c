/* m146_full_fb.c — W26: 五职责全自监督 (plan/nlc 动作后果验证, docs/87)
 *
 * W22 给 anom 加了行动验证 (隔离->state==2->审计 result=1)。W26 补齐 plan/nlc:
 * 内核 act_verify: KILL->state3 / ISOLATE->state2 / RESUME->state1 /
 * SET_CFG->cfg 读回 / ACK->pending 清; nlc 每条策略验证 cfg 读回。
 * 审计 result 字段 = verified 计数 (self-labeled)。
 *
 * T1 plan "isolate task <tid> then resume task <tid>" (force=2, 规则):
 *    ok=2 fail=0, 审计尾条 duty=3 result==2 (动作后果均获状态确证)
 * T2 nlc "ban games 0 24" (force=2): applied>=1, 审计尾条 duty=5 result>=1,
 *    0x8106 cfg3==1 (独立读回)
 * T3 plan "kill task <tid>": ok=1, 审计尾条 duty=3 result==1 (state==3)
 * T4 ANOM 已有 (m142): 无异常事件 -> 审计 duty=2 result==0 (无误报验证)
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

__attribute__((noinline, noreturn)) static void worker(void)
{
    static const char m[] = "m146: worker running\n";
    wr(m, sizeof(m) - 1);
    volatile u64 x = 0;
    for (;;) {
        x += 1;
        if (x > 0xFFFF0000ul)
            x = 0;
    }
}

/* 审计尾条 (0x830D, 88B): [engine,duty,out,a,b,result,text40] */
static unsigned char aud[88];
static long aud_tail_duty(void)
{
    long v = 0;
    int i;
    for (i = 0; i < 8; i++)
        v |= ((long)aud[8 + i]) << (8 * i);
    return v;
}
static long aud_tail_result(void)
{
    long v = 0;
    int i;
    for (i = 0; i < 8; i++)
        v |= ((long)aud[40 + i]) << (8 * i);
    return v;
}

/* 构造 "<verb> task <tid>" goal */
static void fmt_goal(char *out, const char *verb, long tid)
{
    int k = 0, i, n2;
    char nb[10];
    u64 v = (u64)tid;
    for (i = 0; verb[i]; i++)
        out[k++] = verb[i];
    n2 = 0;
    do {
        nb[n2++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (n2)
        out[k++] = nb[--n2];
    out[k] = 0;
}

static void plan_goal(const char *goal, long glen, u64 *o)
{
    sy(0x8305, (long)goal, glen, (long)o, 24, 0);
}

static int run(void)
{
    static const char h[] = "m146: full-duty self-supervision (plan/nlc verify)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;

    sy(0x8101, 6, 0x3F, 0, 0, 0);
    sy(0x830F, 2, 0, 0, 0, 0); /* 全程 force=2: 确定性引擎 */

    long tid = sy(0x8105, 3, (long)&worker, 0, 0, 0);
    wrstr("m146: T0 worker tid=");
    wrdec((u64)tid);
    wrstr("\n");
    if (tid < 0)
        pass_all = 0;

    /* T1 plan isolate+resume -> 两个动作后果验证 */
    {
        char goal[64];
        int k = 0, i, n2;
        static const char s1[] = "isolate task ";
        static const char s2[] = " then resume task ";
        char nb[10];
        u64 v = (u64)tid;
        u64 o[3] = { 0, 0, 0 };
        for (i = 0; s1[i]; i++)
            goal[k++] = s1[i];
        n2 = 0;
        do {
            nb[n2++] = (char)('0' + v % 10);
            v /= 10;
        } while (v);
        while (n2)
            goal[k++] = nb[--n2];
        for (i = 0; s2[i]; i++)
            goal[k++] = s2[i];
        n2 = 0;
        v = (u64)tid;
        do {
            nb[n2++] = (char)('0' + v % 10);
            v /= 10;
        } while (v);
        while (n2)
            goal[k++] = nb[--n2];
        goal[k] = 0;
        plan_goal(goal, k, o);
        int n = (int)sy(0x830D, (long)aud, 88, 0, 0, 0);
        long duty = aud_tail_duty();
        long res = aud_tail_result();
        wrstr("m146: T1 plan ok=");
        wrdec(o[0]);
        wrstr(" fail=");
        wrdec(o[1]);
        wrstr(" audit duty=");
        wrdec((u64)duty);
        wrstr(" verified=");
        wrdec((u64)res);
        wrstr(" (expect 2/0/3/2)\n");
        if (!(n >= 1 && o[0] == 2 && o[1] == 0 && duty == 3 && res == 2))
            pass_all = 0;
    }

    /* T2 nlc ban games 0 24 -> cfg 后果验证 */
    {
        static const char nl[] = "ban games 0 24";
        u64 o[1] = { 0 };
        sy(0x8307, (long)nl, sizeof(nl) - 1, (long)o, 8, 0);
        int n = (int)sy(0x830D, (long)aud, 88, 0, 0, 0);
        long duty = aud_tail_duty();
        long res = aud_tail_result();
        long c3 = sy(0x8106, 3, 0, 0, 0, 0);
        wrstr("m146: T2 nlc applied=");
        wrdec(o[0]);
        wrstr(" audit duty=");
        wrdec((u64)duty);
        wrstr(" verified=");
        wrdec((u64)res);
        wrstr(" cfg3=");
        wrdec((u64)c3);
        wrstr(" (expect >=1/5/>=1/1)\n");
        if (!(n >= 1 && o[0] >= 1 && duty == 5 && res >= 1 && c3 == 1))
            pass_all = 0;
    }

    /* T3 plan kill -> 后果验证 state==3 */
    {
        char goal[64];
        long k;
        u64 o[3] = { 0, 0, 0 };
        fmt_goal(goal, "kill task ", tid);
        k = 0;
        while (goal[k])
            k++;
        plan_goal(goal, k, o);
        int n = (int)sy(0x830D, (long)aud, 88, 0, 0, 0);
        long res = aud_tail_result();
        wrstr("m146: T3 plan-kill ok=");
        wrdec(o[0]);
        wrstr(" verified=");
        wrdec((u64)res);
        wrstr(" (expect 1/1)\n");
        if (!(n >= 1 && o[0] == 1 && res == 1))
            pass_all = 0;
    }

    /* T4 正常事件 (无动作) -> 审计 duty=2 result==0 (无误报验证) */
    {
        static const char ok[] = "ev pid=0 rate=3 wr=ok";
        u64 a[3] = { 0, 0, 0 };
        sy(0x8304, (long)ok, sizeof(ok) - 1, (long)a, 24, 0);
        int n = (int)sy(0x830D, (long)aud, 88, 0, 0, 0);
        long duty = aud_tail_duty();
        long res = aud_tail_result();
        wrstr("m146: T4 normal audit duty=");
        wrdec((u64)duty);
        wrstr(" verified=");
        wrdec((u64)res);
        wrstr(" (expect 2/0)\n");
        if (!(n >= 1 && duty == 2 && res == 0))
            pass_all = 0;
    }
    sy(0x830F, 0, 0, 0, 0, 0);
    sy(0x8105, 4, 3, 0, 0, 0); /* 解禁 */

    if (pass_all) {
        static const char m2[] = "m146: M146 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m146: M146 RESULT: FAIL\n";
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
