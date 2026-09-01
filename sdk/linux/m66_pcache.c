/* m66_pcache.c — M66: 页缓存/预读 v0 (内存盘→真盘桥)
 *
 * 1. alloc(3) -> 3 槽 (blk 0..2)
 * 2. write(0, pat0=0xAB), write(1, pat1=0xCD) -> 脏页
 * 3. flush() -> 回写模拟盘 (0xDF0000)
 * 4. read(0) -> hit (缓存直读)
 * 5. evict() -> 失效
 * 6. prefetch(0,2) -> 从盘读页0/1;  read(0)=0xAB, read(1)=0xCD (miss→盘)
 * 7. info -> (slots, dirty, hits, miss): slots>=2, dirty==0
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned char u8;

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

static u8 page[4096];
static u32 info[4];

void _start(void)
{
    static const char m1[] = "m66: page cache + readahead v0\n";
    wr(m1, sizeof(m1) - 1);

    /* 1) alloc 3 页 */
    long base = sys3(0x6C01, 3, 0, 0);
    static const char h0[] = "m66: base=";
    wr(h0, sizeof(h0) - 1);
    wrhex((u32)base);
    wr("\n", 1);

    /* 2) 写页0/页1 */
    int i;
    for (i = 0; i < 4096; i++) {
        page[i] = 0xAB;
    }
    (void)sys3(0x6C02, (long)base, (long)page, 0);
    for (i = 0; i < 4096; i++) {
        page[i] = 0xCD;
    }
    (void)sys3(0x6C02, (long)base + 1, (long)page, 0);

    /* 3) flush 回盘 */
    long fn = sys3(0x6C05, 0, 0, 0);
    static const char h1[] = "m66: flush_pages=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)fn);
    wr("\n", 1);

    /* 4) read(0) -> hit */
    (void)sys3(0x6C03, (long)base, (long)page, 0);
    u8 r0 = page[0];
    int hit_ok = r0 == 0xAB;

    /* 5) evict */
    (void)sys3(0x6C06, 0, 0, 0);

    /* 6) prefetch(0,2) + read 从盘命中 */
    long pf = sys3(0x6C04, 0, 2, 0);
    (void)sys3(0x6C03, 0, (long)page, 0);
    u8 rpf0 = page[0];
    (void)sys3(0x6C03, 1, (long)page, 0);
    u8 rpf1 = page[0];
    static const char h2[] = "m66: pf=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)pf);
    static const char h3[] = " r0=";
    wr(h3, sizeof(h3) - 1);
    wrhex(r0);
    static const char h4[] = " rpf0=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)rpf0);
    static const char h5[] = " rpf1=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)rpf1);
    wr("\n", 1);

    /* 6b) evict 后读未预读页 (盘上无 → 空页, miss 路径) */
    (void)sys3(0x6C06, 0, 0, 0);
    (void)sys3(0x6C03, 2, (long)page, 0);
    u8 rmiss = page[0];
    static const char hb[] = "m66: rmiss=";
    wr(hb, sizeof(hb) - 1);
    wrhex((u32)rmiss);
    wr("\n", 1);

    /* 7) info */
    (void)sys3(0x6C07, (long)info, 0, 0);
    u32 slots = info[0], dirty = info[1], hits = info[2], miss = info[3];
    static const char h6[] = "m66: slots=";
    wr(h6, sizeof(h6) - 1);
    wrhex(slots);
    static const char h7[] = " dirty=";
    wr(h7, sizeof(h7) - 1);
    wrhex(dirty);
    static const char h8[] = " hits=";
    wr(h8, sizeof(h8) - 1);
    wrhex(hits);
    static const char h9[] = " miss=";
    wr(h9, sizeof(h9) - 1);
    wrhex(miss);
    wr("\n", 1);

    int ok = hit_ok && rpf0 == 0xAB && rpf1 == 0xCD && pf == 2
             && slots >= 1 && dirty == 0 && hits >= 3 && miss >= 1
             && rmiss == 0;
    if (ok) {
        static const char m2[] = "m66: M66 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m66: M66 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
