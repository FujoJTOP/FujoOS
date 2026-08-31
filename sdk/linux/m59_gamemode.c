/* m59_gamemode.c — M59: 游戏模式验证 */
typedef long int64_t;
typedef unsigned long long u64;
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
static void wrdec(u64 v)
{
    char b[24];
    int i = 24;
    if (v == 0) b[--i] = '0';
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m59: game mode - foreground/reserve/fullscreen\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6601, 1, 0, 0);
    u64 st[3];
    (void)sys3(0x6602, (long)st, 0, 0);
    static const char h1[] = "m59: mode=";
    wr(h1, sizeof(h1) - 1);
    wrdec(st[0]);
    static const char h2[] = " ticks=";
    wr(h2, sizeof(h2) - 1);
    wrdec(st[1]);
    static const char h3[] = " heap=";
    wr(h3, sizeof(h3) - 1);
    wrdec(st[2]);
    wr("\n", 1);

    long r = sys3(0x6603, 1, 0, 0);
    u32 wh[2];
    (void)sys3(0x5C02, (long)wh, 0, 0);
    static const char h4[] = "m59: fullscreen rc=";
    wr(h4, sizeof(h4) - 1);
    wrdec((u64)r);
    static const char h5[] = " mode=";
    wr(h5, sizeof(h5) - 1);
    wrdec((u64)wh[0]);
    static const char h6[] = "x";
    wr(h6, 1);
    wrdec((u64)wh[1]);
    wr("\n", 1);

    (void)sys3(0x6601, 0, 0, 0);
    int ok = st[0] == 1 && r == 0 && wh[0] == 1024 && wh[1] == 768;
    if (ok) {
        static const char m2[] = "m59: M59 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m59: M59 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
