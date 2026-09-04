/* m135_fs.c — W20 p5: FJFS 卷经 AHCI 背板 (真机 SATA 持久化; /disk/ 路径)
 *
 * 断言:
 *   T1 open("/disk/f1") (F_KIND_DISK -> fjfs read) 返回 fd
 *   T2 write "ahci-fs-data-42" -> close (写穿刷新)
 *   T3 reopen -> read 回读 == 写入内容 (AHCI 落盘物理往返)
 *   T4 remove? (无) —— 汇总 PASS
 */
typedef long int64_t;
typedef unsigned long u64;

static int64_t sy(int64_t nr, int64_t a, int64_t b, int64_t c, int64_t d, int64_t e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx),
                 "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sy(1, 1, (long)s, len, 0, 0); }
static void wrdec(u64 v)
{
    char b[22];
    int i = 22;
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}

static char rbuf[512];
static const char payload[] = "ahci-fs-data-42";

static void run(void)
{
    static const char h[] = "m135: fjfs-over-ahci (W20 p5)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;

    wrstr("m135: T1 open /disk/f1\n");
    long fd = sy(2, (long)"/disk/f1", 2, 0, 0, 0); /* O_RDWR */
    if (fd >= 3) {
        wrstr("m135:   fd=");
        wrdec((u64)fd);
        wrstr(" ok\n");
    } else {
        wrstr("m135:   open FAIL rc=");
        wrdec((u64)fd);
        wrstr("\n");
        pass = 0;
    }

    if (fd >= 3) {
        wrstr("m135: T2 write+close\n");
        long w = sy(1, fd, (long)payload, sizeof(payload) - 1, 0, 0);
        long c = sy(3, fd, 0, 0, 0, 0);
        if (w == (long)(sizeof(payload) - 1) && c == 0) {
            wrstr("m135:   write ok\n");
        } else {
            wrstr("m135:   write FAIL w=");
            wrdec((u64)w);
            wrstr(" c=");
            wrdec((u64)c);
            wrstr("\n");
            pass = 0;
        }
    }

    wrstr("m135: T3 reopen+read\n");
    if (fd >= 3) {
        long fd2 = sy(2, (long)"/disk/f1", 0, 0, 0, 0); /* O_RDONLY */
        if (fd2 >= 3) {
            long n = sy(0, fd2, (long)rbuf, sizeof(rbuf), 0, 0);
            sy(3, fd2, 0, 0, 0, 0);
            int ok = (n >= (long)(sizeof(payload) - 1));
            if (ok) {
                for (long i = 0; i < (long)(sizeof(payload) - 1); i++) {
                    if (rbuf[i] != payload[i]) { ok = 0; break; }
                }
            }
            if (ok) {
                wrstr("m135:   read ok\n");
            } else {
                wrstr("m135:   read FAIL n=");
                wrdec((u64)n);
                wrstr(" first=");
                wrdec((u64)(unsigned char)rbuf[0]);
                wrstr("\n");
                pass = 0;
            }
        } else {
            wrstr("m135:   reopen FAIL\n");
            pass = 0;
        }
    }

    if (pass) {
        static const char m2[] = "m135: M135 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m135: M135 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(60, 7, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
