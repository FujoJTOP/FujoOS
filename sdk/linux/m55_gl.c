/* m55_gl.c — M55: fujogl v0 software rasterizer */
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
    static const char m1[] = "m55: fujogl v0 software rasterizer\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0);
    static u32 verts[6] = { 100, 100, 300, 400, 500, 100 };
    (void)sys3(0x6203, (long)verts, 0xFF0000, 0);
    (void)sys5(0x6202, 0, 0, 40, 40, 0xFFFFFF);
    (void)sys5(0x6204, 0, 60, 300, 60, 0x00FF00);

    u32 p_in = (u32)sys3(0x6205, 300, 220, 0);
    u32 p_out = (u32)sys3(0x6205, 480, 220, 0);
    u32 p_rect = (u32)sys3(0x6205, 20, 20, 0);
    u32 p_line = (u32)sys3(0x6205, 150, 60, 0);
    static const char h1[] = "m55: tri_in=";
    wr(h1, sizeof(h1) - 1);
    wrhex(p_in);
    static const char s1[] = " out=";
    wr(s1, 5);
    wrhex(p_out);
    static const char s2[] = " rect=";
    wr(s2, 6);
    wrhex(p_rect);
    static const char s3[] = " line=";
    wr(s3, 6);
    wrhex(p_line);
    wr("\n", 1);

    int ok = (p_in & 0xFFFFFF) == 0xFF0000 && (p_out & 0xFFFFFF) == 0
             && (p_rect & 0xFFFFFF) == 0xFFFFFF && (p_line & 0xFFFFFF) == 0xFF00;
    if (ok) {
        static const char m2[] = "m55: M55 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m55: M55 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
