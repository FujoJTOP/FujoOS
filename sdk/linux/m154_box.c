/* m154_box.c — W36/B-2: BOX-BRIDGE v0 通路验证 (LEGO 工程收尾波, docs/110)
 *
 * 三种宿主模式 (由 box_server --mode 决定) 与四种断言路径:
 *   normal  : 4 动词全链 (hash/info/size/echo) + 双列台账 + 检疫门 -> BX V0 RESULT: PASS
 *   offline : 无宿主盒 -> TTL 超时 -> 缺席声明路径 -> BOX OFFLINE PASS
 *   badart  : 宿主返回 ELF 魔数产物 -> 检疫门拒收 -> BOX GATE PASS
 *   adapter : 宿主返回 schema 违约产物 -> 列2a 记败 -> BOX ADAPTER PASS
 * demo 结果驱动, 无需知道运行模式 (与 docs/109 §9 验收一致)。
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

/* 产物含子串? (等待内容含空格, 不用 strcmp 全等) */
static int contains(const char *hay, const char *need)
{
    long hn = sLen(hay), nn = sLen(need);
    long i, j;
    if (nn == 0)
        return 1;
    for (i = 0; i + nn <= hn; i++) {
        for (j = 0; j < nn; j++)
            if (hay[i + j] != need[j])
                break;
        if (j == nn)
            return 1;
    }
    return 0;
}

static int eq(const char *a, const char *b)
{
    long i = 0;
    while (a[i] && b[i] && a[i] == b[i])
        i++;
    return a[i] == b[i];
}

static int do_hash(const char *arg, const char *expect)
{
    char out[96];
    long rc = sy(0x8316, 1, (long)arg, sLen(arg), 0, 0);
    if (rc == -4) {
        wrstr("box  : hash OFFLINE (no provider)\n");
        return -4;
    }
    if (rc == -2) {
        wrstr("box  : hash GATE-REJECT\n");
        return -2;
    }
    if (rc == -3) {
        wrstr("box  : hash SCHEMA-FAIL\n");
        return -3;
    }
    long n = sy(0x8318, (long)out, sizeof(out), 0, 0, 0);
    out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
    wrstr("box  : hash=");
    wr(out, n);
    wrstr("\n");
    if (n != 64 || !eq(out, expect))
        return 0;
    return 1;
}

static int do_info(const char *arg)
{
    char out[128];
    long rc = sy(0x8316, 2, (long)arg, sLen(arg), 0, 0);
    if (rc != 0)
        return 0;
    long n = sy(0x8318, (long)out, sizeof(out), 0, 0, 0);
    out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
    wrstr("box  : info='");
    wr(out, n);
    wrstr("'\n");
    return contains(out, "text") || contains(out, "ASCII") || n >= 4;
}

static int do_size(const char *arg, const char *expect)
{
    char out[32];
    long rc = sy(0x8316, 3, (long)arg, sLen(arg), 0, 0);
    if (rc != 0)
        return 0;
    long n = sy(0x8318, (long)out, sizeof(out), 0, 0, 0);
    out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
    wrstr("box  : size=");
    wr(out, n);
    wrstr("\n");
    return eq(out, expect);
}

static int do_echo(const char *arg)
{
    char out[160];
    long rc = sy(0x8316, 4, (long)arg, sLen(arg), 0, 0);
    if (rc != 0)
        return 0;
    long n = sy(0x8318, (long)out, sizeof(out), 0, 0, 0);
    out[n < sizeof(out) ? n : sizeof(out) - 1] = 0;
    wrstr("box  : echo='");
    wr(out, n);
    wrstr("'\n");
    return n == sLen(arg) && eq(out, arg);
}

static int run(void)
{
    static const char h[] = "m154: BOX-BRIDGE v0 path (LEGO finale)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;
    /* 期望 sha256("fujobox-v0 payload") = d4a997af... (预计算常量) */
    static const char arg[] = "fujobox-v0 payload";
    static const char expect[] = "d4a997afdb0442de74a37dd7cad5eed9822b2f3084f605fd6d172d00d28f1520";
    static const char sz[] = "18";

    /* 授权面: exec 槽 6 全权 0x7F (act7 = BOX_CMD); 域 1 = provider 端口域 */
    sy(0x8101, 6, 0x7F, 0, 0, 0);
    long d1 = sy(0x8107, 0x40, 1, 1, 0, 0); /* perm = act7 初始, as=low, irq=1 */
    sy(0x8108, d1, 0, 0, 0, 0);
    wrstr("m154: T1 provider domain=");
    wrdec((u64)d1);
    wrstr(" (act7 BOX_CMD)\n");

    /* T2 hash (歧义路径: offline/gate/adapter/ok 由 rc 区分) */
    int r = do_hash(arg, expect);
    if (r == -4) {
        wrstr("m154: BOX OFFLINE PASS\n");
        return 0;
    }
    if (r == -2) {
        wrstr("m154: BOX GATE PASS\n");
        return 0;
    }
    if (r == -3) {
        wrstr("m154: BOX ADAPTER PASS\n");
        return 0;
    }
    if (r != 1)
        pass = 0;

    /* T3 info / size / echo */
    if (!do_info(arg))
        pass = 0;
    if (!do_size(arg, sz))
        pass = 0;
    if (!do_echo(arg))
        pass = 0;

    /* T4 双列台账 (duty 7/8) + 检疫审计 (action 9/10) */
    {
        u64 st[4];
        long rc = sy(0x8317, 1, (long)st, 0, 0, 0);
        (void)rc;
        wrstr("m154: T4 ledger up=");
        wrdec(st[0]);
        wrstr(" hit=");
        wrdec(st[1]);
        wrstr(" total=");
        wrdec(st[2]);
        wrstr(" schema=");
        wrdec(st[3]);
        wrstr("\n");
        if (st[0] != 1 || st[1] == 0 || st[2] == 0 || st[3] == 0)
            pass = 0;
    }

    if (pass) {
        static const char m2[] = "m154: BX V0 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m154: BX V0 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(0x8109, d1, 0, 0, 0, 0);
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
