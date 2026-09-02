/* m118_r3.c — M118 (W8 R3): 时延一致性协议验证 (docs/59)
 *
 * 协议: shm 请求帧带快照@t0 (t0 + evw + crit 掩码); 模型回包声明 TTL;
 *   内核 wait_rsp 检查事件环增量, 关键事件到达或超出 TTL -> 丢弃建议走规则。
 * 确定性探针 0x8309 (mode): bit0=快照后注入关键事件, bit1=强制回包过期。
 *
 *   T1 正常路径        mode=0  -> engine=1 reason=0 (模型建议被接受)
 *   T2 关键事件        mode=1  -> engine=2 reason=1 crit>=1 (丢弃->规则)
 *   T3 过期标记        mode=2  -> engine=2 reason=2 (TTL 失效->规则)
 *   T4 主职责回归      0x8304 正常事件 -> anom=0 engine=1 (协议走哨兵)
 *   T5 丢弃后系统继续  mode=0 -> engine=1 (链路未损坏, 模型仍可用)
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

static void run(void)
{
    static const char h[] = "m118: R3 latency-consistency protocol (W8)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 o[8];
    int i;

    /* T1 正常路径 */
    for (i = 0; i < 8; i++) o[i] = 0xDEAD;
    sy(0x8309, 0, (long)o, 64, 0, 0);
    wrstr("m118: T1 normal  engine=");
    wrdec(o[0]);
    wrstr(" reason=");
    wrdec(o[1]);
    wrstr(" crit=");
    wrdec(o[2]);
    wrstr(" (expect 1/0/0)\n");
    if (!(o[0] == 1 && o[1] == 0 && o[2] == 0))
        pass_all = 0;

    /* T2 关键事件 (快照后注入 EV_ANOMALY) -> 丢弃 */
    for (i = 0; i < 8; i++) o[i] = 0xDEAD;
    sy(0x8309, 1, (long)o, 64, 0, 0);
    wrstr("m118: T2 crit    engine=");
    wrdec(o[0]);
    wrstr(" reason=");
    wrdec(o[1]);
    wrstr(" crit=");
    wrdec(o[2]);
    wrstr(" (expect 2/1/1+)\n");
    if (!(o[0] == 2 && o[1] == 1 && o[2] >= 1))
        pass_all = 0;

    /* T3 过期标记 (TTL 失效) -> 丢弃 */
    for (i = 0; i < 8; i++) o[i] = 0xDEAD;
    sy(0x8309, 2, (long)o, 64, 0, 0);
    wrstr("m118: T3 stale   engine=");
    wrdec(o[0]);
    wrstr(" reason=");
    wrdec(o[1]);
    wrstr(" (expect 2/2)\n");
    if (!(o[0] == 2 && o[1] == 2))
        pass_all = 0;

    /* T4 主职责: 哨兵走协议路径 (正常事件 -> 非异常, 模型引擎) */
    {
        static const char ev[] = "ev pid=0 rate=3 wr=ok";
        u64 a[3] = { 0, 0, 0 };
        sy(0x8304, (long)ev, sizeof(ev) - 1, (long)a, 24, 0);
        wrstr("m118: T4 sentinel anom=");
        wrdec(a[0]);
        wrstr(" engine=");
        wrdec(a[2]);
        wrstr(" (expect 0/1)\n");
        if (!(a[0] == 0 && a[2] == 1))
            pass_all = 0;
    }

    /* T5 丢弃后系统继续 (模型仍在线) */
    for (i = 0; i < 8; i++) o[i] = 0xDEAD;
    sy(0x8309, 0, (long)o, 64, 0, 0);
    wrstr("m118: T5 resume  engine=");
    wrdec(o[0]);
    wrstr(" reason=");
    wrdec(o[1]);
    wrstr(" (expect 1/0)\n");
    if (!(o[0] == 1 && o[1] == 0))
        pass_all = 0;

    if (pass_all) {
        static const char m2[] = "m118: M118 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m118: M118 RESULT: FAIL\n";
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
