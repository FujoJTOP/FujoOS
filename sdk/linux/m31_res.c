/* m31_res.c — M31: fujopack 资源化 .run 工具链验证
 *
 * 零 libc ELF: open("/runres/demo.txt") -> read -> 打印内容 -> exit(7)。
 * 由 fujopack.py 与资源一起打包为 .run (FUJR): 内核嗅探 FUJR ->
 * fujr::load (EMBED 提取 + DATA 资源拷入 /runres) -> 本程序读取。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m31_res.c -o sdk/linux/m31_res.elf
 * 打包:
 *   python tools/fujopack.py pack -e sdk/linux/m31_res.elf \
 *         -r demo.txt:sdk/linux/m31_demo.txt -o sdk/build/m31_res.run
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
    static const char m1[] = "m31: fujopack .run container - resource read\n";
    wr(m1, sizeof(m1) - 1);

    long fd = sys3(2, (long)"/runres/demo.txt", 0, 0);
    if (fd < 3) {
        static const char e[] = "m31: open /runres/demo.txt FAILED\n";
        wr(e, sizeof(e) - 1);
        sys3(60, 1, 0, 0);
    } else {
        char buf[128];
        long n = sys3(0, fd, (long)buf, 127);
        buf[n > 0 ? n : 0] = 0;
        static const char t[] = "m31: resource content: ";
        wr(t, sizeof(t) - 1);
        wr(buf, n > 0 ? n : 0);
        wr("\n", 1);
        long cl = sys3(3, fd, 0, 0);
        (void)cl;
        static const char m2[] = "m31: M31 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
