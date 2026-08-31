/* hello.c — FujoOS SDK 样本
 *
 * 一个零依赖、零 libc 的 freestanding 程序：
 *  - Linux 变体直接用 x86_64 syscall（write=1, exit=60）——
 *    这正是 "Linux ABI 第一公民" 路线演示：此类二进制装入 .run 后由
 *    内核 syscall gate 直接执行, 无需任何用户态垫片。
 *  - Windows/Mach-O 变体是结构性载体（装载器 M3/M6 上线后走垫片路径）。
 *
 * 编译:
 *   ELF:   clang --target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -no-pie \
 *                -fuse-ld=lld "-Wl,-e,_start" sdk/hello.c -o sdk/build/hello.elf
 *   PE:    clang --target=x86_64-pc-windows-msvc -nostdlib -fuse-ld=lld \
 *                "-Wl,/entry:_start" "-Wl,/subsystem:console" sdk/hello.c -o sdk/build/hello.exe
 *   MachO: clang --target=x86_64-apple-macos11 -nostdlib -fuse-ld=lld \
 *                sdk/hello.c -o sdk/build/hello.macho   (默认入口 _main)
 */
typedef long int64_t;

static int64_t syscall3(long n, long a, long b, long c) {
    register long rax asm("rax") = n;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall"
                 : "+r"(rax)
                 : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
}

#ifdef __APPLE__
/* macOS 变体: 默认入口 _main (ld64.lld 缺省), 免去 -e 入口符号坑 */
int main(void) {
    for (;;) {
        /* M6 装载器上线后此处走 darwin syscall (0x2000000|nr) */
    }
}
#else
void _start(void) {
#ifdef __linux__
    static const char msg[] =
        "hello, fujo! (ELF64 -> linux-x64 syscall -> fujopack -> .run)\n";
    syscall3(1, 1, (long)msg, (long)(sizeof(msg) - 1)); /* write(1, msg, len) */
    syscall3(60, 0, 0, 0);                              /* exit(0) */
#endif
    for (;;) {
        /* PE 变体在 M3 内走 ntdll/kernel32 垫片路径；样本仅验证打包链路 */
    }
}
#endif
