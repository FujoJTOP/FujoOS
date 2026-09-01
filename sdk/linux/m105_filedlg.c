/* m105_filedlg.c — M105: 文件打开/保存对话框 (FJFS 后端)
 *
 * 用户态: fujokit kt_dialog (Save As / Open) + VFS 磁盘文件:
 *   保存: open /disk/hello.txt -> write 内容 -> close (flush 盘)
 *   打开: open O_RDONLY -> read 26B -> 对比
 * 前提: QEMU -drive 4MiB raw (fjfs 自动格式化)。
 */
#include "kit/fujokit.h"

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
static void wrdec(int v)
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

static const char content[] = "M105 file dialog content\n";
static const char path[] = "/disk/hello.txt";
static char buf[64];

void _start(void)
{
    static const char m1[] = "m105: file open/save dialog (FJFS)\n";
    wr(m1, sizeof(m1) - 1);

    /* Save As 对话框动作 */
    kt_dialog dlg;
    kt_dialog_init(&dlg, 300, 60, 320, 150, "Save As", "hello.txt");
    (void)kt_dialog_click(&dlg, 300 + 12 + 35, 60 + 150 - 27, 1); /* OK */
    long fd = sys3(2, (long)path, 0x401, 0); /* open("/disk/hello.txt", O_WRONLY|O_CREAT?) */
    long wn = sys3(1, fd, (long)content, sizeof(content) - 1);
    sys3(3, fd, 0, 0); /* close -> flush 盘 */

    /* Open 对话框动作 */
    kt_dialog_init(&dlg, 300, 60, 320, 150, "Open", "hello.txt");
    (void)kt_dialog_click(&dlg, 300 + 12 + 35, 60 + 150 - 27, 1); /* OK */
    long fd2 = sys3(2, (long)path, 0x0, 0); /* O_RDONLY */
    long rn = sys3(0, fd2, (long)buf, 64);
    sys3(3, fd2, 0, 0);

    wr("m105: fd=", 9);
    wrdec((int)fd);
    static const char s1[] = " wn=";
    wr(s1, 4);
    wrdec((int)wn);
    static const char s2[] = " rn=";
    wr(s2, 4);
    wrdec((int)rn);
    static const char s3[] = " text='";
    wr(s3, 8);
    wr(buf, rn < 0 ? 0 : (int)rn);
    static const char s4[] = "'\n";
    wr(s4, 2);

    int ok = fd >= 3 && wn == (long)(sizeof(content) - 1)
             && rn == (long)(sizeof(content) - 1)
             && buf[0] == 'M' && buf[5] == 'f' && buf[17] == 'c';
    if (ok) {
        static const char m2[] = "m105: M105 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char m3[] = "m105: M105 RESULT: FAIL\n";
        wr(m3, sizeof(m3) - 1);
    }
    sys3(60, 7, 0, 0);
    for (;;) {
    }
}


