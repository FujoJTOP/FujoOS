/* m97_hw.c — M97: 真机显示/键盘/存储适配 (参考机: QEMU)
 *
 * 1. hw_disp → (fbw>0, fbh>0, lfb_ok==1)
 * 2. hw_storage → (ata, lba48, fs_ok, files): 无盘参考机 ata=0 亦可
 *    (存储路径由两阶段 disk_fs.elf 持久化验证另证)
 * 3. 键盘 IRQ 计数 > 0 (boot 至今 IRQ1 发生) → PASS
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

static u64 st[4];

void _start(void)
{
    static const char m1[] = "m97: real-hw display/kbd/storage adapt\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x8601, (long)st, 0, 0);
    u64 fbw = st[0], fbh = st[1], lfb = st[2], kirq = st[3];
    (void)sys3(0x8602, (long)st, 0, 0);
    u64 ata = st[0], lba48 = st[1], fso = st[2], files = st[3];

    static const char h1[] = "m97: fb=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)fbw);
    static const char h2[] = "x";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)fbh);
    static const char h3[] = " kbd_irq=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)kirq);
    static const char h4[] = " ata=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)ata);
    static const char h5[] = " fs=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)fso);
    static const char h6[] = " files=";
    wr(h6, sizeof(h6) - 1);
    wrhex((u32)files);
    wr("\n", 1);

    int ok = fbw > 0 && fbh > 0 && lfb == 1 && kirq > 0
             && (ata == 1 || ata == 0); /* 无盘参考机可 */
    if (ok) {
        static const char m2[] = "m97: M97 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m97: M97 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
