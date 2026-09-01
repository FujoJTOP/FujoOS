//! save.rs — M60: 存档沙箱 (权限目录 + 版本化)
//!
//! 存档命名空间独立于 VFS (权限: 仅经 save 原语, 无法经路径越权):
//! 8 槽 × 8KiB, 每槽头 12B: [magic u32][version u32][len u32]。
//! 0x6701 save_write(slot, ptr, len) — 含版本头自动写
//! 0x6702 save_read(slot, ptr, len) -> 长度 (版本校验失败 0)
//! 0x6703 save_list(ptr) 8×u32 长
//! 0x6704 save_version(slot) -> 版本号

use crate::serial;

pub const SAVE_MAGIC: u32 = 0x53415631; // "SAV1"
pub const SAVE_VERSION: u32 = 2;

static mut SAVE: [[u8; 8192]; 8] = [[0; 8192]; 8];
static mut SAVE_USE: [bool; 8] = [false; 8];
static mut SAVE_LEN: [u32; 8] = [0; 8];

/// 0x6701
pub fn fujo_save_write(slot: u64, ptr: u64, len: u64) -> i64 {
    if slot >= 8 || len >= 8180 {
        return -22;
    }
    unsafe {
        let s = slot as usize;
        let p = SAVE[s].as_mut_ptr();
        // 版本头 (0x00 magic / 0x04 version / 0x08 len)
        (p as *mut u32).write(SAVE_MAGIC);
        (p.add(4) as *mut u32).write(SAVE_VERSION);
        (p.add(8) as *mut u32).write(len as u32);
        for i in 0..len as usize {
            p.add(12 + i).write(((ptr as *const u8).add(i)).read());
        }
        SAVE_USE[s] = true;
        SAVE_LEN[s] = len as u32;
    }
    serial::write_line("save : archived (sandbox)");
    len as i64
}

/// 0x6702
pub fn fujo_save_read(slot: u64, ptr: u64, _len: u64) -> i64 {
    if slot >= 8 {
        return -22;
    }
    unsafe {
        let s = slot as usize;
        if !SAVE_USE[s] {
            return 0;
        }
        let p = SAVE[s].as_ptr();
        if (p as *const u32).read() != SAVE_MAGIC {
            return 0;
        }
        let v = (p.add(4) as *const u32).read();
        if v > SAVE_VERSION {
            serial::write_line("save : version too new - reject");
            return 0;
        }
        let n = (p.add(8) as *const u32).read() as usize;
        for i in 0..n {
            (((ptr + i as u64) as *mut u8)).write(p.add(12 + i).read());
        }
        n as i64
    }
}

/// 0x6703
pub fn fujo_save_list(ptr: u64) -> i64 {
    unsafe {
        for i in 0..8 {
            ((ptr + (i as u64) * 4) as *mut u32)
                .write(if SAVE_USE[i] { SAVE_LEN[i] } else { 0xFFFF_FFFF });
        }
    }
    0
}

/// 0x6704
pub fn fujo_save_version(slot: u64) -> i64 {
    if slot >= 8 {
        return -22;
    }
    unsafe {
        let s = slot as usize;
        if !SAVE_USE[s] {
            return -19;
        }
        (SAVE[s].as_ptr().add(4) as *const u32).read() as i64
    }
}
