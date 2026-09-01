/* m102_windrag.c — M102: 窗口拖动/焦点/层级 (wmsg 语义: class 注册,
 * move=增量, rect=(x,y,w,h), top=移至表尾/表顶)
 *
 * 1. 注册类 Programs/Files
 * 2. create A(30,40 320x220) B(200,120 280x180): 返回 win id (>0)
 * 3. rect 读回: A=(30,40,320,220) B=(200,120,280,180)
 * 4. 拖动 B: move ×6 (50,45) -> B=(500,390)
 * 5. z 序/焦点: top(B)==0 (存在), remove(B)==0 -> rect(B)==-2 (已删),
 *    top(A)==0 -> remove(A) 完成
 * 6. PASS: rectB 增量正确 + 拖动到位 + 层级/析构链
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

static u32 rA[4], rB[4];

void _start(void)
{
    static const char m1[] = "m102: window drag/focus/z-order\n";
    wr(m1, sizeof(m1) - 1);

    long clsA = sys3(0x5520, (long)"Programs", 0, 0);
    long clsB = sys3(0x5520, (long)"Files", 0, 0);
    if (clsA <= 0) clsA = 1;
    if (clsB <= 0) clsB = 2;

    long winA = sys5(0x5521, clsA, 30, 40, 320, 220);
    long winB = sys5(0x5521, clsB, 200, 120, 280, 180);
    int ok_create = winA > 0 && winB > 0 && winA != winB;

    (void)sys3(0x5526, winA, (long)rA, 0);
    (void)sys3(0x5526, winB, (long)rB, 0);
    int a0 = rA[0] == 30 && rA[1] == 40 && rA[2] == 320 && rA[3] == 220;
    int b0 = rB[0] == 200 && rB[1] == 120 && rB[2] == 280 && rB[3] == 180;

    /* 拖动 B: 6 步增量 (50,45) */
    int step;
    for (step = 0; step < 6; step++) {
        (void)sys3(0x5525, winB, 50, 45);
        (void)sys3(0x6104, 10, 0, 0);
    }
    (void)sys3(0x5526, winB, (long)rB, 0);
    int moved = rB[0] == 500 && rB[1] == 390;

    /* z 序/焦点: top(B)=0; remove(B) -> rect(B)=-2 */
    long topB = sys3(0x5523, winB, 0, 0);
    long rmB = sys3(0x5524, winB, 0, 0);
    long badB = sys3(0x5526, winB, (long)rB, 0);
    long topA = sys3(0x5523, winA, 0, 0);
    long rmA = sys3(0x5524, winA, 0, 0);

    static const char h1[] = "m102: winA=";
    wr(h1, sizeof(h1) - 1);
    wrhex((u32)winA);
    static const char h2[] = " winB=";
    wr(h2, sizeof(h2) - 1);
    wrhex((u32)winB);
    static const char h3[] = " moved=";
    wr(h3, sizeof(h3) - 1);
    wrhex((u32)moved);
    static const char h4[] = " topB=";
    wr(h4, sizeof(h4) - 1);
    wrhex((u32)topB);
    static const char h5[] = " rmB=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)rmB);
    wr("\n", 1);

    int ok = ok_create && a0 && b0 && moved
             && topB == 0 && rmB == 0 && badB == -2
             && topA == 0 && rmA == 0;
    if (ok) {
        static const char m2[] = "m102: M102 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m102: M102 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
