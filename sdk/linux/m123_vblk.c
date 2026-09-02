/* m123_vblk.c — W13a: PCI 总线模型 + virtio-blk 驱动骨架 (docs/64)
 *
 * QEMU: -drive if=none,id=vblk,file=sdk/vblk.img,format=raw
 *       -device virtio-blk-pci,drive=vblk,disable-modern=on
 * 断言 (离线, 检测级; 数据路径 = open item, 见 docs/64 §未完成):
 *   T1 驱动探测: ready=1, io_base!=0 (PCI 配置空间 + 命令使能 + BAR0 I/O)
 *   T2 vring 就绪: qsz=16, vring_phys!=0 (QUEUE_SEL/QUEUE_PFN 写读回)
 *   T3 提交路径: read 返回优雅值 (不挂起/不崩; 数据回读待查)
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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}

static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}

static char buf1[512];

static void run(void)
{
    static const char h[] = "m123: PCI model + virtio-blk skeleton (W13a)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 info[4];

    /* T1 驱动探测 */
    info[0] = info[1] = info[2] = info[3] = 0;
    sy(0x8A02, (long)info, 0, 0, 0, 0);
    wrstr("m123: T1 ready=");
    wrdec(info[0]);
    wrstr(" io=0x");
    {
        static const char H[] = "0123456789abcdef";
        char b[20];
        int i;
        u64 v = info[1];
        b[0] = '0';
        b[1] = 'x';
        for (i = 0; i < 16; i++)
            b[2 + i] = H[(v >> (4 * (15 - i))) & 0xF];
        b[18] = 0;
        wr(b, 18);
    }
    wrstr(" (expect ready=1 io!=0)\n");
    if (!(info[0] == 1 && info[1] != 0))
        pass_all = 0;

    /* T2 vring */
    wrstr("m123: T2 qsz=");
    wrdec(info[2]);
    wrstr(" vring=0x");
    {
        static const char H[] = "0123456789abcdef";
        char b[20];
        int i;
        u64 v = info[3];
        b[0] = '0';
        b[1] = 'x';
        for (i = 0; i < 16; i++)
            b[2 + i] = H[(v >> (4 * (15 - i))) & 0xF];
        b[18] = 0;
        wr(b, 18);
    }
    wrstr(" (expect qsz=16 vring!=0)\n");
    if (!(info[2] == 16 && info[3] != 0))
        pass_all = 0;

    /* T3 提交路径优雅性 */
    long rc = sy(0x8A01, 0, (long)buf1, 512, 0, 0);
    wrstr("m123: T3 read rc=");
    wrdec((u64)rc);
    wrstr(" (graceful; data path = open item)\n");
    if (rc == 0) {
        /* 数据路径若通: 校验模式 (进阶) */
        int bad = 0;
        int i;
        for (i = 0; i < 512; i++) {
            if ((unsigned char)buf1[i] != (unsigned)(i % 256))
                bad = 1;
        }
        wrstr("m123: T3 pattern=");
        wrdec((u64)bad);
        wrstr(" (0=ok)\n");
        if (bad)
            pass_all = 0;
    }

    if (pass_all) {
        static const char m2[] = "m123: M123 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m123: M123 RESULT: FAIL\n";
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
