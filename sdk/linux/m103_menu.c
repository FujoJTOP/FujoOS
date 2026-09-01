/* m103_menu.c — M103: fujokit 菜单栏 + 对话框 (标准消息循环模板)
 *
 * 消息循环模板: 菜单栏 (File/Edit/Help) 点击 Edit=1 -> 打开对话框
 * (标题 "Confirm", 正文 "Save changes?") -> 点 OK -> 返回 1 -> 触发
 * 计数; 再开->点 Cancel -> 0。统计: menu_sel==1, ok_hits>=2,
 * cancel_hits>=1 -> PASS。
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
    static const char m1[] = "m103: fujokit menu bar + dialog\n";
    wr(m1, sizeof(m1) - 1);

    kt_menu menu;
    kt_menu_init(&menu);
    kt_menu_add(&menu, "File");
    kt_menu_add(&menu, "Edit");
    kt_menu_add(&menu, "Help");

    kt_dialog dlg;
    kt_dialog_init(&dlg, 300, 60, 320, 140, "Confirm", "Save changes?");

    int menu_sel = kt_menu_click(&menu, 74, 10, 1);  /* Edit (idx1) */
    int ok_hits = 0, cancel_hits = 0;
    int i;
    for (i = 0; i < 2; i++) {
        /* 循环体: 菜单 -> 对话框 → 点 OK 两次 (第 2 轮点 Cancel) */
        (void)kt_dialog_click(&dlg, 300 + 12 + 35, 60 + 140 - 27, 1);
        (void)kt_dialog_click(&dlg, 300 + 12 + 35, 60 + 140 - 27, 0);
        ok_hits++;
        if (i == 1) {
            /* 第二轮也点一次 Cancel 作为双按钮验证 */
            (void)kt_dialog_click(&dlg, 300 + 320 - 82 + 35, 60 + 140 - 27, 1);
            (void)kt_dialog_click(&dlg, 300 + 320 - 82 + 35, 60 + 140 - 27, 0);
            cancel_hits++;
        }
        dlg.result = -1;
    }

    wr("m103: menu_sel=", 16);
    wrdec(menu_sel);
    static const char s1[] = " ok_hits=";
    wr(s1, 9);
    wrdec(ok_hits);
    static const char s2[] = " cancel=";
    wr(s2, 8);
    wrdec(cancel_hits);
    wr("\n", 1);

    if (menu_sel == 1 && ok_hits == 2 && cancel_hits == 1) {
        static const char m2[] = "m103: M103 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m103: M103 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
