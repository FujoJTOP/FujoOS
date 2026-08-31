/* ipc_test.c — M18 IPC 原语验证 (ring3, 双任务共享镜像)
 *
 * 任务 A (tid=0, 栈 0x600000) / 任务 B (tid=1, 栈 0x640000):
 *   1. pipe: A 创建管道, 写 "ipc: hello through pipe"; B 读回并打印
 *   2. shm:  A 写 32B 模式到 0xA00000; B 读回校验
 *   3. sig:  A 注册 handler (iretq 返回); B 发信号; A 处理并继续
 * 结论: PASS/FAIL (任一任务打印 M18 RESULT: FAIL 则失败; 两个 PASS 均打印为通过)
 *
 * 异步编排: shm[0]=阶段1完成, shm[1]=阶段1可读(B 置), shm[2]=A 可收信号
 */
typedef long long int64_t;
typedef unsigned long long uint64_t;

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

static void phex(long v) {
    const char *hex = "0123456789abcdef";
    char d[18];
    d[0] = '0';
    d[1] = 'x';
    for (int i = 0; i < 16; i++) d[2 + i] = hex[(v >> (4 * (15 - i))) & 0xF];
    sys3(1, 1, (long)d, 18);
}

/* 信号处理函数 (M18): 手工保存/恢复被中断上下文, iretq 返回被中断点
 * (中断现场 9 槽仅恢复到 handler 上下文; handler 必须自行保存/恢复
 *  通用寄存器, 否则被中断循环的寄存器状态被毁 —— 栈上 push/pop 位于
 *  帧下方, 不覆盖内核构造的 iretq 帧) */
__attribute__((naked, noinline)) void sig_handler(void) {
    asm volatile(
        "push %rcx\n\t"
        "push %r11\n\t"
        "push %rax\n\t"
        "push %rdi\n\t"
        "push %rsi\n\t"
        "push %rdx\n\t"
        "movabs $0xA00100, %rax\n\t" /* shm[0x100/8]: 计数槽 */
        "incq (%rax)\n\t"
        "movabs $0x5122, %rax\n\t" /* fujo_sigret() */
        "xor %rdi, %rdi\n\t"
        "xor %rsi, %rsi\n\t"
        "xor %rdx, %rdx\n\t"
        "syscall\n\t"
        "pop %rdx\n\t"
        "pop %rsi\n\t"
        "pop %rdi\n\t"
        "pop %rax\n\t"
        "pop %r11\n\t"
        "pop %rcx\n\t"
        "iretq\n\t");
}

/* 信号帧 (内核构造): [RIP][CS][RFLAGS][RSP][SS] 由 sig_handler iretq 弹出 */
void _start(void) {
    long tid = sys3(0x5105, 0, 0, 0);
    volatile long *shm = (volatile long *)0xA00000;
    int fail = 0;

    puts(tid == 0 ? "ipc: task A up (writer/sender)\n" : "ipc: task B up (reader/sig-sender)\n");

    if (tid == 0) {
        /* --- 1. 管道: 创建 (内核写两个 u32: [0]=rfd, [1]=wfd) --- */
        int fds[2];
        long rc = sys3(0x5110, (long)fds, 0, 0);
        puts("ipc: A pipe rc=");
        pnum(rc);
        putc(' ');
        pnum(fds[0]);
        putc(',');
        pnum(fds[1]);
        putc('\n');
        if (rc != 0) { fail = 1; }
        shm[8] = fds[0]; /* 共享给 B */
        shm[9] = fds[1];
        /* --- 3. 信号: 注册 handler --- */
        rc = sys3(0x5120, (long)&sig_handler, 0, 0);
        puts("ipc: A sigset rc=");
        pnum(rc);
        putc('\n');
        /* shm[1]=0 表示 B 未读; 先写消息到管道 */
        const char *msg = "ipc: hello through pipe (M18)\n";
        long n = 0;
        while (msg[n]) n++;
        rc = sys3(1, fds[1], (long)msg, n);
        puts("ipc: A pipe write rc=");
        pnum(rc);
        putc('\n');
        if (rc != n) { fail = 1; }
        shm[2] = 1; /* 已就绪: B 可发信号 (B 读管道完成后置 3) */

        /* 等 B 读完管道 (shm[1] 由 B 置 1), 然后写 shm 模式 */
        while (shm[1] == 0) { /* 忙等; PIT 时间片让 B 跑 */ }
        long i;
        volatile char *sm = (volatile char *)0xA00000;
        char *mbase = (char *)sm + 0x200;
        for (i = 0; i < 32; i++) mbase[i] = (char)(i * 7 + 3);
        shm[4] = 1; /* 模式已写 */

        /* 等 B 完成发信号 (shm[5]), 再等信号投递 (计数在 0xA00100) */
        while (shm[5] == 0) { }
        volatile long *count = (volatile long *)0xA00100;
        long seen = 0;
        for (long it = 0; it < 20000000; it++) {
            if (*count > 0) { seen = *count; break; }
        }
        puts("ipc: A sig count=");
        pnum(seen);
        putc('\n');
        if (seen < 1) { fail = 1; }
        if (fail) {
            puts("ipc: A M18 RESULT: FAIL\n");
        } else {
            puts("ipc: A M18 RESULT: PASS\n");
        }
        sys3(60, 0, 0, 0);
    } else {
        /* --- 任务 B --- */
        long fds0 = shm[8], fds1 = shm[9];
        char buf[128];
        long n = 0;
        /* 等 A 写完管道 */
        for (long it = 0; it < 100000; it++) {
            n = sys3(0, fds0, (long)buf, 127);
            if (n > 0) break;
        }
        puts("ipc: B pipe read n=");
        pnum(n);
        putc('\n');
        if (n > 0) { buf[n] = 0; puts(buf); }
        if (n <= 0) { fail = 1; }
        shm[1] = 1; /* 标记已读 */

        /* 等 A 写 shm 模式, 校验 */
        while (shm[4] == 0) { }
        char *mbase = ((char *)0xA00000) + 0x200;
        long bad = 0;
        for (long i = 0; i < 32; i++) {
            if (mbase[i] != (char)(i * 7 + 3)) { bad = 1; }
        }
        puts(bad ? "ipc: B shm verify FAIL\n" : "ipc: B shm verify OK (32B pattern)\n");
        if (bad) { fail = 1; }

        /* 发信号给 A (等待 shm[2]==1) */
        while (shm[2] == 0) { }
        long rc = sys3(0x5121, 0, 1, 0);
        puts("ipc: B sigkill(tid=0) rc=");
        pnum(rc);
        putc('\n');
        if (rc != 0) { fail = 1; }
        shm[5] = 1; /* 已发信号, A 可查计数 */
        if (fail) {
            puts("ipc: B M18 RESULT: FAIL\n");
        } else {
            puts("ipc: B M18 RESULT: PASS\n");
        }
        /* B 不退出 (exit 是内核接管停机); 忙等让 A 完成 */
        for (;;) { }
    }
}
