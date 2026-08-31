/* m45_term.c — M45: 终端窗口控件验证
 *
 * 0x5A01 term_put(x,y,ch,color) / 0x5A02 term_draw(ox,oy,scale)
 * 0x5A03 term_pixel(x,y)
 * 流程: write 多行 (屏幕镜像进 80x25) -> term_draw(10,10,2) 渲染
 * -> backbuffer 采样: 首个字符块中心=前景色; 空白处=0 -> PASS。
 */
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
    static const char m1[] = "m45: terminal window control - screen mirror + render\n";
    wr(m1, sizeof(m1) - 1);
    static const char m2[] = "m45: line-two for mirror test\n";
    wr(m2, sizeof(m2) - 1);

    /* 渲染 80x25 屏到 backbuffer (10,10, scale2) */
    (void)sys3(0x5603, 0xFF000000u, 0, 0);
    long n = sys3(0x5A02, 10, 10, 2);
    wr("m45: term_draw chars=", 21);
    {
        long v = n;
        char b[12];
        int i = 12;
        if (v == 0) b[--i] = '0';
        while (v > 0) {
            b[--i] = '0' + (char)(v % 10);
            v /= 10;
        }
        wr(&b[i], 12 - i);
    }
    wr("\n", 1);

    /* 采样: (10+0,10+0) = 首字符 'm' 左上应非空; */
    u32 p0 = (u32)sys3(0x5A03, 10 + 0, 10 + 0, 0);
    /* 空白区 (第 2 行 vs 1: y=10+5*2*2? 用大 gap) */
    u32 p1 = (u32)sys3(0x5A03, 10 + 300, 10 + 300, 0);
    wr("m45: px=", 7);
    wrhex(p0);
    static const char s1[] = " blank=";
    wr(s1, 7);
    wrhex(p1);
    wr("\n", 1);

    int ok = ((p0 & 0xFFFFFF) != 0) && ((p1 & 0xFFFFFF) == 0) && (n > 0);
    if (ok) {
        static const char m3[] = "m45: M45 RESULT: PASS\n";
        wr(m3, sizeof(m3) - 1);
    } else {
        static const char m4[] = "m45: M45 RESULT: FAIL\n";
        wr(m4, sizeof(m4) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
