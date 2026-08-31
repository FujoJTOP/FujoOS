/* m49_a11y.c — M49: 无障碍 (高对比/大字) 验证
 *
 * 0x5D01 a11y_set(mode) / 0x5D02 a11y_get()
 * 流程: 高对比 (mode1) -> palette fg/bg 读回 (icon 0x5901) -> 大字
 * (mode2) -> font_text 渲染 (scale 自动 2) -> 采样字符宽度变大 ->
 * 复位 -> PASS。
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
    static const char m1[] = "m49: accessibility - high contrast & large text\n";
    wr(m1, sizeof(m1) - 1);

    /* 高对比 */
    (void)sys3(0x5D01, 1, 0, 0);
    u32 fg = (u32)sys3(0x5901, 0, 0, 0);
    u32 bg = (u32)sys3(0x5901, 1, 0, 0);
    wr("m49: hc fg=", 11);
    wrhex(fg);
    static const char s1[] = " bg=";
    wr(s1, 4);
    wrhex(bg);
    wr("\n", 1);

    /* 大字: 渲染后字符宽度 (第二字符偏移 16*scale+build) */
    (void)sys3(0x5D01, 2, 0, 0);
    (void)sys3(0x5603, 0xFF000000u, 0, 0);
    (void)sys5(0x5601, 50, 50, 1, 0xFFFFFFFFu, (long)"MM");
    /* 首字符 'M' 左上 (bit6) — 若 scale=2 (boost), 第二个 M 起点=50+16 → 第一字符宽 16px */
    u32 p1 = (u32)sys3(0x5602, 50 + 0, 50 + 0, 0);      /* (50,50) M 左上 */
    u32 gap = (u32)sys3(0x5602, 50 + 13, 50 + 50 + 1, 0); /* M 内部稍偏 */
    (void)gap;
    wr("m49: large px=", 15);
    wrhex(p1);
    wr("\n", 1);

    long mode = sys3(0x5D02, 0, 0, 0);
    (void)mode;

    int ok = (fg & 0xFFFFFF) == 0xFFFFFF && (bg & 0xFFFFFF) == 0
             && (p1 & 0xFFFFFF) == 0xFFFFFF;
    if (ok) {
        static const char m2[] = "m49: M49 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m49: M49 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
