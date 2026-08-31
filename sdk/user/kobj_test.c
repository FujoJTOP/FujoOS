/* kobj_test.c — M19 内核对象/句柄表验证 (ring3)
 *
 * 流程:
 *   1. fujo_kobj_info 初始基线
 *   2. 创建 pipe (2 个对象) + shm (1) + sig (1) + 手工 create (1)
 *   3. fujo_kobj_info 复查 (计数增长)
 *   4. fujo_kobj_free 释放手工对象 + pipe 一端的 vfs 路径 (close)
 *   5. 最终计数 > 基线即 PASS (对象全生命周期可见)
 */
typedef long long int64_t;

static long sys3(long nr, long a, long b, long c) {
    long ret;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c)
                 : "rcx", "r11", "memory");
    return ret;
}

static void puts(const char *s) {
    long n = 0;
    while (s[n] != 0) n++;
    sys3(1, 1, (long)s, n);
}

static void putc(char c) {
    char s[2];
    s[0] = c;
    s[1] = 0;
    puts(s);
}

static void pnum(long v) {
    char d[24];
    long i = 24, x = v;
    if (v == 0) {
        putc('0');
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

static void report(const char *tag, int cnt[]) {
    puts(tag);
    puts(" file=");
    pnum(cnt[0]);
    puts(" pipe=");
    pnum(cnt[1]);
    puts(" shm=");
    pnum(cnt[2]);
    puts(" sig=");
    pnum(cnt[3]);
    putc('\n');
}

/* 信号处理: 裸 asm (iretq 返回) */
__attribute__((naked, noinline)) void handler(void) {
    asm volatile("movabs $0x5122, %rax\n\t"
                 "xor %rdi, %rdi\n\t"
                 "xor %rsi, %rsi\n\t"
                 "xor %rdx, %rdx\n\t"
                 "syscall\n\t"
                 "iretq\n\t");
}

void _start(void) {
    int base[4] = {0, 0, 0, 0};
    int after[4] = {0, 0, 0, 0};
    int fail = 0;

    puts("kobj: M19 kernel object table test\n");

    /* 1. 基线 */
    sys3(0x5132, (long)base, 4, 0);
    report("kobj: baseline", base);

    /* 2. 创建: pipe(2 对象) + shm(1) + sig(1) + 手工(1) */
    int fds[2];
    long rc = sys3(0x5110, (long)fds, 0, 0);
    if (rc != 0) { puts("kobj: pipe FAIL\n"); fail = 1; }
    long shm = sys3(0x5111, 0, 0, 0);
    if (shm != 0xA00000) { puts("kobj: shm addr FAIL\n"); fail = 1; }
    /* 信号: 注册 handler (裸 asm 返回) */
    rc = sys3(0x5120, (long)&handler, 0, 0);
    if (rc != 0) { puts("kobj: sigset FAIL\n"); fail = 1; }
    long h = sys3(0x5130, 1, 0, 0); /* K_FILE 手工 */
    puts("kobj: manual create slot=");
    pnum(h);
    putc('\n');
    if (h < 0) { puts("kobj: manual create FAIL\n"); fail = 1; }

    /* 3. 复查 */
    sys3(0x5132, (long)after, 4, 0);
    report("kobj: after creates", after);
    if (after[1] != base[1] + 2) { puts("kobj: pipe count FAIL\n"); fail = 1; }
    if (after[2] != base[2] + 1) { puts("kobj: shm count FAIL\n"); fail = 1; }
    if (after[3] != base[3] + 1) { puts("kobj: sig count FAIL\n"); fail = 1; }
    if (after[0] != base[0] + 1) { puts("kobj: file count FAIL\n"); fail = 1; }

    /* 4. 释放: 手工对象 + pipe 读端 close (vfs 路径) + sig ret 路径 */
    rc = sys3(0x5131, h, 0, 0);
    if (rc != 0) { puts("kobj: free FAIL\n"); fail = 1; }
    rc = sys3(0x5122, 0, 0, 0); /* sig (清 active) */
    rc = sys3(3, fds[0], 0, 0);  /* close rfd */
    if (rc != 0) { puts("kobj: close FAIL\n"); fail = 1; }

    /* 5. 最终: 至少还可见 2 个 pipe 端点 (其一已关) + 1 shm */
    int fin[4] = {0, 0, 0, 0};
    sys3(0x5132, (long)fin, 4, 0);
    report("kobj: final", fin);
    puts(fail ? "kobj: M19 RESULT: FAIL\n" : "kobj: M19 RESULT: PASS\n");
    sys3(60, 0, 0, 0);
    for (;;) {}
}
