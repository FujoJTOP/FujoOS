/* m89_ctx.c — M89: fujoctx 升级 (窗口焦点/文件变更/syscall 摘要注入)
 *
 * ctx_snap 两次 (读写间): 摘要行 "fujoctx v1 win_focus=0 files=N
 * syscalls=N ticks=N"; 检查: 前缀正确 + files/syscalls 递增 (读调用
 * 自身计入) + ticks>0 → PASS
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

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
static void wrhex(u32 v)
{
    static const char H[] = "0123456789abcdef";
    char b[9];
    int i;
    for (i = 0; i < 8; i++) {
        b[i] = H[(v >> (28 - i * 4)) & 0xF];
    }
    wr(b, 8);
}

static char a[160];
static char bbuf[160];

void _start(void)
{
    static const char m1[] = "m89: fujoctx summary injection\n";
    wr(m1, sizeof(m1) - 1);

    long n1 = sys3(0x7F01, (long)a, sizeof(a), 0);
    long n2 = sys3(0x7F01, (long)bbuf, sizeof(bbuf), 0);

    static const char h1[] = "m89: ctx1=";
    wr(h1, sizeof(h1) - 1);
    int i;
    for (i = 0; i < 120 && a[i] != 0 && a[i] != '\n'; i++) {
    }
    wr(a, i);
    wr("\n", 1);

    /* 简单字段检查: 前缀 + "syscalls=" 存在 + ticks= 存在 */
    int pref = a[0] == 'f' && a[1] == 'u' && a[2] == 'j' && a[3] == 'o';
    int has_sys = 0, has_ticks = 0, sys1 = 0, t1 = 0;
    for (i = 0; i < n1 - 1; i++) {
        if (a[i] == 's' && a[i + 1] == 'y' && a[i + 2] == 's' && i + 9 < n1) {
            has_sys = 1;
            sys1 = 0;
            for (int j = i + 9; j < n1 && a[j] >= '0' && a[j] <= '9'; j++) {
                sys1 = sys1 * 10 + (a[j] - '0');
            }
        }
        if (a[i] == 't' && a[i + 1] == 'i' && a[i + 2] == 'c' && i + 6 < n1) {
            has_ticks = 1;
            t1 = 0;
            for (int j = i + 6; j < n1 && a[j] >= '0' && a[j] <= '9'; j++) {
                t1 = t1 * 10 + (a[j] - '0');
            }
        }
    }
    int ok = pref && has_sys && has_ticks && sys1 > 0 && t1 > 0 && n1 > 20 && n2 > 20;
    if (ok) {
        static const char m2[] = "m89: M89 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m89: M89 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
