/* m132_dirs.c — W18: VFS 目录语义 (stat 类型 / open dir / getdents64)
 *
 * 断言:
 *   T1 stat("/tmp") -> S_IFDIR (busybox ls 的判定路径)
 *   T2 open("/tmp", O_RDONLY|O_DIRECTORY|O_CLOEXEC=0x90000) -> fd>=3
 *   T3 getdents64(fd) 流含 "hello.txt" (tmpfs 种子) + "." ".."
 *   T4 stat("/tmp/hello.txt") -> S_IFREG
 *   T5 open("/dev") getdents 含 "model0"/"tty" (设备枚举)
 */
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;

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
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}
static void wrok(const char *tag, int ok)
{
    wrstr(tag);
    wrstr(ok ? " ok\n" : " FAIL\n");
}

static long sstat(const char *p, void *buf) { return sy(4, (long)p, (long)buf, 0, 0, 0); }
static long sopen(const char *p, long flags) { return sy(2, (long)p, flags, 0, 0, 0); }
static long sgetdents(long fd, void *b, long n) { return sy(217, fd, (long)b, n, 0, 0); }
static long sclose(long fd) { return sy(3, fd, 0, 0, 0, 0); }

/* 在 dirent64 流中查找名字 (d_reclen 步进) */
static int dir_has(char *db, long n, const char *want)
{
    long off = 0;
    while (off + 24 <= n) {
        u16 reclen = *(u16 *)(db + off + 16);
        const char *nm = db + off + 19;
        int i = 0;
        while (nm[i] && want[i] && nm[i] == want[i]) i++;
        if (nm[i] == 0 && want[i] == 0)
            return 1;
        if (reclen == 0)
            break;
        off += reclen;
    }
    return 0;
}

static char st[128];
static char db[4096];

static void run(void)
{
    static const char h[] = "m132: dir semantics (W18)\n";
    wr(h, sizeof(h) - 1);
    int pass = 1;

    wrstr("m132: T1 stat /tmp\n");
    {
        long r = sstat("/tmp", st);
        u32 mode = *(u32 *)(st + 24);
        int ok = (r == 0) && ((mode & 0o170000) == 0o040000);
        wrok("m132:   dir", ok);
        if (!ok) pass = 0;
    }

    wrstr("m132: T2 open /tmp (O_DIRECTORY)\n");
    long fd = 0;
    {
        fd = sopen("/tmp", 0x90000); /* O_RDONLY|O_DIRECTORY(0x10000)|O_CLOEXEC(0x80000) */
        int ok = fd >= 3;
        wrok("m132:   open", ok);
        if (!ok) pass = 0;
    }

    wrstr("m132: T3 getdents64 /tmp\n");
    {
        long n = sgetdents(fd, db, sizeof(db));
        int ok = n > 0 && dir_has(db, n, ".") && dir_has(db, n, "..") && dir_has(db, n, "hello.txt");
        wrok("m132:   entries", ok);
        if (!ok) pass = 0;
        sclose(fd);
    }

    wrstr("m132: T4 stat /tmp/hello.txt\n");
    {
        long r = sstat("/tmp/hello.txt", st);
        u32 mode = *(u32 *)(st + 24);
        int ok = (r == 0) && ((mode & 0o170000) == 0o100000);
        wrok("m132:   reg", ok);
        if (!ok) pass = 0;
    }

    wrstr("m132: T5 open /dev enumerate\n");
    {
        long fd2 = sopen("/dev", 0x10000);
        int ok = fd2 >= 3;
        if (ok) {
            long n = sgetdents(fd2, db, sizeof(db));
            ok = n > 0 && dir_has(db, n, "tty") && dir_has(db, n, "model0");
            sclose(fd2);
        }
        wrok("m132:   dev", ok);
        if (!ok) pass = 0;
    }

    if (pass) {
        static const char m2[] = "m132: M132 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m132: M132 RESULT: FAIL\n";
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
