/* user-high.ld — M108: 窗口程序链接脚本 (高地址 0x1000000, 与桌面代理
 * 0x400000 不冲突; 共享地址空间多程序共处) */
ENTRY(_start)

SECTIONS
{
    . = 0x1000000;
    .text : { *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) *(COMMON) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
