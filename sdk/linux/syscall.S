# syscall.S — W16: tcc 编译产物用的裸 syscall 桥 (C ABI: sy(n,a,b,c,d,e))
	.intel_syntax noprefix
	.text
	.global sy
	.type sy, @function
sy:
	mov	rax, rdi	# n -> rax
	mov	r10, r8		# d -> r10
	mov	r8, r9		# e -> r8
	mov	rdi, rsi	# a
	mov	rsi, rdx	# b
	mov	rdx, rcx	# c
	syscall
	ret
	.size sy, .-sy
	.section .note.GNU-stack,"",@progbits
