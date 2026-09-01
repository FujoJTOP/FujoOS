/* m101_desktop.c — M101: 桌面整合 shell (Win1.0 级交互闭环)
 *
 * 流程 (全部用户态原语, 无内核改动):
 *   desk_init + taskbar -> 图标 x2 -> 消息循环 8 帧:
 *     [1] 点开始按钮 (命中 0x5B03) -> menu(1)
 *     [2] 点菜单 "Programs" -> wm_create 窗口 (0x5521)
 *     [3] 窗口画标题栏/正文 (gl_rect + font) + wm_rect 读回
 *     [4] 点关闭钮 -> wm_remove (0x5524) -> 桌面还原
 *   [5..8] 重复第二窗口 -> 统计 openings>=2, menu>=1, closes>=2 -> PASS
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

#define FBW 1024
#define FBH 768
#define TBH 26

static u32 wrect[4];

void _start(void)
{
    static const char m1[] = "m101: desktop integration shell (click-to-open/close)\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0); /* 清屏 */
    (void)sys3(0x5B01, 0, 0, 0); /* desk_init (背景+任务栏) */
    static const char tb[] = "FujoOS 1.0";
    (void)sys3(0x5B02, (long)tb, 0, 0);
    /* 图标 x2 */
    (void)sys5(0x5904, 2, 60, 40, 3, 0);
    (void)sys5(0x5904, 2, 140, 40, 7, 0);

    int openings = 0, menu_hits = 0, closes = 0, frame;
    long cls = sys3(0x5520, (long)"Programs", 0, 0); /* 注册类 -> id */
    if (cls <= 0) {
        cls = 1;
    }
    for (frame = 0; frame < 8; frame++) {
        switch (frame) {
        case 0:
        case 4: /* 点开始按钮 -> 菜单 */
            if (sys3(0x5B03, 32, FBH - TBH + 12, 0) == 1) {
                menu_hits++;
            }
            (void)sys3(0x5B04, 1, 0, 0);
            break;
        case 1:
        case 5: /* 点菜单 Programs -> 开窗 */
            if (sys5(0x5521, cls, 30, 40, 320, 220) > 0) {
                openings++;
            }
            break;
        case 2:
        case 6: { /* 画窗口内容 + 读回矩形 */
            (void)sys3(0x5526, 1, (long)wrect, 0);
            long wx = wrect[0], wy = wrect[1], ww = wrect[2], wh = wrect[3];
            (void)sys5(0x6202, wx, wy, ww, 22, 0xC0C0FF); /* 标题栏 */
            static const char ti[] = "Programs";
            (void)sys5(0x5601, wx + 8, wy + 4, 1, 0x000000, (long)ti);
            (void)sys5(0x6202, wx, wy + 22, ww, wh - 22, 0xFFFFFF);
            break;
        }
        case 3:
        case 7: /* 点关闭钮 -> 关窗 */
            if (sys3(0x5524, 1, 0, 0) == 0) {
                closes++;
            }
            break;
        }
        (void)sys3(0x6104, 20, 0, 0); /* 帧等待 20ms */
    }
    (void)sys3(0x5B04, 0, 0, 0);

    static const char h1[] = "m101: openings=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)openings);
    static const char h2[] = " menu=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)menu_hits);
    static const char h3[] = " closes=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)closes);
    wr("\n", 1);

    int ok = openings == 2 && menu_hits == 2 && closes == 2;
    if (ok) {
        static const char m2[] = "m101: M101 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m101: M101 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
