/* user_darwin.c — M6 darwinsubsys v0 样例 (Mach-O 64, macOS ABI)
 *
 * 零 libc: 直接使用 Darwin BSD 系统调用 (amd64: 0x2000000 | nr):
 *   write = 0x2000004, exit = 0x2000001 (与 Linux 相同的寄存器映射)。
 * 内核发现 Mach-O -> SEAGMENT 映射 -> iretq ring3 -> 程序运行。
 *
 * 编译 (scripts/build-kernel.ps1 自动执行):
 *   clang --target=x86_64-apple-macos11 -O2 -nostdlib -fuse-ld=lld \
 *         -Wl,-segaddr,__TEXT,0x400000 -Wl,-segaddr,__DATA,0x410000 \
 *         sdk/mac/user_darwin.c -o sdk/mac/user_darwin.macho
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

int main(void) {
    static const char m[] =
        "user : Mach-O program live — darwin bsd syscall write\n";
    sys3(0x2000004, 1, (long)m, (long)(sizeof(m) - 1)); /* write(1, m, len) */
    sys3(0x2000001, 0, 0, 0);                           /* exit(0) */
    for (;;) {
    }
}
