/* m62_shader.c — M62: 着色器内核评估 (compute 子集 v0)
 *
 * 字节码 (每指令 u32: op<<24 | r<<16 | a<<8 | b):
 *   0 halt | 1 const r,v | 2 add r,a,b | 3 mul r,a,b | 4 sub r,a,b
 *   5 color r,a,b (r = (regs[a]&0xFF) | ((b&0xFF)<<8)) | 6 idx 重载索引
 * 每像素: r0 = y*FBW+x, 依次执行 7 条指令, r1 = 输出色。
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

static u32 prog[8] = {
    (1u << 24) | (4u << 16) | 0xFFu,        /* const r4 = 0xFF */
    (1u << 24) | (5u << 16) | 0x00u,        /* const r5 = 0    */
    (1u << 24) | (6u << 16) | 0x01u,        /* const r6 = 1    */
    (2u << 24) | (3u << 16) | (0u << 8) | 5u,  /* r3 = r0 + r5 (idx) */
    (2u << 24) | (7u << 16) | (3u << 8) | 4u,  /* r7 = r3 + r4 (idx+255) */
    (3u << 24) | (7u << 16) | (7u << 8) | 6u,  /* r7 = r7 * r6 */
    (5u << 24) | (1u << 16) | (3u << 8) | 0xFFu, /* r1 = (idx&0xFF) | 0xFF00 */
    0,
};

void _start(void)
{
    static const char m1[] = "m62: shader compute-kernel subset v0\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0);
    (void)sys3(0x6901, (long)prog, 7 * 4, 0);   /* load 7 instrs */
    (void)sys5(0x6902, 0, 0, 16, 16, 0);        /* run 16x16 region */

    u32 p00 = (u32)sys3(0x6903, 0, 0, 0);       /* idx=0   -> 0x0000FF00 */
    u32 p10 = (u32)sys3(0x6903, 1, 0, 0);       /* idx=1   -> 0x0000FF01 */
    u32 p50 = (u32)sys3(0x6903, 5, 0, 0);       /* idx=5   -> 0x0000FF05 */
    u32 p1515 = (u32)sys3(0x6903, 15, 15, 0);   /* idx=255 -> 0x0000FFFF */
    long ops = sys3(0x6904, 0, 0, 0);

    static const char h1[] = "m62: p00=";
    wr(h1, sizeof(h1) - 1);
    wrhex(p00);
    static const char h2[] = " p10=";
    wr(h2, sizeof(h2) - 1);
    wrhex(p10);
    static const char h3[] = " p50=";
    wr(h3, sizeof(h3) - 1);
    wrhex(p50);
    static const char h4[] = " p1515=";
    wr(h4, sizeof(h4) - 1);
    wrhex(p1515);
    wr("\n", 1);

    static const char h5[] = "m62: ops=";
    wr(h5, sizeof(h5) - 1);
    wrhex((u32)ops);
    wr("\n", 1);

    /* FBW=1024: idx(15,15)=15*1024+15=15375; &0xFF=0x0F -> 0x0000FF0F */
    int ok = p00 == 0x0000FF00 && p10 == 0x0000FF01 && p50 == 0x0000FF05
             && p1515 == 0x0000FF0F && ops == ((long)(16 * 16 * 8));
    if (ok) {
        static const char m2[] = "m62: M62 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m62: M62 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}
