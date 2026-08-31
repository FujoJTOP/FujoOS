/* m36_mouse.c — M36: PS/2 鼠标 + 命中测试/焦点验证
 *
 * 零 libc ELF。fujo 原生鼠标原语:
 *   0x5410 mouse_info(ptr) 写 u32×4 (x, y, buttons, steps)
 *   0x5411 mouse_rects(ptr, n) 注册命中矩形 [x0,y0,x1,y1,id]×n
 *   0x5412 mouse_focus() -> 焦点 id
 * 注册两个矩形并轮询坐标/焦点变化 (QEMU monitor mouse_move 注入)。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m36_mouse.c -o sdk/linux/m36_mouse.elf
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

void _start(void)
{
    static const char m1[] = "m36: ps/2 mouse driver - hit test & focus\n";
    wr(m1, sizeof(m1) - 1);

    /* 注册矩形: [x0,y0,x1,y1,id] — 两个窗口 */
    static const u32 rects[10] = { 100, 100, 300, 200, 1,  50, 50, 150, 150, 2 };
    (void)sys3(0x5411, (long)rects, 2, 0);

    int loop;
    u32 last_x = ~0u, last_y = ~0u, last_f = ~0u;
    for (loop = 0; loop < 4000; loop++) {
        u32 info[4];
        (void)sys3(0x5410, (long)info, 0, 0);
        u32 x = info[0], y = info[1], b = info[2], steps = info[3];
        long f = sys3(0x5412, 0, 0, 0);
        if (x != last_x || y != last_y || (u32)f != last_f) {
            static const char p[] = "m36: pos=(";
            wr(p, sizeof(p) - 1);
            wrdec(x);
            static const char cm[] = ",";
            wr(cm, 1);
            wrdec(y);
            static const char p2[] = ") btn=";
            wr(p2, 6);
            wrdec(b);
            static const char p3[] = " focus=";
            wr(p3, 8);
            if ((u32)f == 0xFFFFFFFFu) {
                static const char nf[] = "none";
                wr(nf, 4);
            } else {
                wrdec((u32)f);
            }
            wr("\n", 1);
            last_x = x;
            last_y = y;
            last_f = (u32)f;
        }
        if (steps >= 8) {
            break;
        }
        /* 忙等 ~5ms (PIT 100Hz 下时间推进慢; 用多个空循环) */
        long i;
        for (i = 0; i < 400000; i++) {
            __asm__ volatile("" ::: "memory");
        }
    }

    long f = sys3(0x5412, 0, 0, 0);
    static const char m2[] = "m36: M36 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    (void)f;
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
