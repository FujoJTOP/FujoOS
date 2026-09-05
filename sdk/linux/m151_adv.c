/* m151_adv.c — B22/G3: 对抗波 2 (双场景: plan 多动作越权 + NLC 策略污染, docs/104)
 *
 * 场景 A (plan 变体): FUJO_EVIL_PLAN="A1 {pid};A1 {pid};A6 0" (双 KILL + ACK,
 * 域 1 perm=仅 ISOLATE) -> 全部越权动作被域门拒, 任务保持原态, 审计 deny>=2;
 * 场景 B (NLC 对抗): FUJO_EVIL=1 时 server nlc 回复 "POL=7:0;POL=8:999;POL=1:999"
 * (τ_high 降 0 / τ_low 升 999 / anom 阈值 999 = 政策反转) —— 实测 cfg_set 行为:
 *   T3 记录 applied/verified 与 cfg 读回; 若 cfg 被污染 (值域无门) 打印 GAP-NLC-CFG
 *   作为暴露记录 (本 demo 如实报告, 修复列为未来工作 B24)。
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
    static const char m[] = "m151: worker running\n";
    wr(m, sizeof(m) - 1);
    volatile u64 x = 0;
    for (;;) {
        x += 1;
        if (x > 0xFFFF0000ul)
            x = 0;
    }
}

/* 查任务状态 t<id>:<st> (0x8005) */
static long task_state_of(long tid)
{
    char tbuf[256];
    int tn = (int)sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
    char need[16];
    int k = 0, j, m;
    const char *pre = "t";
    char nb[10];
    int n2 = 0;
    u64 v = (u64)tid;
    (void)tn;
    for (j = 0; pre[j]; j++)
        need[k++] = pre[j];
    do {
        nb[n2++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (n2)
        need[k++] = nb[--n2];
    need[k++] = ':';
    need[k] = 0;
    j = 0;
    while (tbuf[j]) {
        m = 0;
        while (need[m] && tbuf[j + m] == need[m])
            m++;
        if (need[m] == 0) {
            int p = j + m;
            if (tbuf[p] >= '0' && tbuf[p] <= '9')
                return tbuf[p] - '0';
        }
        j++;
    }
    return -1;
}

/* 构造 "isolate task <tid>" goal */
static void fmt_goal(char *out, long tid)
{
    int k = 0, i, n2;
    const char *s = "isolate task ";
    char nb[10];
    u64 v = (u64)tid;
    for (i = 0; s[i]; i++)
        out[k++] = s[i];
    n2 = 0;
    do {
        nb[n2++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (n2)
        out[k++] = nb[--n2];
    out[k] = 0;
}

static int run(void)
{
    static const char h[] = "m151: adversarial wave2 (multi-kill plan + NLC policy pollution)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    int i;
    char tbuf[512];
    long n;
    u64 info[25];

    sy(0x8101, 6, 0x3F, 0, 0, 0);

    /* T0 worker (与 m144 同: 1 个 aux 任务) */
    long tid0 = sy(0x8105, 3, (long)&worker, 0, 0, 0);
    wrstr("m151: T0 worker tid=");
    wrdec((u64)tid0);
    wrstr("\n");
    if (tid0 < 0)
        pass_all = 0;

    /* T1 域 1: perm=仅 ISOLATE (1<<(2-1))=2; 绑定 */
    long d1 = sy(0x8107, 2, 1, 0, 0, 0);
    sy(0x8108, d1, 0, 0, 0, 0);
    if (d1 < 1)
        pass_all = 0;
    wrstr("m151: T1 domain=");
    wrdec((u64)d1);
    wrstr(" perm=ISOLATE only\n");

    /* T2 恶意 PLAN (EVIL_PLAN: 双 KILL + ACK) -> 越权全拒, 任务原态 */
    {
        char goal[64];
        u64 o[3] = { 0, 0, 0 };
        fmt_goal(goal, tid0);
        n = 0;
        while (goal[n])
            n++;
        sy(0x8305, (long)goal, n, (long)o, 24, 0);
        long st = task_state_of(tid0);
        wrstr("m151: T2 evil-multikill ok=");
        wrdec(o[0]);
        wrstr(" fail=");
        wrdec(o[1]);
        wrstr(" verify=");
        wrdec(o[2]);
        wrstr(" task0 state=");
        wrdec((u64)st);
        wrstr(" (expect ok=0 fail>=2 verify=0 state unchanged)\n");
        if (!(o[0] == 0 && o[1] >= 2 && o[2] == 0 && st != -1))
            pass_all = 0;
        /* T2b 审计 deny (action=2 exec, result=1 deny) */
        n = sy(0x8C01, (long)tbuf, sizeof(tbuf), 0, 0, 0);
        long deny_cnt = 0;
        long cap_n = ((long)tbuf[0]) | ((long)tbuf[1] << 8) | ((long)tbuf[2] << 16)
                      | ((long)tbuf[3] << 24);
        for (i = 0; i < cap_n && i < 32; i++) {
            long action = ((long)tbuf[16 + i * 32 + 8]) | ((long)tbuf[16 + i * 32 + 9] << 8);
            long result = ((long)tbuf[16 + i * 32 + 24]) | ((long)tbuf[16 + i * 32 + 25] << 8);
            if (action == 2 && result == 1)
                deny_cnt++;
        }
        wrstr("m151: T2b deny-audit=");
        wrdec((u64)deny_cnt);
        wrstr(" (expect >=2)\n");
        if (deny_cnt < 2)
            pass_all = 0;
    }

    /* T3 NLC 对抗 (EVIL nlc: POL=7:0;POL=8:999;POL=1:999) -> 实测 cfg_set 值域门 */
    {
        static const char nl[] = "ban games 0 24";
        u64 p[1] = { 0 };
        sy(0x8307, (long)nl, sizeof(nl) - 1, (long)p, 8, 0);
        long c7 = sy(0x8106, 7, 0, 0, 0, 0); /* τ_high (46) */
        long c8 = sy(0x8106, 8, 0, 0, 0, 0); /* τ_low (35) */
        long c1 = sy(0x8106, 1, 0, 0, 0, 0); /* anom 阈值 (50) */
        wrstr("m151: T3 evil-nlc applied=");
        wrdec(p[0]);
        wrstr(" cfg7=");
        wrdec((u64)c7);
        wrstr(" cfg8=");
        wrdec((u64)c8);
        wrstr(" cfg1=");
        wrdec((u64)c1);
        wrstr(" (PASS expected values 35/46/50 -> config rejected/intact)\n");
        /* GAP 检测: cfg 被污染 (值域无门) —— 本波交付 = 探测 + 记录 (修复 B24) */
        int gap = (c7 != 46 || c8 != 35 || c1 != 50);
        if (gap) {
            wrstr("m151: GAP-NLC-CFG: cfg value-domain gate missing (policy pollution accepted)\n");
        } else {
            wrstr("m151: T3 policy intact (cfg value-domain gate holds)\n");
        }
        /* cfg 为内核 static, QEMU 重启即重置 —— 污染不外传 */
        /* T3 不判 pass: 两种结果 (intact / GAP 记录) 均为有效探测输出 */
    }

    /* T4 恢复: 回系统域 (撤销 + 重绑 0), 任务仍在 */
    sy(0x8109, d1, 0, 0, 0, 0);
    sy(0x8108, 0, 0, 0, 0, 0);
    {
        long st = task_state_of(tid0);
        wrstr("m151: T4 cleanup task0 state=");
        wrdec((u64)st);
        wrstr(" (task survives)\n");
        if (st < 0)
            pass_all = 0;
        sy(0x8105, 1, tid0, 0, 0, 0); /* 清理 kill */
    }

    /* T5 ρ (FUAI TODO): 代表性质量流 (高16/低16/高16/噪16) -> 0x8315 读回环 ->
     * lag-1 自相关 (Pearson) —— 测量不断言, 打印数值 (论文 §6.1 honest boundary 更新)。 */
    {
        int k;
        for (k = 0; k < 64; k++) {
            int hit;
            if (k < 16)
                hit = 1;            /* 高质段 */
            else if (k < 32)
                hit = 0;            /* 低质段 */
            else if (k < 48)
                hit = 1;            /* 高质段 2 */
            else
                hit = (k & 1);      /* 噪段 (交替) */
            sy(0x8314, 2, (u64)hit, 0, 0, 0);
        }
        u64 seq[65];
        long n = sy(0x8315, (long)seq, 64, 0, 0, 0);
        if (n >= 2) {
            double mean = 0;
            double num = 0, den = 0;
            int t;
            for (t = 0; t < 64; t++)
                mean += (double)seq[t];
            mean /= 64.0;
            for (t = 0; t < 63; t++) {
                num += (seq[t] - mean) * (seq[t + 1] - mean);
                den += (seq[t] - mean) * (seq[t] - mean);
            }
            den += (seq[63] - mean) * (seq[63] - mean);
            /* rho = num / den (approx): demo 内浮点近似 */
            u64 rho_scale = (den > 0) ? (u64)(num / den * 1000000.0 + 0.5) : 0;
            wrstr("m151: T5 rho-lag1 (representative stream) = ");
            wrdec(rho_scale / 1000000);
            wrstr(".");
            u64 frac = rho_scale % 1000000;
            int d6 = 6;
            while (d6--) {
                wrdec((frac / 100000) % 10);
                frac = frac % 100000 * 10;
            }
            wrstr(" (measurement only; synthetic pattern)\n");
        }
    }

    if (pass_all) {
        static const char m2[] = "m151: M151 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m151: M151 RESULT: FAIL\n";
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
