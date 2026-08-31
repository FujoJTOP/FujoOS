/* hello_win.c — FujoOS winsubsys v0 样例 (PE32+, 无 windows.h)
 *
 * 一个真实的 Windows 控制台程序: 通过 PE 导入表绑定 kernel32!WriteFile /
 * kernel32!ExitProcess。FujoOS 内核 PE 装载器解析导入表并把 IAT 槽位绑定
 * 到用户态垫片蹦床(宏原生 syscall), 从而在 ring3 直接运行 PE。
 *
 * 编译 (scripts/build-kernel.ps1 自动执行):
 *   llvm-dlltool -d kernel32.def -l kernel32.lib -D kernel32.dll
 *   clang --target=x86_64-pc-windows-msvc -O2 -nostdlib -fuse-ld=lld \
 *         -Wl,/entry:_start -Wl,/subsystem:console -Wl,/base:0x400000 \
 *         hello_win.c kernel32.lib -o hello_win.exe
 */
#include <stddef.h>

/* Win32 ABI 原型 (自声明, 不依赖 windows.h) */
typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;

extern __declspec(dllimport) BOOL WriteFile(
    HANDLE hFile,
    const void *lpBuffer,
    DWORD nNumberOfBytesToWrite,
    DWORD *lpNumberOfBytesWritten,
    void *lpOverlapped);
extern __declspec(dllimport) void ExitProcess(unsigned int uExitCode);

void _start(void) {
    static const char msg[] =
        "user : PE32+ program live — kernel32!WriteFile via shim\n";
    DWORD written = 0;
    WriteFile((HANDLE)1, msg, (DWORD)(sizeof(msg) - 1), &written, 0);
    ExitProcess(0);
    for (;;) {
    }
}
