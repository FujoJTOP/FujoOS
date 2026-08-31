//! ata.rs — M16 ATA PIO 驱动 v0 (IDE 主通道 0x1F0, LBA28)
//!
//! 仅主盘 (master): IDENTIFY 探测 + 读/写 512B 扇区 (PIO)。
//! 验证: QEMU `-drive file=...,format=raw,if=ide` (piix IDE 默认存在)。

use crate::serial;

const ATA_DATA: u16 = 0x1F0;
const ATA_ERR: u16 = 0x1F1;
const ATA_SECT_CNT: u16 = 0x1F2;
const ATA_LBA_LO: u16 = 0x1F3;
const ATA_LBA_MID: u16 = 0x1F4;
const ATA_LBA_HI: u16 = 0x1F5;
const ATA_DRIVE: u16 = 0x1F6;
const ATA_STATUS: u16 = 0x1F7;
const ATA_COMMAND: u16 = 0x1F7;

const ST_BSY: u8 = 0x80;
const ST_DRDY: u8 = 0x40;
const ST_DRQ: u8 = 0x08;
const ST_ERR: u8 = 0x01;

pub static mut ATA_PRESENT: bool = false;

fn wait_not_busy() -> bool {
    for _ in 0..100_000 {
        let st = unsafe { crate::serial::inb(ATA_STATUS) };
        if st & ST_BSY == 0 {
            return true;
        }
    }
    false
}

fn wait_drq() -> bool {
    for _ in 0..100_000 {
        let st = unsafe { crate::serial::inb(ATA_STATUS) };
        if st & ST_ERR != 0 {
            return false;
        }
        if st & ST_DRQ != 0 {
            return true;
        }
    }
    false
}

/// 探测: IDENTIFY (0xEC); 成功置 ATA_PRESENT。
pub fn init() {
    unsafe {
        crate::serial::outb(ATA_DRIVE, 0xA0); // master, LBA
        crate::serial::outb(ATA_SECT_CNT, 0);
        crate::serial::outb(ATA_LBA_LO, 0);
        crate::serial::outb(ATA_LBA_MID, 0);
        crate::serial::outb(ATA_LBA_HI, 0);
        crate::serial::outb(ATA_COMMAND, 0xEC);
        if wait_not_busy() {
            let st = crate::serial::inb(ATA_STATUS);
            if st & ST_ERR == 0 {
                // 读 IDENTIFY 512B (256 words)
                let mut ident: [u16; 256] = [0; 256];
                for w in ident.iter_mut() {
                    *w = inw();
                }
                let words = ((ident[83] & 0x0400) != 0) || ((ident[86] & 0x0400) != 0);
                ATA_PRESENT = true;
                serial::write_str("ata  : drive present (identify 0x");
                print_hex(ident[0] as u64);
                serial::write_str("), lba48=");
                print_dec(words as u64);
                serial::write_line("");
            }
        }
    }
}

fn inw() -> u16 {
    unsafe { crate::serial::inw(ATA_DATA) }
}

fn outw(v: u16) {
    unsafe { crate::serial::outw(ATA_DATA, v) }
}

fn print_hex(v: u64) {
    const HX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let d = ((v >> (4 * (15 - i))) & 0xF) as u8;
        buf[2 + i] = HX[d as usize];
    }
    serial::write_str(core::str::from_utf8(&buf).unwrap());
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

/// 读扇区 (LBA28, PIO)。返回是否成功。
pub fn read_sectors(lba: u32, count: u32, buf: *mut u8) -> bool {
    unsafe {
        crate::serial::outb(ATA_DRIVE, 0xE0 | ((lba >> 24) as u8 & 0x0F));
        crate::serial::outb(ATA_SECT_CNT, count as u8);
        crate::serial::outb(ATA_LBA_LO, (lba & 0xFF) as u8);
        crate::serial::outb(ATA_LBA_MID, ((lba >> 8) & 0xFF) as u8);
        crate::serial::outb(ATA_LBA_HI, ((lba >> 16) & 0xFF) as u8);
        crate::serial::outb(ATA_COMMAND, 0x20);
        if !wait_not_busy() {
            return false;
        }
        for s in 0..count {
            if !wait_drq() {
                return false;
            }
            let dst = (buf as *mut u16).add((s * 256) as usize);
            for i in 0..256 {
                *dst.add(i) = inw();
            }
        }
    }
    true
}

/// 写扇区 (LBA28, PIO)。
pub fn write_sectors(lba: u32, count: u32, buf: *const u8) -> bool {
    unsafe {
        crate::serial::outb(ATA_DRIVE, 0xE0 | ((lba >> 24) as u8 & 0x0F));
        crate::serial::outb(ATA_SECT_CNT, count as u8);
        crate::serial::outb(ATA_LBA_LO, (lba & 0xFF) as u8);
        crate::serial::outb(ATA_LBA_MID, ((lba >> 8) & 0xFF) as u8);
        crate::serial::outb(ATA_LBA_HI, ((lba >> 16) & 0xFF) as u8);
        crate::serial::outb(ATA_COMMAND, 0x30);
        if !wait_not_busy() {
            return false;
        }
        for s in 0..count {
            if !wait_drq() {
                return false;
            }
            let src = (buf as *const u16).add((s * 256) as usize);
            for i in 0..256 {
                outw(*src.add(i));
            }
            // 等待写完成 (PIO 写后需要一点时间)
            for _ in 0..1000 {
                if crate::serial::inb(ATA_STATUS) & ST_BSY == 0 {
                    break;
                }
            }
        }
    }
    true
}
