/* m32_lib.c — M32: fujorun 多模块/库目录验证
 *
 * 零 libc ELF: open("/lib/catlib.bin") -> read -> 打印库内容 -> exit(7)。
 * 多模块镜像 (fujorun pack): 模块0=可执行体, 模块1=库字节。
 *
 * 编译 + 打包:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m32_lib.c -o sdk/linux/m32_lib.elf
 *   python tools/fujorun.py pack -i sdk/linux/m32_lib.elf \
 *         -i? (no: -i main) --lib sdk/linux/catlib.bin -o sdk/build/m32_multi.initrd
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

void _start(void)
{
    static const char m1[] = "m32: fujorun multi-module initrd - lib dir read\n";
    wr(m1, sizeof(m1) - 1);

    long fd = sys3(2, (long)"/lib/catlib.bin", 0, 0);
    if (fd < 3) {
        static const char e[] = "m32: open /lib/catlib.bin FAILED\n";
        wr(e, sizeof(e) - 1);
        sys3(60, 1, 0, 0);
        for (;;) {
        }
    }
    char buf[64];
    long n = sys3(0, fd, (long)buf, 63);
    buf[n > 0 ? n : 0] = 0;
    static const char t[] = "m32: lib content: ";
    wr(t, sizeof(t) - 1);
    wr(buf, n > 0 ? n : 0);
    wr("\n", 1);
    long cl = sys3(3, fd, 0, 0); /* close */
    (void)cl;
    static const char m2[] = "m32: M32 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
