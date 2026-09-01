/* m73_edit.c — M73: 迷你编辑器 (vi 子集 v0)
 *
 * 1. init; text("abcd\nefg") → 光标 (1,3) (末行末尾)
 * 2. j/k 移动验证: k → (0,3); j → (1,3)
 * 3. k 到 (0,3) → 'x' 删 'd' → "abc\nefg"
 * 4. ^/$ 列移动: $ → col=3, ^ → col=0
 * 5. i 插入 'x' Esc → 行 0 变成 "xabc..."?  ^ 后 i 插入 'X', Esc →
 *    "Xabc\nefg"; dump 校验
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

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

static const char text[] = "abcd\nefg";
static unsigned char dump[64];
static u64 info[4];

void _start(void)
{
    static const char m1[] = "m73: vi-subset editor v0\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    (void)sys3(0x7401, 0, 0, 0);
    (void)sys3(0x7402, (long)text, sizeof(text) - 1, 0);
    (void)sys3(0x7405, (long)info, 0, 0);
    u64 r0 = info[0], c0 = info[1], l0 = info[2];
    int ok1 = r0 == 1 && c0 == 3 && l0 == 2; /* 末行 "efg" 尾 */

    /* j/k */
    (void)sys3(0x7403, 'k', 0, 0);
    (void)sys3(0x7403, 'j', 0, 0);
    (void)sys3(0x7405, (long)info, 0, 0);
    int ok2 = info[0] == 1 && info[1] == 3;

    /* k + x 删 'd' */
    (void)sys3(0x7403, 'k', 0, 0);
    (void)sys3(0x7403, 'x', 0, 0);
    (void)sys3(0x7404, (long)dump, 64, 0);
    int ok3 = dump[0] == 'a' && dump[1] == 'b' && dump[2] == 'c' && dump[3] == '\n'
              && dump[4] == 'e' && dump[5] == 'f' && dump[6] == 'g';

    /* $ / ^ */
    (void)sys3(0x7403, '$', 0, 0);
    (void)sys3(0x7405, (long)info, 0, 0);
    u64 cols = info[1];
    (void)sys3(0x7403, '^', 0, 0);
    (void)sys3(0x7405, (long)info, 0, 0);
    int ok4 = cols == 3 && info[1] == 0;

    /* i 插入 'X' + Esc */
    (void)sys3(0x7403, 'i', 0, 0);
    (void)sys3(0x7403, 'X', 0, 0);
    (void)sys3(0x7403, 0x1B, 0, 0);
    (void)sys3(0x7404, (long)dump, 64, 0);
    int ok5 = dump[0] == 'X' && dump[1] == 'a' && dump[2] == 'b' && dump[3] == 'c'
              && dump[4] == '\n' && dump[5] == 'e' && dump[6] == 'f' && dump[7] == 'g';

    for (i = 0; i < 8; i++) {
        char c[1];
        c[0] = (char)dump[i];
        wr(c, 1);
    }
    wr("\n", 1);

    if (ok1 && ok2 && ok3 && ok4 && ok5) {
        static const char m2[] = "m73: M73 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m73: M73 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
