/* m137_pci.c — W20 p7: PCI 枚举完整化 (0x8503; 多功能设备可见)
 *
 * 断言 (q35 参考机):
 *   T1 0x8503 pci_scan -> n >= 5 (Q35: 主机桥/VGA/网络/ISA/SATA/SMBus)
 *   T2 找到 SATA 控制器 (vid 0x8086 did 0x2922) 且 func==2 (31.2 多功能)
 *   T3 主机桥 (8086:29c0) func==0
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

static u64 devs[24];

static void run(void)
{
    static const char h[] = "m137: pci enum (W20 p7)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;

    wrstr("m137: T1 scan\n");
    long n = sy(0x8503, (long)devs, 0, 0, 0, 0);
    if (n >= 5) {
        wrstr("m137:   devices=");
        wrdec((u64)n);
        wrstr(" ok\n");
    } else {
        wrstr("m137:   scan FAIL n=");
        wrdec((u64)n);
        wrstr("\n");
        pass = 0;
    }

    wrstr("m137: T2 SATA 31.2\n");
    {
        int found = 0;
        for (int i = 0; i < n && i < 24; i++) {
            u64 e = devs[i];
            u64 vid = e & 0xFFFF;
            u64 did = (e >> 16) & 0xFFFF;
            u64 func = (e >> 48) & 0xFF;
            if (vid == 0x8086 && did == 0x2922 && func == 2) found = 1;
        }
        if (found) wrstr("m137:   sata ok\n"); else { wrstr("m137:   sata FAIL\n"); pass = 0; }
    }

    wrstr("m137: T3 host bridge 0\n");
    {
        int found = 0;
        for (int i = 0; i < n && i < 24; i++) {
            u64 e = devs[i];
            u64 vid = e & 0xFFFF;
            u64 did = (e >> 16) & 0xFFFF;
            u64 func = (e >> 48) & 0xFF;
            if (vid == 0x8086 && (did == 0x29c0 || did == 0x2918) && func == 0) found = 1;
        }
        if (found) wrstr("m137:   host ok\n"); else { wrstr("m137:   host FAIL\n"); pass = 0; }
    }

    if (pass) {
        static const char m2[] = "m137: M137 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m137: M137 RESULT: FAIL\n";
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
