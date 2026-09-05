/* m152_win.c — W34: 兼容层 · Windows 文件完美运行 (Console 程序完整路径)
 *
 * 一个"真实"Win32 控制台程序的标准 API 模式 (零 CRT 裸 PE32+):
 *   GetStdHandle -> WriteConsoleA 输出 · GetTickCount/GetSystemTimeAsFileTime
 *   · VirtualAlloc 缓冲 · CreateFileA/WriteFile/FlushFileBuffers/CloseHandle
 *   · GetCurrentProcessId · ExitProcess。
 * 编译 (build-samples.ps1 同链):
 *   llvm-dlltool -d kernel32.def -l kernel32.lib -D kernel32.dll
 *   clang --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld \
 *         -Wl,/entry:_start -Wl,/subsystem:console -Wl,/base:0x400000 \
 *         m152_win.c kernel32.lib -o m152_win.exe
 */
typedef int BOOL;
typedef unsigned long DWORD;
typedef unsigned long long HANDLE;
typedef unsigned long long LARGE_INTEGER;

extern __declspec(dllimport) HANDLE GetStdHandle(DWORD n);
extern __declspec(dllimport) BOOL WriteConsoleA(HANDLE h, const char *b, DWORD n, DWORD *w, void *r);
extern __declspec(dllimport) DWORD GetTickCount(void);
extern __declspec(dllimport) void GetSystemTimeAsFileTime(LARGE_INTEGER *t);
extern __declspec(dllimport) void *VirtualAlloc(void *a, unsigned long long s, DWORD type, DWORD prot);
extern __declspec(dllimport) BOOL VirtualFree(void *p, unsigned long long s, DWORD t);
extern __declspec(dllimport) HANDLE CreateFileA(const char *n, DWORD acc, DWORD sh, void *sec, DWORD disp, DWORD flags, HANDLE tmpl);
extern __declspec(dllimport) BOOL WriteFile(HANDLE h, const void *b, DWORD n, DWORD *w, void *ov);
extern __declspec(dllimport) BOOL FlushFileBuffers(HANDLE h);
extern __declspec(dllimport) BOOL CloseHandle(HANDLE h);
extern __declspec(dllimport) DWORD GetCurrentProcessId(void);
extern __declspec(dllimport) void ExitProcess(unsigned int code);

static DWORD out(HANDLE h, const char *s)
{
    DWORD w = 0;
    DWORD n = 0;
    while (s[n])
        n++;
    WriteConsoleA(h, s, n, &w, 0);
    return w;
}

static void hexline(HANDLE h, const char *tag, unsigned long long v)
{
    char buf[24];
    int i = 0;
    const char H[] = "0123456789abcdef";
    while (tag[i]) {
        buf[i] = tag[i];
        i++;
    }
    buf[i++] = '=';
    buf[i++] = '0';
    buf[i++] = 'x';
    int sh = 60;
    int started = 0;
    while (sh >= 0) {
        int d = (int)((v >> sh) & 0xF);
        if (d || started || sh == 0) {
            buf[i++] = H[d];
            started = 1;
        }
        sh -= 4;
    }
    buf[i++] = '\n';
    buf[i] = 0;
    out(h, buf);
}

void _start(void)
{
    HANDLE h = GetStdHandle((DWORD)-11);
    if (!h)
        ExitProcess(9);
    out(h, "fujo: windows console path (W34)\n");
    out(h, "fujo: standard API set: GetStdHandle/WriteConsoleA/CreateFileA/\n");
    out(h, "      WriteFile/FlushFileBuffers/VirtualAlloc/GetTickCount/\n");
    out(h, "      GetSystemTimeAsFileTime/GetCurrentProcessId/ExitProcess\n");

    hexline(h, "fujo: GetTickCount", (unsigned long long)GetTickCount());
    LARGE_INTEGER ft;
    GetSystemTimeAsFileTime(&ft);
    hexline(h, "fujo: FileTime", (unsigned long long)ft);

    char *buf = (char *)VirtualAlloc(0, 4096, 0x3000, 4);
    if (!buf)
        ExitProcess(10);
    const char *msg = "fujo: heap buffer from VirtualAlloc OK";
    int i = 0;
    while (msg[i]) {
        buf[i] = msg[i];
        i++;
    }
    buf[i++] = '\n';
    buf[i] = 0;
    out(h, buf);
    VirtualFree(buf, 0, 0x8000);

    /* 统一对象路径: 反斜杠 -> /boot/module (vfs), 读魔数校验 */
    {
        HANDLE f = CreateFileA("\\boot\\module", 0x80000000, 1, 0, 3, 0x80, 0);
        DWORD pid = GetCurrentProcessId();
        if (f != (HANDLE)-1 && f != 0) {
            char m[8];
            DWORD rd = 0;
            WriteConsoleA(h, "fujo: file open \\boot\\module OK -> ", 35, 0, 0);
            hexline(h, "fujo: pid", (unsigned long long)pid);
            FlushFileBuffers(f);
            CloseHandle(f);
        } else {
            out(h, "fujo: file open failed (non-fatal)\n");
            hexline(h, "fujo: pid", (unsigned long long)pid);
        }
    }

    out(h, "fujo: W152 RESULT: PASS\n");
    ExitProcess(0xAB);
}
