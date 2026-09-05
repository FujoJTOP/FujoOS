/* fujo_main.c — 散件工厂测试驱动 (工具功能验证: 标准向量 + 多块 + 文件输入)
 * 拼装进单编译单元 (见 tools/make_scatter_tool.py)。
 */
#include "fujo_libc.h"
#include "sha256.h"

static int check(const char *name, const char *got, const char *want)
{
    int ok = strcmp(got, want) == 0;
    printf("sfx: %s = %s (want %s) %s\n", name, got, want, ok ? "OK" : "BAD");
    return ok;
}

/* __attribute__((noreturn)) 保持与 mbuild 一致 (无 CRT, 入口 = _start) */
void _start(void)
{
    char hex[65];
    int pass = 1;

    /* FIPS 180-4 标准向量: "abc" */
    sha256_hex("abc", 3, hex);
    pass &= check("abc", hex,
                  "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    /* 空串 */
    sha256_hex("", 0, hex);
    pass &= check("empty", hex,
                  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

    /* 多块 (>64B: 110B 跨两块) */
    {
        static const char data[] = "abcdefghijklmnopqrstuvwxyz0123456789"
                                   "_abcdefghijklmnopqrstuvwxyz0123456789_"
                                   "abcdefghijklmnopqrstuvwxyz0123456789";
        sha256_hex(data, sizeof(data) - 1, hex);
        pass &= check("data110", hex,
                      "b7121997d66bf89f5078cb7229faf5c7f56ea1a1efd222686500a69de199f1dd");
    }

    /* 文件输入: 读 /tmp/sfx-data, hash, 打印 (散件工厂案例的"文件树"路径) */
    {
        int fd = fj_open("/tmp/sfx-data", O_RDONLY);
        if (fd >= 0) {
            char buf[256];
            long n = fj_read(fd, buf, sizeof(buf));
            fj_close(fd);
            if (n > 0) {
                sha256_hex(buf, (size_t)n, hex);
                printf("sfx: file /tmp/sfx-data (%ldB) = %s\n", n, hex);
            }
        } else {
            printf("sfx: no /tmp/sfx-data (skip file case)\n");
        }
    }

    if (pass) {
        printf("SFACTORY RESULT: PASS\n");
    } else {
        printf("SFACTORY RESULT: FAIL\n");
    }
    fj_exit(0);
    for (;;) {}
}
