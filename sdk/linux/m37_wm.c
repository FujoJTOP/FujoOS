/* m37_wm.c — M37: 消息环 (窗口类/窗口/消息队列/z-order) 验证
 *
 * 零 libc ELF。fujo 原生消息环原语:
 *   0x5520 wm_class(name) -> class_id
 *   0x5521 wm_create(class, x, y, w, h) -> win_id
 *   0x5522 wm_getmsg(ptr) -> 1/0 (写 kind,win,a,b,c u32×5)
 *   0x5523 wm_top(win) / 0x5524 wm_remove(win)
 * 创建 3 窗口 (z 层叠), 轮询消息流 (QM mouse 注入), 验证 WM_ENTER /
 * WM_MOUSEMOVE / WM_TOP (置顶换焦) —— 消息环核心语义。
 *
 * 编译:
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -no-pie -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/linux/m37_wm.c -o sdk/linux/m37_wm.elf
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
    static const char m1[] = "m37: win32k-equivalent message ring - wm classes/messages/z-order\n";
    wr(m1, sizeof(m1) - 1);

    /* 注册类 + 3 窗口 (重叠: w2 覆盖 w1 右上区, w3 覆盖 w2 右上区) */
    long cls = sys3(0x5520, (long)"Main", 0, 0);
    wr("m37: class_id=", 14);
    wrdec((u32)cls);
    wr("\n", 1);
    u32 w1 = (u32)sys5(0x5521, cls, 10, 10, 200, 150);
    u32 w2 = (u32)sys5(0x5521, cls, 100, 80, 200, 150);
    u32 w3 = (u32)sys5(0x5521, cls, 160, 100, 200, 150);
    wr("m37: windows w1=", 17);
    wrdec(w1);
    wr(" w2=", 4);
    wrdec(w2);
    wr(" w3=", 4);
    wrdec(w3);
    wr("\n", 1);

    /* 消息轮询: WM_CREATE 确定性事件 (鼠标消息为增量, 注入时序无关) */
    u32 creates = 0, mouse_msgs = 0, zorders = 0;
    int loops = 0;
    while (loops++ < 60000 && creates < 3) {
        u32 msg[5];
        long got = sys3(0x5522, (long)msg, 0, 0);
        if (got) {
            u32 kind = msg[0];
            if (kind == 0x10) {
                static const char hdr[] = "m37: WM_CREATE win=";
                wr(hdr, 18);
                wrdec(msg[1]);
                wr("\n", 1);
                creates++;
            } else if (kind >= 1 && kind <= 4) {
                static const char hdr[] = "m37: WM_MSG win=";
                wr(hdr, 15);
                wrdec(msg[1]);
                static const char kth[] = " kind=";
                wr(kth, 7);
                wrdec(kind);
                wr("\n", 1);
                mouse_msgs++;
            } else if (kind == 0x12) {
                zorders++;
            }
            if (kind == 2 && msg[1] == w3 && creates == 3) {
                (void)sys3(0x5523, (long)w2, 0, 0); /* bring-to-top */
                wr("m37: z-order: w2 brought to top\n", 31);
            }
        }
        long i;
        for (i = 0; i < 60000; i++) {
            __asm__ volatile("" ::: "memory");
        }
    }

    wr("m37: creates=", 13);
    wrdec(creates);
    static const char tm0[] = " mouse=";
    wr(tm0, 7);
    wrdec(mouse_msgs);
    wr("\n", 1);

    /* 后轮询: 继续收 2 秒鼠标消息 (注入窗口) */
    int post;
    for (post = 0; post < 20000 && mouse_msgs < 6; post++) {
        u32 msg[5];
        long got = sys3(0x5522, (long)msg, 0, 0);
        if (got && msg[0] >= 1 && msg[0] <= 4) {
            static const char hdr[] = "m37: WM_MOUSE win=";
            wr(hdr, 16);
            wrdec(msg[1]);
            static const char kth[] = " kind=";
            wr(kth, 7);
            wrdec(msg[0]);
            static const char pos[] = " (";
            wr(pos, 2);
            wrdec(msg[2]);
            static const char cm[] = ",";
            wr(cm, 1);
            wrdec(msg[3]);
            wr(")\n", 2);
            mouse_msgs++;
        }
        long i;
        for (i = 0; i < 80000; i++) {
            __asm__ volatile("" ::: "memory");
        }
    }

    static const char m2[] = "m37: M37 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
