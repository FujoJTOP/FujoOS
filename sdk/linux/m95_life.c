/* m95_life.c — M95: AI OS 验收: agent 全生命周期
 *
 * 命令 → 模型 → 工具 → 审计:
 * 1. 命令: "open the file" → route_classify == OPEN (3)
 * 2. 模型: fupm 安装 (tiny weight 2KB) + mcard 注册 (perm=1 budget=500)
 *    → infer_run(本地) → 响应非空; mcard call (计费)
 * 3. 工具: kobj create/free (无泄漏) + sess 检查点 (save/load)
 * 4. 审计: aud_log(1=cmd,2=model,3=tool) ×3 → aud_read == 3 条
 * 5. PASS: classify=3 && infer_n>10 && mc_calls>=1 && leaks==0
 *    && ckpt ok && aud==3
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

static int64_t sys3(long nr, long a, long b, long c)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
}
static int64_t run4(long nr, long a, long b, long c, long d)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10)
                 : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrhex(u32 v)
{
    static const char H[] = "0123456789abcdef";
    char b[9];
    int i;
    for (i = 0; i < 8; i++) {
        b[i] = H[(v >> (28 - i * 4)) & 0xF];
    }
    wr(b, 8);
}

static unsigned char weight[2048];
static unsigned char card[64];
static char resp[160];
static unsigned char ckpt[128];
static u64 inf3[4];
static u64 aud[4 * 16];

void _start(void)
{
    static const char m1[] = "m95: AI OS acceptance - agent lifecycle\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    /* 1) 命令 → 意图路由 */
    long intent = sys3(0x8202, (long)"open the file", 14, 0);

    /* 2) 模型: fupm 安装 + 卡注册 + infer 本地执行 + 计费 */
    for (i = 0; i < 2048; i++) {
        weight[i] = 0x5A;
    }
    static const char wname[] = "tiny-lm";
    (void)sys3(0x8401, (long)weight, 2048, (long)wname);
    for (i = 0; i < 64; i++) {
        card[i] = 0;
    }
    static const char cname[] = "tiny-lm-card";
    for (i = 0; cname[i] != 0 && i < 24; i++) {
        card[i] = cname[i];
    }
    card[24] = 1;
    *(u64 *)(card + 32) = 0x1; /* perm */
    *(u64 *)(card + 56) = 500;
    (void)sys3(0x7D01, (long)card, 0, 0);
    (void)sys3(0x8303, 1, 0, 0); /* local executor */
    long nresp = run4(0x8301, (long)"open the file", 14, (long)resp, sizeof(resp));
    long mcrc = sys3(0x7D02, (long)resp, nresp, 0x1); /* 计费 */

    /* 3) 工具: kobj 创建/释放 (无泄漏) + 会话检查点 */
    (void)sys3(0x7A01, 0, 0, 0);
    long h = sys3(0x5130, 3, 0, 0);
    (void)sys3(0x5131, h, 0, 0);
    (void)sys3(0x7A02, (long)inf3, 0, 0);
    u64 leak = inf3[0];
    (void)sys3(0x7E01, 0, 0, 0);
    for (i = 0; i < 128; i++) {
        ckpt[i] = 0x01 + (unsigned char)(i & 7);
    }
    (void)sys3(0x7E02, 0, (long)ckpt, 128);
    for (i = 0; i < 128; i++) {
        ckpt[i] = 0;
    }
    long cn = sys3(0x7E03, 0, (long)ckpt, 0);
    int ck_ok = cn == 128 && ckpt[0] == 1;

    /* 4) 审计 */
    (void)sys3(0x8103, 1, 1, 0); /* cmd */
    (void)sys3(0x8103, 2, 2, 0); /* model */
    (void)sys3(0x8103, 3, 3, 0); /* tool */
    long naud = sys3(0x8104, (long)aud, 4 * 16 * 8, 0);

    static const char h1[] = "m95: intent=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)intent);
    static const char h2[] = " resp_n=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)nresp);
    static const char h3[] = " leak=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)leak);
    static const char h4[] = " aud=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)naud);
    wr("\n", 1);

    int ok = intent == 3 && nresp > 10 && mcrc == 0 && leak == 0
             && ck_ok && naud == 3;
    if (ok) {
        static const char m2[] = "m95: M95 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m95: M95 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
