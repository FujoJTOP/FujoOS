/* m20_crash.c — M20 进程级异常恢复验证 (双任务)
 *
 * 任务 A: 正常循环打印 (定时器轮询)
 * 任务 B: 触发用户态 #GP (ud2 -> invalid opcode, vec=6); 内核应隔离 B,
 *         转场回 A —— A 继续运行并打印 "A survived" 即 PASS。
 *
 * 验证点: 用户态任何异常 (非仅 #PF) 都走崩溃隔离, 不再整机停机。
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

void _start(void) {
    long tid = sys3(0x5105, 0, 0, 0);
    if (tid == 0) {
        /* 任务 A: 持续运行; 每 20 次打印一次 (等待 B 崩溃信号) */
        puts("m20: task A up (survivor)\n");
        volatile long n = 0;
        for (long it = 0; it < 50; it++) {
            for (volatile long i = 0; i < 200000; i++) { n += 1; }
            if (it % 10 == 0) {
                puts("m20: A alive (iteration ");
                /* 简易数字 */
                long d = it;
                char buf[16];
                long j = 16;
                if (d == 0) { buf[--j] = '0'; }
                while (d > 0 && j > 0) {
                    buf[--j] = '0' + (char)(d % 10);
                    d /= 10;
                }
                sys3(1, 1, (long)&buf[j], 16 - j);
                puts(")\n");
            }
        }
        puts("m20: A SURVIVED - user-exception isolation verified\n");
        puts("m20: M20 RESULT: PASS\n");
        sys3(60, 0, 0, 0);
    } else {
        /* 任务 B: 释放几轮后执行 ud2 (#UD, 用户态致命异常) */
        puts("m20: task B up (crash trigger)\n");
        volatile long n = 0;
        for (volatile long i = 0; i < 100000; i++) { n += 1; }
        puts("m20: B triggering #UD (ud2) ...\n");
        asm volatile("ud2");
        /* 不应到达 */
        puts("m20: B UNREACHABLE\n");
        for (;;) { }
    }
}
