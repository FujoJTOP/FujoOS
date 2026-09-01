/* m74_cc.c — M74: fujocc 编译壳 (C 子集 → asm → 汇编 → 链接)
 *
 * src: "int main() { return 0x41; }" (abi=linux)
 * 期望: mov rax, 0x41 / ret → 字节 [48 B8 41 00.. C3] @0x8000,
 * ELF: magic, e_type=2, e_entry=0x400000, total=0x800B
 */
typedef long int64_t;
typedef unsigned int u32;

static int64_t sys5(long nr, long a, long b, long c, long d, long e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8)
                 : "rcx", "r11", "memory");
    return rax;
}
static int64_t sys3(long nr, long a, long b, long c)
{
    return sys5(nr, a, b, c, 0, 0);
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

static const char src[] = "int main() { return 0x41; }";
static unsigned char elf[0x9000];

void _start(void)
{
    static const char m1[] = "m74: fujocc compile shell v0\n";
    wr(m1, sizeof(m1) - 1);

    long v = sys3(0x7502, 0, 0, 0);
    long total = sys5(0x7501, (long)src, sizeof(src) - 1, (long)elf, 0x9000, 1);

    int ok = v == 1 && total == 0x8010;
    if (ok) {
        ok = ok && elf[0] == 0x7F && elf[1] == 'E' && elf[2] == 'L' && elf[3] == 'F';
        ok = ok && elf[16] == 2 && elf[26] == 0x40; /* ET_EXEC; 0x400000 */
        ok = ok && elf[0x8000] == 0x48 && elf[0x8001] == 0xB8 && elf[0x8002] == 0x41;
        ok = ok && elf[0x800A] == 0xC3; /* ret */
    }
    static const char h1[] = "m74: total=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)total);
    static const char h2[] = " b0=";
    wr(h2, sizeof(h2) - 1);
    wrhex(elf[0x8000]);
    static const char h3[] = " b2=";
    wr(h3, sizeof(h3) - 1);
    wrhex(elf[0x8002]);
    static const char h4[] = " b10=";
    wr(h4, sizeof(h4) - 1);
    wrhex(elf[0x800A]);
    wr("\n", 1);

    if (ok) {
        static const char m2[] = "m74: M74 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m74: M74 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
