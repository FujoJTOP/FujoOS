/* crash_demo.c — M14 进程崩溃隔离演示 (ring3): 双任务
 *
 * 任务 A (0x600000 栈): 正常循环打印。
 * 任务 B (0x640000 栈): 打印几轮后执行空指针写 *(int*)0x100 = 1
 *                       -> 用户 #PF -> 内核终止 B -> A 继续运行。
 *
 * 验证目标: "一个进程崩溃不影响其他"。
 */
typedef long long int64_t;

static int64_t sys3(long nr, long a, long b, long c) {
    int64_t ret;
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

static void pnum(long v) {
    char d[24];
    long i = 24, x = v;
    if (v == 0) {
        const char *z = "0";
        sys3(1, 1, (long)z, 1);
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

static void phex(int64_t v) {
    const char *hex = "0123456789abcdef";
    char d[18];
    d[0] = '0';
    d[1] = 'x';
    for (int i = 0; i < 16; i++) d[2 + i] = hex[(v >> (4 * (15 - i))) & 0xF];
    sys3(1, 1, (long)d, 18);
}

void _start(void) {
    /* 内核任务 id (0x5105 原语): 0 = A, 1 = B —— 比栈地址身份可靠 (共享镜像) */
    long tid = sys3(0x5105, 0, 0, 0);
    volatile long n = 0;
    volatile long round = 0;
    int is_b = tid == 1;
    for (;;) {
        for (long i = 0; i < 30000; i++) n++;
        round++;
        if (round % 2000 == 0) {
            puts("task ");
            pnum(tid);
            puts(" alive round=");
            pnum((long)round);
            puts("\n");
        }
        if (is_b && round > 6000) {
            puts("task ");
            pnum(tid);
            puts(" about to CRASH (null write)\n");
            volatile int *null_ptr = (volatile int *)0x100;
            *null_ptr = 1; /* #PF -> 内核终止 */
            /* 不会回到这里 */
        }
    }
}
