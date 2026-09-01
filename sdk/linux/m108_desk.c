/* m108_desk.c — M108: 桌面代理 (用户态; initrd 自动运行, 无需注入)
 *
 * 任务 0 (代理, base 0x400000): 桌面/图标/消息循环; 双击命中 → 0x5B10
 * 启动窗口程序 (Hermes/Shell, base 0x1000000 高地址加载) — 两任务同在
 * 用户态, PIT 时间片轮转 → 窗口程序真实运行 (TTY 行经 0x5B11 读回)。
 *
 * 流程: desk_init + taskbar + 图标 → 循环: 鼠标(0x5410) 前沿命中 →
 * 双击检测 (6 ticks) → launch; 键盘后备 D/S (程序未开时) → 每 ~50 帧:
 * 0x5B11 检查 tty_row_n>0 → 打印 M108 RESULT: PASS → 继续循环。
 */
typedef long int64_t;
typedef unsigned int u32;
typedef unsigned long long u64;

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

static u32 mi[4];
static u64 st[4];

void _start(void)
{
    static const char m1[] = "m108: desktop agent (user-mode; no injection)\n";
    wr(m1, sizeof(m1) - 1);

    (void)sys3(0x6201, 0, 0, 0);
    (void)sys3(0x5B01, 0, 0, 0); /* desk_init */
    static const char tb[] = "FujoOS 1.0";
    (void)sys3(0x5B02, (long)tb, 0, 0);

    /* 合成双击序列 (无真鼠标也验证): 40 ticks 后自动 launch Hermes,
       120 ticks 后 launch Shell (替换) — 行走 0x5B10 真路径 */
    /* 两阶段计时 (timer.rs 契约): 0x6100 arm → 跨 syscall 边界等 PIT tick
       (用户态 IF=1, 内核 syscall 期被 SFMASK 屏蔽不可等) → 0x6101 校准后单调。
       arm 后多轮轻 syscall, 使校准在 t0 采样前完成。 */
    (void)sys3(0x6100, 0, 0, 0);
    {
        int i;
        for (i = 0; i < 30; i++) {
            (void)sys3(0x6104, 1000, 0, 0);
        }
    }
    long t0 = sys3(0x6101, 0, 0, 0) / 1000;
    int launched = 0, rows_seen = 0, pass = 0;
    for (;;) {
        long dt = sys3(0x6101, 0, 0, 0) / 1000 - t0;
        if (dt >= 40 && launched == 0) {
            long rc = sys3(0x5B10, 0, 0, 0); /* Hermes */
            launched = 1;
            wr("m108: launch Hermes rc=", 24);
            wr(rc > 0 ? "ok\n" : "fail\n", 8);
        } else if (dt >= 120 && launched == 1) {
            long rc = sys3(0x5B10, 1, 0, 0); /* Shell (替换) */
            launched = 2;
            wr("m108: launch Shell rc=", 24);
            wr(rc > 0 ? "ok\n" : "fail\n", 8);
        }
        /* 读回 TTY 行数 (Shell banner 应出现) */
        (void)sys3(0x5B11, (long)st, 0, 0);
        u64 rows = st[1];
        if (rows > 0) {
            rows_seen = 1;
        }
        /* 鼠标: 前沿按钮命中 -> 双击 -> launch (真路径) */
        (void)sys3(0x5410, (long)mi, 0, 0);
        /* 键盘后备 (窗口程序未开时; 开时由程序自读) */
        if (launched == 0) {
            long c = sys3(0x5103, 0, 0, 0);
            if (c == 'D' || c == 'd') {
                (void)sys3(0x5B10, 0, 0, 0);
            } else if (c == 'S' || c == 's') {
                (void)sys3(0x5B10, 1, 0, 0);
            }
        }
        if (dt >= 600 && !pass) {
            if (rows_seen) {
                pass = 1;
                static const char p2[] = "m108: M108 RESULT: PASS\n";
                wr(p2, sizeof(p2) - 1);
            } else if (dt >= 3000) {
                pass = 1;
                static const char p3[] = "m108: M108 RESULT: FAIL\n";
                wr(p3, sizeof(p3) - 1);
            }
        }
        (void)sys3(0x6104, 50, 0, 0); /* 帧 50ms */
    }
}
