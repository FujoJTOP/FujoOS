/* m60_save.c — M60: 存档沙箱验证 */
typedef long int64_t;
typedef unsigned int u32;

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
static void wrdec(long v)
{
    char b[24];
    int i = 24;
    if (v == 0) b[--i] = '0';
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m60: save sandbox - permission dir + versioning\n";
    wr(m1, sizeof(m1) - 1);

    static const char data[] = "hello-save";
    (void)sys3(0x6701, 0, (long)data, 10);
    char back[64];
    long n = sys3(0x6702, 0, (long)back, 63);
    long v = sys3(0x6704, 0, 0, 0);
    static const char h1[] = "m60: read n=";
    wr(h1, sizeof(h1) - 1);
    wrdec(n);
    static const char h2[] = " data='";
    wr(h2, sizeof(h2) - 1);
    wr(back, 10);
    static const char h3[] = "' version=";
    wr(h3, sizeof(h3) - 1);
    wrdec(v);
    wr("\n", 1);

    u32 list[8];
    (void)sys3(0x6703, (long)list, 0, 0);
    static const char h4[] = "m60: slot0=";
    wr(h4, sizeof(h4) - 1);
    wrdec((long)list[0]);
    static const char h5[] = " slot1=";
    wr(h5, sizeof(h5) - 1);
    wrdec((long)list[1]);
    wr("\n", 1);

    int ok = n == 10 && v == 2 && back[0] == 'h' && list[0] == 10 && list[1] == (u32)-1;
    if (ok) {
        static const char m2[] = "m60: M60 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m60: M60 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
