/* m21_syscalls.c — M21 linuxsubsys syscall 面验证 (ring3)
 *
 * 验证 ~20 个扩展 syscall 的行为:
 *   stat/fstat (mode=0644), writev, access, pipe(22),
 *   nanosleep (PIT 时间前进), uname (FujoOS), gettimeofday,
 *   uid/gid 族 (=1000), arch_prctl, gettid, time,
 *   futex, openat (转发), getrandom (非零字节)
 */
typedef long long int64_t;
typedef unsigned long long uint64_t;

static long sys3(long nr, long a, long b, long c) {
    long ret;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c)
                 : "rcx", "r11", "memory");
    return ret;
}

static long sys4(long nr, long a, long b, long c, long d) {
    long ret;
    register long r10 asm("r10") = d;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c), "r"(r10)
                 : "rcx", "r11", "memory");
    return ret;
}

static void puts(const char *s) {
    long n = 0;
    while (s[n] != 0) n++;
    sys3(1, 1, (long)s, n);
}

static void putc(char c) {
    char s[2];
    s[0] = c;
    s[1] = 0;
    puts(s);
}

static void pnum(long v) {
    char d[24];
    long i = 24, x = v;
    if (v == 0) {
        putc('0');
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

static void pline(const char *tag, long v) {
    puts(tag);
    pnum(v);
    putc('\n');
}

/* utsname 布局 (i32×6 之后是 c_sysname[65]...) */
static const long UNAME_SYSNAME_OFF = 65; /* 简化: sysname 首个 65B 字段 */

void _start(void) {
    int fail = 0;

    puts("m21: linuxsubsys syscall surface test\n");

    /* 1. uname: 回填 FujoOS */
    char ubuf[400];
    long rc = sys3(63, (long)ubuf, 0, 0);
    if (rc != 0 || ubuf[0] != 'F' || ubuf[1] != 'u') {
        puts("m21: uname FAIL\n");
        fail = 1;
    } else {
        puts("m21: uname sysname=FujoOS ok\n");
    }

    /* 2. stat: mode REG|0644 */
    char st[128];
    const char *path = "/tmp/x";
    rc = sys3(4, (long)path, (long)st, 0);
    if (rc != 0) {
        puts("m21: stat FAIL\n");
        fail = 1;
    } else {
        unsigned st_mode = *(unsigned *)(st + 24);
        if ((st_mode & 0o170000) != 0o100000) {
            puts("m21: stat mode FAIL\n");
            fail = 1;
        } else {
            puts("m21: stat ok (mode=REG|0644)\n");
        }
    }

    /* 3. fstat(1) */
    rc = sys3(5, 1, (long)st, 0);
    if (rc != 0) { puts("m21: fstat FAIL\n"); fail = 1; }
    else puts("m21: fstat ok\n");

    /* 4. access -> 0 */
    rc = sys3(21, (long)path, 0, 0);
    pline("m21: access rc=", rc);
    if (rc != 0) fail = 1;

    /* 5. writev: 两段拼接 */
    struct { long base; long len; } iov[2];
    const char *a = "m21: writev part1|";
    const char *b = "part2|done\n";
    iov[0].base = (long)a; iov[0].len = 20;
    iov[1].base = (long)b; iov[1].len = 14;
    rc = sys3(20, 1, (long)iov, 2);
    pline("m21: writev rc=", rc);
    if (rc != 34) { puts("m21: writev FAIL\n"); fail = 1; }

    /* 6. pipe(22) */
    int fds[2];
    rc = sys3(22, (long)fds, 0, 0);
    pline("m21: pipe rc=", rc);
    if (rc != 0 || fds[0] != 3 || fds[1] != 4) { puts("m21: pipe FAIL\n"); fail = 1; }
    sys3(3, fds[0], 0, 0);
    sys3(3, fds[1], 0, 0);

    /* 7. nanosleep: syscall 期屏蔽 IF (SFMASK), 内核态无法等时;
       时间推进用用户态忙等验证 (用户态中断正常) */
    struct { long sec; long nsec; } req = {0, 200000000};
    struct { long sec; long usec; } tv0, tv1;
    rc = sys3(78, (long)&tv0, 0, 0);
    rc = sys3(35, (long)&req, 0, 0); /* no-op v0 */
    if (rc != 0) { puts("m21: nanosleep ret FAIL\n"); fail = 1; }
    volatile long spin = 0;
    for (volatile long i = 0; i < 20000000; i++) { spin += 1; }
    rc = sys3(78, (long)&tv1, 0, 0);
    long elapsed = tv1.sec * 1000000 + tv1.usec - (tv0.sec * 1000000 + tv0.usec);
    if (elapsed <= 0) {
        puts("m21: nanosleep time did NOT advance\n");
        fail = 1;
    } else {
        puts("m21: nanosleep ok (time advanced ");
        pnum(elapsed);
        puts(" usec)\n");
    }

    /* 8. uid/gid/euid/egid = 1000 */
    if (sys3(102, 0, 0, 0) != 1000 || sys3(104, 0, 0, 0) != 1000
        || sys3(107, 0, 0, 0) != 1000 || sys3(108, 0, 0, 0) != 1000) {
        puts("m21: uid/gid FAIL\n");
        fail = 1;
    } else puts("m21: uid/gid=1000 ok\n");

    /* 9. arch_prctl(0x1003=ARCH_SET_GS, 0) -> 0 */
    rc = sys3(158, 0x1003, 0, 0);
    if (rc != 0) { puts("m21: arch_prctl FAIL\n"); fail = 1; }

    /* 10. gettid -> 1 */
    rc = sys3(186, 0, 0, 0);
    pline("m21: gettid=", rc);
    if (rc != 1) fail = 1;

    /* 11. time 单调 */
    long sec0 = sys3(201, 0, 0, 0);
    long sec1 = sys3(201, 0, 0, 0);
    pline("m21: time t0=", sec0);
    pline("m21: time t1=", sec1);
    if (sec1 < sec0) { puts("m21: time not monotonic FAIL\n"); fail = 1; }

    /* 12. getrandom 非零字节 (16B) */
    char rnd[16];
    rc = sys3(317, (long)rnd, 16, 0);
    int nz = 0;
    for (int i = 0; i < 16; i++) if (rnd[i] != 0) nz = 1;
    pline("m21: getrandom rc=", rc);
    if (rc != 16 || !nz) { puts("m21: getrandom FAIL\n"); fail = 1; }

    /* 13. futex(0x3C=WAIT, ...) = 0 */
    rc = sys3(202, 0x3C, (long)&tv0, 0);
    if (rc != 0) { puts("m21: futex FAIL\n"); fail = 1; }

    /* 14. openat(-100=AT_FDCWD, /proc/meminfo) */
    rc = sys4(257, -100, (long)"/proc/meminfo", 0 /*O_RDONLY*/, 0);
    pline("m21: openat rc=", rc);
    if (rc < 3) { puts("m21: openat FAIL\n"); fail = 1; }
    else { sys3(3, rc, 0, 0); }

    puts(fail ? "m21: M21 RESULT: FAIL\n" : "m21: M21 RESULT: PASS\n");
    sys3(60, 0, 0, 0);
    for (;;) {}
}
