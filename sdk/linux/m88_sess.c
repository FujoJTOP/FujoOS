/* m88_sess.c — M88: agent 会话 (检查点/恢复, 一等进程面)
 *
 * 1. sess_create(0)
 * 2. tick(100)+tick(50) → tokens 150
 * 3. save(blob A[128]) → 改 blob → load → 恢复 A; gen=1
 * 4. save(B) → load → gen=2
 * 5. info: active=1 ck=128 gen=2 tokens=150 → PASS
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

static unsigned char blob[128];
static u64 info[4];

void _start(void)
{
    static const char m1[] = "m88: agent sessions (ckpt/resume)\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x7E01, 0, 0, 0);
    (void)sys3(0x7E05, 0, 100, 0);
    (void)sys3(0x7E05, 0, 50, 0);

    /* A: 全 0xAA */
    int i;
    for (i = 0; i < 128; i++) {
        blob[i] = 0xAA;
    }
    (void)sys3(0x7E02, 0, (long)blob, 128);
    /* 弄脏 blob */
    for (i = 0; i < 128; i++) {
        blob[i] = 0x55;
    }
    long n = sys3(0x7E03, 0, (long)blob, 0);
    int okA = n == 128 && blob[0] == 0xAA && blob[127] == 0xAA;

    /* B: 全 0xBB; 保存+恢复 gen2 */
    for (i = 0; i < 128; i++) {
        blob[i] = 0xBB;
    }
    (void)sys3(0x7E02, 0, (long)blob, 128);
    for (i = 0; i < 128; i++) {
        blob[i] = 0x11;
    }
    (void)sys3(0x7E03, 0, (long)blob, 0);
    int okB = blob[0] == 0xBB && blob[127] == 0xBB;

    (void)sys3(0x7E04, (long)info, 0, 0);
    u64 active = info[0], ck = info[1], gen = info[2], tok = info[3];

    static const char h1[] = "m88: active=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)active);
    static const char h2[] = " ck=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)ck);
    static const char h3[] = " gen=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)gen);
    static const char h4[] = " tok=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)tok);
    wr("\n", 1);

    int ok = okA && okB && active == 1 && ck == 128 && gen == 2 && tok == 150;
    if (ok) {
        static const char m2[] = "m88: M88 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m88: M88 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
