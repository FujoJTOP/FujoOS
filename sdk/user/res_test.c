/* res_test.c — M17 FUJR 容器资源验证 (ring3)
 *
 * 行为: open /runres/icon.bin -> 读(字节数+前几字节 hex) ->
 *       open /runres/hello.txt -> 读内容打印 ->
 *       结论 PASS/FAIL
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

static void phex(int64_t v) {
    const char *hex = "0123456789abcdef";
    char d[18];
    d[0] = '0';
    d[1] = 'x';
    for (int i = 0; i < 16; i++) d[2 + i] = hex[(v >> (4 * (15 - i))) & 0xF];
    sys3(1, 1, (long)d, 18);
}

void _start(void) {
    puts("res: M17 FUJR resources test\n");
    char buf[256];
    int fail = 0;

    /* 1. icon.bin */
    long fd = sys3(2, (long)"/runres/icon.bin", 0, 0);
    long r = 0;
    if (fd >= 3) {
        r = sys3(0, fd, (long)buf, 255);
        sys3(3, fd, 0, 0);
    }
    puts("res: /runres/icon.bin -> len=");
    pnum(r);
    if (r == 64) {
        puts(" (expect 64)");
        putc('\n');
        puts("res: icon bytes: ");
        for (long i = 0; i < r && i < 16; i++) {
            phex((int64_t)(unsigned char)buf[i]);
            putc(' ');
        }
        puts("\n");
    } else {
        puts(" FAIL (expected 64)\n");
        fail = 1;
    }

    /* 2. hello.txt */
    fd = sys3(2, (long)"/runres/hello.txt", 0, 0);
    r = 0;
    if (fd >= 3) {
        r = sys3(0, fd, (long)buf, 255);
        sys3(3, fd, 0, 0);
    } else {
        puts("res: /runres/hello.txt open FAILED\n");
        fail = 1;
    }
    if (r > 0) {
        buf[r] = 0;
        puts("res: /runres/hello.txt content: ");
        puts(buf);
        putc('\n');
    } else {
        puts("res: /runres/hello.txt read FAILED\n");
        fail = 1;
    }

    puts(fail ? "res: M17 RESULT: FAIL\n" : "res: M17 RESULT: PASS\n");
    sys3(60, 0, 0, 0);
    for (;;) {}
}
