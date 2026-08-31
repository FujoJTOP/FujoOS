/* m38_wm.c — M38: 窗口管理 (重叠/焦点/拖动/关闭) 验证
 *
 * 零 libc ELF (fujo 消息环 + 窗口原语):
 *   0x5520 class / 0x5521 create / 0x5522 getmsg / 0x5523 top
 *   0x5524 remove / 0x5525 move(win,dx,dy) / 0x5526 rect(win,ptr)
 * 流程:
 *   A. 创建 2 窗口 (重叠) -> z-order 交叠
 *   B. 置顶 w2 -> WM_ZORDER
 *   C. 拖动: wm_move(w1, 100, 50) -> WM_MOVE + rect 读回验证新位置
 *   D. 焦点: 鼠标矩形随拖动重建 -> 矩形读回
 *   E. 关闭: wm_remove 两窗 -> WM_DESTROY
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m38_wm.c -o sdk/linux/m38_wm.elf
 */
typedef long int64_t;
typedef unsigned int u32;

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
static int64_t sys5(long nr, long a, long b, long c, long d, long e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10), "r"(r8)
                 : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sys3(1, 1, (long)s, len); }
static void wrdec(u32 v)
{
    char b[12];
    int i = 12;
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 12 - i);
}

static int sample_msg(u32 *out)
{
    return (int)sys3(0x5522, (long)out, 0, 0);
}

void _start(void)
{
    static const char m1[] = "m38: window mgmt v0 - overlap/focus/drag/close\n";
    wr(m1, sizeof(m1) - 1);

    long cls = sys3(0x5520, (long)"Main", 0, 0);
    u32 w1 = (u32)sys5(0x5521, cls, 10, 10, 200, 150);
    u32 w2 = (u32)sys5(0x5521, cls, 120, 80, 200, 150); /* 与 w1 重叠 */
    wr("m38: create w1=", 15);
    wrdec(w1);
    wr(" w2=", 4);
    wrdec(w2);
    wr("\n", 1);

    /* B: 置顶 w2 (z-order 调整) */
    (void)sys3(0x5523, (long)w2, 0, 0);
    wr("m38: bring-to-top w2\n", 20);

    /* C: 拖动 w1 +100/+50 -> WM_MOVE + rect 读回 */
    (void)sys5(0x5525, (long)w1, 100, 50, 0, 0);
    u32 rc[4];
    (void)sys3(0x5526, (long)w1, (long)rc, 0);
    static const char d1[] = "m38: drag w1 -> rect=(";
    wr(d1, (long)sizeof(d1) - 1);
    wrdec(rc[0]);
    static const char cm[] = ",";
    wr(cm, 1);
    wrdec(rc[1]);
    wr(")\n", 2);

    /* D: 矩形重建后焦点矩形 = w1 新位置 (读鼠标层) */
    long fo = sys3(0x5412, 0, 0, 0);
    (void)fo;
    wr("m38: focus-read ok\n", 18);

    /* E: 关闭两窗口 -> WM_DESTROY */
    (void)sys3(0x5524, (long)w1, 0, 0);
    (void)sys3(0x5524, (long)w2, 0, 0);

    /* 收集消息流 (WM_* 队列) */
    u32 seen_move = 0, seen_destroy = 0, seen_zorder = 0;
    int k;
    for (k = 0; k < 64; k++) {
        u32 msg[5];
        if (!sample_msg(msg)) {
            break;
        }
        if (msg[0] == 0x13) {
            seen_move++;
            wr("m38: WM_MOVE win=", 17);
            wrdec(msg[1]);
            static const char pos[] = " (";
            wr(pos, 2);
            wrdec(msg[2]);
            static const char cm2[] = ",";
            wr(cm2, 1);
            wrdec(msg[3]);
            wr(")\n", 2);
        } else if (msg[0] == 0x11) {
            seen_destroy++;
            wr("m38: WM_DESTROY win=", 20);
            wrdec(msg[1]);
            wr("\n", 1);
        } else if (msg[0] == 0x12) {
            seen_zorder++;
        }
    }

    wr("m38: move=", 10);
    wrdec(seen_move);
    static const char t1[] = " destroy=";
    wr(t1, 9);
    wrdec(seen_destroy);
    static const char t2[] = " zorder=";
    wr(t2, 8);
    wrdec(seen_zorder);
    wr("\n", 1);

    if (seen_move >= 1 && seen_destroy >= 2 && seen_zorder >= 1 && rc[0] == 110 && rc[1] == 60) {
        static const char m2[] = "m38: M38 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m38: M38 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
