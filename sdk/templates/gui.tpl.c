/* gui.tpl.c — FujoOS SDK 模板: fujokit GUI (窗口/按钮/列表)
 *
 * kit: sdk/kit/fujokit.h (kt_button/kt_textbox/kt_list);
 * 窗口表: 0x55xx wm (wmsg); 入口参照 m41_kit.c / m37-38 wm 样例。
 * 本模板为骨架: 打开窗口 + 注册按钮 + 列表。
 */
typedef long int64_t;

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

void _start(void)
{
    static const char m1[] = "gui: template (fujokit skeleton)\n";
    sys3(1, 1, (long)m1, sizeof(m1) - 1);
    /* 实际项 (详见 sdk/linux/m41_kit.c):
     *   0x5106 kt_window(x,y,w,h,title) → win id
     *   0x5107 kt_button(win,x,y,w,h,label)
     *   0x5108 kt_textbox(win,x,y,w,h)
     *   0x5109 kt_list(win,x,y,w,h,n,items)
     *   0x55xx wm: 窗口表/消息队列/刷新
     */
    sys3(60, 0, 0, 0);
    for (;;) {
    }
}
