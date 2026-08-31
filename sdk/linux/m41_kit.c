/* m41_kit.c — M41: fujokit v0 控件库验证 (按钮/文本框/列表)
 *
 * 零 libc ELF + sdk/kit/fujokit.h。控件布局 + 模拟点击/输入:
 *   - 按钮 "CLICK ME": 3 次模拟点击 -> triggers=3
 *   - 文本框: 追加 "FUJOKIT" 6 字符 -> len=6
 *   - 列表 3 项: 点击第 2 行 -> selected=1
 * 输出三类控件状态 -> PASS。
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
    static const char m1[] = "m41: fujokit v0 - button/textbox/list widgets\n";
    wr(m1, sizeof(m1) - 1);

    kt_button btn;
    kt_textbox tb;
    kt_list ls;
    int i;

    kt_button_init(&btn, 1, 50, 50, 120, 34, "CLICK ME");
    kt_textbox_init(&tb, 2, 50, 120, 200, 30);
    kt_list_init(&ls, 3, 50, 190, 150, 80);
    kt_list_add(&ls, "alpha");
    kt_list_add(&ls, "beta");
    kt_list_add(&ls, "gamma");

    /* 按钮: 3 次命中点击 (按下/释放交替) */
    for (i = 0; i < 3; i++) {
        (void)kt_button_click(&btn, 80, 62, 1);
        (void)kt_button_click(&btn, 80, 62, 0);
    }
    /* 文本框: "FUJOKIT" */
    {
        static const char word[] = "FUJOKIT";
        for (i = 0; i < 6; i++) {
            (void)kt_textbox_append(&tb, word[i]);
        }
    }
    /* 列表: 点击第 2 行 (y=190+12+6) */
    (void)kt_list_click(&ls, 60, 190 + 18, 1);

    wr("m41: button triggers=", 21);
    wrdec(btn.triggers);
    static const char s1[] = " textbox='";
    wr(s1, 11);
    {
        int len = 0;
        while (((const volatile char *)tb.text)[len] != 0) len++;
        wr(tb.text, len);
    }
    static const char s2[] = "' list=";
    wr(s2, 7);
    if (ls.selected >= 0) {
        wr(ls.items[ls.selected], 1);
        {
            int len = 0;
            while (((const volatile char *)ls.items[ls.selected])[len] != 0) len++;
            wr(ls.items[ls.selected], len);
        }
    } else {
        static const char na[] = "none";
        wr(na, 4);
    }
    wr("\n", 1);

    if (btn.triggers == 3 && tb.len == 6 && ls.selected == 1) {
        static const char m2[] = "m41: M41 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m41: M41 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
