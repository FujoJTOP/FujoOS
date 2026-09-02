/* m116_dom.c — M116 (W9): 权限域与爆炸半径测试 (docs/57 阶段一最难波)
 *
 * 域 := { cap 集合, 地址空间, 中断域 }, 可撤销。
 * 爆炸半径定理: 恶意模型输出经 cap_exec/中断配置/AI 原语落地时,
 * 最坏影响被当前任务域界定 —— 逐条断言:
 *   域 1: perm = LAUNCH|SET_CFG|ACK (0x2C), as = LOW 仅 (无 HIGH), irq = 0
 *   敌对动作     KILL / ISOLATE        -> -1 (cap 集合无此位)
 *                 LAUNCH @0x1000000    -> -1 (有 LAUNCH perm 但地址空间无 HIGH)
 *                 0x6D01 中断配置      -> -1 (无中断域)
 *                 0x8005 @0x1005000    -> -14 (AI 原语地址空间封闭)
 *   授权动作     SET_CFG(2,1)          -> 0  (在域内正常生效)
 *   撤销         0x8109               -> 审计 action=3; 再执行 -> -1 (全拒)
 *   无副作用     cfg2 保持; 系统继续 (struct 可调)
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

#define ACT_KILL 1
#define ACT_ISOLATE 2
#define ACT_LAUNCH 3
#define ACT_SET_CFG 4
#define PERM_LS_ACK (((1 << (ACT_LAUNCH - 1)) | (1 << (ACT_SET_CFG - 1)) | (1 << 5)))

static void run(void)
{
    static const char h[] = "m116: privilege domain + explosion-radius (W9)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    long d1 = sy(0x8107, PERM_LS_ACK, 1, 0, 0, 0); /* 域1: LAUNCH|SET_CFG|ACK, LOW only, no irq */
    wrstr("m116: domain=");
    wrdec((u64)d1);
    wrstr(" (expect 1)\n");
    if (d1 != 1)
        pass_all = 0;
    sy(0x8108, d1, 0, 0, 0, 0); /* 绑定当前任务到域 1 */

    /* 敌对动作 —— 每条都应被域边界拒绝 */
    long r1 = sy(0x8105, ACT_KILL, 1, 0, 0, 0);
    long r2 = sy(0x8105, ACT_ISOLATE, 1, 0, 0, 0);
    long r3 = sy(0x8105, ACT_LAUNCH, 0x1000000ul, 0, 0, 0); /* 高区: 有 LAUNCH perm 但无 HIGH */
    long r4 = sy(0x6D01, 8, 0, 0, 0, 0); /* 中断配置: 无中断域 */
    long r5 = sy(0x8005, 0x1005000ul, 64, 0, 0, 0); /* AI 原语高区指针: 地址空间封闭 */
    wrstr("m116: hostile kill=");
    wrdec((u64)r1);
    wrstr(" isolate=");
    wrdec((u64)r2);
    wrstr(" launchHi=");
    wrdec((u64)r3);
    wrstr(" irqCfg=");
    wrdec((u64)r4);
    wrstr(" aiHi=");
    wrdec((u64)r5);
    wrstr(" (expect -1/-1/-1/-1/-14)\n");
    if (!(r1 == -1 && r2 == -1 && r3 == -1 && r4 == -1 && r5 == -14))
        pass_all = 0;

    /* 授权动作在域内正常生效 */
    long r6 = sy(0x8105, ACT_SET_CFG, 2, 1, 0, 0);
    long cfg2 = sy(0x8106, 2, 0, 0, 0, 0);
    wrstr("m116: auth  setCfg=");
    wrdec((u64)r6);
    wrstr(" cfg2=");
    wrdec((u64)cfg2);
    wrstr(" (expect 0/1)\n");
    if (!(r6 == 0 && cfg2 == 1))
        pass_all = 0;

    /* 撤销: 全拒 + 审计 + 无副作用 + 读回证明 */
    long r7 = sy(0x8109, d1, 0, 0, 0, 0);
    long r8 = sy(0x8105, ACT_SET_CFG, 2, 0, 0, 0);
    long cfg2b = sy(0x8106, 2, 0, 0, 0, 0);
    u64 info[25];
    int i;
    for (i = 0; i < 25; i++)
        info[i] = 0;
    sy(0x810A, (long)info, 0, 0, 0, 0);
    u64 granted = info[d1 * 5 + 2];
    u64 perm = info[d1 * 5 + 1];
    /* 审计: 0x8104 读回找 action=3 (域操作) */
    u64 aud[32 * 4];
    long na = sy(0x8104, (long)aud, 32 * 32, 0, 0, 0);
    int saw_dom = 0;
    for (i = 0; i < na && i < 32; i++) {
        if (aud[i * 4 + 1] == 3)
            saw_dom = 1;
    }
    wrstr("m116: revoke rc=");
    wrdec((u64)r7);
    wrstr(" thenExec=");
    wrdec((u64)r8);
    wrstr(" cfg2=");
    wrdec((u64)cfg2b);
    wrstr(" infoPerm=");
    wrdec(perm);
    wrstr(" granted=");
    wrdec(granted);
    wrstr(" audDom=");
    wrdec((u64)saw_dom);
    wrstr(" (expect 0/-1/1/44/0/1)\n");
    if (!(r7 == 0 && r8 == -1 && cfg2b == 1 && perm == PERM_LS_ACK && granted == 0 && saw_dom == 1))
        pass_all = 0;

    /* 系统继续 (爆炸半径之外无损坏) */
    {
        char tbuf[256];
        long rc = sy(0x8005, (long)tbuf, sizeof(tbuf), 0, 0, 0);
        if (rc < 0)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m116: M116 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m116: M116 RESULT: FAIL\n";
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
