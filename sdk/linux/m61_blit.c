/* m61_blit.c — M61: blit/缩放硬件路径抽象验证 */
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

void _start(void)
{
    static const char m1[] = "m61: blit & scale hardware-path abstraction\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0);
    /* 源缓冲 16x16: 左上角 8x8 红, 其余蓝 */
    static u32 sbuf[16 * 16];
    int i;
    for (i = 0; i < 16 * 16; i++) {
        u32 x = (u32)(i % 16), y = (u32)(i / 16);
        sbuf[i] = (x < 8 && y < 8) ? 0xFF0000 : 0x0000FF;
    }
    /* blit 1:1 到 (100,100) */
    (void)sys5(0x6801, (long)sbuf, 100, 100, 16, 16);
    /* blit 缩放 2x (16x16 -> 32x32) 到 (200,200) */
    static u32 dims[4] = { 16, 16, 32, 32 };
    (void)sys5(0x6802, (long)sbuf, 200, 200, (long)dims, 0);

    u32 a = (u32)sys3(0x6205, 104, 104, 0);   /* blit 内红 */
    u32 b = (u32)sys3(0x6205, 112, 104, 0);   /* blit 内蓝 */
    u32 c_ = (u32)sys3(0x6205, 204, 204, 0);  /* scaled 内红 2x */
    u32 d = (u32)sys3(0x6205, 230, 230, 0);   /* scaled 内蓝 2x */
    static const char h1[] = "m61: b1=";
    wr(h1, sizeof(h1) - 1);
    wrhex(a);
    static const char h2[] = " b2=";
    wr(h2, sizeof(h2) - 1);
    wrhex(b);
    static const char h3[] = " s1=";
    wr(h3, sizeof(h3) - 1);
    wrhex(c_);
    static const char h4[] = " s2=";
    wr(h4, sizeof(h4) - 1);
    wrhex(d);
    wr("\n", 1);

    int ok = (a & 0xFFFFFF) == 0xFF0000 && (b & 0xFFFFFF) == 0xFF
             && (c_ & 0xFFFFFF) == 0xFF0000 && (d & 0xFFFFFF) == 0xFF;
    if (ok) {
        static const char m2[] = "m61: M61 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m61: M61 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
