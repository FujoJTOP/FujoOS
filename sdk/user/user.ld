/* user.ld — M1 用户态测试程序链接脚本 (单段 @0x400000) */
ENTRY(_start)

SECTIONS
{
    . = 0x400000;
    .text : { *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) *(COMMON) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
