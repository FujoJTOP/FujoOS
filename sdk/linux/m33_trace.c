/* m33_trace.c — M33: 系统调用追踪 (trace 工具 + 计数)
 *
 * 零 libc ELF。fujo 原生 trace 原语:
 *   0x5301 trace_enable(on) / 0x5302 trace_show() / 0x5303 trace_count(nr)。
 * 流程: open/read/close 一次(不追踪) -> enable(1) -> write/open/read/close
 * 若干 -> trace_count 校验计数 -> trace_show 输出 ring -> PASS。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m33_trace.c -o sdk/linux/m33_trace.elf
 */
typedef long int64_t;

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

static void wrdec(int64_t v)
{
    char b[24];
    int i = 24;
    if (v < 0) {
        b[--i] = '-';
        v = -v;
    }
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m33: syscall trace tool - counters demo\n";
    wr(m1, sizeof(m1) - 1);

    /* 先不发 trace: open/read/close 各一 */
    long fd0 = sys3(2, (long)"/boot/module", 0, 0);
    char b0[8];
    (void)sys3(0, fd0, (long)b0, 8);
    (void)sys3(3, fd0, 0, 0);

    /* 开启 trace */
    (void)sys3(0x5301, 1, 0, 0);

    /* 受追踪的调用: write×3, open×2, read×2, close×2 */
    (void)sys3(1, 1, (long)m1, 1); /* 单字符 write */
    (void)sys3(1, 1, (long)m1, 1);
    (void)sys3(1, 1, (long)m1, 1);
    long fd1 = sys3(2, (long)"/boot/module", 0, 0);
    char b1[8];
    (void)sys3(0, fd1, (long)b1, 8);
    (void)sys3(3, fd1, 0, 0);
    long fd2 = sys3(2, (long)"/tmp/hello.txt", 0, 0);
    (void)sys3(0, fd2, (long)b1, 4);
    (void)sys3(3, fd2, 0, 0);

    /* 计数校验: write=1, open=2, close=2 */
    long wc = sys3(0x5303, 1, 0, 0);
    long oc = sys3(0x5303, 2, 0, 0);
    long cc = sys3(0x5303, 3, 0, 0);
    wr("m33: counts write=", 19);
    wrdec(wc);
    wr(" open=", 6);
    wrdec(oc);
    wr(" close=", 7);
    wrdec(cc);
    wr("\n", 1);

    if (wc >= 3 && oc >= 2 && cc >= 2) {
        static const char ok[] = "m33: counters OK\n";
        wr(ok, sizeof(ok) - 1);
    } else {
        static const char bad[] = "m33: counters MISMATCH\n";
        wr(bad, sizeof(bad) - 1);
    }

    /* 输出 ring */
    (void)sys3(0x5302, 0, 0, 0);

    static const char m2[] = "m33: M33 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
