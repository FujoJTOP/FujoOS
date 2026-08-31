/* m53_xin.c — M53: XInput 式输入抽象验证
 * 0x6001 xin_get / 0x6002 xin_reset / 0x6003 xin_press(bit)
 */
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
static void wrdec(u32 v)
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
    static const char m1[] = "m53: XInput-style input abstraction\n";
    wr(m1, sizeof(m1) - 1);
    (void)sys3(0x6002, 0, 0, 0);
    u32 s0[5];
    (void)sys3(0x6001, (long)s0, 0, 0);
    static const char a1[] = "m53: init buttons=";
    wr(a1, sizeof(a1) - 1);
    wrdec(s0[0]);
    wr("\n", 1);
    (void)sys3(0x6003, 1, 0, 0);
    (void)sys3(0x6003, 4, 0, 0);
    u32 s1v[5];
    (void)sys3(0x6001, (long)s1v, 0, 0);
    static const char b1[] = "m53: after press buttons=";
    wr(b1, sizeof(b1) - 1);
    wrdec(s1v[0]);
    wr("\n", 1);
    int ok = s0[0] == 0 && s1v[0] == 5;
    if (ok) {
        static const char m2[] = "m53: M53 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m53: M53 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
