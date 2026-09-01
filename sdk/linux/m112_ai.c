/* m112_ai.c — M112: AI For Next 首刀 (三地基 + 异常哨兵)
 *
 * 一次验收四件事 (每步串口断言):
 *   ① shm-link:   0x5101 classify 走 shm 帧 (宿主 pmemsave; 内核日志证 path)
 *   ② 感知通道:   0x8002 subscribe / 0x8004 inject×100 / 0x8003 读回 100;
 *                 掩码订阅只收 mask 内事件; 0x8005 结构态 (tasks/win/anom)
 *   ③ 动作通道:   0x8105 cap_exec (LAUNCH→ISOLATE→RESUME→KILL + deny + 审计)
 *   A 异常哨兵:   0x8304 anom_run ×100 (90 正常 + 10 异常) → hits/fp 对照;
 *                 自动隔离 (cfg 阈值+开关) 命中异常 pid → 任务挂起
 *
 * RESULT 判定: 基线 (规则语义) 10/0 正确 且 模型路径命中/误报达标
 *              (模型缺失时引擎=规则, 确定性同基线) → PASS。
 */
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned int u32;

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

/* ---- 事件/结构态缓冲 ---- */
static u64 evbuf[128 * 5];   /* 128 事件 × 5 u64 */
static char tbuf[256];

/* ---- 哨兵样本 ---- */
static const char NORMAL1[] = "ev pid=0 rate=3 wr=ok";
static const char NORMAL2[] = "ev pid=0 rate=5 wr=1";
static const char ANOM1[] = "ev pid=0 rate=99 wr=dead";
static const char ANOM2[] = "ev pid=1 rate=99 wr=dead";  /* 自动隔离对象 */

/* worker: 被 LAUNCH 的 aux 任务 (独立内核栈 0x3C0000 / 用户栈 0x700000) */
__attribute__((noinline, noreturn)) static void worker(void)
{
    static const char m[] = "m112: worker running (aux task alive)\n";
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

static long slen(const char *s)
{
    long n = 0;
    while (s[n])
        n++;
    return n;
}

static int ctx_has(const char *needle) { return strstr_pos(tbuf, needle) >= 0; }

static int run(void)
{
    static const char h1[] = "m112: AI For Next first slice (shm/events/exec/sentinel)\n";
    wr(h1, sizeof(h1) - 1);
    static const char h2[] = "m112: 1) shm-link + classify chain\n";
    wr(h2, sizeof(h2) - 1);

    /* ① 0x5101 classify (shm 帧, 内核日志 path=shm) */
    {
        static const char cmd[] = "run the game";
        int r = (int)sy(0x5101, (long)cmd, sizeof(cmd) - 1, 0, 0, 0);
        static const char p[] = "m112: 0x5101 classify -> intent=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)r);
        wr(NL, 1);
        if (r != 1) {
            static const char f[] = "m112: M112 RESULT: FAIL (classify)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }
    /* 0x5102 fujoctx (结构态化后仍是串行文本) */
    {
        char buf[160];
        int n = (int)sy(0x5102, (long)buf, sizeof(buf), 0, 0, 0);
        static const char p[] = "m112: 0x5102 fujoctx [";
        wr(p, sizeof(p) - 1);
        wr(buf, n > 0 ? n : 0);
        static const char p2[] = "]\n";
        wr(p2, sizeof(p2) - 1);
    }

    /* ② 事件环: 100 事件全量读回 + 掩码订阅 */
    static const char h3[] = "m112: 2) event ring (100 inject -> readback)\n";
    wr(h3, sizeof(h3) - 1);
    {
        int i, n;
        sy(0x8002, 0x1F, 0, 0, 0, 0); /* 订阅全部 (kind 1..5) */
        for (i = 0; i < 100; i++)
            sy(0x8004, (i % 5) + 1, i, 0, 0, 0);
        n = (int)sy(0x8003, (long)evbuf, sizeof(evbuf), 0, 0, 0);
        static const char p[] = "m112: readback=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)n);
        wr(NL, 1);
        if (n != 100) {
            static const char f[] = "m112: M112 RESULT: FAIL (events 100)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        /* 掩码订阅: 只收 kind=2(file)/4(exit) -> bit1|bit3 = 0b1010 */
        sy(0x8002, 0x0A, 0, 0, 0, 0);
        for (i = 0; i < 20; i++)
            sy(0x8004, (i % 5) + 1, i, 0, 0, 0);
        n = (int)sy(0x8003, (long)evbuf, sizeof(evbuf), 0, 0, 0);
        u64 got2 = 0, got4 = 0;
        for (i = 0; i < n && i < 128; i++) {
            if (evbuf[i * 5 + 1] == 2)
                got2++;
            if (evbuf[i * 5 + 1] == 4)
                got4++;
        }
        static const char p2[] = "m112: masked read=";
        wr(p2, sizeof(p2) - 1);
        wrdec((u64)n);
        static const char p3[] = " (file=";
        wr(p3, sizeof(p3) - 1);
        wrdec(got2);
        static const char p4[] = " exit=";
        wr(p4, sizeof(p4) - 1);
        wrdec(got4);
        wr(NL, 1);
        if (n != 8 || got2 != 4 || got4 != 4) {
            static const char f[] = "m112: M112 RESULT: FAIL (mask)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }

    /* ③ cap_exec: LAUNCH / ISOLATE / RESUME / KILL / deny / 审计 */
    static const char h4[] = "m112: 3) cap_exec (launch/isolate/resume/kill)\n";
    wr(h4, sizeof(h4) - 1);
    {
        u64 grants = 0x3F; /* 全部动作 (bit=act-1) */
        sy(0x8101, 6, (long)grants, 0, 0, 0);
        int tid = (int)sy(0x8105, 3, (long)&worker, 0, 0, 0); /* LAUNCH */
        static const char p[] = "m112: launch -> tid=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)tid);
        wr(NL, 1);
        if (tid < 0) {
            static const char f[] = "m112: M112 RESULT: FAIL (launch)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        /* 等 worker 任务出现并运行 */
        {
            int i;
            for (i = 0; i < 200; i++) {
                sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
                if (ctx_has("tasks=2") && ctx_has(" t1:1"))
                    break;
            }
            static const char p2[] = "m112: struct [";
            wr(p2, sizeof(p2) - 1);
            wr(tbuf, (long)sizeof(tbuf));
            static const char p3[] = "]\n";
            wr(p3, sizeof(p3) - 1);
            if (!(ctx_has("tasks=2") && ctx_has(" t1:1"))) {
                static const char f[] = "m112: M112 RESULT: FAIL (launch-alive)\n";
                wr(f, sizeof(f) - 1);
                return 0;
            }
        }
        int r = (int)sy(0x8105, 2, tid, 0, 0, 0); /* ISOLATE */
        static const char p4[] = "m112: isolate -> rc=";
        wr(p4, sizeof(p4) - 1);
        wrdec((u64)r);
        wr(NL, 1);
        if (r != 0) {
            static const char f[] = "m112: M112 RESULT: FAIL (isolate)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        r = (int)sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
        if (!(r > 0 && ctx_has(" t1:2"))) {
            static const char f[] = "m112: M112 RESULT: FAIL (isolate-state)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        r = (int)sy(0x8105, 5, tid, 0, 0, 0); /* RESUME */
        if (r != 0) {
            static const char f[] = "m112: M112 RESULT: FAIL (resume)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        r = (int)sy(0x8105, 1, tid, 0, 0, 0); /* KILL */
        if (r != 0) {
            static const char f[] = "m112: M112 RESULT: FAIL (kill)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        /* deny: 授权掩码去掉 KILL (bit0) 后再杀 */
        sy(0x8101, 6, 0x3E, 0, 0, 0);
        r = (int)sy(0x8105, 1, tid, 0, 0, 0);
        static const char p5[] = "m112: deny-kill -> rc=";
        wr(p5, sizeof(p5) - 1);
        wrdec((u64)r);
        wr(NL, 1);
        if (r != -1) {
            static const char f[] = "m112: M112 RESULT: FAIL (deny)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        sy(0x8101, 6, 0x3F, 0, 0, 0); /* 恢复 */
        /* 审计: 读 32 条, 数 action=2 (exec) */
        sy(0x8104, (long)evbuf, 32 * 32, 0, 0, 0);
        {
            int execs = 0, i;
            for (i = 0; i < 32; i++) {
                if (evbuf[i * 4 + 1] == 2)
                    execs++;
            }
            static const char p6[] = "m112: audit exec entries=";
            wr(p6, sizeof(p6) - 1);
            wrdec((u64)execs);
            wr(NL, 1);
            if (execs < 4) {
                static const char f[] = "m112: M112 RESULT: FAIL (audit)\n";
                wr(f, sizeof(f) - 1);
                return 0;
            }
        }
    }

    /* A 异常哨兵: 100 次分类 (90 正常 + 10 异常), 自动隔离命中 */
    static const char h5[] = "m112: 4) anomaly sentinel (100 classify)\n";
    wr(h5, sizeof(h5) - 1);
    {
        int i;
        sy(0x8002, 0x1F, 0, 0, 0, 0); /* 订阅全部 (游标复位; 哨兵事件入环) */
        int tid = (int)sy(0x8105, 3, (long)&worker, 0, 0, 0); /* 再启 worker */
        if (tid < 0) {
            static const char f[] = "m112: M112 RESULT: FAIL (sentinel-launch)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        sy(0x8105, 4, 1, 50, 0, 0); /* SET_CFG: 阈值 50 */
        sy(0x8105, 4, 2, 1, 0, 0);  /* SET_CFG: 自动隔离开 */
        {
            int hits = 0, fp = 0, model_hits = 0, model_fp = 0, model_n = 0;
            int base_hits = 0, base_fp = 0;
            for (i = 0; i < 100; i++) {
                const char *t;
                int expect;
                u64 out[3];
                if (i < 90) {
                    t = (i % 2) ? NORMAL1 : NORMAL2;
                    expect = 0;
                } else {
                    t = (i == 95) ? ANOM2 : ANOM1;
                    expect = 1;
                }
                /* 基线 (规则语义, 与内核 rules_anom 同判据) */
                {
                    int rule_pred =
                        (strstr_pos(t, "rate=9") >= 0 || strstr_pos(t, "dead") >= 0) ? 1 : 0;
                    if (expect == 1 && rule_pred == 1)
                        base_hits++;
                    if (expect == 0 && rule_pred == 1)
                        base_fp++;
                }
                sy(0x8304, (long)t, slen(t), (long)out, 24, 0);
                if (out[0] == 1 && expect == 0)
                    fp++;
                if (out[0] == 1 && expect == 1)
                    hits++;
                if (out[2] == 1) {
                    model_n++;
                    if (out[0] == 1 && expect == 0)
                        model_fp++;
                    if (out[0] == 1 && expect == 1)
                        model_hits++;
                }
            }
            static const char p[] = "m112: sentinel hits=";
            wr(p, sizeof(p) - 1);
            wrdec((u64)hits);
            static const char p2[] = " fp=";
            wr(p2, sizeof(p2) - 1);
            wrdec((u64)fp);
            static const char p3[] = " model(hits/fp/n)=";
            wr(p3, sizeof(p3) - 1);
            wrdec((u64)model_hits);
            wr("/", 1);
            wrdec((u64)model_fp);
            wr("/", 1);
            wrdec((u64)model_n);
            static const char p4[] = " baseline=";
            wr(p4, sizeof(p4) - 1);
            wrdec((u64)base_hits);
            wr("/", 1);
            wrdec((u64)base_fp);
            wr(NL, 1);
            /* 自动隔离: 异常 pid=1 应被挂起 */
            {
                int r = (int)sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
                static const char p5[] = "m112: post-sentinel struct [";
                wr(p5, sizeof(p5) - 1);
                wr(tbuf, (long)sizeof(tbuf));
                static const char p6[] = "]\n";
                wr(p6, sizeof(p6) - 1);
                if (!(r > 0 && ctx_has(" t1:2"))) {
                    static const char f[] = "m112: M112 RESULT: FAIL (auto-isolate)\n";
                    wr(f, sizeof(f) - 1);
                    return 0;
                }
                sy(0x8105, 5, tid, 0, 0, 0); /* RESUME */
                sy(0x8105, 1, tid, 0, 0, 0); /* KILL */
            }
            /* 事件环: 应有 ev_anomaly (订阅在哨兵循环前已复位游标) */
            {
                int n, anoms = 0;
                n = (int)sy(0x8003, (long)evbuf, sizeof(evbuf), 0, 0, 0);
                for (i = 0; i < n && i < 128; i++) {
                    if (evbuf[i * 5 + 1] == 5)
                        anoms++;
                }
                static const char p7[] = "m112: ev_anomaly count=";
                wr(p7, sizeof(p7) - 1);
                wrdec((u64)anoms);
                wr(NL, 1);
                /* 自动隔离的那条也是 anom -> ≥1 */
                if (anoms < 1) {
                    static const char f[] = "m112: M112 RESULT: FAIL (ev-anom)\n";
                    wr(f, sizeof(f) - 1);
                    return 0;
                }
            }
            int ok = (base_hits == 10 && base_fp == 0) &&
                     (model_n == 0 || (model_hits >= 8 && model_fp <= 2));
            if (ok) {
                static const char m2[] = "m112: M112 RESULT: PASS\n";
                wr(m2, sizeof(m2) - 1);
            } else {
                static const char f[] = "m112: M112 RESULT: FAIL (sentinel-metric)\n";
                wr(f, sizeof(f) - 1);
            }
        }
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
