/* m51_disp.c — M51: 显示驱动抽象 (VBE/virtio-gpu 探测) 验证
 *
 * 0x5E01 disp_info(ptr -> u32×5 backend,vendor,device,w,h)
 * 0x5E02 disp_set_backend(which)
 * 流程: QEMU 默认 std-vga -> backend=0 (0x1234:0x1111) + 分辨率
 * 读回; virtio-gpu 未添加时 backend=0; 一致 -> PASS。
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
    static const char m1[] = "m51: display driver abstraction probe\n";
    wr(m1, sizeof(m1) - 1);

    u32 di[5];
    (void)sys3(0x5E01, (long)di, 0, 0);
    wr("m51: backend=", 14);
    wrdec(di[0]);
    static const char s1[] = " vendor=";
    wr(s1, 8);
    wrhex(di[1]);
    static const char s2[] = " device=";
    wr(s2, 8);
    wrhex(di[2]);
    static const char s3[] = " mode=";
    wr(s3, 6);
    wrdec(di[3]);
    static const char ss[] = "x";
    wr(ss, 1);
    wrdec(di[4]);
    wr("\n", 1);

    /* 后端应为 qemu std-vga (bochs-vbe) — virtio-gpu 未接入时 absent */
    int ok = di[0] == 0 && di[1] == 0x1234 && di[3] == 1024 && di[4] == 768;
    if (ok) {
        static const char m2[] = "m51: M51 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m51: M51 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
