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

// ---------------------------------------------------------------------------
// W17: AP 启动 (SMP) —— 映射 LAPIC MMIO + 拷贝 trampoline@0x8000 + SIPI
// ---------------------------------------------------------------------------

pub static mut AP_ONLINE: bool = false;
static AP_STACK: u64 = 0x3A0000; // TSS1.rsp0 同栈区 (gdt.rs M65)

fn mmio_wr32(addr: u64, v: u32) {
    unsafe {
        core::arch::asm!("mov [{}], eax", in(reg) addr, in("eax") v, options(nomem));
    }
}

fn mmio_rd32(addr: u64) -> u32 {
    let v: u32;
    unsafe {
        core::arch::asm!("mov eax, [{}]", in(reg) addr, out("eax") v, options(nomem));
    }
    v
}

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

/// W17: TCG 忙等 (INIT->SIPI 间隔用; PIT 被屏蔽时唯一可靠)。
fn delay_ticks(n: u64) {
    let t0 = rdtsc();
    while rdtsc().wrapping_sub(t0) < n {
        core::hint::spin_loop();
    }
}

/// W17: 映射 LAPIC MMIO (0xFEE00000) 进恒等页表 (PML4[0].PDPT[3].PD[503]→PT[0])。
/// boot 恒等只到 1GiB; 0xFEE00000 = 3GB+0x2E00000 → PDPT[3], PD 索引 0x7F7 & 0x1FF = 503。
fn map_lapic() -> bool {
    unsafe {
        let cr3 = crate::mem::cr3_phys();
        let pm4 = cr3 as *mut u64;
        let pdpt_raw = pm4.read();
        if pdpt_raw & 1 == 0 {
            return false;
        }
        let pdpt = (pdpt_raw & 0x000F_FFFF_FFFF_F000) as *mut u64;
        // 0xFD000000 (LFB) 与 0xFEE00000 (LAPIC) 同属 PDPT[3] (pdi 488/503);
        // 不复用整链, 只复用旧 PD, 在 PD[503] 插 LAPIC PT (LFB 链不动)。
        let old_pd_raw = pdpt.add(3).read();
        let old_pd = old_pd_raw & 0x000F_FFFF_FFFF_F000;
        let pd: u64;
        if old_pd_raw & 1 != 0 && old_pd != 0 {
            pd = old_pd;
        } else {
            pd = match crate::mem::alloc_frames_kernel(2) {
                Some(p) => p,
                None => return false,
            };
            pdpt.add(3).write(pd | 0x3);
        }
        let pdt = pd as *mut u64;
        let la = match crate::mem::alloc_frame_kernel() {
            Some(p) => p,
            None => return false,
        };
        // PT 帧清零后填 1 项
        for k in 0..512 {
            ((la as *mut u64).add(k)).write(0);
        }
        ((la as *mut u64)).write(0xFEE00000 | 0x0B); // RW|P|PWT|PCD
        pdt.add(503).write(la | 0x3);
        serial::write_str("smp  : lapic pd=0x");
        crate::syscall::log_hex(pd);
        serial::write_str(" pt=0x");
        crate::syscall::log_hex(la);
        serial::write_line("");
        serial::write_line("smp  : lapic MMIO mapped (0xFEE00000)");
    }
    let id = mmio_rd32(0xFEE00020);
    serial::write_str("smp  : lapic id=");
    serial::write_str(if (id >> 24) == 0 { "0 (BSP)" } else { "?" });
    serial::write_line("");
    // LAPIC SVR: APIC software enable (bit 8) — 复位后 ICR 不执行直到启用
    mmio_wr32(0xFEE003F0, 0x1FF);
    serial::write_str("smp  : svr=0x");
    crate::syscall::log_hex(mmio_rd32(0xFEE003F0) as u64);
    serial::write_line("");
    true
}

// ---------------------------------------------------------------------------
// W17: AP 入口 (trampoline retf 到 0x08:fujo_ap_entry; AP GDT = trampoline GDT)
// ---------------------------------------------------------------------------
core::arch::global_asm!(r#"
    .text
    .global fujo_ap_entry
    .p2align 4
fujo_ap_entry:
    cli
    mov ax, 0x10
    mov ds, ax
    mov ss, ax
    mov rsp, 0x3A0000
    call fujo_ap_main
.aphlt:
    hlt
    jmp .aphlt
"#);

extern "C" {
    fn fujo_ap_entry();
}

/// W17: AP 启动序列 (仅 ncpu>=2 时; 拷贝 trampoline -> 回填 -> SIPI@0x8000)。
pub fn ap_bringup() {
    if ncpu() < 2 {
        return;
    }
    unsafe {
        if !map_lapic() {
            return;
        }
        let tramp: &[u8] = include_bytes!("../../sdk/linux/tramp.bin");
        let base = 0x8000u64 as *mut u8;
        for k in 0..tramp.len() {
            base.add(k).write(tramp[k]);
        }
        // 回填数据槽 (+0x200 cr3, +0x204 entry)
        let cr3 = crate::mem::cr3_phys() as u32;
        (base.add(0x200) as *mut u32).write(cr3);
        let entry = fujo_ap_entry as *const () as u64 as u32;
        (base.add(0x204) as *mut u32).write(entry);
        serial::write_line("smp  : SIPI -> AP @0x8000 (trampoline)");
        // INIT(短延时) + SIPI×2 (TCG AP 冷启动需 INIT 唤醒; 长 delay 会导致 BSP 假死)
        mmio_wr32(0xFEE00310, 0x01000000);
        mmio_wr32(0xFEE00300, 0x00000500); // INIT
        delay_ticks(200_000);
        for _ in 0..2 {
            mmio_wr32(0xFEE00310, 0x01000000); // dest APIC ID 1
            mmio_wr32(0xFEE00300, 0x00000608); // SIPI, vector 0x8 (=> 0x8000)
        }
        serial::write_str("smp  : icr_readback=0x");
        crate::syscall::log_hex(mmio_rd32(0xFEE00300) as u64);
        serial::write_line("");
        serial::write_line("smp  : SIPI sent");
        // 探测: AP 是否执行到 trampoline 末尾 (marker @0x8220)
        delay_ticks(100_000);
        let mk = (0x8220u64 as *const u32).read_volatile();
        serial::write_str("smp  : ap marker=0x");
        crate::syscall::log_hex(mk as u64);
        serial::write_line("");
    }
}

/// AP 主入口 (trampoline retf 到达; AP 自己的栈/GDT (trampoline 内核段)).
#[no_mangle]
pub extern "C" fn fujo_ap_main() {
    unsafe {
        core::arch::asm!("mov rsp, {}", in(reg) AP_STACK, options(nostack));
        serial::write_line("smp  : AP1 online (id=1, cli+hlt loop)");
        AP_ONLINE = true;
    }
    loop {
        crate::hlt();
    }
}

/// W17: 0x8B03 — smp_state(ptr): u64×3 = (ncpu, ap_online, lapic_id)。
#[no_mangle]
pub extern "C" fn fujo_smp_state(ptr: u64) -> i64 {
    unsafe {
        if !(0x400000..0xC00000).contains(&ptr) {
            return -14;
        }
        let w = ptr as *mut u64;
        w.write(ncpu() as u64);
        w.add(1).write(if AP_ONLINE { 1 } else { 0 });
        w.add(2).write(lapic_id() as u64);
    }
    0
}
