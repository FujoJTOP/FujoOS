/* m122_dev.c — W12: VFS 抽象 + tmpfs + /dev/model0 模型设备 (docs/63)
 *
 * AI 接口 UNIX 化: open("/dev/model0") -> write(请求) -> read(响应文本):
 *   T1 模型设备往返: write "run the game" -> read "intent=1" (与 0x5101 同核;
 *      R5 规则字节码优先 -> 离线零模型调用)
 *   T2 模型设备一致性: write "hello there" -> read "intent=2"
 *   T3 与 0x5101 交叉验证: 两路径 intent 相同 (UNIX 面 == 原语面)
 *   T4 tmpfs: open /tmp/devdemo.txt (建) -> write -> close -> 重开 -> read 回读一致
 *   T5 tmpfs 既有文件: /tmp/hello.txt 读回种子内容 (M15 兼容)
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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}

static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}

#include "../rulebook/rulebook.h"

/* 读 fd 全文到 buf, 返回长度 (<=cap) */
static long read_all(long fd, char *buf, long cap, long *shown_n)
{
    long n = 0;
    while (n < cap) {
        long r = sy(0, fd, (long)(buf + n), cap - n, 0, 0);
        if (r <= 0)
            break;
        n += r;
    }
    if (shown_n)
        *shown_n = n;
    return n;
}

static long strstr_pos(const char *hay, const char *needle)
{
    long i = 0, j;
    while (hay[i]) {
        j = 0;
        while (needle[j] && hay[i + j] == needle[j])
            j++;
        if (!needle[j])
            return i;
        i++;
    }
    return -1;
}

static void run(void)
{
    static const char h[] = "m122: VFS abstraction + tmpfs + /dev/model0 (W12)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    char buf[128];
    long n;

    /* R5 规则字节码载入 (离线走规则 -> 设备零模型调用) */
    long rn = sy(0x830B, (long)RULEBOOK, RULEBOOK_LEN, 0, 0, 0);
    wrstr("m122: rulebook=");
    wrdec((u64)rn);
    wrstr(" (expect >0)\n");
    if (rn <= 0)
        pass_all = 0;

    /* T1: /dev/model0 往返 */
    {
        long fd = sy(2, (long)"/dev/model0", 0, 2, 0, 0); /* open RDWR */
        wrstr("m122: open model0 fd=");
        wrdec((u64)fd);
        wrstr(" (expect >=3)\n");
        if (fd < 3)
            pass_all = 0;
        static const char req[] = "run the game";
        long w = sy(1, fd, (long)req, sizeof(req) - 1, 0, 0);
        n = read_all(fd, buf, 64, 0);
        buf[n] = 0;
        wrstr("m122: T1 write=");
        wrdec((u64)w);
        wrstr(" read='");
        wrstr(buf);
        wrstr("' (expect intent=1)\n");
        if (!(w == sizeof(req) - 1 && strstr_pos(buf, "intent=1") == 0))
            pass_all = 0;
        sy(3, fd, 0, 0, 0, 0); /* close */
    }

    /* T2: 多样性请求 (hello -> intent=2) */
    {
        long fd = sy(2, (long)"/dev/model0", 0, 0, 0, 0);
        static const char req[] = "hello there";
        sy(1, fd, (long)req, sizeof(req) - 1, 0, 0);
        n = read_all(fd, buf, 64, 0);
        buf[n] = 0;
        wrstr("m122: T2 read='");
        wrstr(buf);
        wrstr("' (expect intent=2)\n");
        if (strstr_pos(buf, "intent=2") != 0)
            pass_all = 0;
        sy(3, fd, 0, 0, 0, 0);
    }

    /* T3: 交叉验证 0x5101 (原语面 == 设备面) */
    {
        long fd = sy(2, (long)"/dev/model0", 0, 0, 0, 0);
        static const char req[] = "open a file";
        sy(1, fd, (long)req, sizeof(req) - 1, 0, 0);
        n = read_all(fd, buf, 64, 0);
        buf[n] = 0;
        int dev = 0;
        if (strstr_pos(buf, "intent=3") == 0)
            dev = 3;
        int prim = (int)sy(0x5101, (long)req, sizeof(req) - 1, 0, 0, 0);
        wrstr("m122: T3 device=");
        wrdec((u64)dev);
        wrstr(" primitive=");
        wrdec((u64)prim);
        wrstr(" (expect 3/3)\n");
        if (!(dev == 3 && prim == 3))
            pass_all = 0;
        sy(3, fd, 0, 0, 0, 0);
    }

    /* T4: tmpfs 建/写/关/重开/读回 */
    {
        long fd = sy(2, (long)"/tmp/devdemo.txt", 0, 2, 0, 0);
        static const char data[] = "hello model device";
        long w = sy(1, fd, (long)data, sizeof(data) - 1, 0, 0);
        sy(3, fd, 0, 0, 0, 0);
        fd = sy(2, (long)"/tmp/devdemo.txt", 0, 0, 0, 0);
        n = read_all(fd, buf, 128, 0);
        buf[n] = 0;
        wrstr("m122: T4 tmpfs read='");
        wrstr(buf);
        wrstr("' (expect hello model device)\n");
        if (!(w == sizeof(data) - 1 && strstr_pos(buf, "hello model device") == 0))
            pass_all = 0;
        sy(3, fd, 0, 0, 0, 0);
    }

    /* T5: tmpfs 种子兼容 (M15) */
    {
        long fd = sy(2, (long)"/tmp/hello.txt", 0, 0, 0, 0);
        n = read_all(fd, buf, 128, 0);
        buf[n] = 0;
        wrstr("m122: T5 seed read='");
        wrstr(buf);
        wrstr("'\n");
        if (strstr_pos(buf, "hello from FujoOS ramdisk") < 0)
            pass_all = 0;
        sy(3, fd, 0, 0, 0, 0);
    }

    if (pass_all) {
        static const char m2[] = "m122: M122 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m122: M122 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(60, 7, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
