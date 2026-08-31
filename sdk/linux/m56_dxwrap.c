/* m56_dxwrap.c — M56: DXVK 式翻译原型验证 */
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
    static const char m1[] = "m56: DXVK-style translation prototype\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0);
    static u32 verts[6] = { 100, 100, 300, 400, 500, 100 };
    (void)sys3(0x6301, (long)verts, 6, 0);
    static u32 mat[4] = { 2, 2, 0, 0 };
    (void)sys3(0x6302, (long)mat, 0, 0);
    (void)sys3(0x6303, 0xFF0000, 0, 0);

    u32 p = (u32)sys3(0x6205, 600, 440, 0);   /* 变换后中心 */
    u32 r_ = (u32)sys3(0x6205, 150, 150, 0);  /* 原三角内/放大三角外 */
    u32 c50 = (u32)sys3(0x6205, 50, 50, 0);   /* 极角 */
    static const char h1[] = "m56: center=";
    wr(h1, sizeof(h1) - 1);
    wrhex(p);
    static const char s1[] = " orig=";
    wr(s1, sizeof(s1) - 1);
    wrhex(r_);
    static const char s2[] = " corner=";
    wr(s2, sizeof(s2) - 1);
    wrhex(c50);
    wr("\n", 1);

    int ok = (p & 0xFFFFFF) == 0xFF0000 && (r_ & 0xFFFFFF) == 0 && (c50 & 0xFFFFFF) == 0;
    if (ok) {
        static const char m2[] = "m56: M56 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m56: M56 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
