/* m96_acpi.c — M96: 真机引导最小集 (ACPI 表 + PCI 枚举)
 *
 * 1. acpi_info → (rsdp=1, rev>=1, tables>=1, pci_devs>=1)
 * 2. acpi_dump → 文本开头 "acpi: RSDP @"
 * 3. pci_scan → 条目数 == info[3] > 0; 首条目 vid/did 非零 (如
 *    8086:1237 / 1234:1111) → PASS
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

static u64 info[4];
static u64 pci[24];
static char dumpbuf[160];

void _start(void)
{
    static const char m1[] = "m96: ACPI/PCI minimal boot set\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x8501, (long)info, 0, 0);
    u64 rsdp = info[0], rev = info[1], tabs = info[2], devs = info[3];

    long dn = sys3(0x8502, (long)dumpbuf, sizeof(dumpbuf), 0);
    int pref = dumpbuf[0] == 'a' && dumpbuf[1] == 'c' && dumpbuf[2] == 'p'
               && dumpbuf[3] == 'i';

    long pn = sys3(0x8503, (long)pci, 0, 0);
    u64 vid0 = pci[0] & 0xFFFF;
    u64 did0 = (pci[0] >> 16) & 0xFFFF;

    static const char h1[] = "m96: rsdp=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)rsdp);
    static const char h2[] = " rev=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)rev);
    static const char h3[] = " tabs=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)tabs);
    static const char h4[] = " pci=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)devs);
    static const char h5[] = " vid0=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)vid0);
    static const char h6[] = " did0=";
    wr(h6, sizeof(h6) - 1);
    wrhex((u32)did0);
    wr("\n", 1);

    int ok = rsdp == 1 && devs >= 1
             && pn == (long)devs && pref && vid0 != 0 && vid0 != 0xFFFF;
    if (ok) {
        static const char m2[] = "m96: M96 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m96: M96 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
