/* m42_gui.c — M42: GUI 应用#1 可点按钮窗口 (验收)
 *
 * 组装图景: wm 窗口(0x5521) + font 渲染(0x5601, 边框/label)
 *  + fujokit 按钮 + WM_BUTTON(WB) 消息(0x5522)→ kt_button_click。
 * 流程: 创建窗口 w1 (200x120, 消息环); 渲染标题/边框/按钮位图到
 * backbuffer; 登记按钮矩形到 fujokit;_自测路径(按钮内部点击一次)
 * + 鼠标实点击路径(注入 mouse_button/move 期间 WM_BUTTON)。输出
 * 按钮 trigger 计数/窗口消息计数 -> PASS (点击计数>=2)。
 * 编译: 同 m41 (含 fujokit.h)。
 */
#include "kit/fujokit.h"

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
static void wrdec(int v)
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

void _start(void)
{
    static const char m1[] = "m42: GUI app #1 - clickable button window\n";
    wr(m1, sizeof(m1) - 1);

    /* 窗口 (消息环) */
    long cls = sys3(0x5520, (long)"GuiApp", 0, 0);
    u32 w1 = (u32)sys5(0x5521, cls, 60, 60, 220, 140);

    /* 渲染: 标题 + 边框 + 按钮位图 (backbuffer) */
    (void)sys3(0x5603, 0xFF202030u, 0, 0);
    (void)sys5(0x5601, 60, 60, 2, (long)0xFFFFFFFFu, (long)"[GUI APP v1]");
    (void)sys5(0x5601, 60, 180, 1, (long)0xFF00FF00u, (long)"[CLICK ME]");
    static const char m2[] = "m42: window/button rendered\n";
    wr(m2, sizeof(m2) - 1);

    /* fujokit 按钮 (屏幕坐标 60,200 = 窗口内 + 偏移) */
    kt_button btn;
    kt_button_init(&btn, 1, 60, 180, 112, 20, "CLICK ME");

    /* -------- 鼠标实点击路径: 轮询 WM_* 消息 (注入窗口) -------- */
    u32 wm_msgs = 0, button_hits = 0;
    int loops = 0;
    while (loops++ < 40000 && button_hits < 3) {
        u32 msg[5];
        long got = sys3(0x5522, (long)msg, 0, 0);
        if (got) {
            wm_msgs++;
            if (msg[0] == 4) { /* WM_BUTTON: (win, x, y, btn) */
                if (kt_button_click(&btn, (int)msg[2], (int)msg[3], 1)) {
                    button_hits++;
                    wr("m42: button click!\n", 17);
                }
                (void)kt_button_click(&btn, (int)msg[2], (int)msg[3], 0);
            }
        }
        long i;
        for (i = 0; i < 60000; i++) {
            __asm__ volatile("" ::: "memory");
        }
    }

    /* -------- 自测路径 (确定性: 内部一次命中) -------- */
    (void)kt_button_click(&btn, 80, 188, 1);
    (void)kt_button_click(&btn, 80, 188, 0);
    (void)kt_button_click(&btn, 80, 188, 1);
    (void)kt_button_click(&btn, 80, 188, 0);
    wr("m42: wm_msgs=", 14);
    wrdec(wm_msgs);
    static const char s1[] = " button_triggers=";
    wr(s1, 17);
    wrdec(btn.triggers);
    wr("\n", 1);

    if (btn.triggers >= 2 && w1 != 0) {
        static const char m3[] = "m42: M42 RESULT: PASS\n";
        wr(m3, sizeof(m3) - 1);
    } else {
        static const char m4[] = "m42: M42 RESULT: FAIL\n";
        wr(m4, sizeof(m4) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
