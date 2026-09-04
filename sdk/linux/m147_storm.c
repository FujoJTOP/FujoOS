/* m147_storm.c — W27: 哨兵接管真实事件流 (ev_digest -> sentinel -> 自动处置, docs/88)
 *
 * 之前哨兵只处理 demo 构造文本; 本 demo 让哨兵感知系统自身:
 *   内核 0x8312 ev_digest: 从事件环统计"最近 100 ticks 事件速率 + 最近事件
 *   (pid/kind)" -> 摘要 "ev pid=<p> rate=<r> wr=<kind>"。
 *
 *   T0 LAUNCH 风暴任务 (循环 syscall 写事件, 真实事件流)
 *   T1 digest -> rate 高 (>=90; 事件风暴)
 *   T2 cfg 开自动隔离 (阈值 50); 0x8304 (digest 摘要) -> anom=1 -> 自动隔离
 *      -> 风暴任务被挂起
 *   T3 再 digest -> rate 显著回落 (< 风暴时一半) -> 0x8304 -> anom=0 (系统恢复)
 *   T4 resume 风暴任务清理
 * 闭环: 真实事件 -> digest -> 哨兵 -> 处置 -> 状态改善可测。
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

/* 风暴任务: 事件注入循环 (0x8004 inject -> 事件环真实写入, 无输出污染) */
__attribute__((noinline, noreturn)) static void storm(void)
{
    volatile u64 x = 0;
    for (;;) {
        x += 1;
        if (x > 0xFFFF0000ul)
            x = 0;
        sy(0x8004, 1, 0, 0, 0, 0); /* EV_SYSCALL 注入 */
    }
}

/* 睡眠: 忙等若干轮 (无 PIT 睡眠原语时) */
static void spin(long ms)
{
    volatile u64 i;
    for (i = 0; i < (u64)ms * 4000000ul; i++)
        ;
}

/* 读取 digest -> 解析 rate (返回 rate) */
static u64 digest_rate(void)
{
    char buf[64];
    int n = (int)sy(0x8312, (long)buf, sizeof(buf), 0, 0, 0);
    u64 rate = 0;
    int i = 0;
    if (n < 1)
        return 0;
    /* 找 " rate=" */
    while (buf[i] && i < n - 6) {
        if (buf[i] == ' ' && buf[i + 1] == 'r' && buf[i + 2] == 'a' && buf[i + 3] == 't'
            && buf[i + 4] == 'e' && buf[i + 5] == '=') {
            i += 6;
            while (buf[i] >= '0' && buf[i] <= '9') {
                rate = rate * 10 + (u64)(buf[i] - '0');
                i++;
            }
            break;
        }
        i++;
    }
    wrstr("m147: digest '");
    wrstr(buf);
    wrstr("' rate=");
    wrdec(rate);
    wrstr("\n");
    return rate;
}

static int run(void)
{
    static const char h[] = "m147: sentinel on real event flow (digest -> detect -> isolate)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;

    sy(0x8101, 6, 0x3F, 0, 0, 0);

    /* T0 启动风暴任务 */
    long tid = sy(0x8105, 3, (long)&storm, 0, 0, 0);
    wrstr("m147: T0 storm task tid=");
    wrdec((u64)tid);
    wrstr("\n");
    if (tid < 0)
        pass_all = 0;

    /* T1 事件风暴: 等 ~1s 再 digest */
    spin(300);
    u64 rate1 = digest_rate();
    wrstr("m147: T1 storm rate=");
    wrdec(rate1);
    wrstr(" (expect >=90)\n");
    if (rate1 < 90)
        pass_all = 0;

    /* T2 哨兵: cfg 开自动隔离; digest 摘要 -> 0x8304 -> 自动隔离风暴任务 */
    sy(0x8105, 4, 2, 1, 0, 0);
    sy(0x8105, 4, 1, 50, 0, 0);
    {
        char buf[64];
        int n = (int)sy(0x8312, (long)buf, sizeof(buf), 0, 0, 0);
        u64 o[3] = { 0, 0, 0 };
        sy(0x8304, (long)buf, n, (long)o, 24, 0);
        wrstr("m147: T2 sentinel anom=");
        wrdec(o[0]);
        wrstr(" conf=");
        wrdec(o[1]);
        wrstr(" (expect 1, >=50)\n");
        if (!(o[0] == 1 && o[1] >= 50))
            pass_all = 0;
    }

    /* T3 系统恢复: 隔离后速率回落 -> 哨兵判正常 */
    spin(300);
    u64 rate2 = digest_rate();
    wrstr("m147: T3 after-isolate rate=");
    wrdec(rate2);
    wrstr(" (expect < storm rate)\n");
    if (!(rate2 < rate1))
        pass_all = 0;
    {
        char buf[64];
        int n = (int)sy(0x8312, (long)buf, sizeof(buf), 0, 0, 0);
        u64 o[3] = { 0, 0, 0 };
        sy(0x8304, (long)buf, n, (long)o, 24, 0);
        wrstr("m147: T3b sentinel anom=");
        wrdec(o[0]);
        wrstr(" (expect 0)\n");
        if (o[0] != 0)
            pass_all = 0;
    }

    /* T4 清理 */
    sy(0x8105, 5, tid, 0, 0, 0);
    sy(0x8105, 1, tid, 0, 0, 0);
    sy(0x8105, 4, 2, 0, 0, 0);

    if (pass_all) {
        static const char m2[] = "m147: M147 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m147: M147 RESULT: FAIL\n";
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
