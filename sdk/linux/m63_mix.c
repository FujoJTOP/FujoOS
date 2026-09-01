/* m63_mix.c — M63: 音频混音器/效果链 v0 (CPU 采样级)
 *
 * 通道 4 路 i16 单声道 (128 样本/路):
 *   ch0 = 64×10000, ch1 = 64×5000, ch2 = 32×4000 (ch3 空)
 * 混音: i<32: 19000; 32<=i<64: 15000 (增益 256 = 100%)
 * 效果链: 低通 k=192/256 → y0=7500, 收敛 10000; 增益 50% → 5000。
 */
typedef long int64_t;
typedef unsigned int u32;
typedef short i16;

static int64_t sys3(long nr, long a, long b, long c)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrhex(u32 v)
{
    static const char H[] = "0123456789abcdef";
    char b[9];
    int i;
    for (i = 0; i < 8; i++) {
        b[i] = H[(v >> (28 - i * 4)) & 0xF];
    }
    wr(b, 8);
}

static i16 buf[64];

void _start(void)
{
    static const char m1[] = "m63: audio mixer + effect-chain v0\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    for (i = 0; i < 64; i++) {
        buf[i] = 10000;
    }
    (void)sys3(0x5F05, 0, 0, 0);
    (void)sys3(0x5F06, 0, (long)buf, 64);
    for (i = 0; i < 64; i++) {
        buf[i] = 5000;
    }
    (void)sys3(0x5F05, 1, 0, 0);
    (void)sys3(0x5F06, 1, (long)buf, 64);
    for (i = 0; i < 32; i++) {
        buf[i] = 4000;
    }
    (void)sys3(0x5F05, 2, 0, 0);
    (void)sys3(0x5F06, 2, (long)buf, 32);

    /* 混音: gain=256 (100%) */
    (void)sys3(0x5F07, (long)buf, 64, 256);
    int v0 = buf[0] == 19000;    /* 10000+5000+4000 */
    int v40 = buf[40] == 15000;  /* 10000+5000 (ch2 结束) */
    static const char h1[] = "m63: mix0=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)buf[0]);
    static const char h2[] = " mix40=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)buf[40]);
    wr("\n", 1);

    /* 效果链: ch0 低通 k=192 (y0=7500, 收敛 10000); 清 ch1/ch2 */
    (void)sys3(0x5F05, 1, 0, 0);
    (void)sys3(0x5F05, 2, 0, 0);
    (void)sys3(0x5F05, 0, 0, 0);
    for (i = 0; i < 64; i++) {
        buf[i] = 10000;
    }
    (void)sys3(0x5F06, 0, (long)buf, 64);
    (void)sys3(0x5F08, 0, 1, 192);
    (void)sys3(0x5F07, (long)buf, 8, 256);
    int l0 = buf[0] == 7500;
    int l7 = buf[7] > 9900;
    static const char h3[] = "m63: lp0=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)buf[0]);
    static const char h4[] = " lp7=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)buf[7]);
    wr("\n", 1);

    /* 增益 50% (k 归 256 直通 + gain=128) */
    (void)sys3(0x5F08, 0, 1, 256);
    (void)sys3(0x5F08, 0, 2, 128);
    (void)sys3(0x5F07, (long)buf, 8, 256);
    int g0 = buf[0] == 5000;
    static const char h5[] = "m63: gain=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)buf[0]);
    wr("\n", 1);

    if (v0 && v40 && l0 && l7 && g0) {
        static const char m2[] = "m63: M63 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m63: M63 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
