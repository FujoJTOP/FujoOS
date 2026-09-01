/* m99_upd.c — M99: 签名/更新机制 v0 (单簇 2048B; FJFS 多簇往返为
 * 已知限制 (M16 卷), filesz>2048 的读写校验由 m98 install 面另查)
 *
 * 1. upd_check(cur blob) → hash
 * 2. upd_apply(blob, 2048, hash) → 0 (校验通过, 写盘替换)
 * 3. upd_apply(blob 篡改 1 字节, len, hash) → -22 (拒绝)
 * 4. upd_status → kernel_hash 一致, pending=0, upd_count>=1 → PASS
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

static unsigned char blob[2048];
static u64 st[3];

void _start(void)
{
    static const char m1[] = "m99: signing/update mechanism v0\n";
    wr(m1, sizeof(m1) - 1);

    int i;
    volatile unsigned char *vp = blob;
    for (i = 0; i < 2048; i++) {
        vp[i] = (unsigned char)i;
    }
    (void)sys3(0x8801, (long)blob, 2048, 0);
    (void)sys3(0x8803, (long)st, 0, 0);
    u64 h = st[0];

    long ok = sys3(0x8802, (long)blob, 2048, h); /* 正常更新 */
    blob[100] ^= 0x01;                            /* 篡改 */
    long tampered = sys3(0x8802, (long)blob, 2048, h);

    (void)sys3(0x8803, (long)st, 0, 0);
    u64 h2 = st[0], pend = st[1], cnt = st[2];

    static const char h1[] = "m99: ok=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)ok);
    static const char h2c[] = " tamper=";
    wr(h2c, sizeof(h2c) - 1);
    wrhex((u32)tampered);
    static const char h3[] = " cnt=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)cnt);
    static const char h4[] = " h=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)h2);
    wr("\n", 1);
    int pass = ok == 0 && tampered == -22 && h2 == h && pend == 0 && cnt >= 1;
    if (pass) {
        static const char m2[] = "m99: M99 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m99: M99 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
