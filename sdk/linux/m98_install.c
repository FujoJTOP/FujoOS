/* m98_install.c — M98: live 镜像 + 安装器 v0
 *
 * 前提: 参考盘 (-drive 格式化卷, fjfs 自动格式化)
 * 1. inst_install() → 内核 boot 模块 → /system/fujo-kernel.bin
 * 2. inst_status → installed=1, kernel_size>0, volume_ok=1,
 *    boot_count=1 (阶段1) / 2 (阶段2 同盘重启)
 * 3. 两阶段: 阶段2 boot_count 递增 = 安装持久
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
    static const char m1[] = "m98: live image + installer v0\n";
    wr(m1, sizeof(m1) - 1);
    wr("", 0);

    long rc = sys3(0x8701, 0, 0, 0);
    (void)sys3(0x8702, (long)st, 0, 0);
    u64 inst = st[0], ksz = st[1], vol = st[2], bc = st[3];

    static const char h1[] = "m98: rc=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)rc);
    static const char h2[] = " inst=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)inst);
    static const char h3[] = " ksz=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)(ksz >> 8));
    static const char h4[] = " vol=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)vol);
    static const char h5[] = " boot=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)bc);
    wr("\n", 1);

    int ok = rc == 0 && inst == 1 && ksz > 1000 && vol == 1 && bc >= 1;
    if (ok) {
        static const char m2[] = "m98: M98 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m98: M98 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
