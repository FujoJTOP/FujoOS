/* m107_tty.c — 桌面 "Shell" 窗口程序 (M107): 迷你 TTY
 *
 * 用户态窗口化 shell: banner -> 循环读键盘 (0x5103) -> 回显 (write);
 * 键盘在窗口打开时由桌面透传 (共享 kbd ring), 无需注入。
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

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }

void _start(void)
{
    static const char b[] = "FujoOS window shell v0; type anything\n";
    wr(b, sizeof(b) - 1);
    for (;;) {
        long c = sys3(0x5103, 0, 0, 0); /* kbd try_poll */
        if (c > 0 && c < 256) {
            char ch = (char)c;
            if (ch >= 32) {
                wr(&ch, 1);
            }
        }
    }
}
