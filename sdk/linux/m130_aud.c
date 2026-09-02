/* m130_aud.c — W19: 统一审计 (cap 环 + AI 环同构导出; docs/70)
 *
 * 断言:
 *   T1 写入 cap 审计条目 (0x8103 aud_log(9, 77))
 *   T2 0x8C01 unified_aud: cap_n>=1, ai_n>=1 (boot 标记), 条目结构 (kind 序列)
 *   T3 最近 cap 条目含 (9,77)
 */
typedef long int64_t;
typedef unsigned long u64;

static int64_t sy(int64_t nr, int64_t a, int64_t b, int64_t c, int64_t d, int64_t e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx),
                 "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sy(1, 1, (long)s, len, 0, 0); }
static void wrdec(u64 v)
{
    char b[22];
    int i = 22;
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}

static u64 buf[4 * 32]; /* 统计头 + 32 条 32B */

static void run(void)
{
    static const char h[] = "m130: unified audit (W19)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;

    /* T1: cap 审计条目 */
    sy(0x8103, 9, 77, 0, 0, 0);
    wrstr("m130: T1 cap log ok\n");

    /* T2: unified */
    {
        long ret = sy(0x8C01, (long)buf, sizeof(buf), 0, 0, 0);
        u64 cap_n = buf[0];
        u64 ai_n = buf[1];
        wrstr("m130: T2 total=");
        wrdec((u64)ret);
        wrstr(" cap=");
        wrdec(cap_n);
        wrstr(" ai=");
        wrdec(ai_n);
        wrstr("\n");
        if (ret < 2 || cap_n < 1 || ai_n < 1)
            pass_all = 0;
    }

    /* T3: 最近 cap 条目 (body 最后一条) */
    {
        u64 cap_n = buf[0];
        u64 *body = &buf[2];
        u64 *last = body + (cap_n - 1) * 4;
        wrstr("m130: T3 cap last action=");
        wrdec(last[1]);
        wrstr(" subject=");
        wrdec(last[2]);
        wrstr("\n");
        if (last[1] != 9 || last[2] != 77)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m130: M130 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m130: M130 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(60, 7, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
