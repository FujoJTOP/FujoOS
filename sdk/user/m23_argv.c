/* m23_argv.c — M23 验证: argc/argv 栈帧正确性 (ring3)
 *
 * 与 busybox 相同入口约定 (glibc _start 风格):
 *   pop rsi = argc; rdx = argv; rdi = stack_end...
 * 这里更简单: 读 [rsp]=argc, [rsp+8]=argv[0] 字符串。
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

void msys_start(void) {
    /* _start 语义: rsp 指向 argc */
    long rsp;
    asm volatile("mov %%rsp, %0" : "=r"(rsp));
    long argc = *(volatile long *)rsp;
    char **argv = (char **)(void *)(rsp + 8);
    puts("m23: argv test argc=");
    pnum(argc);
    putc('\n');
    if (argc == 1) {
        char *a0 = argv[0];
        puts("m23: argv[0]='");
        long n = 0;
        while (a0[n] != 0 && n < 32) {
            putc(a0[n]);
            n += 1;
        }
        puts("'\n");
        puts("m23: M23 ARGV PASS\n");
        sys3(60, 0, 0, 0);
    } else {
        puts("m23: ARGV FAIL (argc!=1)\n");
    }
    for (;;) {}
}
