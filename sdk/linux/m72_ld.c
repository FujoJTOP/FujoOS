/* m72_ld.c — M72: 系统内链接器 (ELF64 静态最小)
 *
 * text1 = [90 C3] (nop ret), text2 = [CC] (int3)
 * syms: foo @vma 0x100 → 绝对 0x400100
 * reloc: place=0x8003 (text1 数据后 pad) ← foo
 * 期望: magic 7f 45 4c 46; e_type=2; e_entry=0x400000;
 *   off1(0x8000)=0x90; off2(0x8010)=0xCC; reloc@0x8003=0x400100;
 *   total=0x8011
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

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

static unsigned char text1[2] = { 0x90, 0xC3 };
static unsigned char text2[1] = { 0xCC };
static unsigned char symbuf[40];
static u64 relocbuf[2];
static unsigned char elf[0x9000];
static u64 cfg[9];

void _start(void)
{
    static const char m1[] = "m72: in-kernel linker v0 (elf64 static)\n";
    wr(m1, sizeof(m1) - 1);

    /* 符号: "foo" vma=0x100 */
    int i;
    for (i = 0; i < 32; i++) {
        symbuf[i] = 0;
    }
    symbuf[0] = 'f';
    symbuf[1] = 'o';
    symbuf[2] = 'o';
    ((u64 *)(symbuf + 32))[0] = 0x100;

    /* 重定位: place=0x8003, symidx=0 */
    relocbuf[0] = 0x8003;
    relocbuf[1] = 0;

    /* cfg */
    cfg[0] = (u64)elf;
    cfg[1] = (u64)text1;
    cfg[2] = 2;
    cfg[3] = (u64)text2;
    cfg[4] = 1;
    cfg[5] = (u64)symbuf;
    cfg[6] = 1;
    cfg[7] = (u64)relocbuf;
    cfg[8] = 1;

    long total = sys3(0x7101, (long)cfg, 0, 0);

    int ok = total == 0x8011;
    if (ok) {
        ok = ok && elf[0] == 0x7F && elf[1] == 'E' && elf[2] == 'L' && elf[3] == 'F';
        ok = ok && elf[16] == 2 && elf[17] == 0;      /* ET_EXEC */
        ok = ok && elf[24] == 0x00 && elf[26] == 0x40; /* e_entry 0x400000 */
        ok = ok && elf[0x8000] == 0x90 && elf[0x8001] == 0xC3;
        ok = ok && elf[0x8010] == 0xCC;
        ok = ok && *(u64 *)(elf + 0x8003) == 0x400100;
        ok = ok && elf[0x44] == 7; /* p_flags RWX */
    }
    static const char h1[] = "m72: total=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)total);
    static const char h2[] = " reloc=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)(*(u64 *)(elf + 0x8003) >> 16));
    wr("\n", 1);

    if (ok) {
        static const char m2[] = "m72: M72 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m72: M72 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
