//! smp.rs — M64: 多核并行 v0 (探测 / 调度亲和 / 负载均衡统计)
//!
//! 范围 (M64): CPUID 核探测 (leaf 1 EBX[23..16]) + 亲和位图 (每任务
//! AFF[tid], 默认 0xFF) + 调度侧核归属统计 (负载均衡 v0: 每任务每次
//! PIT 切换按其亲和最低置位 bit 记入该核负载)。单 PIT 时钟源下先记录
//! 策略/统计; 真 SMP 启动 (多 APIC 定时器/每核 TSS) 由 M65 承接。
//!
//! 接口: 0x6A01 aff_set(tid, mask) / 0x6A02 aff_get(tid) /
//!       0x6A03 smp_info(ptr) 写 u32×4: (ncpu, aff_n, locks, unknown)
//!       0x6A04 smp_stats(ptr) 写 u32×4: (ncpu, core0_count, core1_count, switches)

use crate::serial;

// CPUID leaf 1 桥 (rbx - LLVM 保留)。
core::arch::global_asm!(r#"
    .text
    .global fujo_cpuid_leaf1
    .p2align 4
fujo_cpuid_leaf1:
    push rbx
    push rcx
    push rdx
    mov rax, 1
    xor rcx, rcx
    cpuid
    mov [rdi + 0], eax
    mov [rdi + 4], ebx
    mov [rdi + 8], edx
    mov [rdi + 12], ecx
    pop rdx
    pop rcx
    pop rbx
    ret
"#);

extern "C" {
    fn fujo_cpuid_leaf1(buf: *mut u32);
}

static mut NCPU: u32 = 1;
static mut AFF: [u8; 8] = [0xFF; 8]; // 每任务亲和位图 (仅低 2 bit 有效 v0)
static mut CORE0: u64 = 0;
static mut CORE1: u64 = 0;
static mut SWITCHES: u64 = 0;

/// 探测并缓存核数 (启动时调一次; 并行失效时不重复探测)。
pub fn init() {
    let mut b = [0u32; 4];
    unsafe { fujo_cpuid_leaf1(b.as_mut_ptr()) };
    let logical = ((b[1] >> 16) & 0xFF) + 1; // EBX[23..16] 逻辑核数 - 1
    unsafe {
        NCPU = logical.max(1).min(2); // v0: 上限 2 核统计桶
    }
    serial::write_str("smp  : cpuid logical CPUs = ");
    let nc = unsafe { NCPU };
    serial::write_str(if nc >= 2 { "2 (affinity v0 armed)" } else { "1 (single-core mode)" });
    serial::write_line("");
}

pub fn ncpu() -> u32 {
    unsafe { NCPU }
}

// ---------------------------------------------------------------------------
// 亲和位图
// ---------------------------------------------------------------------------

pub fn aff_set(tid: u64, mask: u64) -> i64 {
    let t = (tid as usize).min(7);
    unsafe { AFF[t] = (mask as u8) & 0x03 };
    0
}

pub fn aff_get(tid: u64) -> i64 {
    let t = (tid as usize).min(7);
    unsafe { AFF[t] as i64 }
}

// ---------------------------------------------------------------------------
// 负载均衡 v0: 每次用户态切换把任务核归属记入统计。
// 核选择: 亲和位图最低置位 bit (0xFF → 轮换, 伪随机取 task id & ncpu)。
// ---------------------------------------------------------------------------

pub fn balance_task(tid: usize) {
    let nc = unsafe { NCPU } as u64;
    if nc < 2 {
        unsafe { CORE0 += 1 };
        return;
    }
    unsafe {
        let m = AFF[tid % 8] as u64;
        let core = if m == 0xFF { (tid as u64) % 2 } else { m.trailing_zeros() as u64 };
        if core == 0 {
            CORE0 += 1;
        } else {
            CORE1 += 1;
        }
    }
}

pub fn note_switch(tid: usize) {
    unsafe {
        SWITCHES += 1;
    }
    balance_task(tid);
}

/// 0x6A04
pub fn fujo_smp_stats(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u32;
        w.write(NCPU);
        w.add(1).write(CORE0 as u32);
        w.add(2).write(CORE1 as u32);
        w.add(3).write(SWITCHES as u32);
    }
    0
}
