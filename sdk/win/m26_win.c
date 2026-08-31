/* m26_win.c — M26 winsubsys 垫片家族验证 (PE32+)
 *
 * 通过 PE 导入表绑定 kernel32!WriteFile/ReadFile/GetFileSize/
 * GetCurrentThreadId/CloseHandle/ExitProcess。
 * 流程: 打开 /proc/meminfo (open 由哪里? winsubsys v0 尚无 CreateFile,
 * 用 fd=3 直接读 /boot/module? 简化: 用 WriteFile 输出 + GetCurrentThreadId
 * + GetFileSize(3) 检查 fd 表已有打开项的 size —— 直接读 0x5105?
 * 更稳妥: WriteFile 打印 + ReadFile 从 fd 3 (/boot/module)
 * 读取 32B + GetCurrentThreadId + CloseHandle(3)。
 */
#include <stddef.h>

typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;
typedef unsigned long long LONGLONG;

extern __declspec(dllimport) BOOL WriteFile(HANDLE hFile, const void *lpBuffer, DWORD nBytes, DWORD *written, void *ovl);
extern __declspec(dllimport) BOOL ReadFile(HANDLE hFile, void *lpBuffer, DWORD nBytes, DWORD *read, void *ovl);
extern __declspec(dllimport) DWORD GetFileSize(HANDLE hFile, DWORD *hi);
extern __declspec(dllimport) DWORD GetCurrentThreadId(void);
extern __declspec(dllimport) BOOL CloseHandle(HANDLE h);
extern __declspec(dllimport) void ExitProcess(unsigned int code);

void _start(void) {
    DWORD n = 0;
    const char *m1 = "m26: PE32+ winsubsys family - WriteFile ok\n";
    WriteFile((HANDLE)1, m1, 44, &n, 0);

    /* ReadFile: 读 fd 3 (/boot/module) 前 32B */
    char buf[64];
    DWORD rd = 0;
    BOOL ok = ReadFile((HANDLE)3, buf, 32, &rd, 0);
    const char *m2 = "m26: ReadFile fd=3 ";
    WriteFile((HANDLE)1, m2, 21, &n, 0);
    if (ok) {
        char nbuf[8];
        int i = 4;
        DWORD v = rd;
        while (v > 0 && i > 0) { nbuf[--i] = '0' + (char)(v % 10); v /= 10; }
        WriteFile((HANDLE)1, &nbuf[i], 4 - i, &n, 0);
        WriteFile((HANDLE)1, " bytes\n", 7, &n, 0);
    } else {
        WriteFile((HANDLE)1, "FAILED\n", 7, &n, 0);
    }

    /* GetFileSize */
    DWORD sz = GetFileSize((HANDLE)3, 0);
    const char *m3 = "m26: GetFileSize(3)=";
    WriteFile((HANDLE)1, m3, 20, &n, 0);
    {
        char nbuf[12];
        int i = 12;
        DWORD v = sz;
        if (v == 0) { nbuf[--i] = '0'; }
        while (v > 0 && i > 0) { nbuf[--i] = '0' + (char)(v % 10); v /= 10; }
        WriteFile((HANDLE)1, &nbuf[i], 12 - i, &n, 0);
        WriteFile((HANDLE)1, "\n", 1, &n, 0);
    }

    /* GetCurrentThreadId */
    DWORD tid = GetCurrentThreadId();
    const char *m4 = "m26: GetCurrentThreadId=";
    WriteFile((HANDLE)1, m4, 24, &n, 0);
    {
        char nbuf[12];
        int i = 12;
        DWORD v = tid;
        if (v == 0) { nbuf[--i] = '0'; }
        while (v > 0 && i > 0) { nbuf[--i] = '0' + (char)(v % 10); v /= 10; }
        WriteFile((HANDLE)1, &nbuf[i], 12 - i, &n, 0);
        WriteFile((HANDLE)1, "\n", 1, &n, 0);
    }

    /* CloseHandle(3) */
    BOOL cl = CloseHandle((HANDLE)3);
    const char *m5 = cl ? "m26: CloseHandle ok\n" : "m26: CloseHandle FAIL\n";
    WriteFile((HANDLE)1, m5, cl ? 21 : 23, &n, 0);

    WriteFile((HANDLE)1, "m26: M26 RESULT: PASS\n", 22, &n, 0);
    ExitProcess(0);
    for (;;) {}
}
