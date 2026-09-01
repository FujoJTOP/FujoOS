/* m114_nlc.c — M114: 自然语言配置 (D) + 环境侦察 (E)
 *
 * D: "ban games during 0 to 24" → 0x8307 nlc_set → 模型/规则 →
 *    策略对象 POL=3:1;4:0;5:24 → cfg_set → 0x8106 读回校验
 *    → 策略执行: 0x6601 game_mode(1) 命中禁玩时段 → 返回 -1 (拒绝)
 * E: 0x8308 env_scan → hw/acpi/storage 摘要 → 模型场景/档案
 *    → cfg_set(6, profile) → out {profile, scene_code, len} 校验
 *
 * RESULT: D 策略对象生效 + 执行面拒绝 + E 档案落地 → PASS。
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

static const char NL[] = "\n";

static int run(void)
{
    static const char h1[] = "m114: natural-language config + env recon\n";
    wr(h1, sizeof(h1) - 1);

    /* D: NL → 策略对象 */
    static const char h2[] = "m114: 1) nlc policy object\n";
    wr(h2, sizeof(h2) - 1);
    {
        static const char g1[] = "ban games during 0 to 24";
        u64 out[1] = { 0 };
        sy(0x8307, (long)g1, sizeof(g1) - 1, (long)out, 8, 0);
        long b = sy(0x8106, 3, 0, 0, 0, 0);
        long s = sy(0x8106, 4, 0, 0, 0, 0);
        long e2 = sy(0x8106, 5, 0, 0, 0, 0);
        static const char p[] = "m114: policy applied=";
        wr(p, sizeof(p) - 1);
        wrdec(out[0]);
        static const char p2[] = " cfg(3/4/5)=";
        wr(p2, sizeof(p2) - 1);
        wrdec((u64)b);
        wr("/", 1);
        wrdec((u64)s);
        wr("/", 1);
        wrdec((u64)e2);
        wr(NL, 1);
        if (!(out[0] >= 3 && b == 1 && s == 0 && e2 == 24)) {
            static const char f[] = "m114: M114 RESULT: FAIL (policy)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }
    /* D2: 策略执行面 (0x6601 被拒) */
    static const char h3[] = "m114: 2) policy enforcement (game_mode denied)\n";
    wr(h3, sizeof(h3) - 1);
    sy(0x8101, 6, 0x3F, 0, 0, 0); /* exec 槽全授权 (SET_CFG 经 cap_exec) */
    {
        long r = sy(0x6601, 1, 0, 0, 0, 0);
        static const char p[] = "m114: game_mode(1) rc=";
        wr(p, sizeof(p) - 1);
        wrdec((u64)r);
        wr(NL, 1);
        if (r != -1) {
            static const char f[] = "m114: M114 RESULT: FAIL (enforce)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
        /* 解除后再启: 应成功 */
        sy(0x8105, 4, 3, 0, 0, 0); /* SET_CFG(3,0) 需授权 */
        {
            long r2 = sy(0x6601, 1, 0, 0, 0, 0);
            static const char p2[] = "m114: after unban game_mode(1) rc=";
            wr(p2, sizeof(p2) - 1);
            wrdec((u64)r2);
            wr(NL, 1);
            if (r2 != 0) {
                static const char f[] = "m114: M114 RESULT: FAIL (unban)\n";
                wr(f, sizeof(f) - 1);
                return 0;
            }
            sy(0x6601, 0, 0, 0, 0, 0); /* 还原 */
        }
    }
    /* E: 环境侦察 */
    static const char h4[] = "m114: 3) env recon (scene/profile)\n";
    wr(h4, sizeof(h4) - 1);
    {
        u64 out[3] = { 0, 0, 0 };
        sy(0x8308, (long)out, 24, 0, 0, 0);
        long cfg_prof = sy(0x8106, 6, 0, 0, 0, 0);
        static const char p[] = "m114: env(profile/scene/len)=";
        wr(p, sizeof(p) - 1);
        wrdec(out[0]);
        wr("/", 1);
        wrdec(out[1]);
        wr("/", 1);
        wrdec(out[2]);
        static const char p2[] = " cfg(6)=";
        wr(p2, sizeof(p2) - 1);
        wrdec((u64)cfg_prof);
        wr(NL, 1);
        if (!(out[0] >= 1 && out[1] >= 1 && cfg_prof == (long)out[0])) {
            static const char f[] = "m114: M114 RESULT: FAIL (env)\n";
            wr(f, sizeof(f) - 1);
            return 0;
        }
    }
    static const char m2[] = "m114: M114 RESULT: PASS\n";
    wr(m2, sizeof(m2) - 1);
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
