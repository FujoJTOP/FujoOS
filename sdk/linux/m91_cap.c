/* m91_cap.c — M91: 权限与审计 (能力表 + 审计日志)
 *
 * 1. cap_grant(0, 0x1) → cap_check(0,0x1)=0 (允许)
 * 2. cap_check(0,0x2)=-1 (deny → 自动审计条目)
 * 3. aud_log(7, 9) 显式审计
 * 4. aud_read → 2 条: 第1条 (deny, subject=0, result=1),
 *    第2条 (action=7, subject=9) → PASS
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

static u64 aud[4 * 16];

void _start(void)
{
    static const char m1[] = "m91: capability table + audit log\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x8101, 0, 0x1, 0);
    long ok = sys3(0x8102, 0, 0x1, 0);   /* allow (0) */
    long deny = sys3(0x8102, 0, 0x2, 0); /* deny (-1) */
    (void)sys3(0x8103, 7, 9, 0);         /* 显式审计 */

    long n = sys3(0x8104, (long)aud, 4 * 16 * 8, 0);
    u64 a0 = aud[1], s0 = aud[2], r0 = aud[3];      /* 第 1 条 (deny) */
    u64 a1 = aud[5], s1 = aud[6];                    /* 第 2 条 (7,9) */

    static const char h1[] = "m91: ok=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)ok);
    static const char h2[] = " deny=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)deny);
    static const char h3[] = " aud=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)n);
    wr("\n", 1);

    int pass = ok == 0 && deny == -1 && n == 2 && a0 == 1 && s0 == 0 && r0 == 1
               && a1 == 7 && s1 == 9;
    if (pass) {
        static const char m2[] = "m91: M91 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m91: M91 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
