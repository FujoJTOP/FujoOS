/* m75_dbg.c — M75: 调试器 v0 (单步/断点, 调试寄存器)
 *
 * 1. 断点: dbg_bp0(&dummy) → 调 dummy() → #DB 命中 (一次) → 清;
 * 2. 单步: 3 次 [pushfq|or 0x100|popfq] (用户态 TF) → 每次下条指令
 *    #DB → 内核清 TF;
 * 3. dbg_info → (count>=4, last_rip 非零, steps>=3, bps>=1) → PASS
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

static volatile long g_counter;
static long dummy(void) __attribute__((noinline));
static long dummy(void) { g_counter += 1; return 42; }

static void step_once(void)
{
    asm volatile("pushfq; orq $0x100, (%%rsp); popfq" : : : "memory");
}

static u64 info[4];

void _start(void)
{
    static const char m1[] = "m75: debugger v0 (step/bp via DR)\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x7604, 0, 0, 0);

    /* 0) 裸 int3 测试 (#BP 向量路径) */
    asm volatile("int3" : : : "memory");
    (void)sys3(0x7603, (long)info, 0, 0);
    u64 c0 = info[0];

    /* 1) 断点 @dummy */
    (void)sys3(0x7602, (long)&dummy, 0, 0);
    static const char h0[] = "m75: dummy@";
    wr(h0, sizeof(h0) - 1);
    wrhex((u32)((long)&dummy >> 0));
    wr("\n", 1);
    long v = dummy();
    int bp_ok = v == 42;
    (void)sys3(0x7603, (long)info, 0, 0);
    u64 c1 = info[0], b1 = info[3];

    /* 2) 单步 ×3 */
    (void)sys3(0x7601, 1, 0, 0);
    step_once();
    step_once();
    step_once();
    (void)sys3(0x7603, (long)info, 0, 0);
    u64 count = info[0], rip = info[1], steps = info[2], bps = info[3];

    static const char h1[] = "m75: bp_count=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)c1);
    static const char h2[] = " total=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)count);
    static const char h3[] = " steps=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)steps);
    static const char h4[] = " bps=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)bps);
    static const char h5[] = " rip=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)(rip >> 24));
    wr("\n", 1);

    int ok = bp_ok && c1 >= 1 && b1 >= 1 && count >= 4 && steps >= 3
             && rip != 0 && bps == b1;
    if (ok) {
        static const char m2[] = "m75: M75 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m75: M75 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
