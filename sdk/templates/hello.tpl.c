/* hello.tpl.c — FujoOS SDK 模板: Hello (elf-linux ABI)
 *
 * 用法: 见 docs/29-sdk-close.md §3 ELF 构建; QEMU 运行经
 *   python tools/fujorun.py run -k kernel/fujo-kernel.bin -i hello.elf \
 *       --keys "os spc run spc hermes" --timeout 20
 * 说明: 无 libc; _start 为入口 (user.ld 脚本); syscall 内联 asm。
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

void _start(void)
{
    static const char msg[] = "hello: FujoOS template app\n";
    sys3(1, 1, (long)msg, sizeof(msg) - 1); /* write(1, msg, len) */
    sys3(60, 7, 0, 0);                      /* exit(7) */
    for (;;) {
    }
}
