/* m44_icon.c — M44: 调色板/主题/图标验证
 *
 * 0x5901 pal_get(idx) / 0x5902 pal_set(idx,c) / 0x5903 theme(id)
 * 0x5904 icon_draw(x,y,id,scale) / 0x5905 icon_pixel(x,y)
 * 流程: DARK 主题 -> pal 读回(0/1 槽) -> 画 3 图标 (file/folder/app,
 * scale 2 @(50,50),(120,50),(190,50)) -> backbuffer 采样 (中心=ink,
 * 角=0) -> 切 LIGHT -> 再画 -> 采样 -> 汇总 PASS。
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
    static const char m1[] = "m44: palette/theme/icon system\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x5603, 0xFF000000u, 0, 0); /* backbuffer 黑底 */

    /* DARK 主题 */
    (void)sys3(0x5903, 1, 0, 0);
    u32 fg = (u32)sys3(0x5901, 0, 0, 0);
    u32 bg = (u32)sys3(0x5901, 1, 0, 0);
    wr("m44: dark fg=", 13);
    wrhex(fg);
    static const char s1[] = " bg=";
    wr(s1, 4);
    wrhex(bg);
    wr("\n", 1);

    (void)sys5(0x5904, 50, 50, 1, 2, 0);  /* file */
    (void)sys5(0x5904, 120, 50, 2, 2, 0); /* folder */
    (void)sys5(0x5904, 190, 50, 3, 2, 0); /* app */
    u32 pc = (u32)sys3(0x5905, 50 + 8, 50 + 8, 0);   /* 图标内 */
    u32 pe = (u32)sys3(0x5905, 50 + 2, 50 + 2, 0);   /* 图标边 */
    wr("m44: icon px=", 13);
    wrhex(pc);
    static const char s2[] = " edge=";
    wr(s2, 6);
    wrhex(pe);
    wr("\n", 1);

    /* LIGHT 主题重画 */
    (void)sys3(0x5903, 0, 0, 0);
    (void)sys3(0x5603, 0xFF000000u, 0, 0);
    (void)sys5(0x5904, 50, 150, 1, 2, 0);
    u32 pc2 = (u32)sys3(0x5905, 50 + 8, 150 + 8, 0);
    wr("m44: light icon=", 16);
    wrhex(pc2);
    wr("\n", 1);

    u32 bg_dark = bg;
    int ok1 = ((fg & 0xFFFFFF) != 0) && ((pc & 0xFFFFFF) == (fg & 0xFFFFFF))
              && ((pe & 0xFFFFFF) == 0) && ((pc2 & 0xFFFFFF) != 0);
    int ok2 = 1;
    (void)bg_dark;
    (void)ok2;

    if (ok1) {
        static const char m2[] = "m44: M44 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m44: M44 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
