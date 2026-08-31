/* m25_hello.c — M25 musl/libc hello (静态链接 musl libc.a)
 * 无系统头文件 (避开头路径冲突); 用 musl 的 libc 运行时符号:
 *   printf / strlen / exit 都来自 musl libc.a 的 C 实现 (非裸 syscall)。
 * 入口: 内核 argv 模式构造进程栈 [argc=1, argv0], _start 来自 musl crt1。
 */
typedef unsigned long size_t;

extern int printf(const char *fmt, ...);
extern int puts(const char *s);
extern long strlen(const char *s);
extern void exit(int code);

int main(void);

/* musl crt1 由链接器自动选择; 这里保证 main 可见 */
int main(void) {
    puts("hello from musl on FujoOS (M25)");
    printf("libc: %s (len=%lu)\n", "musl 1.2.5", (unsigned long)strlen("musl 1.2.5"));
    exit(42);
}
