/* m87_mcard.c — M87: 模型卡 (权限/计费/审计元数据, 资源节面)
 *
 * 1. mc_register(card): name="qwen3-0.6b" version=1 perm=0x3 cost=1
 *    budget=1000
 * 2. mc_call ×3 (tokens=100, perm_need=0x1) → allowed
 * 3. mc_call (perm_need=0x8 > perm) → denied (-1)
 * 4. mc_call tokens=900+ → budget over → denied
 * 5. info: calls=3 tokens=300; audit: 5 条 (3 ok + 2 deny) → PASS
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

static unsigned char card[64];
static u64 info[4];
static u64 audit[4 * 16];

void _start(void)
{
    static const char m1[] = "m87: model card (perm/billing/audit)\n";
    wr(m1, sizeof(m1) - 1);

    /* 模型卡 */
    int i;
    for (i = 0; i < 64; i++) {
        card[i] = 0;
    }
    static const char nm[] = "qwen3-0.6b";
    for (i = 0; nm[i] != 0 && i < 24; i++) {
        card[i] = nm[i];
    }
    card[24] = 1;              /* version */
    *(u64 *)(card + 32) = 0x3; /* perm_mask */
    *(u32 *)(card + 40) = 1;   /* cost */
    *(u64 *)(card + 56) = 1000; /* budget */
    (void)sys3(0x7D01, (long)card, 0, 0);

    /* 3 次正常调用 */
    (void)sys3(0x7D02, 0, 100, 0x1);
    (void)sys3(0x7D02, 0, 100, 0x1);
    (void)sys3(0x7D02, 0, 100, 0x1);
    /* 越权 (perm_need=0x8) */
    long denied = sys3(0x7D02, 0, 100, 0x8);
    /* 超预算 (1000 + 900 > 1000) */
    long over = sys3(0x7D02, 0, 900, 0x1);

    (void)sys3(0x7D03, (long)info, 0, 0);
    u64 calls = info[0], tokens = info[1], budget = info[2], perm = info[3];
    long naud = sys3(0x7D04, (long)audit, 4 * 16 * 8, 0);
    u64 r_last = audit[(naud - 1) * 4 + 3];
    u64 r_deny = audit[(naud - 2) * 4 + 3];

    static const char h1[] = "m87: calls=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)calls);
    static const char h2[] = " tokens=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)tokens);
    static const char h3[] = " aud=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)naud);
    wr("\n", 1);

    int ok = calls == 3 && tokens == 300 && budget == 1000 && perm == 0x3
             && denied == -1 && over == -1 && naud == 5
             && r_last == 0xFFFFFFFFFFFFFFFFULL /* -1 转 u64 */
             && r_deny == 0xFFFFFFFFFFFFFFFFULL;
    if (ok) {
        static const char m2[] = "m87: M87 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m87: M87 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
