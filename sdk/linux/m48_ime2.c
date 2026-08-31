/* m48_ime2.c — M48: 输入法候选窗 + fujokit 集成
 *
 * ime (0x5701 begin / 0x5702 key / 0x5703 candidates / 0x5704 commit /
 * 0x5706 out) + fujokit kt_list 候选 + font 候选窗渲染 (backbuffer)。
 * 流程: begin -> 'zhongguo' 输入 -> candidates(2) -> kt_list 装候选
 * -> 候选窗渲染 (font 两行) -> commit(0) -> out 打印汉字 ->
 * fujokit 列表选中校验 -> PASS。
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

void _start(void)
{
    static const char m1[] = "m48: IME candidate window + fujokit\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x5701, 0, 0, 0);
    {
        static const char py[] = "zhongguo";
        int i;
        for (i = 0; py[i]; i++) {
            (void)sys3(0x5702, (long)py[i], 0, 0);
        }
    }
    u32 cands[4];
    long n = sys3(0x5703, (long)cands, 4, 0);
    wr("m48: candidates=", 16);
    {
        char b[8];
        int i = 8;
        long v = n;
        if (v == 0) b[--i] = '0';
        while (v > 0) {
            b[--i] = '0' + (char)(v % 10);
            v /= 10;
        }
        wr(&b[i], 8 - i);
    }
    wr("\n", 1);

    /* fujokit 列表装候选 */
    kt_list ls;
    kt_list_init(&ls, 1, 300, 100, 200, 50);
    kt_list_add(&ls, "1: China");
    kt_list_add(&ls, "2: Nation");
    (void)kt_list_click(&ls, 320, 112, 1); /* 选第 1 行 */

    /* 候选窗渲染 (font 两行, 模拟窗面) */
    (void)sys3(0x5603, 0xFF000000u, 0, 0);
    (void)sys5(0x5601, 300, 100, 1, 0xFFFFFFFFu, (long)"1: China");
    (void)sys5(0x5601, 300, 108, 1, 0xFFFFFFFFu, (long)"2: Nation");

    (void)sys3(0x5704, 0, 0, 0);
    char out[32];
    (void)sys3(0x5706, (long)out, 0, 0);
    {
        int olen = 0;
        while (((const volatile char *)out)[olen] != 0) olen++;
        static const char pre[] = "m48: commit='";
        wr(pre, 12);
        wr(out, olen);
        static const char post[] = "'\n";
        wr(post, 2);
    }

    int ok = n >= 2 && ls.selected >= 0;
    if (ok) {
        static const char m2[] = "m48: M48 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m48: M48 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
