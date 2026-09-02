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

// ---------------------------------------------------------------------------
// M65: 每核 TSS / 中断注入优化 v0
//
//  - 双 TSS (GDT 槽 5/6=TSS0 0x28, 7/8=TSS1 0x38), 每核独立 RSP0
//    (核0=0x300000, 核1=0x3A0000) —— gdt.rs。
//  - LAPIC 探测: 基址 0xFEE00000 读 ID 寄存器 (offset 0x20 bits 31..24)
//    —— QEMU TCG 虚拟 APIC 提供真实 CPU 标识。
//  - 中断注入优化 v0: IRQ_ROUTE 掩码 (默认 3=双核轮转); 每次 PIT 中断
//    按掩码归属核桶 (3 → 轮转 0/1; 1 → 核0; 2 → 核1)。
// ---------------------------------------------------------------------------

static mut IRQ_ROUTE: u64 = 3;
static mut IRQ_INJ: u64 = 0;
static mut IRQ_R0: u64 = 0;
static mut IRQ_R1: u64 = 0;

fn lapic_id() -> u32 {
    // CPUID leaf 1 EBX[31..24] = 初始 APIC ID (BSP=0)。LAPIC MMIO
    // (0xFEE00000) 未映射进 boot 页表 (M65 已知限制, 记录见文档 14);
    // v0 以 CPUID ID 为核标识 —— QEMU TCG 下与虚拟 LAPIC ID 一致。
    let mut b = [0u32; 4];
    unsafe { fujo_cpuid_leaf1(b.as_mut_ptr()) };
    (b[1] >> 24) & 0xFF
}

/// PIT 中断侧钩子 (sched 桩每 tick 调; 先于任何切换逻辑)。
pub fn intr_note() {
    crate::irq::note(); // M67: 中断合并/成本记账
    unsafe {
        IRQ_INJ += 1;
        let m = IRQ_ROUTE & 3;
        let core = match m {
            3 => IRQ_INJ % 2,
            0 => 2, // 全禁: 不入桶 (保持总和语义)
            _ => m - 1, // 1 → 0, 2 → 1
        };
        if core == 0 {
            IRQ_R0 += 1;
        } else if core == 1 {
            IRQ_R1 += 1;
        }
    }
}

/// 0x6B01: 当前核 id (LAPIC).
pub fn fujo_core_id() -> i64 {
    lapic_id() as i64
}

/// 0x6B02: tss_info(ptr) — u64×3: (tss0_rsp0, tss1_rsp0, gdt_limit)。
pub fn fujo_tss_info(ptr: u64) -> i64 {
    unsafe {
        let (a, b) = crate::gdt::tss_rsp0s();
        let w = ptr as *mut u64;
        w.write(a);
        w.add(1).write(b);
        w.add(2).write(16 * 8 - 1);
    }
    0
}

/// 0x6B04: irq_route(mask) — 中断目标核掩码。
pub fn fujo_irq_route(mask: u64) -> i64 {
    // M116: 中断域门 —— 域无 IRQ 权限禁止改路由。
    if !crate::capability::dom_irq_ok() {
        crate::serial::write_line("irq  : deny route (domain irq off)");
        return -1;
    }
    unsafe {
        IRQ_ROUTE = mask & 3;
    }
    0
}

/// 0x6B05: irq_stats(ptr) — u32×4: (lapic_id, r0, r1, inj)。
pub fn fujo_irq_stats(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u32;
        w.write(lapic_id());
        w.add(1).write(IRQ_R0 as u32);
        w.add(2).write(IRQ_R1 as u32);
        w.add(3).write(IRQ_INJ as u32);
    }
    0
}
