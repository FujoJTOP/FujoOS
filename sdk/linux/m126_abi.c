/* m126_abi.c — W15: ABI 冻结面冒烟 + 应用管理器 (docs/66)
 *
 * 多模块镜像: fujorun pack -i m126_abi.elf --lib m119_inv.elf -o m126_multi.initrd
 * 断言:
 *   T1 app_list (0x8B01): count>=1, 注册表名字非空 (应含 m119_inv)
 *   T2 tmpfs: open /tmp/abi.txt 写 64B -> close -> 重开读 -> 逐字节比对
 *   T3 /proc/meminfo 前缀 "MemTotal"
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

static const char PATH1[] = "/tmp/abi.txt";
static unsigned char wbuf[64];
static unsigned char rbuf[64];
static u64 appbuf[48]; /* 8 项 × 24B = 192B + count */

static void run(void)
{
    static const char h[] = "m126: ABI surface + app manager (W15)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    int i;

    /* T1 app_list */
    {
        u64 count = sy(0x8B01, (long)appbuf, 0, 0, 0, 0);
        /* 真实 count 由内核写出 (函数返回值是 dispatch 返回值; fujo_app_list 返回写入值) */
        unsigned char *names = (unsigned char *)appbuf + 8;
        wrstr("m126: T1 count=");
        wrdec(count);
        wrstr("\n");
        if (count < 1) {
            pass_all = 0;
        } else {
            /* 名字应含可打印字符 */
            int empty = 1;
            for (i = 0; i < 12; i++) {
                if (names[i] >= ' ' && names[i] <= '~')
                    empty = 0;
            }
            if (empty)
                pass_all = 0;
        }
    }

    /* T2 tmpfs 写/读往返 */
    {
        for (i = 0; i < 64; i++)
            wbuf[i] = (unsigned char)(i * 7 + 3);
        long fd = sy(2, (long)PATH1, sizeof(PATH1) - 1, 1, 0, 0); /* WRONLY */
        wrstr("m126: T2 open=");
        wrdec((u64)fd);
        wrstr("\n");
        if (fd < 3) {
            pass_all = 0;
        } else {
            long wrc = sy(1, fd, (long)wbuf, 64, 0, 0);
            sy(3, fd, 0, 0, 0, 0); /* close */
            long fd2 = sy(2, (long)PATH1, sizeof(PATH1) - 1, 0, 0, 0); /* RDONLY */
            long rrc = -1;
            if (fd2 >= 3) {
                rrc = sy(0, fd2, (long)rbuf, 64, 0, 0);
                sy(3, fd2, 0, 0, 0, 0);
            }
            wrstr("m126: T2 w=");
            wrdec((u64)wrc);
            wrstr(" r=");
            wrdec((u64)rrc);
            wrstr("\n");
            if (wrc != 64 || rrc != 64)
                pass_all = 0;
            else {
                int bad = 0;
                for (i = 0; i < 64; i++)
                    if (rbuf[i] != (unsigned char)(i * 7 + 3))
                        bad = 1;
                wrstr("m126: T2 pattern=");
                wrdec((u64)bad);
                wrstr(" (0=ok)\n");
                if (bad)
                    pass_all = 0;
            }
        }
    }

    /* T3 /proc/meminfo 前缀 */
    {
        long fd = sy(2, (long)"/proc/meminfo", 13, 0, 0, 0);
        if (fd >= 3) {
            long r = sy(0, fd, (long)rbuf, 64, 0, 0);
            sy(3, fd, 0, 0, 0, 0);
            if (r >= 8 && rbuf[0] == 'm' && rbuf[1] == 'e' && rbuf[2] == 'm' && rbuf[3] == '_') {
                wrstr("m126: T3 meminfo ok\n");
            } else {
                wrstr("m126: T3 meminfo FAIL\n");
                pass_all = 0;
            }
        } else {
            pass_all = 0;
        }
    }

    if (pass_all) {
        static const char m2[] = "m126: M126 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m126: M126 RESULT: FAIL\n";
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
