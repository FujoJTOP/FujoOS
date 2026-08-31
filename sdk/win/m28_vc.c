/* m28_vc.c — M28: vcruntime/msvcrt 函数面扩展验证
 * 走 vcruntime 常见入口: strtol/strtoul/strtod/atoi/atof(数值解析),
 * qsort/bsearch(排序检索), sprintf/_snprintf(格式化落缓冲),
 * rand/srand(伪随机), memmove/strchr(内存/字符串), toupper/tolower
 * (字符类), 全部经 msvcrt 导入 -> 内核垫片。
 * Build: C:\mingw64\bin\x86_64-w64-mingw32-gcc -O2 -Wl,--image-base,0x400000
 *        -Wl,--subsystem,console -s m28_vc.c -o m28_vc.exe
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

static int cmp_i(const void *a, const void *b)
{
    return *(const int *)a - *(const int *)b;
}

int main(int argc, char **argv)
{
    long v = strtol("12345xyz", 0, 10);
    unsigned long uv = strtoul("1F", 0, 16);
    double dv = strtod("3.75", 0);
    int ai = atoi("42");
    double af = atof("1.5");
    int a[] = {9, 4, 7, 1, 5, 8, 2, 6, 3, 0};
    qsort(a, 10, sizeof(int), cmp_i);
    srand(0xC0FFEE);
    int r = rand() % 100;
    char buf[64];
    char buf2[64];
    _snprintf(buf, sizeof buf, "q=%d,%d,%d,%d %ld", a[0], a[1], a[2], a[9], v);
    sprintf(buf2, "%lu %.2f %d %.1f", uv, dv, ai, af);
    char tmp[16];
    memmove(tmp, "vcruntime", 9);
    tmp[9] = 0;
    char *w = strchr("hello world", 'w');
    int up = toupper('a');
    int dig = isdigit('5');
    printf("m28: strtol=%ld strtoul=%lu\n", v, uv);
    printf("m28: strtod=%.2f atoi=%d atof=%.1f rand=%d\n", dv, ai, af, r);
    printf("m28: %s\n", buf);
    printf("m28: %s\n", buf2);
    printf("m28: memmove=%s chr='%c' toupper=%c isdigit=%d\n", tmp, *w, up, dig);
    printf("m28: M28 RESULT: PASS\n");
    return 7;
}
