//! acpi.rs — M96: 真机引导最小集 v0 (ACPI 表 + PCI 枚举)
//!
//! - RSDP 搜索 (0xE0000..0x100000, 16 对齐, magic "RSD PTR ");
//! - RSDT/XSDT 遍历 (首个表计数 + 摘要);
//! - PCI 配置空间枚举 (CF8/CFC, bus 0..2 × slot 0..31,
//!   func 0; 非 FFFF 设备记录, ≤24 条 {vid16, did16, bus, slot})。
//! 接口: 0x8501 acpi_info(ptr) → (rsdp_found, rev, table_count, pci_devs)
//!       0x8502 acpi_dump(ptr, cap) → 摘要文本长度
//!       0x8503 pci_scan(ptr) → pci 条目数 (缓冲 24×8B)。

use crate::serial;

const PCI_MAX: usize = 24;

static mut PCI_BUF: [u64; PCI_MAX] = [0; PCI_MAX];
static mut PCI_N: usize = 0;

fn rd32(p: u64) -> u32 {
    unsafe { (p as *const u32).read_volatile() }
}
fn rd8(p: u64) -> u8 {
    unsafe { (p as *const u8).read_volatile() }
}
fn rd16(p: u64) -> u16 {
    unsafe { (p as *const u16).read_volatile() }
}

fn pci_read_cfg(bus: u8, slot: u8, func: u8, reg: u8) -> u32 {
    unsafe {
        let addr = 0x8000_0000u32 | ((bus as u32) << 16) | ((slot as u32) << 11)
            | ((func as u32) << 8) | ((reg as u32) & 0xFC);
        let mut lo: u32 = addr;
        core::arch::asm!(
            "mov edx, 0xCF8",
            "out dx, eax",
            "mov edx, 0xCFC",
            "in eax, dx",
            inlateout("eax") lo,
            lateout("edx") _,
            options(nomem, nostack)
        );
        lo
    }
}

/// W13b: 配置空间读 (公开)。
pub fn pci_cfg_read(bus: u8, slot: u8, func: u8, reg: u8) -> u32 {
    pci_read_cfg(bus, slot, func, reg)
}

/// W13: PCI 配置空间写 (virtio BAR/命令寄存器等)。
pub fn pci_write_cfg(bus: u8, slot: u8, func: u8, reg: u8, val: u32) {
    let addr = 0x8000_0000u32 | ((bus as u32) << 16) | ((slot as u32) << 11)
        | ((func as u32) << 8) | ((reg as u32) & 0xFC);
    let port_cfg = 0xCF8u16;
    let port_data = 0xCFCu16;
    unsafe {
        core::arch::asm!(
            "out dx, eax",
            in("dx") port_cfg,
            in("eax") addr,
            options(nomem, nostack)
        );
        core::arch::asm!(
            "out dx, eax",
            in("dx") port_data,
            in("eax") val,
            options(nomem, nostack)
        );
    }
}

/// W13: 按 (vendor, device) 查找 PCI 设备; 返回 (bus, slot, func, bar0)。
/// W20 修复: 原 func 循环在 func0 不匹配时 break 整个 slot —— 多功能
/// slot (Q35: 31.0=ISA 31.2=SATA AHCI) 永远找不到; 现在独立遍历每个 func。
pub fn pci_find(vid: u16, did: u16) -> Option<(u8, u8, u8, u32)> {
    for bus in 0..3u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let v = pci_read_cfg(bus, slot, func, 0x00);
                if v == 0xFFFF_FFFF || v == 0 {
                    continue; // 该 func 无设备; 其它 func 可能有
                }
                if v & 0xFFFF == vid as u32 && (v >> 16) & 0xFFFF == did as u32 {
                    let bar0 = pci_read_cfg(bus, slot, func, 0x10);
                    return Some((bus, slot, func, bar0));
                }
            }
        }
    }
    None
}

/// 搜索 RSDP (magic "RSD PTR " 前 8 字节)。
fn find_rsdp() -> Option<u64> {
    let mut p = 0xE0000u64;
    while p < 0x100000 {
        if rd8(p) == b'R' && rd8(p + 1) == b'S' && rd8(p + 2) == b'D' && rd8(p + 3) == b' '
            && rd8(p + 4) == b'P' && rd8(p + 5) == b'T' && rd8(p + 6) == b'R'
            && rd8(p + 7) == b' '
        {
            return Some(p);
        }
        p += 16;
    }
    None
}

fn table_count_from(rsdp: u64) -> u64 {
    let rev = rd8(rsdp + 15);
    // 表体通常在 >64MiB (QEMU 0xFFExxx) — boot 页表恒等 0..64MiB,
    // 未映射区读 → #PF; v0 采样仅当指针落在映射区。
    const MAP_END: u64 = 0x400_0000;
    if rev >= 2 {
        let xsdt = rd32(rsdp + 24) as u64;
        if xsdt < MAP_END && rd32(xsdt) == 0x4344_5358 { // "XSDT"
            let len = rd32(xsdt + 4) as u64;
            return (len - 36) / 8;
        }
    }
    let rsdt = rd32(rsdp + 16) as u64;
    if rsdt < MAP_END && rd32(rsdt) == 0x5444_5352 { // "RSDT"
        let len = rd32(rsdt + 4) as u64;
        return (len - 36) / 4;
    }
    0
}

/// 0x8501: (rsdp_found, rev, table_count, pci_devs)。
pub fn fujo_acpi_info(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        match find_rsdp() {
            Some(p) => {
                w.write(1);
                w.add(1).write(rd8(p + 15) as u64);
                w.add(2).write(table_count_from(p));
                w.add(3).write(PCI_N as u64);
            }
            None => {
                w.write(0);
                w.add(1).write(0);
                w.add(2).write(0);
                w.add(3).write(PCI_N as u64);
            }
        }
    }
    0
}

/// 0x8502: 摘要文本。
pub fn fujo_acpi_dump(ptr: u64, cap: u64) -> i64 {
    unsafe {
        let b = ptr as *mut u8;
        let cap = cap as usize;
        let mut pos = 0usize;
        let mut put = |s: &[u8]| {
            let n = s.len().min(cap.saturating_sub(pos));
            for i in 0..n {
                b.add(pos + i).write(s[i]);
            }
            pos += n;
        };
        match find_rsdp() {
            Some(p) => {
                put(b"acpi: RSDP @");
                let mut num = [0u8; 20];
                let mut ni = 20;
                let mut x = p;
                while x > 0 {
                    ni -= 1;
                    num[ni] = b'0' + (x % 10) as u8;
                    x /= 10;
                }
                put(&num[ni..]);
                put(b" rev=");
                put(&[b'0' + (rd8(p + 15) % 10)]);
                put(b" tables=");
                let t = table_count_from(p);
                let mut num2 = [0u8; 20];
                let mut ni = 20;
                let mut x = t;
                if x == 0 {
                    put(&[b'0']);
                } else {
                    while x > 0 {
                        ni -= 1;
                        num2[ni] = b'0' + (x % 10) as u8;
                        x /= 10;
                    }
                    put(&num2[ni..]);
                }
                put(b" pci_devs=");
            }
            None => put(b"acpi: RSDP not found"),
        }
        let mut num3 = [0u8; 20];
        let mut ni = 20;
        let mut x = PCI_N as u64;
        if x == 0 {
            put(&[b'0']);
        } else {
            while x > 0 {
                ni -= 1;
                num3[ni] = b'0' + (x % 10) as u8;
                x /= 10;
            }
            put(&num3[ni..]);
        }
        if pos < cap {
            b.add(pos).write(0);
        }
    }
    (unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, (cap as usize).min(96))) }).len() as i64
}

/// 0x8503: PCI 枚举 (写入 PCI_MAX×8B 条目:
/// [vid16|did16<<16|bus<<32|slot<<40|func<<48])。
pub fn fujo_pci_scan(ptr: u64) -> i64 {
    unsafe {
        let w = ptr as *mut u64;
        for i in 0..PCI_MAX {
            if PCI_BUF[i] != 0 {
                w.add(i).write(PCI_BUF[i]);
            }
        }
    }
    unsafe {
        let n = PCI_N;
        n as i64
    }
}

/// 启动时执行一次 PCI 枚举 (boot-M96)。
/// W20 p7: 多功能遍历 —— 原只扫 func0 (Q35 SATA=31.2 等不可见);
/// 每条目编码: vid(0-15)|did(16-31)|bus(32-39)|slot(40-47)|**func(48-55)**。
pub fn scan_all() {
    let mut n = 0usize;
    'outer: for bus in 0..3u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let v = pci_read_cfg(bus, slot, func, 0);
                if v == 0xFFFF_FFFF || v == 0 {
                    continue; // 该 func 无设备; 其它 func 可能有
                }
                if n < PCI_MAX {
                    let vid = v & 0xFFFF;
                    let did = (v >> 16) & 0xFFFF;
                    unsafe {
                        PCI_BUF[n] = (vid as u64) | ((did as u64) << 16)
                            | ((bus as u64) << 32) | ((slot as u64) << 40)
                            | ((func as u64) << 48);
                    }
                    n += 1;
                }
            }
        }
    }
    unsafe {
        PCI_N = n;
    }
    serial::write_str("pci  : scanned ");
    crate::syscall::debug_dec(n as u64);
    serial::write_line(" devices (multifunc)");
}
