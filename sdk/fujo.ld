/* fujo.ld — SDK 样本链接脚本 (hello-fujo.rs)
 * 简单放置: 文本 1M 起, BSS 后置。仅供样本回归, 不用于内核。
 */
ENTRY(_start)

SECTIONS
{
    . = 1M;
    .text : { *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) *(COMMON) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
