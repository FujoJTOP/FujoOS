//! gdt.rs — 全局描述符表 + TSS（M1: 用户态进入所需）
//!
//! 分段布局 (与 syscall STAR/sysret 数学一致):
//!   0x00 null | 0x08 kcode(L=1) | 0x10 kdata | 0x18 udata(DPL3)
//!   0x20 ucode(DPL3,L=1) | 0x28 TSS(2 个描述符, 64-bit)
//!
//! syscall: CS=STAR[47:32](0x08), SS=0x10
//! sysret : CS=STAR[63:48]+16(0x20), SS=STAR[63:48]+8(0x18)
//! iretq  : 由调用方显式压入 0x23/0x1B (RPL=3)
//!
//! ⚠️ 优化器陷阱记录: lgdt/ltr 的 asm 若带 options(nomem), LLVM 认为
//! 其不读内存, 会把 GDT/TSS 的普通写入当作"死存储"优化掉(实测 rsp0 被
//! 清零)。因此: 所有表写入必须 volatile, asm 一律去掉 nomem。

use core::arch::asm;

pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
pub const USER_DS: u16 = 0x18;
pub const USER_CS: u16 = 0x20;
pub const TSS_SEL: u16 = 0x28;
/// M65: 核1 TSS 选择子 (GDT 槽 7/8)。
pub const TSS1_SEL: u16 = 0x38;

#[repr(C, packed)]
struct GdtPtr {
    limit: u16,
    base: u64,
}

static mut GDT_PTR: GdtPtr = GdtPtr { limit: 0, base: 0 };

/// x86 64-bit TSS 布局是**非对齐**的: rsp0 位于偏移 4 (u64, 未对齐)!
/// 因此必须 repr(C, packed) —— 否则 repr(C) 会把 rsp0 对齐到偏移 8,
/// 硬件读到的是 padding (0), 用户态异常分发时栈切换 -> #SS (实测踩坑)。
#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

impl Tss {
    const fn zero() -> Self {
        Tss {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist1: 0,
            ist2: 0,
            ist3: 0,
            ist4: 0,
            ist5: 0,
            ist6: 0,
            ist7: 0,
            reserved2: 0,
            reserved3: 0,
            iomap_base: 0,
        }
    }
}

/// 数组与 TSS 采用 `#[no_mangle]` + 无别名 static, 便于诊断与链接期验证。
#[no_mangle]
pub static mut GDT: [u64; 16] = [0; 16];
#[no_mangle]
pub static mut TSS: Tss = Tss::zero();
/// M65: 核1 TSS (每核独立 RSP0; AP 启动后由 SIPI 路径装载)。
#[no_mangle]
pub static mut TSS1: Tss = Tss::zero();

/// 64-bit TSS 描述符（两个 8 字节）。
fn tss_desc(base: u64, limit: u32) -> (u64, u64) {
    let mut lo: u64 = (limit & 0xFFFF) as u64;
    lo |= (base & 0xFFFF) << 16;
    lo |= ((base >> 16) & 0xFF) << 32;
    lo |= 0x89u64 << 40; // P | avail 64-bit TSS (0x9)
    lo |= (((limit >> 16) & 0xF) as u64) << 48;
    lo |= ((base >> 24) & 0xFF) << 56;
    let hi = base >> 32;
    (lo, hi)
}

pub fn init() {
    unsafe {
        // —— 全部 volatile 写入: 防止 LLVM 以"asm 不读内存"为由做死存储消除 ——
        let g = core::ptr::addr_of_mut!(GDT) as *mut u64;
        core::ptr::write_volatile(g.add(0), 0u64);
        core::ptr::write_volatile(g.add(1), 0x00AF_9B00_0000_FFFFu64); // kcode L=1 -> 0x08
        core::ptr::write_volatile(g.add(2), 0x00CF_9300_0000_FFFFu64); // kdata      -> 0x10
        core::ptr::write_volatile(g.add(3), 0x00CF_F300_0000_FFFFu64); // udata DPL3 -> 0x18
        core::ptr::write_volatile(g.add(4), 0x00AF_FB00_0000_FFFFu64); // ucode DPL3 -> 0x20
        // 注意: 0x18=数据 / 0x20=代码 —— 与 STAR(0x10): sysret CS=0x20, SS=0x18 一致

        let t = core::ptr::addr_of_mut!(TSS);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*t).rsp0), 0x300000u64);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*t).iomap_base), 0xFFFFu16);

        let tss_base = &raw mut TSS as u64;
        let (lo, hi) = tss_desc(tss_base, 103);
        core::ptr::write_volatile(g.add(5), lo);
        core::ptr::write_volatile(g.add(6), hi);

        // —— M65: 核1 TSS (槽 7/8, 选择子 0x38; 独立核栈 0x3A0000) ——
        let t1 = core::ptr::addr_of_mut!(TSS1);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*t1).rsp0), 0x3A0000u64);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*t1).iomap_base), 0xFFFFu16);
        let tss1_base = &raw mut TSS1 as u64;
        let (lo1, hi1) = tss_desc(tss1_base, 103);
        core::ptr::write_volatile(g.add(7), lo1);
        core::ptr::write_volatile(g.add(8), hi1);

        // lgdt/ltr 注意: 不得声明 nomem（它们确确实实读内存中的表）
        core::ptr::write_volatile(core::ptr::addr_of_mut!(GDT_PTR.limit), 16 * 8 - 1);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(GDT_PTR.base), &raw mut GDT as u64);
        asm!("lgdt [{}]", in(reg) core::ptr::addr_of_mut!(GDT_PTR), options(nostack));
        asm!("ltr ax", in("ax") TSS_SEL, options(nostack));
    }
}

/// W17b: AP 装载内核 GDT (含双 TSS) + 核1 TSS (选择子 0x38, rsp0=0x3A0000)。
/// trampoline GDT 只有 null/kcode/kdata —— 无 TSS -> AP 上中断 #GP;
/// 换内核 GDT 后 CS/DS 选择子 (0x08/0x10) 语义一致, 无需重载段寄存器。
pub fn ap_load() {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(GDT_PTR.limit), 16 * 8 - 1);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(GDT_PTR.base), &raw mut GDT as u64);
        asm!("lgdt [{}]", in(reg) core::ptr::addr_of_mut!(GDT_PTR), options(nostack));
        asm!("ltr ax", in("ax") TSS1_SEL, options(nostack));
    }
}

pub fn kernel_rsp0() -> u64 {
    0x300000
}

/// M13: 运行时切换 TSS.rsp0 (每任务独立内核栈; 线程切换时更新)。
pub fn set_rsp0(v: u64) {
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(TSS).cast::<u8>().add(4).cast::<u64>(),
            v,
        );
    }
}

/// 调试: 读取 TSS.rsp0 实际值 (验证 desc 指向的 TSS 数据完好)
pub fn debug_tss_rsp0() -> u64 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TSS).cast::<u8>().add(4).cast::<u64>()) }
}

/// M65: 各 TSS rsp0 值 (诊断面: 0x6B02 tss_info)。
pub fn tss_rsp0s() -> (u64, u64) {
    unsafe {
        (
            core::ptr::read_volatile(core::ptr::addr_of!(TSS).cast::<u8>().add(4).cast::<u64>()),
            core::ptr::read_volatile(core::ptr::addr_of!(TSS1).cast::<u8>().add(4).cast::<u64>()),
        )
    }
}
