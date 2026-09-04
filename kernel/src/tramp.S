/* tramp.S — W17: SMP AP 启动 trampoline (链接基址 0x8000; 拷贝到物理 0x8000 执行)
 *
 * 布局 (固定 data slot):
 *   +0x200 cr3_slot (u32, phys CR3 低 32 位)      BSP 回填
 *   +0x204 entry_slot (u32, ap_entry 低 32 位)    BSP 回填
 * 流程: cli -> lgdt [bp+gdtr] -> PE -> PAE -> EFER.LME -> CR3 -> PG
 *       -> push 0x08; push [entry]; retf (到 64 位 ap_entry)
 */
	.code16
	.section .text
	.global tramp_start
tramp_start:
	cli
	mov  $0x8000, %bp
	movl $0xCAFEBABE, 0x230(%bp)   # 执行标记 (trampoline 首指令被执行)
	xor  %ax, %ax
	mov  %ax, %ds
	mov  $0x8000, %bp
	.byte 0x0f, 0x01, 0x56        # lgdt [bp + disp8]
	.byte (tramp_gdtr - tramp_start)
pm_stub:
	mov  %cr0, %eax
	or   $1, %eax
	mov  %eax, %cr0
	mov  %cr4, %eax
	or   $0x20, %eax
	mov  %eax, %cr4
	mov  $0xC0000080, %ecx
	rdmsr
	or   $0x100, %eax
	wrmsr
	movl 0x200(%bp), %eax         # cr3
	mov  %eax, %cr3
	mov  %cr0, %eax
	or   $0x80000001, %eax
	mov  %eax, %cr0
	pushw $0x08
	movl 0x204(%bp), %eax         # ap_entry
	pushl %eax
	movl $0xDEADBEEF, 0x220(%bp)  # 完成标记 (trampoline 已走到)
	.byte 0x66, 0xcb              # lretw: 32 位操作数 retf -> pop 32-bit EIP + 16-bit CS
	                              # 进入 GDT[1] (L=1, 64-bit code)。裸 retf=16 位只 pop 4B
	                              # (push 是 6B) -> 栈错位跳垃圾 -> 空 IDT 三重故障 (W17b 实证)。

	.global tramp_gdtr
tramp_gdtr:
	.word 24
	.long 0x8000 + (tramp_gdt - tramp_start)
	.global tramp_gdt
tramp_gdt:
	.quad 0
	.quad 0x00AF9B000000FFFF
	.quad 0x00CF93000000FFFF

	.space 0x200 - (. - tramp_start)
	.global cr3_slot
cr3_slot:
	.long 0
	.global entry_slot
entry_slot:
	.long 0
	.space 0x40
