/* m43_clip.c — M43: 剪贴板 + 拖放雏形验证
 *
 * 0x5801 clip_set(ptr,len) / 0x5802 clip_get(ptr,n) / 0x5803 clip_len
 * 0x5804 dnd_begin(win,x,y) / 0x5805 dnd_move(x,y) -> hit win
 * 0x5806 dnd_drop(x,y,payload) -> hit win (WM_DROPFILES 0x14 队列投递)
 * 流程: 创建窗口 w1 (100,100,200,120) -> clip_set("hello-clipboard")
 * -> clip_get 回读校验 -> dnd_begin/dnd_move(150,160 -> hit w1)。
 * -> 设 w1 命中矩形 -> dnd_drop(150,160, payload=0xBEEF) -> 事件队列
 * 收 WM_DROPFILES -> 校验 -> PASS。
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
static void wrdec(int64_t v)
{
    char b[24];
    int i = 24;
    if (v < 0) {
        b[--i] = '-';
        v = -v;
    }
    if (v == 0) b[--i] = '0';
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(&b[i], 24 - i);
}

void _start(void)
{
    static const char m1[] = "m43: clipboard + drag&drop skeleton\n";
    wr(m1, sizeof(m1) - 1);

    /* 剪贴板往返 */
    static const char payload[] = "hello-clipboard";
    long len = sizeof(payload) - 1;
    (void)sys3(0x5801, (long)payload, len, 0);
    char back[32];
    long got = sys3(0x5802, (long)back, 31, 0);
    wr("m43: clip set/go=", 18);
    wrdec(got);
    static const char s1[] = " back='";
    wr(s1, 8);
    {
        int n = 0;
        while (((const volatile char *)back)[n] != 0) n++;
        wr(back, n);
    }
    wr("'\n", 2);

    int clip_ok = (got >= 15);
    int k;
    for (k = 0; k < 15; k++) {
        if (back[k] != payload[k]) {
            clip_ok = 0;
            break;
        }
    }

    /* 拖放: 窗口 + 移动命中 + 掉落 */
    long cls = sys3(0x5520, (long)"Drop", 0, 0);
    u32 w1 = (u32)sys5(0x5521, cls, 100, 100, 200, 120);
    (void)sys3(0x5804, (long)w1, 150, 160);
    long hit = sys3(0x5805, 150, 160, 0);
    wr("m43: dnd_move hit=", 18);
    wrdec(hit);
    wr("\n", 1);
    long dest = sys3(0x5806, 150, 160, 0xBEEF);
    wr("m43: dnd_drop dest=", 19);
    wrdec(dest);
    wr("\n", 1);

    /* 消息队列: WM_DROPFILES (0x14) */
    u32 seen = 0, ev_win = 0, ev_payload = 0;
    for (k = 0; k < 32; k++) {
        u32 msg[5];
        if (!sys3(0x5522, (long)msg, 0, 0)) {
            break;
        }
        if (msg[0] == 0x14) {
            seen++;
            ev_win = msg[1];
            ev_payload = msg[4];
        }
    }
    wr("m43: wm_dropfiles=", 18);
    wrdec(seen);
    static const char s2[] = " win=";
    wr(s2, 5);
    wrdec(ev_win);
    static const char s3[] = " payload=0x";
    wr(s3, 11);
    {
        static const char H[] = "0123456789abcdef";
        char hx[9];
        int i2;
        for (i2 = 0; i2 < 8; i2++) {
            hx[i2] = H[(ev_payload >> (28 - i2 * 4)) & 0xF];
        }
        wr(hx, 8);
    }
    wr("\n", 1);

    if (clip_ok && hit == (long)w1 && dest == (long)w1 && seen >= 1 && ev_payload == 0xBEEF) {
        static const char m2[] = "m43: M43 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m43: M43 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
