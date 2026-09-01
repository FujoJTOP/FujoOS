/* m86_wmap.c — M86: 权重 mmap 对象 (资源 → 内核按需页)
 *
 * 1. wmap_load(blob, 4096): 权重 blob (pattern) → 内核 WLIB (0xF30000)
 * 2. wmap_res(0xB90000, 4096): 登记权重 VA 区 (需求段)
 * 3. 读权重 VA (未映射 → #PF → 按需页从 WLIB 拷贝)
 * 4. 求和 == blob 求和; stats: pfa>=1, pages>=1 → PASS
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

static unsigned char blob[4096];
static u64 st[4];

void _start(void)
{
    static const char m1[] = "m86: weights as mmap objects (demand pages)\n";
    wr(m1, sizeof(m1) - 1);

    /* 权重 blob: pattern (i&0xFF) */
    int i;
    for (i = 0; i < 4096; i++) {
        blob[i] = (unsigned char)(i & 0xFF);
    }
    (void)sys3(0x7C01, (long)blob, 4096, 0);
    (void)sys3(0x7C02, 0xB90000, 4096, 0);

    /* 读权重 VA (未映射 → 按需页) */
    volatile unsigned char *w = (volatile unsigned char *)0xB90000;
    u64 sum = 0;
    for (i = 0; i < 4096; i++) {
        sum += w[i];
    }
    u64 sum0 = 0;
    for (i = 0; i < 4096; i++) {
        sum0 += blob[i];
    }

    (void)sys3(0x7C03, (long)st, 0, 0);
    u64 pfa = st[0], pages = st[1], wlen = st[2], maps = st[3];

    static const char h1[] = "m86: sum=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)(sum >> 16));
    static const char h2[] = " pfa=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)pfa);
    static const char h3[] = " pages=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)pages);
    static const char h4[] = " wlen=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)wlen);
    wr("\n", 1);

    int ok = sum == sum0 && pfa >= 1 && pages >= 1 && wlen == 4096 && maps == 1;
    if (ok) {
        static const char m2[] = "m86: M86 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m86: M86 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
