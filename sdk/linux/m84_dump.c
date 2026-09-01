/* m84_dump.c — M84: 崩溃转储 (minidump 雏形)
 *
 * 1. dump_arm(1) → fork: 子任务 ud2 (#UD vec6) 崩溃隔离 (转场前捕获);
 * 2. 父忙等 → dump_info/dump_read:
 *    count>=1, vec==6, magic "FUJDUMP", rip 非零 → PASS
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
static unsigned char dmp[128];

void _start(void)
{
    static const char m1[] = "m84: crash minidump v0\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x7B01, 1, 0, 0); /* arm 捕获 */
    long rc = sys3(57, 0, 0, 0); /* fork */
    if (rc == 0) {
        /* 子: ud2 (#UD) → 崩溃隔离 (父/子共享地址: 转场前 note_exc) */
        asm volatile("ud2");
        for (;;) {
        }
    } else if (rc > 0) {
        /* 父: 忙等给子时间片 */
        volatile long sp = 0;
        for (volatile long i = 0; i < 15000000; i++) {
            sp += 1;
        }
        (void)sys3(0x7B03, (long)info, 0, 0);
        u64 cnt = info[0], vec = info[1], rip = info[2];
        long n = sys3(0x7B02, (long)dmp, 128, 0);
        int magic_ok = dmp[0] == 'F' && dmp[1] == 'U' && dmp[2] == 'J' && dmp[3] == 'D';
        static const char h1[] = "m84: count=";
        wr(h1, sizeof(h1) - 1);
        wrhex((u32)cnt);
        static const char h2[] = " vec=";
        wr(h2, sizeof(h2) - 1);
        wrhex((u32)vec);
        static const char h3[] = " rip=";
        wr(h3, sizeof(h3) - 1);
        wrhex((u32)(rip >> 16));
        static const char h4[] = " n=";
        wr(h4, sizeof(h4) - 1);
        wrhex((u32)n);
        wr("\n", 1);

        int ok = cnt >= 1 && vec == 6 && rip != 0 && magic_ok && n == 120;
        if (ok) {
            static const char m2[] = "m84: M84 RESULT: PASS\n";
            wr(m2, sizeof(m2) - 1);
        } else {
            static const char m3[] = "m84: M84 RESULT: FAIL\n";
            wr(m3, sizeof(m3) - 1);
        }
        for (;;) {
        }
    } else {
        static const char m4[] = "m84: M84 RESULT: FAIL (fork)\n";
        wr(m4, sizeof(m4) - 1);
        for (;;) {
        }
    }
}
