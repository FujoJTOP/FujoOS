/* hello2.c — single-file, no external object (tcc: no GOT/PLT) */
typedef long i64;
static i64 sy4(i64 n, i64 a, i64 b, i64 c)
{
    i64 r;
    asm volatile("syscall" : "=a"(r) : "a"(n), "D"(a), "S"(b), "d"(c) : "rcx", "r11", "memory");
    return r;
}
static const char MSG[] = "tcc-compiled hello from fujo!\n";
void _start(void)
{
    sy4(1, 1, (i64)MSG, sizeof(MSG) - 1);
    for (;;) { }
}
