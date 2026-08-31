/* m40_ime.c — M40: IME 预留骨架 (拼音输入流程) 验证
 *
 * fujo 原语: 0x5701 begin / 0x5702 key(ch) / 0x5703 candidates(ptr,n)
 *            0x5704 commit(i) / 0x5705 reset / 0x5706 out(ptr)
 * 流程: begin -> 逐字符 "nihao" -> candidates -> commit(0) ->
 *        out() 读回 -> 打印; 再 "beijing"/"zhongguo" 两个词验证。
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

static int run_ime_word(const char *py)
{
    /* volatile 读防 clang 内建化回 strlen */
    int len = 0;
    while (((const volatile char *)py)[len] != 0) len++;
    (void)sys3(0x5701, 0, 0, 0);
    const char *p;
    for (p = py; *p; p++) {
        (void)sys3(0x5702, (long)*p, 0, 0);
    }
    u32 cands[4];
    long n = sys3(0x5703, (long)cands, 4, 0);
    if (n <= 0) {
        return -1;
    }
    (void)sys3(0x5704, 0, 0, 0);
    char out[32];
    (void)sys3(0x5706, (long)out, 0, 0);
    static const char lb[] = "m40: pinyin='";
    wr(lb, 17);
    wr(py, len);
    static const char lb2[] = "' -> ";
    wr(lb2, 6);
    {
        int olen = 0;
        while (((const volatile char *)out)[olen] != 0) olen++;
        wr(out, olen);
    }
    wr("\n", 1);
    return (int)n;
}

void _start(void)
{
    static const char m1[] = "m40: IME skeleton - pinyin framework demo\n";
    wr(m1, sizeof(m1) - 1);

    int total = 0;
    int r = run_ime_word("nihao");
    if (r > 0) total += r;
    r = run_ime_word("beijing");
    if (r > 0) total += r;
    r = run_ime_word("zhongguo");
    if (r > 0) total += r;

    if (total >= 3) {
        static const char m2[] = "m40: M40 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m40: M40 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
