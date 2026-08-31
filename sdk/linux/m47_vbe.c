/* m47_vbe.c — M47: 多屏/分辨率切换 (VBE 枚举) 验证
 *
 * 0x5C01 vbe_set(which) / 0x5C02 vbe_actual(ptr -> w,h)
 * 流程: 枚举 3 模式 (1024x768 / 640x480 / 1280x1024): 切换->读回实际
 * -> 检查一致; 三模式逐一验证 + 切换后收尾回 1024x768 -> PASS。
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
    static const char m1[] = "m47: multi-resolution VBE switch\n";
    wr(m1, sizeof(m1) - 1);

    int all_ok = 1;
    int m;
    for (m = 0; m < 3; m++) {
        long r = sys3(0x5C01, m, 0, 0);
        u32 wh[2];
        (void)sys3(0x5C02, (long)wh, 0, 0);
        static const char pre[] = "m47: mode#";
        wr(pre, 9);
        {
            char b[3];
            int i = 2;
            b[--i] = '0' + (char)((u32)m % 10);
            wr(&b[i], 2 - i);
        }
        static const char pre2[] = " -> ";
        wr(pre2, 4);
        wrdec(wh[0]);
        static const char cm[] = "x";
        wr(cm, 1);
        wrdec(wh[1]);
        static const char pre3[] = " rc=";
        wr(pre3, 5);
        wrdec((u32)r);
        wr("\n", 1);
        if (r != 0) {
            all_ok = 0;
        }
        /* 期望值核对 */
        if (!((m == 0 && wh[0] == 1024 && wh[1] == 768)
              || (m == 1 && wh[0] == 640 && wh[1] == 480)
              || (m == 2 && wh[0] == 1280 && wh[1] == 1024))) {
            all_ok = 0;
        }
    }
    /* 收尾: 回 1024x768 */
    (void)sys3(0x5C01, 0, 0, 0);

    if (all_ok) {
        static const char m2[] = "m47: M47 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m47: M47 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
