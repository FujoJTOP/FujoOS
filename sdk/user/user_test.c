/* user_test.c — M1 Ring3 用户态测试程序 (linux-x64 ABI)
 *
 * 诊断版: 第一条指令就是 syscall; rax 由内核在 iret 前预置为 60 (exit)。
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -nostdlib -static -fno-pie -no-pie \
 *         -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld sdk/user/user_test.c \
 *         -o sdk/user/user_test.elf
 *   python tools/flatten_elf.py sdk/user/user_test.elf kernel/src/user_test.bin
 */
void _start(void) {
    asm volatile("syscall" ::: "rcx", "r11", "memory");
    for (;;) {
    }
}
