/* fs_test.c — M15 VFS 验证 (ring3, linux ABI)
 *
 * 流程: open("/proc/meminfo") -> read 打印 ->
 *       open("/tmp/hello.txt") -> read 打印 ->
 *       open("/boot/module") -> read 前 32 字节(hex) ->
 *       open("/dev/tty") -> write 一行(串口) ->
 *       写 /tmp/hello.txt 追加 -> 重新 open 读取(应含追加) -> exit
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

/* 打印文件内容 (读到 buf, 输出前 n 字节) */
static long dump_fd(long fd, char *buf, long n, const char *label) {
    puts(label);
    long r = sys3(0, fd, (long)buf, n);
    puts(" -> read=");
    pnum(r);
    puts("\n");
    if (r > 0 && r < 400) {
        buf[r] = 0;
        puts(buf);
        puts("\n");
    }
    return r;
}

void _start(void) {
    puts("fs: M15 VFS test\n");
    char buf[256];

    /* 1. /proc/meminfo (生成内容) */
    long fd = sys3(2, (long)"/proc/meminfo", 0, 0);
    puts("fs: open /proc/meminfo fd=");
    pnum(fd);
    puts("\n");
    if (fd >= 3) dump_fd(fd, buf, 240, "fs: read meminfo");

    /* 2. /tmp/hello.txt (内存盘种子) */
    fd = sys3(2, (long)"/tmp/hello.txt", 0, 0);
    puts("fs: open /tmp/hello.txt fd=");
    pnum(fd);
    puts("\n");
    if (fd >= 3) dump_fd(fd, buf, 256, "fs: read ramdisk");
    sys3(3, fd, 0, 0); /* close */

    /* 3. /boot/module (initrd 前 32 字节 hex) */
    fd = sys3(2, (long)"/boot/module", 0, 0);
    long r = sys3(0, fd, (long)buf, 32);
    puts("fs: read /boot/module 32B = ");
    for (long i = 0; i < r && i < 32; i++) {
        phex((int64_t)(unsigned char)buf[i]);
        putc(' ');
    }
    puts("\n");

    /* 4. /dev/tty 写一串口行 */
    fd = sys3(2, (long)"/dev/tty", 1, 0);
    if (fd >= 3) {
        sys3(1, fd, (long)"fs: hello via /dev/tty (vfs serial)\n", 36);
    }

    /* 5. 追加 /tmp/hello.txt 后重读 */
    fd = sys3(2, (long)"/tmp/hello.txt", 1, 0);
    if (fd >= 3) {
        sys3(1, fd, (long)"append-M15\n", 11);
        sys3(3, fd, 0, 0);
        fd = sys3(2, (long)"/tmp/hello.txt", 0, 0);
        if (fd >= 3) dump_fd(fd, buf, 256, "fs: ramdisk after append");
    }

    /* 6. ENOENT 路径 */
    fd = sys3(2, (long)"/no/such", 0, 0);
    puts("fs: open /no/such -> ");
    pnum(fd);
    puts(" (expect -2)\n");

    puts("fs: M15 VFS test done\n");
    sys3(60, 0, 0, 0);
    for (;;) {}
}
