/* user_test.c — M1 Ring3 用户态测试程序 (linux-x64 ABI)
 *
 * 分离实验: iretq 到 CPL0 运行同一程序 (改 fujo_enter_user 的 CS/SS 即可)。
 * 本版程序: 写 syscall + exit (写时用), 用于验证执行路径本身。
 */
typedef long int64_t;

static int64_t sc3(long nr, long a, long b, long c) {
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx) : "rcx", "r11", "memory");
    return rax;
}

void _start(void) {
    static const char m[] = "user : ring3 program live — syscall write ok\n";
    sc3(1, 1, (long)m, (long)(sizeof(m) - 1));
    sc3(60, 0, 0, 0);
    for (;;) {
    }
}
