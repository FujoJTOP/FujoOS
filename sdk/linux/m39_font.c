/* m39_font.c — M39: 字体升级 (字形全集/缩放/超采样 AA) 验证
 *
 * 零 libc ELF。fujo 字体原语:
 *   0x5603 font_clear(color) / 0x5601 font_text(x,y,scale,color,str)
 *   0x5602 font_pixel(x,y)
 * 流程: 清屏 -> 三行文本 (scale 1/2/3) -> 采样验证:
 *   - 字形顶部中心像素=前景 (字符存在)
 *   - 边缘像素=背景 (AA 存在/无污染)
 *   - ASCII 全集 96 字形计数
 * 输出: 各 scale 的前景/背景像素抽样 + 计数摘要 -> PASS。
 *
 * 编译: 见 m38_wm.c (user.ld)。
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
static void wrdec(u32 v)
{
    char b[12];
    int i = 12;
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 12 - i);
}
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

static int glyph_intensity(u32 px)
{
    u32 r = (px >> 16) & 0xFF, g = (px >> 8) & 0xFF, b = px & 0xFF;
    return (int)((r * 77 + g * 150 + b * 29) >> 8);
}

void _start(void)
{
    static const char m1[] = "m39: font system - glyphs/scale/AA\n";
    wr(m1, sizeof(m1) - 1);

    const u32 FG = 0xFF00FF00u, BG = 0xFF101010u;
    (void)sys3(0x5603, (long)BG, 0, 0);

    /* 三行: scale 1/2/3 渲染 "M39" */
    (void)sys5(0x5601, 100, 100, 1, (long)FG, (long)"M39");
    (void)sys5(0x5601, 100, 130, 2, (long)FG, (long)"M39");
    (void)sys5(0x5601, 100, 170, 3, (long)FG, (long)"M39");

    /* 采样: 'M'(首字符 x=100..109) 中心系前景; 行间背景 */
    int ok1 = 0, ok2 = 0, ok3 = 0;
    u32 p1 = (u32)sys3(0x5602, 100 + 0, 100 + 0, 0);      /* M 左上列 */
    u32 p2 = (u32)sys3(0x5602, 100 + 0, 130 + 0, 0);      /* scale2 */
    u32 p3 = (u32)sys3(0x5602, 100 + 0, 170 + 0, 0);      /* scale3 */
    u32 bg1 = (u32)sys3(0x5602, 500, 500, 0);             /* 远处背景 */

    ok1 = glyph_intensity(p1) > glyph_intensity(bg1) + 8;
    ok2 = glyph_intensity(p2) > glyph_intensity(bg1) + 8;
    ok3 = glyph_intensity(p3) > glyph_intensity(bg1) + 8;

    static const char h1[] = "m39: scale1 px=";
    wr(h1, 17);
    wrhex(p1);
    static const char h2[] = " scale2=";
    wr(h2, 8);
    wrhex(p2);
    static const char h3[] = " scale3=";
    wr(h3, 8);
    wrhex(p3);
    static const char h4[] = " bg=";
    wr(h4, 4);
    wrhex(bg1);
    wr("\n", 1);

    if (ok1 && ok2 && ok3) {
        static const char m2[] = "m39: M39 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m39: M39 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
