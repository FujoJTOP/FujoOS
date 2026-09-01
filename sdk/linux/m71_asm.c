/* m71_asm.c — M71: 系统内汇编器 (最小 .s 子集)
 *
 * 源码:
 *   .text
 *   nop
 *   mov rax, 0x42
 *   xor rcx, rcx
 *   add rcx, 3
 *   je L0
 * L0:
 *   inc rcx
 *   ret
 * 期望: 1 + 10 + 3 + 4 + 6 + 3 + 1 = 28 字节; 首 0x90; rax imm=0x42;
 * je rel32 = 0 (L0 紧跟); 末 0xC3; verify 指令数 = 8。
 */
typedef long int64_t;
typedef unsigned int u32;

static int64_t sys4(long nr, long a, long b, long c, long d)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10)
                 : "rcx", "r11", "memory");
    return rax;
}
static int64_t sys3(long nr, long a, long b, long c)
{
    return sys4(nr, a, b, c, 0);
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

static const char src[] =
    ".text\n"
    "nop\n"
    "mov rax, 0x42\n"
    "xor rcx, rcx\n"
    "add rcx, 3\n"
    "je L0\n"
    "L0:\n"
    "inc rcx\n"
    "ret\n";

static unsigned char out[256];

void _start(void)
{
    static const char m1[] = "m71: in-kernel assembler v0\n";
    wr(m1, sizeof(m1) - 1);

    long n = sys4(0x7001, (long)src, sizeof(src) - 1, (long)out, 256);
    long ninst = sys3(0x7002, (long)out, n, 0);

    static const char h1[] = "m71: n=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)n);
    wr("", 0);
    static const char h2[] = " inst=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)ninst);
    wr("", 0);
    wr("\n", 1);

    /* 校验字段 */
    int ok = n == 28;
    if (ok) {
        ok = ok && out[0] == 0x90;
        ok = ok && out[3] == 0x42 && out[10] == 0x00;
        ok = ok && out[18] == 0x0F && out[19] == 0x84;
        ok = ok && out[20] == 0x00 && out[21] == 0x00 && out[22] == 0x00
             && out[23] == 0x00; /* je rel32=0: L0 紧跟 */
        ok = ok && out[24] == 0x48 && out[25] == 0xFF && out[26] == 0xC1; /* inc rcx */
        ok = ok && out[27] == 0xC3;
        ok = ok && ninst == 7;
    }
    static const char h3[] = "m71: b0=";
    wr(h3, sizeof(h3) - 1);
    wrhex(out[0]);
    static const char h3b[] = " b2=";
    wr(h3b, sizeof(h3b) - 1);
    wrhex(out[2]);
    static const char h3c[] = " b9=";
    wr(h3c, sizeof(h3c) - 1);
    wrhex(out[9]);
    static const char h4[] = " b18=";
    wr(h4, sizeof(h4) - 1);
    wrhex(out[18]);
    static const char h4b[] = " b19=";
    wr(h4b, sizeof(h4b) - 1);
    wrhex(out[19]);
    static const char h4c[] = " b20=";
    wr(h4c, sizeof(h4c) - 1);
    wrhex(out[20]);
    static const char h5[] = " b24=";
    wr(h5, sizeof(h5) - 1);
    wrhex(out[24]);
    static const char h5b[] = " b25=";
    wr(h5b, sizeof(h5b) - 1);
    wrhex(out[25]);
    static const char h6[] = " b27=";
    wr(h6, sizeof(h6) - 1);
    wrhex(out[27]);
    wr("\n", 1);

    if (ok) {
        static const char m2[] = "m71: M71 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m71: M71 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
