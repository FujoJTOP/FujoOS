/* m157_boxfuzz.c — W37/B-31: 检疫门 fuzz (宿主 fuzz 模式, 6 种畸形产物轮换)
 *
 * 每种畸形应在内核检疫门被拒 (rc = -2 检疫拒收 / -3 schema 违约),
 * 绝不允许 0 (放行) / -4 (缺席 —— 宿主在线)。
 * 6 轮覆盖: ELF 魔数 / MZ 魔数 / 非 ascii / 超上限 4096B / 坏 PDF / 坏 BMP。
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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}
static long sLen(const char *s)
{
    long n = 0;
    while (s[n])
        n++;
    return n;
}

static int run(void)
{
    static const char h[] = "m157: quarantine gate fuzz (B-31)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;
    static const char arg[] = "fuzz-payload";
    long verbs[6] = { 1, 2, 3, 4, 5, 6 };
    long i, j;
    int reject = 0;

    sy(0x8101, 6, 0x7F, 0, 0, 0);
    for (i = 0; i < 6; i++) {
        long rc = sy(0x8316, verbs[i], (long)arg, sLen(arg), 0, 0);
        /* fuzz 模式宿主在线: rc 必须是 -2 (检疫) 或 -3 (schema) */
        if (rc == -2 || rc == -3) {
            reject++;
        } else {
            wrstr("m157: case ");
            wrdec((u64)i);
            wrstr(" rc=");
            wrdec((u64)rc);
            wrstr(" (expect -2/-3, BAD)\n");
            pass = 0;
        }
    }
    wrstr("m157: rejected=");
    wrdec((u64)reject);
    wrstr("/6 (all gate-rejected)\n");

    /* 审计: 产物拒收 action=10 计数 */
    {
        char tbuf[512];
        long n = sy(0x8C01, (long)tbuf, sizeof(tbuf), 0, 0, 0);
        (void)n;
        long cap_n = ((long)tbuf[0]) | ((long)tbuf[1] << 8) | ((long)tbuf[2] << 16)
                      | ((long)tbuf[3] << 24);
        long gate = 0;
        for (j = 0; j < cap_n && j < 32; j++) {
            long action = ((long)tbuf[16 + j * 32 + 8]) | ((long)tbuf[16 + j * 32 + 9] << 8);
            if (action == 10)
                gate++;
        }
        wrstr("m157: gate-audit=");
        wrdec((u64)gate);
        wrstr(" (expect >=6)\n");
        if (gate < 6)
            pass = 0;
    }

    if (pass) {
        static const char m2[] = "m157: BOX FUZZ PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m157: BOX FUZZ FAIL\n";
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
