/*
 * fujo-kernel 链接脚本
 *
 * 内存布局（与 kernel/boot/gen_stub32.py 的常量严格一致）:
 *   0x0010_0000  .multiboot   12 B    multiboot v1 头（文件前 8 KiB 内）
 *   0x0010_1000  .boot_blob   28 KiB  32 位引导桩 + 前 1 GiB 恒等页表
 *   0x0020_0000  .text.entry  长模式 Rust 入口（far-jump 目标固定在此）
 *   ...          内核本体（文本/只读/数据/BSS）
 *   0x0030_0000  内核栈顶（引导桩设定 rsp，向下 64 KiB）
 *
 * 说明: 生产版（M1）改用 Limine/UEFI 引导并切换到 5 级页表 + 高半区；
 *       本脚本为 Windows 开发环 QEMU 快速迭代保留。
 */

ENTRY(_start)

SECTIONS
{
  . = 1M;

  .multiboot : { KEEP(*(.multiboot)) }

  . = 1M + 0x1000;
  .boot_blob : { KEEP(*(.boot_blob)) }

  . = 0x200000;
  .text.entry : { KEEP(*(.text.entry)) }

  .text   : { *(.text*) }
  .rodata : { *(.rodata*) }
  .data   : { *(.data*) }
  .bss    : { *(.bss*) *(COMMON) }

  . = ALIGN(16);
  __kernel_end = .;

  /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) *(.llvm*) }
}
