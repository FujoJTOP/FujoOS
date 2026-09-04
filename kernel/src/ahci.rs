//! ahci.rs — W20: AHCI (SATA) 驱动 v0 (ICH9/QEMU ich9-ahci; 真机 SATA 主盘)
//!
//! 定位: 审计表 #5b —— 真机硬碟经 SATA/AHCI, QEMU 参考机用
//! `-device ich9-ahci` 可复现开发。与 ATA PIO (M16) 并存:
//! 有 AHCI 设备 → HBA 引擎; 无 → ATA 路径 (兼容)。
//!
//! v0: 槽 0 单命令引擎, PIO 无关 (DMA: PRDT 1 项 ≤ 512B/命令, 方便 demo 断言)。
//! 原语: 0x8E01 ahci_read(lba, buf) / 0x8E02 ahci_write(lba, buf)
//!       0x8E03 ahci_info(ptr) -> (present, ports, sig, lba_cap)。
//!
//! 驱动序列 (铁律): PCI 命令寄存器先写 0x7 (IO|MEM|BUSMASTER, 铁律 15)
//! -> BAR5 映射 (map_phys_identity) -> HBA 全局复位 (GHC.HR) -> 手动
//! GHC.AE -> 端口: PxCMD 停 (ST=0) -> PxCLB/PxFB 填 -> FRE|ST -> 签名检查。

use crate::serial;

static mut HBA: u64 = 0;
static mut PORTS: u32 = 0;
static mut ACTIVE_PORT: u32 = 0;
static mut CMD_BUF: u64 = 0; // 命令列表 (1 帧, 32 槽 × 32B)
static mut FIS_BUF: u64 = 0; // FIS 接收区 (1 帧, 用前 256B)
static mut TAB_BUF: u64 = 0; // 槽 0 命令表 (1 帧)
static mut AHCI_READY: bool = false;
static mut LBA_CAP: u64 = 0;

// W20 debug→final: AHCI DMA 缓冲改内核 static (BSS, 恒等映射确定)
// 帧分配器在任务页表下虚拟≠物理疑点 (差分 m123 难缠) —— 直接消除:
// 命令列表/命令表/FIS 区 = 内核 BSS, boot 与任务页表恒等, QEMU 可见。
#[repr(C, align(4096))]
struct Aligned4K {
    d: [u8; 0x1000],
}
static mut CMD_HDR_BUF: Aligned4K = Aligned4K { d: [0; 0x1000] };
static mut CMD_TAB_BUF2: Aligned4K = Aligned4K { d: [0; 0x1000] };
static mut FIS_RX_BUF: Aligned4K = Aligned4K { d: [0; 0x1000] };

const GHC: u64 = 0x04;
const P: u64 = 0x100;
const P_CMD: u64 = 0x18;
const P_TFD: u64 = 0x20;
const P_SIG: u64 = 0x24;
const P_SSTS: u64 = 0x28;
const P_CI: u64 = 0x38;
const P_CLB: u64 = 0x00;
const P_FB: u64 = 0x08;
const P_IS: u64 = 0x10;

fn mmio_wr(addr: u64, v: u32) {
    unsafe {
        core::arch::asm!("mov [{}], eax", in(reg) addr, in("eax") v, options(nomem));
    }
}

fn mmio_rd(addr: u64) -> u32 {
    let v: u32;
    unsafe {
        core::arch::asm!("mov eax, [{}]", in(reg) addr, out("eax") v, options(nomem));
    }
    v
}

fn rdtsc_smp() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

/// W20: 探测/初始化 (main.rs 在 ATA 之后调用; 有 HBA 即接管)。
pub fn init() -> bool {
    unsafe {
        match crate::acpi::pci_find(0x8086, 0x2922) {
            Some((bus, slot, func, _bar0)) => {
                let bar5 = crate::acpi::pci_cfg_read(bus, slot, func, 0x24);
                if bar5 == 0 || bar5 == 0xFFFF_FFFF {
                    serial::write_line("ahci : bar5=0 - device inactive");
                    return false;
                }
                // 铁律 15: 命令寄存器默认 0 (MEM 未使能) -> 先写 0x7
                crate::acpi::pci_write_cfg(bus, slot, func, 0x04, 0x7);
                let phys = (bar5 & 0xFFFF_F000) as u64;
                if !crate::mem::map_phys_identity(phys, 2) {
                    serial::write_line("ahci : bar5 mmap failed");
                    return false;
                }
                HBA = phys;
                serial::write_str("ahci : HBA @ ");
                crate::syscall::log_hex(phys);
                serial::write_line("");
                // HBA 全局复位 (GHC.HR) + 使能 (AE)
                mmio_wr(HBA + GHC, 0x8000_0001);
                let mut spins = 0u32;
                while mmio_rd(HBA + GHC) & 0x8000_0000 != 0 && spins < 100_000 {
                    core::hint::spin_loop();
                    spins += 1;
                }
                let cap = mmio_rd(HBA);
                serial::write_str("ahci : cap=0x");
                crate::syscall::log_hex(cap as u64);
                serial::write_line("");
                // 端口数: CAP[31:24]=NPS-1; QEMU 实测高位宽松 (0xC0) → 上限 8 防越界
                PORTS = ((((cap >> 24) & 0xFF) + 1).min(8));
                serial::write_str("ahci : ports=");
                print_dec(PORTS as u64);
                serial::write_line("");
                // 缓冲: 内核 static (BSS 恒等; 4K 对齐容器 —— 铁律: 对齐写类型不写 static)
                CMD_BUF = core::ptr::addr_of!(CMD_HDR_BUF) as u64;
                FIS_BUF = core::ptr::addr_of!(FIS_RX_BUF) as u64;
                TAB_BUF = core::ptr::addr_of!(CMD_TAB_BUF2) as u64;
                serial::write_str("ahci : clb=0x");
                crate::syscall::log_hex(CMD_BUF);
                serial::write_str(" fis=0x");
                crate::syscall::log_hex(FIS_BUF);
                serial::write_str(" ctba=0x");
                crate::syscall::log_hex(TAB_BUF);
                serial::write_line("");
                // 端口扫描: 首个签名 0x101 (ATA) 的端口为 active
                for i in 0..PORTS {
                    let p = HBA + P + (i as u64) * 0x80;
                    // 端口停止
                    mmio_wr(p + P_CMD, mmio_rd(p + P_CMD) & !1);
                    mmio_wr(p + P_CLB, CMD_BUF as u32);
                    mmio_wr(p + P_CLB + 4, 0);
                    mmio_wr(p + P_FB, FIS_BUF as u32);
                    mmio_wr(p + P_FB + 4, 0);
                    mmio_wr(p + P_IS, 0xFFFF_FFFF); // 清中断
                    mmio_wr(p + P_CMD, mmio_rd(p + P_CMD) | 0x10 | 0x1); // FRE | ST
                    let mut w = 0u32;
                    while w < 100_000 {
                        let ssts = mmio_rd(p + P_SSTS);
                        if ssts != 0 && ssts != 1 && ssts != 0x1000 {
                            break;
                        }
                        core::hint::spin_loop();
                        w += 1;
                    }
                    let sig = mmio_rd(p + P_SIG) & 0xFF;
                    serial::write_str("ahci : port ");
                    print_dec(i as u64);
                    serial::write_str(" sig=0x");
                    crate::syscall::log_hex(mmio_rd(p + P_SIG) as u64);
                    serial::write_line("");
                    if sig == 0x01 {
                        // ATA (签名 0x00000101; 低字节 0x01)
                        ACTIVE_PORT = i;
                        AHCI_READY = true;
                        serial::write_line("ahci : ATA device on (LBA48 ok)");
                        break;
                    }
                }
                AHCI_READY
            }
            None => {
                serial::write_line("ahci : no ICH9 AHCI device - ATA PIO path stays");
                false
            }
        }
    }
}

pub fn ready() -> bool {
    unsafe { AHCI_READY }
}

/// W20 p5: 磁盘背板原语 (内核缓冲; virt_to_phys 恒等 pass)。
pub fn disk_read(lba: u32, buf: *mut u8) -> bool {
    if !unsafe { AHCI_READY } {
        return false;
    }
    cmd(unsafe { ACTIVE_PORT }, 0x25, lba, 1, buf)
}

pub fn disk_write(lba: u32, buf: *const u8) -> bool {
    if !unsafe { AHCI_READY } {
        return false;
    }
    cmd(unsafe { ACTIVE_PORT }, 0x35, lba, 1, buf)
}

/// W20: 单命令执行 (槽 0; kind: 0x25=read DMA, 0x35=write DMA)。
fn cmd(port: u32, kind: u8, lba: u32, count: u16, buf: *const u8) -> bool {
    unsafe {
        let p = HBA + P + (port as u64) * 0x80;
        // 命令表清零 + CFIS 填充 (FIS_REG_H2D, 20B)
        let ct = TAB_BUF as *mut u32;
        for i in 0..256 {
            ct.add(i).write(0);
        }
        let cfis = TAB_BUF as *mut u8;
        cfis.write(0x27); // FIS type H2D
        cfis.add(1).write(0x80); // C=1 (command FIS)
        cfis.add(2).write(kind); // command (offset 2! W20 实证: 误放 3 被当 features)
        cfis.add(3).write(0); // features
        cfis.add(4).write((lba & 0xFF) as u8);
        cfis.add(5).write(((lba >> 8) & 0xFF) as u8);
        cfis.add(6).write(((lba >> 16) & 0xFF) as u8);
        cfis.add(7).write(0xE0 | (((lba >> 24) & 0x0F) as u8)); // A | lba hi
        // QEMU 9.2 源码: nsector = (fis[13]<<8)|fis[12]; [11]=hob_feature
        cfis.add(11).write(0); // hob_feature
        cfis.add(12).write((count & 0xFF) as u8);
        cfis.add(13).write(((count >> 8) & 0xFF) as u8);
        // PRDT @ ct+0x80 (QEMU AHCI_SG: {u64 addr, u32 reserved, u32 flags_size})
        // W20: DMA 用 guest-physical (M121 任务页表下用户虚拟≠物理);
        // 缓冲必须整页 (512B 扇区固定; 跨页拒绝)。
        let dma_phys = match crate::mem::virt_to_phys(crate::mem::cr3_phys(), buf as u64) {
            Some(p) => p,
            None => {
                serial::write_line("ahci : buf phys walk fail");
                return false;
            }
        };
        if (dma_phys & 0xFFF) > (0x1000 - 512) {
            serial::write_line("ahci : buf crosses page");
            return false;
        }
        let prd = TAB_BUF + 0x80;
        (prd as *mut u64).write(dma_phys);
        ((prd as *mut u32).add(2)).write(0); // reserved
        ((prd as *mut u32).add(3)).write(count as u32 * 512 - 1); // flags_size (0-based!)
        // 命令头 (槽 0) —— QEMU 9.2 AHCICmdHdr 布局 (源码取证 ahci-internal.h):
        //   opts@0x00 {bits4:0=CFL(64B units), bit5=ATAPI, bit6=WRITE, bit10=CLR_BUSY}
        //   prdtl@0x02, prdbc@0x04, tbl_addr@0x08 (u64!), reserved@0x10..0x1F
        let hdr = CMD_BUF as *mut u32;
        let opts: u16 = 0x02 | if kind == 0x35 { 0x40 } else { 0 }; // CFL=2(128B表) + WRITE
        (hdr as *mut u16).write(opts);
        ((hdr as *mut u16).add(1)).write(1); // PRDTL=1
        (hdr.add(1)).write(count as u32 * 512); // PRDBC
        (hdr.add(2)).write(TAB_BUF as u32); // tbl_addr low @0x08
        (hdr.add(3)).write(0); // tbl_addr high @0x0C
        // 槽 0 执行 (带重试: QEMU SeaBIOS 探测命令可能占住 IDE 引擎,
        // handle_cmd 早退 busy → PxCI 残留 → 清位重发; 真机同场景)
        for _attempt in 0..4 {
            mmio_wr(p + P_CI, 0); // 清残留 issue 位
            mmio_wr(p + P_CI, 1);
            let mut spins = 0u32;
            while mmio_rd(p + P_CI) & 1 != 0 && spins < 2_000_000 {
                core::hint::spin_loop();
                spins += 1;
            }
            let is = mmio_rd(p + P_IS);
            mmio_wr(p + P_IS, is); // 清
            let tfd = mmio_rd(p + P_TFD);
            let done = spins < 2_000_000;
            let err = (is & 0x7E00) != 0 || (tfd & 1) != 0; // PxIS 错误位簇 (TFE 等)/TFD.ERR
            if done && !err {
                // W20 p5: 写后盘需回到空闲 (TFD.DRQ/BSY 清) 才能继续下一命令;
                // QEMU AIO 完成时序 + 真机盘 TRDY 延迟, 连续写必须等待 (0x44=DRQ|BSY)。
                let mut w = 0u32;
                while (mmio_rd(p + P_TFD) & 0x44) != 0 && w < 200_000 {
                    core::hint::spin_loop();
                    w += 1;
                }
                // 额外 settle (QEMU 写完成回调时序; rdtsc 忙等 1M ≈ ms 级)
                let t0 = rdtsc_smp();
                while rdtsc_smp().wrapping_sub(t0) < 1_000_000 {
                    core::hint::spin_loop();
                }
                LBA_CAP = LBA_CAP.max(((lba as u64) + count as u64) * 512);
                return true;
            }
            if _attempt == 3 {
                serial::write_str("ahci : cmd fail is=0x");
                crate::syscall::log_hex(is as u64);
                serial::write_str(" tfd=0x");
                crate::syscall::log_hex(tfd as u64);
                serial::write_line("");
                return false;
            }
        }
        false
    }
}

/// 0x8E01: ahci_read(lba, buf) — 读 1 扇区 (512B) 到用户缓冲。
#[no_mangle]
pub extern "C" fn fujo_ahci_read(lba: u64, buf: u64) -> i64 {
    if !unsafe { AHCI_READY } {
        return -1;
    }
    if !(0x400000..0xC00000).contains(&buf) {
        return -14;
    }
    if lba > 0xFFFF_FFFF {
        return -22; // v0: LBA28
    }
    if cmd(unsafe { ACTIVE_PORT }, 0x25, lba as u32, 1, buf as *const u8) {
        0
    } else {
        -5
    }
}

/// 0x8E02: ahci_write(lba, buf) — 写 1 扇区。
#[no_mangle]
pub extern "C" fn fujo_ahci_write(lba: u64, buf: u64) -> i64 {
    if !unsafe { AHCI_READY } {
        return -1;
    }
    if !(0x400000..0xC00000).contains(&buf) {
        return -14;
    }
    if lba > 0xFFFF_FFFF {
        return -22;
    }
    if cmd(unsafe { ACTIVE_PORT }, 0x35, lba as u32, 1, buf as *const u8) {
        0
    } else {
        -5
    }
}

/// 0x8E03: ahci_info(ptr) -> u64×3 = (present, active_port, lba_cap)。
#[no_mangle]
pub extern "C" fn fujo_ahci_info(ptr: u64) -> i64 {
    if !(0x400000..0xC00000).contains(&ptr) {
        return -14;
    }
    unsafe {
        let w = ptr as *mut u64;
        w.write(if AHCI_READY { 1 } else { 0 });
        w.add(1).write(ACTIVE_PORT as u64);
        w.add(2).write(LBA_CAP);
    }
    0
}

fn print_dec(v: u64) {
    let mut buf = [0u8; 24];
    let mut i = 24;
    let mut x = v;
    if x == 0 {
        serial::write_str("0");
        return;
    }
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    serial::write_str(core::str::from_utf8(&buf[i..]).unwrap());
}
