/* m104_textbox.c — M104: 文本框光标/输入/退格 (caret 感知)
 *
 * 1. append 'H','i' -> "Hi" caret=2
 * 2. caret 移到 0 -> insert 'X' -> "XHi" (中部插入) caret=1
 * 3. backspace (caret=1) -> 删 'H' -> "Xi" caret=0? 删后 caret=0
 * 4. append 尾部 's' -> "Xis" ; 断言 len==3 文本 "Xis" -> PASS
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
    static const char m1[] = "m104: textbox caret insert/backspace\n";
    wr(m1, sizeof(m1) - 1);

    kt_textbox t;
    kt_textbox_init(&t, 1, 50, 50, 200, 30);

    kt_textbox_append(&t, 'H');
    kt_textbox_append(&t, 'i'); /* "Hi" */

    t.caret = 0;               /* 光标移到行首 */
    kt_textbox_append(&t, 'X'); /* 中部插入 -> "XHi" */

    kt_textbox_append(&t, 8);  /* backspace 删 caret-1 ('X') -> "Hi" */
    kt_textbox_append(&t, 's'); /* caret=0 处插入 -> "sHi" */

    int ok_len = t.len == 3;
    int ok_txt = t.text[0] == 's' && t.text[1] == 'H' && t.text[2] == 'i';
    wr("m104: text='", 12);
    wr(t.text, t.len);
    static const char s1[] = "' caret=";
    wr(s1, 8);
    wrdec(t.caret);
    wr("\n", 1);

    if (ok_len && ok_txt) {
        static const char m2[] = "m104: M104 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m104: M104 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
