/* m133_plat.c — W20: 平台检测 (QEMU vs 真机; 0x8D01 platform_info)
 *
 * 断言:
 *   T1 0x8D01 -> [is_qemu, vbe_id, icr_mode] 三字段
 *   T2 一致性: icr_mode == is_qemu (ICR 语义选择随平台)
 *   T3 QEMU 证据链: is_qemu=1 时 vbe_id==0xB0C5 (Bochs VBE 特征 ID);
 *      真机 (is_qemu=0) 时跳过 (真机 VBE/无 Bochs ID)
 */
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned int u32;

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

static u64 buf[3];

static void run(void)
{
    static const char h[] = "m133: platform info (W20)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;

    long ret = sy(0x8D01, (long)buf, 0, 0, 0, 0);
    u64 is_qemu = buf[0], vbe_id = buf[1], icr_mode = buf[2];

    wrstr("m133: T1 is_qemu=");
    wrdec(is_qemu);
    wrstr(" vbe_id=0x");
    {
        char hex[6];
        int i = 16;
        const char *d = "0123456789abcdef";
        u64 v = vbe_id;
        while (i > 0) { hex[--i] = d[v & 0xF]; v >>= 4; }
        wr(hex + 12, 4);
    }
    wrstr(" icr_mode=");
    wrdec(icr_mode);
    wrstr("\n");
    if (ret != 0)
        pass = 0;

    wrstr("m133: T2 icr session aligns platform\n");
    {
        int aligned = (is_qemu == 1 && icr_mode == 0) || (is_qemu == 0 && icr_mode == 1);
        if (!aligned)
            pass = 0;
    }

    wrstr("m133: T3 qemu vbe chain\n");
    if (is_qemu == 1 && vbe_id != 0xB0C5)
        pass = 0;

    if (pass) {
        static const char m2[] = "m133: M133 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m133: M133 RESULT: FAIL\n";
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
