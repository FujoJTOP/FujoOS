/* m83_leak.c — M83: 内存泄漏检测 (分配器统计, 快照差分)
 *
 * 1. leak_begin 快照 → kobj_create ×4 (kind 2/3/4/4) → leak_end:
 *    delta=+4 (未释放候选, 泄漏可检)
 * 2. free 全部 → leak_end: delta=0 → "balanced"
 * 3. PASS: 阶段1 delta==4 && 阶段2 delta==0
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

static u64 st[4];

void _start(void)
{
    static const char m1[] = "m83: allocator leak detection v0\n";
    wr(m1, sizeof(m1) - 1);

    /* 1) 分配 4 个 kobj (不释放 → 泄漏面) */
    (void)sys3(0x7A01, 0, 0, 0);
    long h0 = sys3(0x5130, 2, 0, 0); /* pipe */
    long h1 = sys3(0x5130, 3, 0, 0); /* shm */
    long h2 = sys3(0x5130, 4, 0, 0); /* sig */
    long h3 = sys3(0x5130, 4, 0, 0); /* sig */
    (void)sys3(0x7A02, (long)st, 0, 0);
    u64 d1 = st[0];
    static const char h1s[] = "m83: after-alloc delta=";
    wr(h1s, sizeof(h1s) - 1);
    wrhex((u32)d1);
    wr("\n", 1);

    /* 2) 全释放 → 无泄漏 */
    (void)sys3(0x7A01, 0, 0, 0);
    (void)sys3(0x5131, h0, 0, 0);
    (void)sys3(0x5131, h1, 0, 0);
    (void)sys3(0x5131, h2, 0, 0);
    (void)sys3(0x5131, h3, 0, 0);
    (void)sys3(0x7A02, (long)st, 0, 0);
    u64 d2 = st[0];
    static const char h2s[] = "m83: after-free delta=";
    wr(h2s, sizeof(h2s) - 1);
    wrhex((u32)d2);
    wr("\n", 1);

    int ok = d1 == 4 && d2 == 0;
    if (ok) {
        static const char m2[] = "m83: M83 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m83: M83 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
