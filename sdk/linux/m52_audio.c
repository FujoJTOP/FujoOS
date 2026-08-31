/* m52_audio.c — M52: AC97 音频驱动接入验证
 *
 * 0x5F01 audio_info(ptr u32×2 present,vendor)
 * 0x5F02 audio_enable(on) / 0x5F03 audio_volume(v) / 0x5F04 playback
 * 流程: QEMU -device AC97 -> present=1 vendor=8086 -> enable -> volume
 * -> playback(64) -> 汇总 PASS。
 */
typedef long int64_t;
typedef unsigned int u32;

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
static void wrdec(u32 v)
{
    char b[12];
    int i = 12;
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 12 - i);
}
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

void _start(void)
{
    static const char m1[] = "m52: AC97 audio driver & playback entry\n";
    wr(m1, sizeof(m1) - 1);

    u32 info[2];
    (void)sys3(0x5F01, (long)info, 0, 0);
    wr("m52: present=", 13);
    wrdec(info[0]);
    static const char s1[] = " vendor=";
    wr(s1, 8);
    wrhex(info[1]);
    wr("\n", 1);

    long en = sys3(0x5F02, 1, 0, 0);
    wr("m52: enable rc=", 15);
    {
        char b[8];
        int i = 8;
        long v = en;
        if (v == 0) b[--i] = '0';
        while (v > 0) {
            b[--i] = '0' + (char)(v % 10);
            v /= 10;
        }
        wr(&b[i], 8 - i);
    }
    wr("\n", 1);

    (void)sys3(0x5F03, 0x6060, 0, 0);
    long pb = sys3(0x5F04, 0, 64, 0);
    wr("m52: playback queued=", 20);
    wrdec((u32)pb);
    wr("\n", 1);

    int ok = info[0] == 1 && info[1] == 0x8086 && en == 0 && pb == 64;
    if (ok) {
        static const char m2[] = "m52: M52 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m52: M52 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
