/* m82_ut.c — M82: 单元测试框架 (kernel 内断言自检)
 *
 * ut_run() → 7 用例 → (pass, fail, total) → 断言 pass==7 fail==0。
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

static u64 info[4];

void _start(void)
{
    static const char m1[] = "m82: kernel unit test suite\n";
    wr(m1, sizeof(m1) - 1);

    long rc = sys3(0x7901, 0, 0, 0);
    (void)sys3(0x7902, (long)info, 0, 0);
    u64 pass = info[0], fail = info[1], total = info[2], allp = info[3];

    static const char h1[] = "m82: pass=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)pass);
    static const char h2[] = " fail=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)fail);
    static const char h3[] = " total=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)total);
    wr("\n", 1);

    int ok = rc >= 0 && pass == 7 && fail == 0 && total == 7 && allp == 1;
    if (ok) {
        static const char m2[] = "m82: M82 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m82: M82 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
