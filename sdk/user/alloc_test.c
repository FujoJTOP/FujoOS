/* alloc_test.c — M11 虚拟内存/堆验证 (ring3, linux ABI)
 *
 * 流程: brk 扩展 1MiB -> 写入模式 -> 回读校验 ->
 *       mmap(匿名私有) 2MiB -> 写入/回读 -> 校验和/错误计数打印 -> exit
 *
 * 编译 (scripts/build-kernel.ps1 自动执行):
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -fuse-ld=lld -fno-builtin -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/user/alloc_test.c -o sdk/user/alloc_test.elf
 */
typedef long long int64_t;

/* syscall 包装 (M11: mmap 需 6 参数 -> r10/r8/r9; 内核入口已全寄存器保留) */
static int64_t sys3(long nr, long a, long b, long c) {
    int64_t ret;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c)
                 : "rcx", "r11", "memory");
    return ret;
}
static int64_t sys6(long nr, long a, long b, long c, long d, long e, long f) {
    int64_t ret;
    register long r10 asm("r10") = d; /* syscall ABI: 第4参数 = r10 */
    register long r8 asm("r8") = e;
    register long r9 asm("r9") = f;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9)
                 : "rcx", "r11", "memory");
    return ret;
}

static void puts(const char *s) {
    long n = 0;
    while (s[n] != 0) n++;
    sys3(1, 1, (long)s, n);
}

static void putc(char c) {
    char s[2];
    s[0] = c;
    s[1] = 0;
    puts(s);
}

static void pnum(long v) {
    char d[24];
    long i = 24, x = v;
    if (v == 0) {
        putc('0');
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

static void phex(int64_t v) {
    const char *hex = "0123456789abcdef";
    char d[18];
    d[0] = '0';
    d[1] = 'x';
    for (int i = 0; i < 16; i++) d[2 + i] = hex[(v >> (4 * (15 - i))) & 0xF];
    sys3(1, 1, (long)d, 18);
}

/* 模式写入: p[i] = (i*13 + 7) & 0xFF */
static void fill_pattern(char *p, long n) {
    for (long i = 0; i < n; i++) {
        p[i] = (char)((i * 13 + 7) & 0xFF);
    }
}

/* 模式回读校验: 返回错误数 */
static long check_pattern(const char *p, long n) {
    long errs = 0;
    for (long i = 0; i < n; i++) {
        char want = (char)((i * 13 + 7) & 0xFF);
        if (p[i] != want) errs++;
    }
    return errs;
}

void _start(void) {
    puts("alloc: M11 virtual memory test\n");

    /* ---- brk ---- */
    int64_t b0 = sys3(12, 0, 0, 0);
    int64_t b1 = sys3(12, b0 + (1 << 20), 0, 0);
    puts("alloc: brk ");
    phex(b0);
    puts(" -> ");
    phex(b1);
    putc('\n');
    if (b1 != b0 + (1 << 20)) {
        puts("alloc: brk FAIL (b1 != b0+1MiB)\n");
        sys3(60, 0, 0, 0);
        for (;;) {}
    }
    fill_pattern((char *)b0, 1 << 20);
    long errs = check_pattern((const char *)b0, 1 << 20);
    puts("alloc: brk 1MiB pattern errors=");
    pnum(errs);
    putc('\n');

    /* ---- mmap ---- */
    int64_t m = sys6(9, 0, (long)(2 << 20), 3 /*PROT_RW*/, 0x22 /*PRIVATE|ANON*/, -1, 0);
    puts("alloc: mmap -> ");
    phex(m);
    putc('\n');
    if (m <= 0) {
        puts("alloc: mmap FAILED\n");
        sys3(60, 0, 0, 0);
        for (;;) {}
    }
    fill_pattern((char *)m, 2 << 20);
    long errs2 = check_pattern((const char *)m, 2 << 20);
    puts("alloc: mmap 2MiB pattern errors=");
    pnum(errs2);
    putc('\n');

    if (errs == 0 && errs2 == 0) {
        puts("alloc: M11 PASS (brk+mmap zero-error readback)\n");
    } else {
        puts("alloc: M11 FAIL\n");
    }

    sys3(60, 0, 0, 0);
    for (;;) {}
}
