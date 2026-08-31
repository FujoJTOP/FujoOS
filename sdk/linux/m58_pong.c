/* m58_pong.c — M58: 2D 游戏#1 (fujogl + fujokit 引擎选择) 最小验证
 * Pong: 球(12x12 红) + 拍(20x60 绿), 60 帧运动 + 上下回弹, 记录
 * 轨迹, 采样验证运动 -> PASS。 */
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
static int64_t sys5(long nr, long a, long b, long c, long d, long e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8)
                 : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrdec(long v)
{
    char b[24];
    int i = 24;
    int neg = 0;
    if (v < 0) { neg = 1; v = -v; }
    if (v == 0) b[--i] = '0';
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    if (neg) b[--i] = '-';
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m58: 2D game #1 pong (fujogl engine)\n";
    wr(m1, sizeof(m1) - 1);

    long bx = 100, by = 200, vx = 5, vy = 3;
    long px = 200;
    long minx = 9999, maxx = -1;
    long miny = 9999, maxy = -1;
    int f;
    (void)sys3(0x6201, 0, 0, 0);
    for (f = 0; f < 60; f++) {
        (void)sys5(0x6202, bx, by, 12, 12, 0xFF0000);  /* ball */
        (void)sys5(0x6202, px, 300, 20, 60, 0x00FF00); /* paddle */
        if (bx < minx) minx = bx;
        if (bx > maxx) maxx = bx;
        if (by < miny) miny = by;
        if (by > maxy) maxy = by;
        bx += vx;
        by += vy;
        if (by < 0 || by > 400) vy = -vy;
        if (bx < 0 || bx > 600) vx = -vx;
        if (bx > 250) px = bx - 100; /* 拍跟球 */
    }
    u32 p1 = (u32)sys3(0x6205, minx + 2, miny + 2, 0);
    u32 p2 = (u32)sys3(0x6205, maxx - 2, maxy - 2, 0);
    static const char h1[] = "m58: track x=";
    wr(h1, sizeof(h1) - 1);
    wrdec(minx);
    static const char h2[] = "..";
    wr(h2, 2);
    wrdec(maxx);
    static const char h3[] = " y=";
    wr(h3, sizeof(h3) - 1);
    wrdec(miny);
    static const char h4[] = "..";
    wr(h4, 2);
    wrdec(maxy);
    static const char h5[] = " sampled=";
    wr(h5, sizeof(h5) - 1);
    wrdec((p1 & 0xFFFFFF) == 0xFF0000 ? 1 : 0);
    wrdec((p2 & 0xFFFFFF) == 0xFF0000 ? 1 : 0);
    wr("\n", 1);

    int ok = (maxx - minx) > 100 && (maxy - miny) > 30
             && (p1 & 0xFFFFFF) == 0xFF0000 && (p2 & 0xFFFFFF) == 0xFF0000;
    if (ok) {
        static const char m2[] = "m58: M58 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m58: M58 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
