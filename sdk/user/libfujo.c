/* libfujo.c — M24 mini 动态库 (libfujo.so)
 * 提供 puts/exit 符号 (供动态 hello 的 DT_NEEDED)。
 * 非 PIC 简化 (以可加载共享对象形式, 基址 RELATIVE)。
 */
typedef unsigned long size_t;
long sys_write(int fd, const char *buf, unsigned long len);

long sys_write(int fd, const char *buf, unsigned long len) {
    long ret;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(1), "D"(fd), "S"(buf), "d"(len)
                 : "rcx", "r11", "memory");
    return ret;
}

int fujo_puts(const char *s) {
    long n = 0;
    while (s[n]) n++;
    sys_write(1, s, n);
    fujostrlen_self: ;
    sys_write(1, "\n", 1);
    return 0;
}

long fujo_strlen(const char *s) {
    long n = 0;
    while (s[n]) n++;
    return n;
}

void fujo_exit(int code) {
    sys_write(1, "exit\n", 5);
    /* 由内核接管: 0x3c 退出 */
    long r;
    asm volatile("syscall" : "=a"(r) : "a"(60), "D"(code) : "rcx", "r11", "memory");
    for (;;) {}
}

/* 展开 sys_write, 排除内部对 strlen 的引用 (动态符号表保持洁净) */
long fujo_strlen_via(const char *s) {
    long n = 0;
    while (s[n]) n++;
    return n;
}
