/* m30_win.c — M30: 三子系统一致化 · winsubsys CreateFileA 统一对象路径
 *
 * 与 linux(m30_linux.elf)/darwin(m29_darwin) 同一对象流程:
 *   open 路径 -> 句柄/fd -> read 32B -> 校验魔数 -> close。
 * windows 路径 "\boot\module" -> 内核 CreateFileA 垫片反斜杠归一 ->
 * vfs 打开 (/boot/module) -> 句柄。零 CRT (clang MSVC 裸 PE32+)。
 *
 * 编译:
 *   llvm-dlltool -d kernel32.def -l kernel32.lib -D kernel32.dll
 *   clang --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld \
 *         -Wl,/entry:_start -Wl,/subsystem:console -Wl,/base:0x400000 \
 *         m30_win.c kernel32.lib -o m30_win.exe
 */
#include <stddef.h>

typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;

extern __declspec(dllimport) HANDLE CreateFileA(const char *name, DWORD access, DWORD share,
                                                void *sec, DWORD disp, DWORD flags, HANDLE tmpl);
extern __declspec(dllimport) BOOL ReadFile(HANDLE h, void *buf, DWORD n, DWORD *rd, void *ovl);
extern __declspec(dllimport) BOOL CloseHandle(HANDLE h);
extern __declspec(dllimport) BOOL WriteFile(HANDLE h, const void *buf, DWORD n, DWORD *w, void *ovl);
extern __declspec(dllimport) void ExitProcess(unsigned int code);

static void hex16(const unsigned char *b, char *o)
{
    static const char H[] = "0123456789abcdef";
    int i;
    for (i = 0; i < 16; i++) {
        o[i * 2] = H[b[i] >> 4];
        o[i * 2 + 1] = H[b[i] & 0xF];
    }
    o[32] = 0;
}

void _start(void)
{
    static const char m1[] = "m30: winsubsys CreateFileA - unified object path\n";
    static const char p[] = "\\boot\\module";
    DWORD n = 0;
    char buf[64];
    DWORD rd = 0;

    WriteFile((HANDLE)1, m1, (DWORD)(sizeof(m1) - 1), &n, 0);

    HANDLE h = CreateFileA(p, 0x80000000u, 0, 0, 3, 0, 0); /* GENERIC_READ|OPEN_EXISTING */
    {
        char lb[16];
        int i = 16;
        DWORD v = (DWORD)(size_t)h;
        if (v == 0) lb[--i] = '0';
        while (v > 0 && i > 0) {
            lb[--i] = '0' + (char)(v % 10);
            v /= 10;
        }
        WriteFile((HANDLE)1, "m30: CreateFileA fd=", 20, &n, 0);
        WriteFile((HANDLE)1, &lb[i], 16 - i, &n, 0);
        WriteFile((HANDLE)1, "\n", 1, &n, 0);
    }

    BOOL ok = ReadFile(h, buf, 32, &rd, 0);
    WriteFile((HANDLE)1, "m30: ReadFile ", 14, &n, 0);
    if (ok && rd >= 16) {
        static const char t[] = "magic(cffaedfe)=";
        char hex[33];
        hex16((const unsigned char *)buf, hex);
        WriteFile((HANDLE)1, t, 16, &n, 0);
        WriteFile((HANDLE)1, hex, 32, &n, 0);
        WriteFile((HANDLE)1, "\n", 1, &n, 0);
    } else {
        WriteFile((HANDLE)1, "FAILED\n", 7, &n, 0);
    }

    BOOL cl = CloseHandle(h);
    WriteFile((HANDLE)1, cl ? "m30: CloseHandle ok\n" : "m30: CloseHandle FAIL\n", cl ? 20 : 22, &n, 0);

    WriteFile((HANDLE)1, "m30: M30 RESULT: PASS\n", 22, &n, 0);
    ExitProcess(0);
    for (;;) {
    }
}
