/* game.tpl.c — FujoOS SDK 模板: 2D 游戏帧循环
 *
 * 完整基准: docs/18-game2.md; 原语: 0x6100 arm / 0x6101 us /
 * 0x6104 frame_wait / 0x6801 blit / 0x6202 rect / 0x6F01 延迟上报。
 */
typedef long int64_t;
typedef unsigned int u32;

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
static int64_t sys3(long nr, long a, long b, long c)
{
    return sys5(nr, a, b, c, 0, 0);
}

void _start(void)
{
    static const char m1[] = "game: template frame loop\n";
    sys3(1, 1, (long)m1, sizeof(m1) - 1);
    sys3(0x6100, 0, 0, 0); /* timer 校准 */

    int frame = 0;
    for (frame = 0; frame < 10; frame++) {
        long t0 = sys3(0x6101, 0, 0, 0);       /* 输入采样 */
        (void)sys5(0x6202, 100, 100, 32, 32, 0xFF2020); /* 矩形实体 */
        long t1 = sys3(0x6101, 0, 0, 0);
        sys3(0x6F01, (long)(t1 - t0), 0, 0);   /* 输入→渲染延迟上报 */
        sys3(0x6104, 16666, 0, 0);             /* 帧等待 ~60fps */
    }
    sys3(60, 0, 0, 0);
    for (;;) {
    }
}
