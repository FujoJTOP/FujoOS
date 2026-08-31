/* thread_demo.c — M13 线程演示 (ring3): 同一镜像双任务, PIT 时间片轮转
 *
 * 验证目标: 两个任务各自独立栈 (A=0x600000 / B=0x640000) 交替运行,
 *           现场 (计数器/栈) 各自保持 —— 抢占切换零丢失。
 * 任务标识: 取本地变量地址 (与用户栈同区域, 差 4MiB 可区分)。
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
    /* 任务 id ≈ 用户栈区 (A: 0x600000 / B: 0x640000) */
    long local;
    long tid = (long)&local & ~0xFFF;
    long n = 0;
    long round = 0;
    for (;;) {
        /* 忙转 (可被 PIT 任意打断并恢复 —— 现场即证明) */
        for (long i = 0; i < 40000; i++) n++;
        round++;
        if (round % 2000 == 0) {
            puts("task ");
            phex(tid);
            puts(" alive round=");
            pnum(round);
            puts(" n=");
            pnum(n);
            puts("\n");
        }
    }
}
