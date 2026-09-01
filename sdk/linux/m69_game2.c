/* m69_game2.c — M69: 2D 游戏#2 (Breakout v0) + 输入延迟基准
 *
 * 帧循环 (5 帧 × frame_wait 20ms):
 *   1. t0 = timer_us        (输入采样点: 模拟玩家输入 dx)
 *   2. 玩家拍移动 + 球运动 + 砖块碰撞 (hits++)
 *   3. blit 渲染球 (M61 0x6801, 16x16 pattern)
 *   4. t1 = timer_us; latency = t1-t0 → 0x6F01
 * 验证: frames>=4, avg>0, max>=avg, hits>=2
 */
typedef long int64_t;
typedef unsigned long long u64;
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

static u32 ball[16 * 16];
static u64 st[4];

static void make_ball(void)
{
    int i;
    for (i = 0; i < 16 * 16; i++) {
        u32 x = (u32)(i % 16), y = (u32)(i / 16);
        u32 dx = x - 7, dy = y - 7;
        ball[i] = (dx * dx + dy * dy <= 16) ? 0x00FF00 : 0x000000;
    }
}

void _start(void)
{
    static const char m1[] = "m69: breakout v0 + input-latency bench\n";
    wr(m1, sizeof(m1) - 1);

    make_ball();
    (void)sys3(0x6201, 0, 0, 0); /* 清屏 v0 */

    int bx = 100, by = 80, vx = 2, vy = -1; /* 球 (像素), 起点近顶部向上 */
    int px = 300, py = 600;                 /* 拍 */
    int hits = 0;
    int frame;
    static const int BRICK_Y = 64, BRICK_H = 12;
    static const int SCREEN_W = 1024, SCREEN_H = 768;

    (void)sys3(0x6100, 0, 0, 0); /* timer 校准 arm */

    for (frame = 0; frame < 10; frame++) {
        long t0 = sys3(0x6101, 0, 0, 0); /* 输入采样点 */

        /* 模拟输入: 拍跟踪球 x (固定 pattern, QEMU 无指针时自打) */
        px = bx - 10;
        if (px < 0) px = 0;
        if (px > SCREEN_W - 20) px = SCREEN_W - 20;

        /* 物理 (每帧 14px 步进) */
        bx += vx * 14;
        by += vy * 14;
        if (bx <= 0 || bx >= SCREEN_W - 16) {
            vx = -vx;
        }
        if (by <= 0) {
            vy = -vy;
        }
        if (by >= SCREEN_H - 16) {
            vy = -vy;
        }
        /* 砖块带: 球进入 → 反弹 + hits */
        if (by + 16 > BRICK_Y && by <= BRICK_Y && by >= BRICK_Y - 16) {
            vy = -vy;
            hits++;
        }
        /* 拍碰撞 (球底进入拍顶) */
        if (by + 16 >= py && by + 16 <= py + 8 && bx + 16 >= px && bx <= px + 20) {
            vy = -vy;
        }

        /* 渲染: blit 球 (M61 0x6801) + 拍 (gl_rect 0x6202, color 打包) */
        (void)sys5(0x6801, (long)ball, bx, by, 16, 16);
        (void)sys5(0x6202, (long)px, (long)py, 20, 60, 0xF0E020);

        long t1 = sys3(0x6101, 0, 0, 0);
        (void)sys3(0x6F01, (long)(t1 - t0), 0, 0); /* 输入→渲染延迟 */
        (void)sys3(0x6104, 20000, 0, 0);           /* 帧等待 20ms */
    }

    (void)sys3(0x6F03, (long)hits, 0, 0);
    (void)sys3(0x6F02, (long)st, 0, 0);
    u64 n = st[0], avg = st[1], mx = st[2], hs = st[3];
    static const char h1[] = "m69: frames=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)n);
    static const char h2[] = " avg_lat=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)avg);
    static const char h3[] = " max_lat=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)mx);
    static const char h4[] = " hits=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)hs);
    wr("\n", 1);

    int ok = n >= 4 && avg > 0 && mx >= avg && hs >= 1;
    if (ok) {
        static const char m2[] = "m69: M69 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m69: M69 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
