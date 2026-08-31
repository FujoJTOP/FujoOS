/* m22_fork.c — M22 fork 验证 (ring3, fork 克隆到 sched)
 *
 * 父任务: fork() 返回子 tid (>0)
 * 子任务: fork() 返回 0; 写 shm[0x300] 标记 (共享地址空间)
 * 调度: PIT 轮转两者; 子先跑 (从 fork 返回, rax=0)
 * 验证:
 *   1. 父得 tid>0, 子得 0 (两个 task 都打印)
 *   2. 子写入的 shm 标记父可读
 *   3. 子 pid (0x5105) = 1, 父 = 0
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

int main_global = 0; /* noop */

void _start(void) {
    long mytid = sys3(0x5105, 0, 0, 0);
    volatile long *mark = (volatile long *)0xA00300;

    puts("m22: before fork (task ");
    pnum(mytid);
    puts(")\n");

    long rc = sys3(57, 0, 0, 0); /* fork() */

    if (rc == 0) {
        /* 子 */
        long t2 = sys3(0x5105, 0, 0, 0);
        puts("m22: CHILD returned 0 (tid=");
        pnum(t2);
        puts("), writing shm mark\n");
        *mark = 0x5A5A + t2; /* 共享地址空间 -> 父可读 */
        /* 子忙等 (2 秒 -> 让调度器给父时间片); 不退出 (exit=整机停机) */
        volatile long spin = 0;
        for (volatile long i = 0; i < 20000000; i++) { spin += 1; }
        puts("m22: CHILD done\n");
        for (;;) { }
    } else if (rc > 0) {
        /* 父 */
        puts("m22: PARENT returned tid=");
        pnum(rc);
        putc('\n');
        /* 等子写标记 (忙等 + 让时间片给子) */
        long seen = 0;
        for (long it = 0; it < 10000000; it++) {
            if (*mark != 0) { seen = *mark; break; }
        }
        puts("m22: PARENT shm mark=");
        pnum(seen);
        putc('\n');
        if (seen == 0x5A5A + 1) {
            puts("m22: M22 RESULT: PASS\n");
        } else {
            puts("m22: M22 RESULT: FAIL\n");
        }
        /* 父继续忙等 (子系统退出兜底) */
        for (;;) { }
    } else {
        puts("m22: fork FAILED rc=");
        pnum(rc);
        putc('\n');
        puts("m22: M22 RESULT: FAIL\n");
        for (;;) { }
    }
}
