/* disk_fs.c — M16 FJFS 持久化验证 (ring3)
 *
 * 模式选择 (argv 无需; 命令行):
 *   os run hermes   -> 本测试: 若 /disk/hello.txt 已存在则证明持久化
 *
 * 行为: 打开 /disk/hello.txt 读取 ->
 *       若为空: 写入 "FJFS persistent data #1\n" 并 close (刷盘) ->
 *       再 open 读回打印;
 *       若非空: 打印内容, 追加一行 "seen-boot2\n" 再写回。
 * 验证: 两次启动同一磁盘镜像: 第一次写入, 第二次读到 boot#1 内容。
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
        puts("0");
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

void _start(void) {
    puts("fjfs: M16 persistence test\n");

    static char buf[512];
    long fd = sys3(2, (long)"/disk/hello.txt", 0, 0);
    long r = 0;
    if (fd >= 3) {
        r = sys3(0, fd, (long)buf, 500);
        sys3(3, fd, 0, 0);
    } else {
        puts("fjfs: open /disk/hello.txt FAILED\n");
        sys3(60, 0, 0, 0);
        for (;;) {}
    }

    if (r > 0) {
        puts("fjfs: boot#N found existing content (len=");
        pnum(r);
        puts("):\n");
        buf[r] = 0;
        puts(buf);
        /* 追加一行并写回 (验证写穿+再读) */
        fd = sys3(2, (long)"/disk/hello.txt", 1, 0);
        if (fd >= 3) {
            sys3(1, fd, (long)"seen-boot2\n", 11);
            sys3(3, fd, 0, 0);
        }
        fd = sys3(2, (long)"/disk/hello.txt", 0, 0);
        if (fd >= 3) {
            long r2 = sys3(0, fd, (long)buf, 500);
            sys3(3, fd, 0, 0);
            puts("fjfs: after append (len=");
            pnum(r2);
            puts("):\n");
            if (r2 > 0) {
                buf[r2] = 0;
                puts(buf);
            }
        }
    } else {
        puts("fjfs: boot#1 empty volume - writing first record...\n");
        fd = sys3(2, (long)"/disk/hello.txt", 1, 0);
        if (fd >= 3) {
            sys3(1, fd, (long)"FJFS persistent data #1\n", 24);
            sys3(3, fd, 0, 0);
        }
        /* 立即重读 */
        fd = sys3(2, (long)"/disk/hello.txt", 0, 0);
        long r2 = sys3(0, fd, (long)buf, 500);
        sys3(3, fd, 0, 0);
        puts("fjfs: readback after write (len=");
        pnum(r2);
        puts("):\n");
        if (r2 > 0) {
            buf[r2] = 0;
            puts(buf);
        }
    }

    puts("fjfs: test done - reboot with SAME drive to verify persistence\n");
    sys3(60, 0, 0, 0);
    for (;;) {}
}
