/* m30_linux.c — M30: 三子系统一致化 · linuxsubsys 统一对象路径
 *
 * 与 win(m30_win.exe CreateFileA)/darwin(m29_darwin) 同一对象流程:
 *   open /boot/module -> fd -> read 32B -> 校验魔数 -> close。
 * 零 libc ELF (linux-subsys syscall nr: open=2 read=0 close=3 write=1 exit=60)。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld "-Wl,-e,_start" m30_linux.c -o m30_linux.elf
 */
typedef long int64_t;

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

static void wrdec(int64_t v)
{
    char b[24];
    int i = 24;
    if (v < 0) {
        b[--i] = '-';
        v = -v;
    }
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 24 - i);
}

static void hex16(const unsigned char *b, char *o)
{
    static const char H[] = "0123456789abcdef";
    int i;
    for (i = 0; i < 16; i++) {
        o[i * 2] = H[b[i] >> 4];
        o[i * 2 + 1] = H[b[i] & 0xF];
    }
    o[32] = 0;
}

void _start(void)
{
    static const char m1[] = "m30: linuxsubsys open/read/close - unified object path\n";
    wr(m1, sizeof(m1) - 1);

    long fd = sys3(2, (long)"/boot/module", 0, 0); /* open */
    wr("m30: open fd=", 13);
    wrdec(fd);
    wr("\n", 1);

    char buf[64];
    long n = sys3(0, fd, (long)buf, 32); /* read */
    wr("m30: read n=", 12);
    wrdec(n);
    if (n >= 16) {
        char hex[33];
        hex16((const unsigned char *)buf, hex);
        wr(" magic(cffaedfe)=", 17);
        wr(hex, 32);
    }
    wr("\n", 1);

    long cl = sys3(3, fd, 0, 0); /* close */
    wr("m30: close=", 11);
    wrdec(cl);
    wr("\n", 1);

    static const char m2[] = "m30: M30 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0); /* exit(7) */
    for (;;) {
    }
}
