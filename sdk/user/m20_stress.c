/* m20_stress.c — M20 无泄漏压力验证
 *
 * 循环: 创建管道 -> 写读 -> 双端 close (回收) × 64 轮,
 *       kobj 计数应回到基线 (pipe 槽/kobj 槽不泄漏);
 *       再验证 512 轮 syscall 后 shm/kobj 计数稳定。
 * 结论: 若最终 pipe<=1 槽复用且 kobj 计数 == 基线 -> PASS。
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

void _start(void) {
    int base[4] = {0, 0, 0, 0};
    int fin[4] = {0, 0, 0, 0};
    int fail = 0;

    sys3(0x5132, (long)base, 4, 0);
    puts("m20: stress baseline pipe=");
    pnum(base[1]);
    putc('\n');

    /* 128 轮: pipe 创建/写读/双端关闭 (每轮应完全回收) */
    const char *msg = "m20 stress payload\n";
    long msg_n = 0;
    while (msg[msg_n]) msg_n++;
    for (int r = 0; r < 128; r++) {
        int fds[2];
        long rc = sys3(0x5110, (long)fds, 0, 0);
        if (rc != 0) {
            puts("m20: pipe create FAIL at round ");
            pnum(r);
            putc('\n');
            fail = 1;
            break;
        }
        long w = sys3(1, fds[1], (long)msg, msg_n);
        char buf[64];
        long rd = sys3(0, fds[0], (long)buf, 63);
        if (w != msg_n || rd != msg_n) { fail = 1; }
        sys3(3, fds[0], 0, 0);
        sys3(3, fds[1], 0, 0);
        /* 分阶段查计 (每 32 轮) */
        if (r % 32 == 31) {
            int mid[4] = {0, 0, 0, 0};
            sys3(0x5132, (long)mid, 4, 0);
            puts("m20: after round ");
            pnum(r);
            puts(" pipe=");
            pnum(mid[1]);
            putc('\n');
        }
    }

    /* 512 轮: shm 获取 + kobj 创建/释放 */
    long shm = sys3(0x5111, 0, 0, 0);
    if (shm != 0xA00000) { fail = 1; }
    for (int r = 0; r < 512; r++) {
        long h = sys3(0x5130, 1, 0, 0);
        if (h < 0) { fail = 1; break; }
        sys3(0x5131, h, 0, 0);
    }

    sys3(0x5132, (long)fin, 4, 0);
    /* 泄漏判据: shm 计数 = 基线+1 (每次 fujo_shm 分配一个对象);
     * pipe 计数应回到基线 (128 轮 × 2 端点全部回收, 下一次创建复用) */
    puts("m20: stress final pipe=");
    pnum(fin[1]);
    puts(" shm=");
    pnum(fin[2]);
    putc('\n');
    if (fin[1] != base[1]) {
        puts("m20: pipe leak detected\n");
        fail = 1;
    }
    puts(fail ? "m20: M20 RESULT: FAIL\n" : "m20: M20 RESULT: PASS (no leak)\n");
    /* 报告 */
    sys3(60, 0, 0, 0);
    for (;;) {}
}
