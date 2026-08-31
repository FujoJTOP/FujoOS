/* m46_desk.c — M46: 桌面环境 v0 (任务栏/开始菜单) 验证
 *
 * 0x5B01 desk_init / 0x5B02 taskbar(text) / 0x5B03 start(x,y) hit
 * 0x5B04 menu(on) / 0x5B05 pixel(x,y)
 * 流程: desk_init -> taskbar("FujoOS v0") -> 开始按钮命中自测 ->
 * menu(1) 渲染 -> 采样: 任务栏颜色(底部), 菜单框面(左上), 桌面背景
 * -> PASS。
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
    static const char m1[] = "m46: desktop v0 - taskbar & start menu\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x5B01, 0, 0, 0);
    (void)sys3(0x5B02, (long)"FujoOS v0", 0, 0);

    /* 开始按钮命中: y 在任务栏内 (768-40+20=748) */
    long hit = sys3(0x5B03, 30, 748, 0);
    wr("m46: start-hit=", 15);
    {
        char b[12];
        int i = 12;
        long v = hit;
        if (v == 0) b[--i] = '0';
        while (v > 0) {
            b[--i] = '0' + (char)(v % 10);
            v /= 10;
        }
        wr(&b[i], 12 - i);
    }
    wr("\n", 1);

    (void)sys3(0x5B04, 1, 0, 0); /* 菜单开 */

    u32 taskbar = (u32)sys3(0x5B05, 500, 760, 0);   /* 任务栏体 */
    u32 menu_face = (u32)sys3(0x5B05, 100, 100, 0); /* 菜单面 */
    u32 bg = (u32)sys3(0x5B05, 500, 300, 0);        /* 桌面背景(菜单外) */
    wr("m46: tb=", 7);
    wrhex(taskbar);
    static const char s1[] = " menu=";
    wr(s1, 6);
    wrhex(menu_face);
    static const char s2[] = " bg=";
    wr(s2, 4);
    wrhex(bg);
    wr("\n", 1);

    int ok = hit == 1 && (taskbar & 0xFFFFFF) != 0 && (menu_face & 0xFFFFFF) != 0
             && (bg & 0xFFFFFF) != 0 && (bg & 0xFFFFFF) != (taskbar & 0xFFFFFF);
    if (ok) {
        static const char m2[] = "m46: M46 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m46: M46 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
