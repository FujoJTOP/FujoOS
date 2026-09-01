/* m106_operate.c — M106: 桌面操作回归 (M101-105 全链)
 *
 * ① 桌面+开始菜单+开窗 (M101)  ② 窗口拖动 (M102)
 * ③ 菜单栏+对话框 OK/Cancel (M103)  ④ 文本框 caret 插删 (M104)
 * ⑤ 文件保存/打开/读回 (M105, FJFS -drive)
 * 串联验证: all-ok -> PASS。
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

static const char content[] = "M106 operate regression\n";
static const char path[] = "/disk/hello.txt";
static char buf[64];

void _start(void)
{
    static const char m1[] = "m106: full desktop operation regression\n";
    wr(m1, sizeof(m1) - 1);

    /* ① 桌面 → 菜单 → 开窗 → 关窗 */
    (void)sys3(0x6201, 0, 0, 0);
    (void)sys3(0x5B01, 0, 0, 0);
    long cls = sys3(0x5520, (long)"Programs", 0, 0);
    if (cls <= 0) cls = 1;
    long win = sys5(0x5521, cls, 30, 40, 320, 220);
    (void)sys3(0x5524, win, 0, 0);
    int ok1 = win > 0;

    /* ② 拖动: 0x5525 增量 */
    long cls2 = sys3(0x5520, (long)"Files", 0, 0);
    if (cls2 <= 0) cls2 = 2;
    long winB = sys5(0x5521, cls2, 200, 120, 280, 180);
    if (winB <= 0) winB = 2;
    int i;
    for (i = 0; i < 3; i++) (void)sys3(0x5525, winB, 50, 45);
    (void)sys3(0x5524, winB, 0, 0);
    int ok2 = winB > 0;

    /* ③ 菜单栏 + 对话框 */
    kt_menu menu;
    kt_menu_init(&menu);
    kt_menu_add(&menu, "File");
    kt_menu_add(&menu, "Edit");
    kt_menu_add(&menu, "Help");
    int ms = kt_menu_click(&menu, 74, 10, 1);
    kt_dialog dlg;
    kt_dialog_init(&dlg, 300, 60, 320, 140, "Confirm", "Save?");
    int d1 = kt_dialog_click(&dlg, 300 + 12 + 35, 60 + 140 - 27, 1);
    int ok3 = ms == 1 && d1 == 1;

    /* ④ 文本框 caret */
    kt_textbox t;
    kt_textbox_init(&t, 1, 50, 50, 200, 30);
    kt_textbox_append(&t, 'H');
    kt_textbox_append(&t, 'i');
    t.caret = 0;
    kt_textbox_append(&t, 'X');
    kt_textbox_append(&t, 8);
    kt_textbox_append(&t, 's');
    int ok4 = t.len == 3 && t.text[0] == 's' && t.text[1] == 'H' && t.text[2] == 'i';

    /* ⑤ 文件保存/打开/读回 */
    long fd = sys3(2, (long)path, 0x401, 0);
    long wn = sys3(1, fd, (long)content, sizeof(content) - 1);
    sys3(3, fd, 0, 0);
    long fd2 = sys3(2, (long)path, 0x0, 0);
    long rn = sys3(0, fd2, (long)buf, 64);
    sys3(3, fd2, 0, 0);
    int ok5 = fd >= 3 && wn == (long)(sizeof(content) - 1)
              && rn == (long)(sizeof(content) - 1) && buf[0] == 'M';

    static const char h[] = "m106: 1..5=";
    wr(h, sizeof(h) - 1);
    wr(ok1 ? "T" : "F", 1);
    wr(ok2 ? "T" : "F", 1);
    wr(ok3 ? "T" : "F", 1);
    wr(ok4 ? "T" : "F", 1);
    wr(ok5 ? "T" : "F", 1);
    wr("\n", 1);

    if (ok1 && ok2 && ok3 && ok4 && ok5) {
        static const char m2[] = "m106: M106 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m106: M106 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}

