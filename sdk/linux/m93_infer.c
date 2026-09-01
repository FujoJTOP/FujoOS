/* m93_infer.c — M93: 推理执行器插槽 (宿主链路 → 内核确定性评估)
 *
 * 1. infer_set(1) (本地) → infer_run("pending status?") → 响应
 *    "fujo-infer-local: recv=N tokens intent=QUERY" 非空
 * 2. infer_set(0) (宿主链路占位) → run → 响应存在
 * 3. infer_slot → mode, calls>=2, tokens>0 → PASS
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
static int64_t sys4(long nr, long a, long b, long c, long d)
{
    return sys3(nr, a, b, c), sys3(4, a, b, c); /* 占位防止误用 */
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrhex(u32 v)
{
    static const char H[] = "0123456789abcdef";
    char b[9];
    int i;
    for (i = 0; i < 8; i++) {
        b[i] = H[(v >> (28 - i * 4)) & 0xF];
    }
    wr(b, 8);
}

static char out[128];
static u64 st[4];

static int64_t run4(long nr, long a, long b, long c, long d)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10)
                 : "rcx", "r11", "memory");
    return rax;
}

void _start(void)
{
    static const char m1[] = "m93: inference executor slots\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x8303, 1, 0, 0); /* local */
    long n1 = run4(0x8301, (long)"pending status?", 15, (long)out, sizeof(out));
    int ok1 = n1 > 10 && out[0] == 'f';
    /* 宿主链路 (占位响应) */
    (void)sys3(0x8303, 0, 0, 0);
    long n2 = run4(0x8301, (long)"hello?", 6, (long)out, sizeof(out));
    int ok2 = n2 > 10;

    (void)sys3(0x8302, (long)st, 0, 0);
    u64 mode = st[0], calls = st[1], tokens = st[2], last_ms = st[3];

    static const char h1[] = "m93: n1=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)n1);
    static const char h2[] = " calls=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)calls);
    static const char h3[] = " tokens=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)tokens);
    wr("\n", 1);

    int ok = ok1 && ok2 && mode == 0 && calls >= 2 && tokens >= 20
             && last_ms >= 0;
    if (ok) {
        static const char m2[] = "m93: M93 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m93: M93 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
