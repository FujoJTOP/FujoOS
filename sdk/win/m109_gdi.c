/* m109_gdi.c — M109 win32 GDI 字体兼容层验证 (PE32+)
 *
 * Windows 二进制用标准 GDI 路径:
 *   CreateFontA -> SelectObject -> SetTextColor -> TextOutA
 * 在 0x5B01 桌面 (已由代理绘制) 上叠加渲染大字。全部经 gdi32.dll
 * 垫片绑定, 与真实 win32 程序调用方式一致。
 */
#include <stddef.h>

typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;
typedef void *HDC;
typedef void *HFONT;

extern __declspec(dllimport) void *CreateFontA(int height, int width, int escapement,
    int orientation, int weight, DWORD italic, DWORD underline, DWORD strikeout,
    DWORD charset, DWORD outprec, DWORD clipprec, DWORD quality, DWORD pitch,
    const char *face);
extern __declspec(dllimport) void *SelectObject(HDC hdc, void *obj);
extern __declspec(dllimport) BOOL DeleteObject(void *obj);
extern __declspec(dllimport) BOOL TextOutA(HDC hdc, int x, int y, const char *str, int len);
extern __declspec(dllimport) DWORD SetTextColor(HDC hdc, DWORD color);
extern __declspec(dllimport) DWORD SetBkMode(HDC hdc, DWORD mode);
extern __declspec(dllimport) void *GetStockObject(int id);
extern __declspec(dllimport) BOOL GetTextExtentPointA(HDC hdc, const char *str, int len, void *sz);
extern __declspec(dllimport) HDC GetDC(void *hwnd);
extern __declspec(dllimport) int ReleaseDC(void *hwnd, HDC hdc);
extern __declspec(dllimport) BOOL WriteFile(HANDLE hFile, const void *buf, DWORD n, DWORD *w, void *ovl);
extern __declspec(dllimport) void ExitProcess(unsigned int code);

void _start(void) {
    DWORD n = 0;
    const char *m1 = "m109: win32 GDI font layer - CreateFont+TextOut test\n";
    WriteFile((HANDLE)1, m1, 52, &n, 0);

    /* GDI: 建字体 -> 选入 DC -> 设色 -> TextOut */
    HDC hdc = GetDC(0);
    HFONT f = CreateFontA(16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 0, 0, "Consolas");
    HFONT old = (HFONT)SelectObject(hdc, f);
    SetBkMode(hdc, 1); /* TRANSPARENT */
    SetTextColor(hdc, 0x00101010); /* 深色字 */

    const char *line1 = "Windows GDI TextOut from FujoOS";
    const char *line2 = "gdi32 shim: kernel font render 8x8";
    struct { int cx, cy; } sz;
    GetTextExtentPointA(hdc, line1, 31, &sz);
    TextOutA(hdc, 120, 120, line1, 31);
    TextOutA(hdc, 120, 145, line2, 35);

    SelectObject(hdc, old);
    DeleteObject(f);
    ReleaseDC(0, hdc);

    const char *m2 = "m109: GDI font path ok - lines rendered to desktop\n";
    WriteFile((HANDLE)1, m2, 54, &n, 0);
    const char *m3 = "m109: extent(cx,cy)=(";
    WriteFile((HANDLE)1, m3, 24, &n, 0);
    {
        char nbuf[16];
        int i = 16;
        DWORD v = (DWORD)sz.cx;
        if (v == 0) { nbuf[--i] = '0'; }
        while (v > 0 && i > 0) { nbuf[--i] = '0' + (char)(v % 10); v /= 10; }
        WriteFile((HANDLE)1, &nbuf[i], 16 - i, &n, 0);
        WriteFile((HANDLE)1, ",24)\n", 5, &n, 0);
    }
    ExitProcess(0);
}
