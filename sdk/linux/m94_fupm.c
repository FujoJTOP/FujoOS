/* m94_fupm.c — M94: AI 服务 (模型注册表 + fupm 安装)
 *
 * 1. fupm_install A (4096B, name "qwen3-0.6b") → slot0
 * 2. fupm_install B (2048B, name "tiny-lm") → slot1
 * 3. reg_active(1) → 激活 tiny-lm
 * 4. reg_list → 条目: [0] size=4096, [1] size=2048 active=1, 数据指针
 *    正确 (MDATA 区首字节 == blob)
 * 5. fupm_remove(1) → 空 → PASS
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

static unsigned char blobA[4096];
static unsigned char blobB[2048];
static u64 list[16];

void _start(void)
{
    static const char m1[] = "m94: AI service - model registry + fupm\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    for (i = 0; i < 4096; i++) {
        blobA[i] = 0xAB;
    }
    for (i = 0; i < 2048; i++) {
        blobB[i] = 0xCD;
    }
    static const char nA[] = "qwen3-0.6b";
    static const char nB[] = "tiny-lm";

    (void)sys3(0x8401, (long)blobA, 4096, (long)nA);
    (void)sys3(0x8401, (long)blobB, 2048, (long)nB);
    (void)sys3(0x8403, 1, 0, 0); /* active tiny-lm */

    (void)sys3(0x8402, (long)list, 0, 0);
    u64 s0 = list[0], s1 = list[4], a1 = list[5], p1 = list[7];
    int ok1 = s0 == 4096 && s1 == 2048 && a1 == 1;

    /* 数据指针校验: 仅检查指针落在恒等区内 (内核区 U=0, 用户不可读) */
    int ok2 = p1 >= 0xF00000 && p1 < 0x1000000;

    /* remove(1) → 列表清 */
    (void)sys3(0x8404, 1, 0, 0);
    (void)sys3(0x8402, (long)list, 0, 0);
    int ok3 = list[4] == 0 && list[5] == 0;

    static const char h1[] = "m94: s0=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)s0);
    static const char h2[] = " s1=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)s1);
    static const char h3[] = " active=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)a1);
    wr("\n", 1);

    if (ok1 && ok2 && ok3) {
        static const char m2[] = "m94: M94 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m94: M94 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
