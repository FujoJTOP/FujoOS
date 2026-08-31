/* m24_dyn.c — M24 动态 ELF hello (PT_INTERP 缺失但带 -q 重定位记录)
 * 演示内核"动态链接最小化": ELF 带 DT_RELA (R_X86_64_RELATIVE), 内核
 * 装载后应用相对重定位; 符号调用由链接器合并 (无未定义符号, 无 ld.so)。
 * 编译: clang -fPIC -fuse-ld=lld -nostdlib -Wl,-q -Wl,-e,_start (静态合并,
 *       保留重定位记录 + PT_DYNAMIC, 无 PT_INTERP —— M24 v0 重定位验证)。
 */
typedef unsigned long size_t;

static long sys_write(long fd, const char *s, unsigned long n) {
    long ret;
    asm volatile("syscall" : "=a"(ret) : "a"(1), "D"(fd), "S"(s), "d"(n) : "rcx", "r11", "memory");
    return ret;
}

static void my_puts(const char *s) {
    long n = 0;
    while (s[n]) n++;
    sys_write(1, s, n);
    sys_write(1, "\n", 1);
}

void _start(void) {
    my_puts("hello from dynamic ELF (M24)");
    long r;
    asm volatile("syscall" : "=a"(r) : "a"(60), "D"(7) : "rcx", "r11", "memory");
    for (;;) {}
}
