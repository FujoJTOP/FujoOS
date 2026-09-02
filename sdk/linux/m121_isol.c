/* m121_isol.c — W11 (阶段二首站): 独立地址空间 / 进程隔离 (docs/62)
 *
 * 每任务私有页表链 (PML4→PDPT→PD→PT0/PT1) + CR3 切换:
 *   隐式/系统任务 = 静态堆 PT (兼容); 派生任务 (LAUNCH/fork/window) = 私有链。
 * 断言 (全部离线):
 *   ① 同 VA 不同物: 隐式任务在 0x800000 写 0x5A5A; LAUNCH 的 worker
 *      读自有 0x800000 = 0 (零页, 不共享!) -> 写 0xDEAD -> 隐式任务仍见 0x5A5A。
 *   ② fork 拷贝语义: 子先见父预值 0x5A5A (堆页物理拷贝), 子写 0xCAFE 后
 *      父仍 0x5A5A (隔离)。
 *   ③ munmap 撤销: mmap 2 页写标记 -> munmap -> 重读 = 0 (帧释放 + 重新按需零页)。
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

static void wrhex(u64 v)
{
    static const char H[] = "0123456789abcdef";
    char b[20];
    int i;
    b[0] = '0';
    b[1] = 'x';
    for (i = 0; i < 16; i++)
        b[2 + i] = H[(v >> (4 * (15 - i))) & 0xF];
    b[18] = 0;
    wr(b, 18);
}

static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}

/* LAUNCH 的 worker: 自有地址空间 (私有堆 PT) */
__attribute__((noinline, noreturn)) static void worker(void)
{
    u64 cr3[2];
    volatile u64 *p = (volatile u64 *)0x800000;
    sy(0x830E, (long)cr3, 0, 0, 0, 0);
    wrstr("m121: worker cr3=");
    wrhex(cr3[0]);
    wrstr("\n");
    u64 saw = *p; /* 首次读 -> 按需零页 (自有帧) = 0 */
    wrstr("m121: worker saw=");
    wrdec(saw);
    wrstr(" (expect 0 - not shared)\n");
    *p = 0xDEADDEADUL;
    sy(0x8004, 5, 0x4242, 0, 0, 0); /* 握手事件 (内核环, 全域可见) */
    for (;;) {
    }
}

/* 事件环等待: 匹配 a == tag (长轮询让出时间片, PIT 轮转保证派生任务运行) */
static int wait_ev(u64 tag)
{
    u64 ev[25];
    int i, j;
    for (i = 0; i < 200000; i++) {
        long n = sy(0x8003, (long)ev, sizeof(ev), 0, 0, 0);
        for (j = 0; j < n && j < 5; j++) {
            if (ev[j * 5 + 3] == tag)
                return 1;
        }
    }
    return 0;
}

static void run(void)
{
    static const char h[] = "m121: per-task address space / process isolation (W11)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    volatile u64 *p = (volatile u64 *)0x800000;

    *p = 0x5A5A5A5AUL; /* 隐式任务: 系统堆 PT, 值 0x5A5A */

    /* ① LAUNCH worker (私有页表链) */
    sy(0x8101, 6, 0x3F, 0, 0, 0);
    long tid = sy(0x8105, 3, (long)&worker, 0, 0, 0);
    long got_w = wait_ev(0x4242);
    u64 own = *p;
    wrstr("m121: LAUNCH tid=");
    wrdec((u64)tid);
    wrstr(" own=");
    wrhex(own);
    wrstr(" (expect 0x5a5a5a5a after worker wrote DEAD)\n");
    if (!(tid == 1 && got_w == 1 && own == 0x5A5A5A5AUL))
        pass_all = 0;

    /* ② fork: 子见父预值 (拷贝), 子写隔离 */
    long child = sy(57, 0, 0, 0, 0, 0);
    if (child == 0) {
        u64 v = *p; /* 应见 0x5A5A5A5A (fork 堆页拷贝) */
        wrstr("m121: child saw=");
        wrhex(v);
        wrstr(" (expect 0x5a5a5a5a)\n");
        if (v != 0x5A5A5A5AUL)
            pass_all = 0;
        *p = 0xCAFECAFEUL;
        sy(0x8004, 5, 0x5151, 0, 0, 0);
        for (;;) {
        }
    } else {
        long got_c = wait_ev(0x5151);
        u64 v = *p;
        wrstr("m121: parent after child own=");
        wrhex(v);
        wrstr(" (expect still 0x5a5a5a5a)\n");
        if (!(got_c == 1 && v == 0x5A5A5A5AUL))
            pass_all = 0;
    }

    /* ③ munmap 撤销: mmap 2 页 -> 写标记 -> munmap -> 重读 0 */
    {
        long base = sy(9, 0, 0x2000, 3, 2 | 0x20, 0); /* mmap(0, len, prot, anon|private) */
        if (base > 0) {
            *(volatile u64 *)base = 0xBEEFBEEFUL;
            sy(11, base, 0x2000, 0, 0, 0); /* munmap */
            u64 v = *(volatile u64 *)base; /* PTE 清 -> 按需零页 -> 0 */
            wrstr("m121: munmap reread=");
            wrhex(v);
            wrstr(" (expect 0)\n");
            if (v != 0)
                pass_all = 0;
        } else {
            pass_all = 0;
        }
    }

    if (pass_all) {
        static const char m2[] = "m121: M121 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m121: M121 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
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
