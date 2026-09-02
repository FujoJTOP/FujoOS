/* m119_inv.c — M119 (W8 R1): AI 接口四条公理的可断言自检 (docs/59)
 *
 * 内核侧 0x830A fujo_inv_run 在模型完全离线时也执行 (纯内核, 无链路等待):
 *   I1 模型永不能执行未授权动作 (cap_exec 门)      -> out bit0
 *   I2 每个动作有审计记录 (aud 不变式)             -> out bit1
 *   I3 模型缺席时系统继续运行 (规则兜底确定)        -> out bit2
 *   I4 失败被计数并降级 (fail-safe)                -> out bit3
 * 前置条件: exec 槽未授权时调用 (引导默认); 调用后槽 6 被授予 0x3F。
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
    static const char h[] = "m119: AI interface axioms (R1, offline)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 o[8];
    int i;
    for (i = 0; i < 8; i++) o[i] = 0xDEAD;

    sy(0x830A, (long)o, 64, 0, 0, 0);
    wrstr("m119: invariants mask=");
    wrdec(o[0]);
    wrstr(" denies=");
    wrdec(o[1]);
    wrstr(" aud=");
    wrdec(o[2]);
    wrstr(" (expect mask=0xF)\n");
    if (o[0] != 0xF)
        pass_all = 0;

    /* 验证 I2/I4 的副作用可从用户态观察: cfg2=1 (授权动作生效, 系统继续) */
    {
        long cfg2 = sy(0x8106, 2, 0, 0, 0, 0);
        wrstr("m119: cfg2=");
        wrdec((u64)cfg2);
        wrstr(" (expect 1)\n");
        if (cfg2 != 1)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m119: M119 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m119: M119 RESULT: FAIL\n";
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
