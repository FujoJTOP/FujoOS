/* m127_exec.c — W16a: 内存执行 (exec-from-mem) 机制冒烟 (docs/67)
 *
 * 内嵌 tiny ELF (tools/mk_tiny_elf.py 生成): 打印 "exec-child-ok" 后 exit。
 * 流程: T1 内嵌 ELF 魔法/长度检查 -> T2 0x8B02 exec -> 内核装载 + iretq
 * (不返回); child 输出为 PASS 证据 (fujoregress needle = "exec-child-ok")。
 */
typedef long int64_t;
typedef unsigned long u64;

static int64_t sy(int64_t nr, int64_t a, int64_t b, int64_t c, int64_t d, int64_t e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx),
                 "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sy(1, 1, (long)s, len, 0, 0); }
static void wrdec(u64 v)
{
    char b[22];
    int i = 22;
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}

#include "tiny_elf.h"

static void run(void)
{
    static const char h[] = "m127: exec-from-mem (W16a)\n";
    wr(h, sizeof(h) - 1);
    /* T1 魔法/长度 */
    if (tiny_elf[0] != 0x7f || tiny_elf[1] != 'E' || tiny_elf[2] != 'L' ||
        tiny_elf[3] != 'F' || tiny_elf_len < 100) {
        wrstr("m127: T1 bad embedded elf\n");
        sy(60, 1, 0, 0, 0, 0);
        for (;;) {}
    }
    wrstr("m127: T1 embedded elf ok len=");
    wrdec(tiny_elf_len);
    wrstr("\n");
    /* T2 exec (不返回) */
    wrstr("m127: T2 exec-mem -> child\n");
    sy(0x8B02, (long)tiny_elf, tiny_elf_len, 0, 0, 0);
    wrstr("m127: T2 FAIL (returned)\n");
    sy(60, 1, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
