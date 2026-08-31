//! hvm.rs — M57: 虚拟化加速探测 (KVM/TCG/其它) + 基准对照规程
//!
//! cpuid 0x40000000 (hypervisor leaf): EBX/ECX/EDX = 厂家串
//!   "TCGTCGTCG" (QEMU 软件) / "KVMKVMKVM" (KVM) / "Microsoft Hv"
//! 0x6401 accel_info(ptr: 16B) -> 写 12 字节厂家 + NUL, 返回 accel id
//!   (0=TCG 1=KVM 2=其它; cpuid ebx 经全局 asm 桥 — LLVM 保留 rbx)。

extern "C" {
    fn fujo_cpuid_hv(out: *mut u32); // 调 cpuid 0x40000000, 写 4×u32 (eax,ebx,ecx,edx)
}

core::arch::global_asm!(r#"
    .text
    .p2align 4
    .global fujo_cpuid_hv
fujo_cpuid_hv:
    push rbx
    push rcx
    push rdx
    mov eax, 0x40000000
    xor ecx, ecx
    cpuid
    mov [rdi], eax
    mov [rdi+4], ebx
    mov [rdi+8], ecx
    mov [rdi+12], edx
    pop rdx
    pop rcx
    pop rbx
    ret
"#);

/// 0x6401: accel_info(ptr) -> accel id (0=TCG 1=KVM 2=其它)。
pub fn fujo_accel_info(ptr: u64) -> i64 {
    let mut out = [0u32; 4];
    unsafe {
        fujo_cpuid_hv(out.as_mut_ptr());
    }
    let ebx = out[1];
    let ecx = out[2];
    let edx = out[3];
    let vendor: [u8; 12] = [
        (ebx & 0xFF) as u8,
        ((ebx >> 8) & 0xFF) as u8,
        ((ebx >> 16) & 0xFF) as u8,
        ((ebx >> 24) & 0xFF) as u8,
        (ecx & 0xFF) as u8,
        ((ecx >> 8) & 0xFF) as u8,
        ((ecx >> 16) & 0xFF) as u8,
        ((ecx >> 24) & 0xFF) as u8,
        (edx & 0xFF) as u8,
        ((edx >> 8) & 0xFF) as u8,
        ((edx >> 16) & 0xFF) as u8,
        ((edx >> 24) & 0xFF) as u8,
    ];
    let accel = if vendor.starts_with(b"TCGTCGTCG") {
        0
    } else if vendor.starts_with(b"KVMKVMKVM") {
        1
    } else if vendor.starts_with(b"Microsoft Hv") {
        2
    } else {
        2
    };
    unsafe {
        for i in 0..12 {
            ((ptr + i as u64) as *mut u8).write(vendor[i]);
        }
        ((ptr + 12) as *mut u8).write(0);
    }
    accel
}
