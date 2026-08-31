/* m27_mingw.c — M27: real mingw-w64 console program (CRT startup + msvcrt shims)
 * Build: C:\mingw64\bin\x86_64-w64-mingw32-gcc -O2 -Wl,--image-base,0x400000
 *        -Wl,--subsystem,console -s m27_mingw.c -o m27_mingw.exe
 * 原生 mingw CRT (mainCRTStartup) 全流程: TEB(GS:0x30) -> __set_app_type ->
 * __getmainargs -> argv-dup -> _initterm -> main -> printf/malloc/strcpy/exit。
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv)
{
    char *p;
    printf("m27: mingw console app alive\n");
    printf("m27: argc=%d\n", argc);
    printf("m27: argv[0]=%s\n", argv[0]);
    p = malloc(64);
    if (!p) { printf("m27: malloc FAIL\n"); return 1; }
    strcpy(p, "heap works");
    printf("m27: %s\n", p);
    free(p);
    printf("m27: M27 RESULT: PASS\n");
    return 7;
}
