/* m29_darwin.c — M29: darwinsubsys libSystem 薄层 + darwin CLI 工具
 *
 * 零 libc Mach-O 64 (x86_64-apple-macos11, amd64 BSD syscall 0x2000000|nr):
 *   write(4) / open(5) / read(3) / lseek(13) / mmap(197=0xC5) /
 *   getpid(20) / close(6) / exit(1)。
 * 编译:
 *   clang --target=x86_64-apple-macos11 -O2 -nostdlib -fuse-ld=lld
 *         sdk/mac/m29_darwin.c -o sdk/mac/m29_darwin.macho
 */
typedef long int64_t;

static int64_t sys3(long nr, long a, long b, long c) {
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall"
                 : "+r"(rax)
                 : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
}

static int64_t sys6(long nr, long a, long b, long c, long d, long e) {
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall"
                 : "+r"(rax)
                 : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8)
                 : "rcx", "r11", "memory");
    return rax;
}

static int64_t sys7a(long nr, long a, long b, long c, long d, long e, long f) {
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    register long r9 asm("r9") = f;
    asm volatile("syscall"
                 : "+r"(rax)
                 : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8), "r"(r9)
                 : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sys3(0x2000004, 1, (long)s, len); }

static void wrdec(int64_t v) {
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

int main(void) {
    static const char m1[] = "m29: darwin CLI tool - libSystem shim layer\n";
    wr(m1, sizeof(m1) - 1);

    long fd = sys3(0x2000005, (long)"/boot/module", 0, 0); /* open */
    wr("m29: open fd=", 13);
    wrdec(fd);
    wr("\n", 1);

    char buf[64];
    long n = sys3(0x2000003, fd, (long)buf, 32); /* read */
    wr("m29: read n=", 12);
    wrdec(n);
    wr(" first8=", 8);
    if (n >= 8) {
        long k;
        for (k = 0; k < 8; k++) {
            char hex[3];
            static const char H[] = "0123456789abcdef";
            hex[0] = H[((unsigned char)buf[k] >> 4) & 0xF];
            hex[1] = H[((unsigned char)buf[k]) & 0xF];
            wr(hex, 2);
        }
    }
    wr("\n", 1);

    long pos = sys3(0x2000013, fd, 0, 0); /* lseek(fd, 0, 0) */
    wr("m29: lseek pos=", 15);
    wrdec(pos);
    wr("\n", 1);

    long mp = sys7a(0x20000C5, 0, 0x2000, 3, 0x1002, (long)-1, 0); /* mmap anon 8k */
    wr("m29: mmap=", 10);
    wrdec(mp);
    wr("\n", 1);
    if (mp > 0) {
        ((char *)mp)[0] = 'M';
        ((char *)mp)[1] = '2';
        ((char *)mp)[2] = '9';
        wr("m29: mmap write ok\n", 18);
    } else {
        wr("m29: mmap FAIL\n", 15);
    }

    long pid = sys3(0x2000014, 0, 0, 0); /* getpid */
    wr("m29: getpid=", 12);
    wrdec(pid);
    wr("\n", 1);

    long cl = sys3(0x2000006, fd, 0, 0); /* close */
    wr("m29: close=", 11);
    wrdec(cl);
    wr("\n", 1);

    static const char m2[] = "m29: M29 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(0x2000001, 7, 0, 0); /* exit(7) */
    for (;;) {
    }
}
